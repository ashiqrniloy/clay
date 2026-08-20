//! Clay binary root: thin composition root.
//!
//! CLI parsing lives in `cli`, server/client startup + window creation live in
//! `launch`, and app event dispatch + native dialog/action routing live in
//! `app_driver`. `main` parses args, installs the perf recorder, and dispatches
//! the typed `ClayCommand` to the matching `launch::run_*` entry point.

use std::error::Error;

use clay::perf::metrics::{PerfConfig, install_global_recorder};

mod app_driver;
mod cli;
mod driver;
mod launch;

use cli::{CLI_USAGE, ClayCommand, extract_profile_perf_flag, parse_command};
use launch::{
    run_client, run_package_subcommand, run_perf_fixture, run_restart, run_server, run_smoke_gui,
};

fn main() -> Result<(), Box<dyn Error>> {
    let (args, profile_perf) = extract_profile_perf_flag(std::env::args_os().skip(1));
    install_global_recorder(PerfConfig::from_env().with_flag(profile_perf));

    match parse_command(args)? {
        ClayCommand::Server {
            endpoint,
            configuration_root,
        } => run_server(endpoint, configuration_root),
        ClayCommand::Client { endpoint } => run_client(endpoint, false),
        ClayCommand::Restart { endpoint } => run_restart(endpoint),
        ClayCommand::Auto { endpoint } => run_client(endpoint, true),
        ClayCommand::SmokeGui {
            endpoint,
            configuration_root,
        } => run_smoke_gui(endpoint, configuration_root),
        ClayCommand::PerfFixture {
            kind,
            size_mib,
            seed,
            output,
        } => run_perf_fixture(kind, size_mib, seed, output),
        ClayCommand::Help => {
            println!("{CLI_USAGE}");
            Ok(())
        }
        ClayCommand::Package { subcommand } => run_package_subcommand(subcommand),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

    #[cfg(not(windows))]
    use super::app_driver::handle_client_ui_command;
    use super::app_driver::{
        ClientUiCommandResult, SelectedPathKind, client_dialog_result_to_command_result,
        is_linux_portal_dialog_command,
    };
    use super::cli::{ClayCommand, extract_profile_perf_flag, parse_command};
    #[cfg(target_os = "linux")]
    use super::launch::linux_command_line_is_default_server;
    use super::launch::{
        LaunchDiagnostic, LaunchReadinessFailure, background_server_command, connect_with_retry,
        connect_with_retry_while, managed_server_command,
    };
    use crate::driver::tests::test_driver_with_tabs;
    use clay::client::{ClientBootstrapError, ClientConnectionEvent};
    use clay::editor::{EditorSurface, is_printable_text};
    use clay::ipc::default_endpoint;
    use clay::perf::fixtures::FixtureKind;
    use clay::protocol::codec::CodecError;

    #[test]
    fn parses_server_subcommand() {
        assert!(matches!(
            parse_command(vec!["server".into()]).expect("server parses"),
            ClayCommand::Server { .. }
        ));
    }

    #[test]
    fn parses_client_subcommand() {
        assert!(matches!(
            parse_command(vec!["client".into()]).expect("client parses"),
            ClayCommand::Client { .. }
        ));
    }

    #[test]
    fn parses_restart_subcommand() {
        assert!(matches!(
            parse_command(vec!["restart".into()]).expect("restart parses"),
            ClayCommand::Restart { .. }
        ));
        assert!(parse_command(vec!["restart".into(), "extra".into()]).is_err());
    }

    #[test]
    fn parses_no_args_as_auto() {
        assert!(matches!(
            parse_command(vec![]).expect("bare clay parses"),
            ClayCommand::Auto { .. }
        ));
    }

    #[test]
    fn parses_smoke_gui_subcommand() {
        assert!(matches!(
            parse_command(vec!["smoke-gui".into()]).expect("smoke-gui parses"),
            ClayCommand::SmokeGui { .. }
        ));
    }

    #[test]
    fn parses_profile_perf_as_global_developer_flag() {
        let (args, enabled) = extract_profile_perf_flag(
            vec!["smoke-gui".into(), "--profile-perf".into()].into_iter(),
        );

        assert!(enabled);
        assert_eq!(args, vec![OsString::from("smoke-gui")]);
        assert!(matches!(
            parse_command(args).expect("global profiling flag is stripped before parsing"),
            ClayCommand::SmokeGui { .. }
        ));
    }

    #[test]
    fn parses_default_launch_modes() {
        assert!(matches!(
            parse_command(vec![]).expect("bare clay parses"),
            ClayCommand::Auto { .. }
        ));
        assert!(matches!(
            parse_command(vec!["server".into()]).expect("server parses"),
            ClayCommand::Server { .. }
        ));
        assert!(matches!(
            parse_command(vec!["client".into()]).expect("client parses"),
            ClayCommand::Client { .. }
        ));
    }

    #[test]
    fn launch_modes_do_not_require_manual_endpoint() {
        for args in [
            vec![],
            vec!["server".into()],
            vec!["client".into()],
            vec!["restart".into()],
            vec!["smoke-gui".into()],
        ] {
            let command = parse_command(args).expect("mode parses with default endpoint");
            match command {
                ClayCommand::Auto { endpoint }
                | ClayCommand::Client { endpoint }
                | ClayCommand::Restart { endpoint }
                | ClayCommand::Server { endpoint, .. }
                | ClayCommand::SmokeGui { endpoint, .. } => {
                    assert!(!endpoint.to_string().is_empty())
                }
                ClayCommand::PerfFixture { .. } => {
                    panic!("perf fixture should not be selected by launch modes")
                }
                ClayCommand::Help => panic!("help should not be selected by launch modes"),
                ClayCommand::Package { .. } => {
                    panic!("package subcommand should not be selected by launch modes")
                }
            }
        }
    }

    #[test]
    fn default_server_and_clients_use_same_platform_endpoint() {
        let expected = default_endpoint();

        for args in [
            vec![],
            vec!["server".into()],
            vec!["client".into()],
            vec!["restart".into()],
        ] {
            let command = parse_command(args).expect("default launch mode parses");
            let endpoint = match command {
                ClayCommand::Auto { endpoint }
                | ClayCommand::Client { endpoint }
                | ClayCommand::Restart { endpoint }
                | ClayCommand::Server { endpoint, .. } => endpoint,
                ClayCommand::SmokeGui { .. } => {
                    panic!("default smoke endpoint must remain isolated")
                }
                ClayCommand::PerfFixture { .. } => {
                    panic!("perf fixture should not be selected by default launch modes")
                }
                ClayCommand::Help => panic!("help should not be selected by default launch modes"),
                ClayCommand::Package { .. } => {
                    panic!("package subcommand should not be selected by default launch modes")
                }
            };
            assert_eq!(endpoint, expected);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn restart_matches_only_default_server_command_lines() {
        let endpoint = b"/run/user/1000/clay.sock";

        assert!(linux_command_line_is_default_server(
            b"/tmp/clay\0server\0/run/user/1000/clay.sock\0",
            endpoint
        ));
        assert!(linux_command_line_is_default_server(
            b"/tmp/clay\0server\0",
            endpoint
        ));
        assert!(!linux_command_line_is_default_server(
            b"/tmp/clay\0server\0/tmp/smoke.sock\0",
            endpoint
        ));
        assert!(!linux_command_line_is_default_server(
            b"/tmp/clay\0client\0/run/user/1000/clay.sock\0",
            endpoint
        ));
    }

    #[test]
    fn parses_perf_fixture_subcommand() {
        match parse_command(vec![
            "perf-fixture".into(),
            "--kind".into(),
            "mixed-unicode".into(),
            "--size-mib".into(),
            "16".into(),
            "--output".into(),
            "target/perf-fixtures/mixed-16m.txt".into(),
            "--seed".into(),
            "42".into(),
        ])
        .expect("perf fixture parses")
        {
            ClayCommand::PerfFixture {
                kind,
                size_mib,
                seed,
                output,
            } => {
                assert_eq!(kind, FixtureKind::MixedUnicode);
                assert_eq!(size_mib, 16);
                assert_eq!(seed, 42);
                assert_eq!(
                    output.unwrap(),
                    PathBuf::from("target/perf-fixtures/mixed-16m.txt")
                );
            }
            command => panic!("expected perf fixture command, got {command:?}"),
        }
    }

    #[test]
    fn cli_parses_platform_endpoint() {
        let endpoint = "clay-test-endpoint";

        match parse_command(vec!["server".into(), endpoint.into()]).expect("server endpoint parses")
        {
            ClayCommand::Server {
                endpoint: parsed, ..
            } => {
                assert_eq!(parsed.as_child_arg(), OsString::from(endpoint));
            }
            command => panic!("expected server command, got {command:?}"),
        }

        match parse_command(vec!["client".into(), endpoint.into()]).expect("client endpoint parses")
        {
            ClayCommand::Client { endpoint: parsed } => {
                assert_eq!(parsed.as_child_arg(), OsString::from(endpoint));
            }
            command => panic!("expected client command, got {command:?}"),
        }
    }

    #[test]
    fn rejects_extra_cli_arguments() {
        let error = parse_command(vec!["server".into(), "one".into(), "two".into()])
            .expect_err("extra arguments should fail");
        assert!(error.to_string().contains("unexpected extra argument"));

        let smoke_error = parse_command(vec!["smoke-gui".into(), "manual-endpoint".into()])
            .expect_err("smoke-gui owns endpoint selection");
        assert!(
            smoke_error
                .to_string()
                .contains("unexpected extra argument")
        );
    }

    #[test]
    fn auto_start_uses_current_exe_without_shell() {
        let executable = OsString::from("clay-test-executable");
        let endpoint = clay::ipc::IpcEndpoint::from_argument("clay-test-endpoint");
        let endpoint_arg = endpoint.as_child_arg();
        let command = background_server_command(executable.clone(), &endpoint);

        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_owned())
                .collect::<Vec<_>>(),
            vec![OsString::from("server"), endpoint_arg]
        );
    }

    #[test]
    fn managed_server_command_uses_current_exe_without_shell() {
        let executable = OsString::from("clay-test-executable");
        let endpoint = clay::ipc::smoke_endpoint("gui");
        let endpoint_arg = endpoint.as_child_arg();
        let command = managed_server_command(executable.clone(), &endpoint, None);

        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_owned())
                .collect::<Vec<_>>(),
            vec![OsString::from("server"), endpoint_arg]
        );
    }

    #[test]
    fn smoke_launch_evaluates_runtime_config_fixture() {
        let command = parse_command(vec![
            "smoke-gui".into(),
            "--config-fixture".into(),
            "runtime-sdui".into(),
        ])
        .expect("runtime SDUI smoke fixture parses");

        match command {
            ClayCommand::SmokeGui {
                configuration_root: Some(root),
                ..
            } => {
                assert!(root.ends_with("runtime-sdui"));
                assert!(root.join("init.js").is_file());
            }
            command => panic!("expected smoke GUI fixture command, got {command:?}"),
        }
    }

    #[test]
    fn managed_server_command_forwards_config_fixture_without_shell() {
        let executable = OsString::from("clay-test-executable");
        let endpoint = clay::ipc::smoke_endpoint("gui");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("runtime-sdui");
        let endpoint_arg = endpoint.as_child_arg();
        let command = managed_server_command(executable.clone(), &endpoint, Some(&fixture));

        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_owned())
                .collect::<Vec<_>>(),
            vec![
                OsString::from("server"),
                endpoint_arg,
                OsString::from("--config-fixture"),
                OsString::from("runtime-sdui"),
            ]
        );
    }

    #[tokio::test]
    async fn connect_retry_reports_last_error() {
        let endpoint = clay::ipc::smoke_endpoint("missing-server");
        let error = connect_with_retry(&endpoint)
            .await
            .expect_err("missing server should exhaust readiness retry");

        assert_eq!(error.attempts, 50);
        assert!(matches!(
            error.failure,
            LaunchReadinessFailure::ConnectFailed(_)
        ));
        assert!(error.to_string().contains("did not become ready"));
    }

    #[test]
    fn client_mode_falls_back_with_status_when_server_missing() {
        let endpoint = clay::ipc::smoke_endpoint("fallback-message");
        let error = ClientBootstrapError::Codec(CodecError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing endpoint",
        )));
        let diagnostic = LaunchDiagnostic::local_fallback(&endpoint, &error).to_string();

        assert!(diagnostic.contains("local fallback editor"));
        assert!(diagnostic.contains("TransportUnavailable"));
        assert!(diagnostic.contains(&endpoint.to_string()));
    }

    #[test]
    fn file_dialog_cancellation_is_a_no_op() {
        let result = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Cancelled,
            SelectedPathKind::File,
        );

        assert!(matches!(result, ClientUiCommandResult::None));
    }

    #[test]
    fn file_dialog_result_conversion_reports_selected_and_sanitized_failures() {
        let selected_path = PathBuf::from(r"C:\Users\tester\note.md");
        let selected = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Selected(selected_path.clone()),
            SelectedPathKind::File,
        );
        assert!(
            matches!(selected, ClientUiCommandResult::SelectedFile(path) if path == selected_path)
        );
        let selected_folder = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Selected(selected_path.clone()),
            SelectedPathKind::Folder,
        );
        assert!(
            matches!(selected_folder, ClientUiCommandResult::SelectedFolder(path) if path == selected_path)
        );

        let unsupported = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Unsupported {
                message: "Windows only".to_string(),
            },
            SelectedPathKind::File,
        );
        assert!(matches!(
            unsupported,
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
                if diagnostic.code == "client.file_dialog.unsupported"
                    && diagnostic.message == "Windows only"
        ));

        let failed = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Failed {
                message: "dialog failed".to_string(),
            },
            SelectedPathKind::File,
        );
        assert!(matches!(
            failed,
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
                if diagnostic.code == "client.file_dialog.failed"
                    && diagnostic.message == "dialog failed"
        ));
    }

    #[test]
    fn client_copy_selection_command_routes_to_editor_widget() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientCopySelection".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(result, ClientUiCommandResult::CopySelection));
    }

    #[test]
    fn client_cut_selection_command_routes_to_editor_widget() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientCutSelection".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(result, ClientUiCommandResult::CutSelection));
    }

    #[test]
    fn client_paste_clipboard_command_routes_to_editor_widget() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientPasteClipboard".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(result, ClientUiCommandResult::PasteClipboard));
    }

    #[test]
    fn client_undo_and_redo_commands_route_to_editor_widget() {
        let undo = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientUndo".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(undo, ClientUiCommandResult::Undo));

        let redo = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientRedo".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(redo, ClientUiCommandResult::Redo));
        let show = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientShowOpenDocuments".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(show, ClientUiCommandResult::ShowOpenDocuments));
        let resync = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientRequestResync".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(resync, ClientUiCommandResult::RequestResync));
        let dismiss = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientDismissRecovery".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(dismiss, ClientUiCommandResult::DismissRecovery));
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    #[test]
    fn unsupported_platform_client_open_file_dialog_command_reports_status_diagnostic() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "documents.clientOpenFileDialog".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(
            result,
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
                if diagnostic.code == "client.file_dialog.unsupported"
                    && diagnostic.message.contains("not supported on this platform")
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_native_dialog_commands_use_non_blocking_driver_path() {
        assert!(is_linux_portal_dialog_command(
            "documents.clientOpenFileDialog"
        ));
        assert!(is_linux_portal_dialog_command(
            "workspace.clientOpenFolderDialog"
        ));
        assert!(!is_linux_portal_dialog_command(
            "editor.clientCopySelection"
        ));
    }

    #[test]
    fn native_dialog_generations_limit_duplicates_and_reject_stale_results() {
        let mut driver = test_driver_with_tabs(BTreeMap::new());

        let file_generation = driver.reserve_file_dialog().expect("first file dialog");
        let folder_generation = driver.reserve_folder_dialog().expect("first folder dialog");
        assert_eq!(driver.reserve_file_dialog(), None);
        assert_eq!(driver.reserve_folder_dialog(), None);

        assert!(driver.finish_file_dialog(file_generation));
        let next_file_generation = driver.reserve_file_dialog().expect("next file dialog");
        assert_ne!(next_file_generation, file_generation);
        assert!(!driver.finish_file_dialog(file_generation));
        assert_eq!(driver.file_dialog_in_flight, Some(next_file_generation));
        assert!(driver.finish_file_dialog(next_file_generation));

        driver.clear_native_dialogs();
        assert_eq!(driver.file_dialog_in_flight, None);
        assert_eq!(driver.folder_dialog_in_flight, None);
        assert!(!driver.finish_folder_dialog(folder_generation));
    }

    #[tokio::test]
    async fn smoke_mode_fails_if_child_server_exits_before_ready() {
        let endpoint = clay::ipc::smoke_endpoint("early-exit");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn helper process");

        let error = connect_with_retry_while(&endpoint, || child.try_wait())
            .await
            .expect_err("exited child should fail smoke readiness");
        let _ = child.wait();

        assert!(matches!(
            error.failure,
            LaunchReadinessFailure::ChildExited(_)
        ));
        assert!(error.to_string().contains("exited before readiness"));
    }

    #[test]
    fn editor_appends_input() {
        let mut editor = EditorSurface::default();

        editor.insert_text("Hello");
        editor.insert_text(", Clay");

        assert_eq!(editor.visible_text(), "Hello, Clay");
    }

    #[test]
    fn editor_backspace_removes_last_scalar() {
        let mut editor = EditorSurface::default();
        editor.insert_text("aé🦀");

        editor.backspace();
        assert_eq!(editor.visible_text(), "aé");

        editor.backspace();
        assert_eq!(editor.visible_text(), "a");

        editor.backspace();
        assert_eq!(editor.visible_text(), "");

        editor.backspace();
        assert_eq!(editor.visible_text(), "");
    }

    #[test]
    fn printable_text_filter_accepts_plain_text_and_rejects_controls() {
        assert!(is_printable_text("abc é 🦀"));
        assert!(!is_printable_text(""));
        assert!(!is_printable_text("\r"));
        assert!(!is_printable_text("\n"));
        assert!(!is_printable_text("a\n"));
    }

    #[test]
    fn tab_command_ids_route_to_shell_tab_variants() {
        for (id, expected) in [
            (
                "shell.clientTabNext",
                clay::masonry_shell::ShellClientCommand::TabNext,
            ),
            (
                "shell.clientTabPrev",
                clay::masonry_shell::ShellClientCommand::TabPrev,
            ),
            (
                "shell.clientTabNew",
                clay::masonry_shell::ShellClientCommand::TabNew,
            ),
            (
                "shell.clientTabClose",
                clay::masonry_shell::ShellClientCommand::TabClose,
            ),
            (
                "shell.clientTabMoveLeft",
                clay::masonry_shell::ShellClientCommand::TabMoveLeft,
            ),
            (
                "shell.clientTabMoveRight",
                clay::masonry_shell::ShellClientCommand::TabMoveRight,
            ),
            (
                "shell.clientTabActivate.3",
                clay::masonry_shell::ShellClientCommand::TabActivate(3),
            ),
            (
                "shell.clientTabMoveTo.9",
                clay::masonry_shell::ShellClientCommand::TabMoveTo(9),
            ),
        ] {
            let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
                command_id: id.to_string(),
                routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
            });
            assert!(
                matches!(result, ClientUiCommandResult::ShellCommand(command) if command == expected),
                "{id} must route to {expected:?}"
            );
        }
    }
}
