use std::{
    error::Error,
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::Duration,
};

use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::theme::default_property_set;
use masonry_winit::app::{
    AppDriver, DriverCtx, EventLoop, EventLoopProxy, MasonryUserEvent, NewWindow, WindowId,
};
use masonry_winit::winit::dpi::LogicalSize;
use masonry_winit::winit::window::Window;
use tokio::sync::mpsc;

use clay::client::{self, ClientConnectionEvent};
use clay::ipc::{IpcEndpoint, default_endpoint, smoke_endpoint};
use clay::masonry_editor::{EditorAction, EditorStatus, EditorWidget};
use clay::masonry_shell::ClayShellWidget;
use clay::perf::fixtures::{FixtureKind, FixtureSpec, default_fixture_path, generate_fixture_file};
use clay::perf::metrics::{PERF_PROFILE_FLAG, PerfConfig, install_global_recorder};
#[cfg(any(unix, windows))]
use clay::server::{IpcServer, ServerConfig};

const WINDOW_TITLE: &str = "Clay Phase 4";
const WINDOW_WIDTH: f64 = 900.0;
const WINDOW_HEIGHT: f64 = 600.0;

struct Driver {
    editor_widget_id: WidgetId,
}

impl Driver {
    fn editor_action_target(&self, _source_widget_id: WidgetId) -> WidgetId {
        // Phase 18.2 has one editor component under the shell root. Keep
        // editor-specific actions aimed at that child even if Masonry reports a
        // shell/root source while the container boundary is settling.
        self.editor_widget_id
    }
}

impl AppDriver for Driver {
    fn on_start(&mut self, state: &mut masonry_winit::app::MasonryState<'_>) {
        for root in state.roots() {
            root.set_focus_fallback(Some(self.editor_widget_id));
        }
    }

    fn on_action(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_, '_>,
        widget_id: WidgetId,
        action: ErasedAction,
    ) {
        let Ok(action) = action.downcast::<EditorAction>() else {
            return;
        };

        match *action {
            EditorAction::ExitRequested => ctx.exit(),
            EditorAction::ClientConnection(event) => {
                let editor_widget_id = self.editor_action_target(widget_id);
                ctx.render_root(window_id)
                    .edit_widget(editor_widget_id, |mut widget| {
                        if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                            let changed = editor.widget.apply_connection_event(event);
                            if changed {
                                editor.ctx.request_render();
                                editor.ctx.request_accessibility_update();
                            }
                        }
                    });
            }
            EditorAction::ClientUiCommand(command) => match handle_client_ui_command(&command) {
                ClientUiCommandResult::None => {}
                ClientUiCommandResult::ConnectionEvent(event) => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed = editor.widget.apply_connection_event(event);
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::SelectedFile(path) => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed =
                                    editor.widget.request_selected_file_open(path).is_some_and(
                                        |event| editor.widget.apply_connection_event(event),
                                    );
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::SelectedFolder(path) => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed = editor
                                    .widget
                                    .request_selected_workspace_root(path)
                                    .is_some_and(|event| {
                                        editor.widget.apply_connection_event(event)
                                    });
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::CopySelection => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed = editor
                                    .widget
                                    .copy_selection_to_system_clipboard()
                                    .is_some_and(|event| {
                                        editor.widget.apply_connection_event(event)
                                    });
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
            },
        }
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "GUI command results are low-volume and only one variant carries a selected path"
)]
enum ClientUiCommandResult {
    None,
    ConnectionEvent(ClientConnectionEvent),
    SelectedFile(PathBuf),
    SelectedFolder(PathBuf),
    CopySelection,
}

fn handle_client_ui_command(command: &clay::client::ClientUiCommandRoute) -> ClientUiCommandResult {
    match command.command_id.as_str() {
        "clay.documents.clientOpenFileDialog" => client_dialog_result_to_command_result(
            clay::client::open_markdown_file_dialog(),
            SelectedPathKind::File,
        ),
        "clay.workspace.clientOpenFolderDialog" => client_dialog_result_to_command_result(
            clay::client::open_folder_dialog(),
            SelectedPathKind::Folder,
        ),
        "clay.editor.clientCopySelection" => ClientUiCommandResult::CopySelection,
        _ => ClientUiCommandResult::None,
    }
}

#[derive(Clone, Copy)]
enum SelectedPathKind {
    File,
    Folder,
}

fn client_dialog_result_to_command_result(
    result: clay::client::FileDialogResult,
    kind: SelectedPathKind,
) -> ClientUiCommandResult {
    match result {
        clay::client::FileDialogResult::Selected(path) => match kind {
            SelectedPathKind::File => ClientUiCommandResult::SelectedFile(path),
            SelectedPathKind::Folder => ClientUiCommandResult::SelectedFolder(path),
        },
        clay::client::FileDialogResult::Cancelled => ClientUiCommandResult::None,
        clay::client::FileDialogResult::Unsupported { message } => {
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(
                clay::protocol::RuntimeDiagnostic::error(
                    "clay.client.file_dialog.unsupported",
                    message,
                ),
            ))
        }
        clay::client::FileDialogResult::Failed { message } => {
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(
                clay::protocol::RuntimeDiagnostic::error("clay.client.file_dialog.failed", message),
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClayCommand {
    Auto {
        endpoint: IpcEndpoint,
    },
    Client {
        endpoint: IpcEndpoint,
    },
    Server {
        endpoint: IpcEndpoint,
        configuration_root: Option<PathBuf>,
    },
    SmokeGui {
        endpoint: IpcEndpoint,
        configuration_root: Option<PathBuf>,
    },
    PerfFixture {
        kind: FixtureKind,
        size_mib: usize,
        seed: u64,
        output: Option<PathBuf>,
    },
    Help,
    Package {
        subcommand: PackageCliSubcommand,
    },
}

/// Subcommand for `clay package <op> [args...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PackageCliSubcommand {
    /// Install a package by spec (delegates to the configured npm-compatible manager).
    Add {
        package_spec: String,
        /// Allow third-party lifecycle scripts to run during install.
        allow_scripts: bool,
    },
    /// Remove an installed package.
    Remove { package_name: String },
    /// List all installed packages and their enabled status.
    List,
    /// Enable a previously installed package (runs Clay-owned validation).
    Enable { package_name: String },
    /// Disable a currently enabled package.
    Disable { package_name: String },
    /// Inspect metadata for a specific package.
    Inspect { package_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(formatter, "{}", self.message)?;
        formatter.write_str(CLI_USAGE)
    }
}

impl Error for CliError {}

const CLI_USAGE: &str = "Usage:\n  clay\n  clay server [endpoint] [--config-fixture <name>]\n  clay client [endpoint]\n  clay smoke-gui [--config-fixture <name>]\n  clay perf-fixture --kind <kind> --size-mib <n> [--output <path>] [--seed <n>]\n  clay package add <spec> [--allow-scripts]\n  clay package remove <name>\n  clay package list\n  clay package enable <name>\n  clay package disable <name>\n  clay package inspect <name>\n  clay <endpoint>\n\nModes:\n  clay                  Connect to the default local endpoint, start a background server if missing, then open the GUI.\n  clay server           Run a foreground server on the default local endpoint.\n  clay client           Connect to the default local endpoint, or open a local fallback GUI if missing.\n  clay smoke-gui        App-managed GUI smoke mode; starts an isolated child server, opens a client, then cleans up.\n  clay perf-fixture     Generate deterministic large UTF-8 plain-text performance fixtures.\n  clay package         Manage Clay packages (install/enable/disable/list/inspect).\n  clay <endpoint>       Advanced debugging shorthand for 'clay client <endpoint>'.\n\nOptions:\n  --config-fixture <name>  Development smoke fixture under tests/fixtures/configuration/<name>.\n  --allow-scripts          Allow package lifecycle scripts during `clay package add` (dangerous).\n  --profile-perf          Enable internal developer performance metric snapshots for this process.\n\nEnvironment:\n  CLAY_ALLOW_LIFECYCLE_SCRIPTS=1  Same as --allow-scripts (dangerous).\n\nPerf fixture kinds:\n  long-lines, many-short-lines, mixed-unicode, newline-heavy\n";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchDiagnostic {
    message: String,
}

impl LaunchDiagnostic {
    fn server_starting(endpoint: &IpcEndpoint) -> Self {
        Self::new(format!(
            "clay server starting on local IPC endpoint {endpoint}"
        ))
    }

    fn smoke_server_starting(endpoint: &IpcEndpoint) -> Self {
        Self::new(format!(
            "clay smoke-gui starting managed local server at {endpoint}"
        ))
    }

    fn connected(endpoint: &IpcEndpoint) -> Self {
        Self::new(format!("clay client connected to {endpoint}"))
    }

    fn auto_starting_server(endpoint: &IpcEndpoint, error: &client::ClientBootstrapError) -> Self {
        Self::new(format!(
            "no Clay server was ready at {endpoint} ({:?}: {error}); starting a background local server",
            error.kind()
        ))
    }

    fn local_fallback(endpoint: &IpcEndpoint, error: &client::ClientBootstrapError) -> Self {
        Self::new(format!(
            "Clay server unavailable at {endpoint} ({:?}: {error}); opening a local fallback editor",
            error.kind()
        ))
    }

    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for LaunchDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Debug)]
struct LaunchError {
    endpoint: IpcEndpoint,
    failure: LaunchReadinessFailure,
    attempts: usize,
}

impl LaunchError {
    fn readiness(endpoint: IpcEndpoint, attempts: usize, failure: LaunchReadinessFailure) -> Self {
        Self {
            endpoint,
            attempts,
            failure,
        }
    }

    fn server_start_failed(endpoint: IpcEndpoint, error: impl Into<String>) -> Self {
        Self::readiness(
            endpoint,
            0,
            LaunchReadinessFailure::ServerStart(error.into()),
        )
    }
}

#[derive(Debug)]
enum LaunchReadinessFailure {
    ConnectFailed(client::ClientBootstrapError),
    ChildExited(ExitStatus),
    ChildStatus(std::io::Error),
    ServerStart(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.failure {
            LaunchReadinessFailure::ConnectFailed(error) => write!(
                formatter,
                "Clay server at {} did not become ready after {} attempts ({:?}: {error})",
                self.endpoint,
                self.attempts,
                error.kind()
            ),
            LaunchReadinessFailure::ChildExited(status) => write!(
                formatter,
                "managed Clay server for {} exited before readiness after {} attempts with status {status}",
                self.endpoint, self.attempts
            ),
            LaunchReadinessFailure::ChildStatus(error) => write!(
                formatter,
                "failed to inspect managed Clay server for {} after {} attempts: {error}",
                self.endpoint, self.attempts
            ),
            LaunchReadinessFailure::ServerStart(error) => write!(
                formatter,
                "Clay server failed to start on {}: {error}",
                self.endpoint
            ),
        }
    }
}

impl Error for LaunchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.failure {
            LaunchReadinessFailure::ConnectFailed(error) => Some(error),
            LaunchReadinessFailure::ChildStatus(error) => Some(error),
            LaunchReadinessFailure::ChildExited(_) | LaunchReadinessFailure::ServerStart(_) => None,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let (args, profile_perf) = extract_profile_perf_flag(std::env::args_os().skip(1));
    install_global_recorder(PerfConfig::from_env().with_flag(profile_perf));

    match parse_command(args)? {
        ClayCommand::Server {
            endpoint,
            configuration_root,
        } => run_server(endpoint, configuration_root),
        ClayCommand::Client { endpoint } => run_client(endpoint, false),
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

fn extract_profile_perf_flag(args: impl Iterator<Item = OsString>) -> (Vec<OsString>, bool) {
    let mut profile_perf = false;
    let mut retained = Vec::new();
    for argument in args {
        if argument == PERF_PROFILE_FLAG {
            profile_perf = true;
        } else {
            retained.push(argument);
        }
    }
    (retained, profile_perf)
}

fn parse_command(args: Vec<OsString>) -> Result<ClayCommand, CliError> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(ClayCommand::Auto {
            endpoint: default_endpoint(),
        });
    };

    match first.to_string_lossy().as_ref() {
        "help" | "--help" | "-h" => Ok(ClayCommand::Help),
        "server" | "--server" => parse_server_subcommand(args),
        "client" | "--client" => parse_endpoint_subcommand("client", args)
            .map(|endpoint| ClayCommand::Client { endpoint }),
        "smoke-gui" | "smoke" | "--smoke-gui" => parse_smoke_gui_subcommand(args),
        "perf-fixture" => parse_perf_fixture_subcommand(args),
        "package" => parse_package_subcommand(args),
        _ => {
            if let Some(extra) = args.next() {
                return Err(CliError::new(format!(
                    "unexpected extra argument after endpoint shorthand: {}",
                    extra.to_string_lossy()
                )));
            }
            Ok(ClayCommand::Client {
                endpoint: IpcEndpoint::from_argument(first),
            })
        }
    }
}

fn parse_perf_fixture_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let mut kind = None;
    let mut size_mib = None;
    let mut seed = 0xC1A4_F14E;
    let mut output = None;
    let mut args = args.peekable();

    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--kind" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --kind for 'perf-fixture'",
                    ));
                };
                let value = value.to_string_lossy();
                kind = Some(FixtureKind::parse(&value).ok_or_else(|| {
                    CliError::new(format!("unknown performance fixture kind '{value}'"))
                })?);
            }
            "--size-mib" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --size-mib for 'perf-fixture'",
                    ));
                };
                size_mib = Some(parse_positive_usize("--size-mib", &value)?);
            }
            "--seed" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --seed for 'perf-fixture'",
                    ));
                };
                seed = parse_u64("--seed", &value)?;
            }
            "--output" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --output for 'perf-fixture'",
                    ));
                };
                output = Some(PathBuf::from(value));
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for 'perf-fixture': {other}"
                )));
            }
        }
    }

    Ok(ClayCommand::PerfFixture {
        kind: kind.ok_or_else(|| CliError::new("missing --kind for 'perf-fixture'"))?,
        size_mib: size_mib.ok_or_else(|| CliError::new("missing --size-mib for 'perf-fixture'"))?,
        seed,
        output,
    })
}

fn parse_positive_usize(option: &str, value: &OsString) -> Result<usize, CliError> {
    let text = value.to_string_lossy();
    let parsed = text
        .parse::<usize>()
        .map_err(|_| CliError::new(format!("invalid numeric value for {option}: {text}")))?;
    if parsed == 0 {
        return Err(CliError::new(format!("{option} must be greater than zero")));
    }
    Ok(parsed)
}

fn parse_u64(option: &str, value: &OsString) -> Result<u64, CliError> {
    let text = value.to_string_lossy();
    text.parse::<u64>()
        .map_err(|_| CliError::new(format!("invalid numeric value for {option}: {text}")))
}

fn parse_endpoint_subcommand(
    mode: &str,
    mut args: impl Iterator<Item = OsString>,
) -> Result<IpcEndpoint, CliError> {
    let endpoint = args
        .next()
        .map(IpcEndpoint::from_argument)
        .unwrap_or_else(default_endpoint);

    if let Some(extra) = args.next() {
        return Err(CliError::new(format!(
            "unexpected extra argument for '{mode}': {}",
            extra.to_string_lossy()
        )));
    }

    Ok(endpoint)
}

fn parse_server_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let (endpoint, configuration_root) = parse_endpoint_and_config_fixture("server", args, true)?;
    Ok(ClayCommand::Server {
        endpoint,
        configuration_root,
    })
}

fn parse_smoke_gui_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let (_endpoint, configuration_root) =
        parse_endpoint_and_config_fixture("smoke-gui", args, false)?;
    Ok(ClayCommand::SmokeGui {
        endpoint: smoke_endpoint("gui"),
        configuration_root,
    })
}

fn parse_endpoint_and_config_fixture(
    mode: &str,
    args: impl Iterator<Item = OsString>,
    allow_endpoint: bool,
) -> Result<(IpcEndpoint, Option<PathBuf>), CliError> {
    let mut endpoint = None;
    let mut configuration_root = None;

    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        if argument == "--config-fixture" {
            let Some(name) = args.next() else {
                return Err(CliError::new(format!(
                    "missing fixture name after --config-fixture for '{mode}'"
                )));
            };
            if configuration_root.is_some() {
                return Err(CliError::new(format!(
                    "duplicate --config-fixture option for '{mode}'"
                )));
            }
            configuration_root = Some(resolve_config_fixture(&name)?);
        } else if allow_endpoint && endpoint.is_none() {
            endpoint = Some(IpcEndpoint::from_argument(argument));
        } else {
            return Err(CliError::new(format!(
                "unexpected extra argument for '{mode}': {}",
                argument.to_string_lossy()
            )));
        }
    }

    Ok((
        endpoint.unwrap_or_else(default_endpoint),
        configuration_root,
    ))
}

fn resolve_config_fixture(name: &OsString) -> Result<PathBuf, CliError> {
    let name = name.to_string_lossy();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(CliError::new(format!(
            "invalid configuration fixture name '{name}'"
        )));
    }

    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join(name.as_ref());
    if !fixture_root.join("init.js").is_file() {
        return Err(CliError::new(format!(
            "configuration fixture '{}' does not contain init.js",
            name
        )));
    }
    Ok(fixture_root)
}

fn parse_package_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let mut args = args.peekable();
    let Some(op) = args.next() else {
        return Err(CliError::new(
            "clay package requires a subcommand: add | remove | list | enable | disable | inspect",
        ));
    };
    match op.to_string_lossy().as_ref() {
        "add" => {
            let mut spec = None;
            let mut allow_scripts = false;
            for arg in args {
                let text = arg.to_string_lossy();
                if text == "--allow-scripts" {
                    allow_scripts = true;
                } else if spec.is_none() {
                    spec = Some(arg);
                } else {
                    return Err(CliError::new(
                        "clay package add takes one package spec and optional --allow-scripts",
                    ));
                }
            }
            let spec = spec.ok_or_else(|| {
                CliError::new("clay package add requires a package spec, e.g. @clay/markdown")
            })?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Add {
                    package_spec: spec.to_string_lossy().into_owned(),
                    allow_scripts,
                },
            })
        }
        "remove" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package remove requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Remove {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "list" => Ok(ClayCommand::Package {
            subcommand: PackageCliSubcommand::List,
        }),
        "enable" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package enable requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Enable {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "disable" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package disable requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Disable {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "inspect" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package inspect requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Inspect {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        unknown => Err(CliError::new(format!(
            "unknown clay package subcommand `{unknown}`; expected: add | remove | list | enable | disable | inspect"
        ))),
    }
}

fn run_package_subcommand(subcommand: PackageCliSubcommand) -> Result<(), Box<dyn Error>> {
    use clay::packages::manager::PnpmBackend;
    use clay::packages::service::PackageService;

    // Default store: ~/.config/clay/packages
    let store_root = dirs_home_config_clay_packages();
    let mut service = PackageService::new(store_root, Box::new(PnpmBackend::new()));

    // A fresh service starts with an empty installed map. Repopulate it from
    // the package-manager store so `list`/`enable`/`disable`/`inspect`/`remove`
    // reflect packages installed by previous `clay package add` invocations.
    // `add` skips this: it installs via the backend (which re-discovers
    // internally) and a missing pnpm binary should fail at `pnpm add`, not at
    // the pre-list step.
    if !matches!(&subcommand, PackageCliSubcommand::Add { .. }) {
        service.refresh_installed()?;
    }

    match subcommand {
        PackageCliSubcommand::Add {
            package_spec,
            allow_scripts,
        } => {
            let allow_scripts = allow_scripts
                || std::env::var_os("CLAY_ALLOW_LIFECYCLE_SCRIPTS")
                    .is_some_and(|value| value == "1" || value == "true");
            println!("Installing {package_spec}…");
            service.install(
                &package_spec,
                clay::packages::manager::PackageInstallOptions {
                    allow_lifecycle_scripts: allow_scripts,
                },
            )?;
            println!("Installed {package_spec}");
        }
        PackageCliSubcommand::Remove { package_name } => {
            println!("Removing {package_name}…");
            service.remove(&package_name)?;
            println!("Removed {package_name}");
        }
        PackageCliSubcommand::List => {
            let packages = service.list();
            if packages.is_empty() {
                println!("No packages installed.");
            } else {
                for pkg in &packages {
                    let status = if pkg.is_enabled {
                        "[enabled]"
                    } else {
                        "[installed]"
                    };
                    println!("  {} {} {} {status}", pkg.name, pkg.version, pkg.api_prefix);
                }
            }
        }
        PackageCliSubcommand::Enable { package_name } => {
            println!("Enabling {package_name}…");
            service.enable(&package_name)?;
            println!("Enabled {package_name}");
        }
        PackageCliSubcommand::Disable { package_name } => {
            println!("Disabling {package_name}…");
            service.disable(&package_name)?;
            println!("Disabled {package_name}");
        }
        PackageCliSubcommand::Inspect { package_name } => match service.inspect(&package_name) {
            Some(inspection) => {
                println!("Package:     {}", inspection.name);
                println!("Version:     {}", inspection.version);
                println!("API prefix:  {}", inspection.api_prefix);
                println!(
                    "Status:      {}",
                    if inspection.is_enabled {
                        "enabled"
                    } else {
                        "installed"
                    }
                );
                println!("Modes:       {:?}", inspection.modes);
                println!("Permissions: {:?}", inspection.permissions);
                println!("Commands:    {}", inspection.command_count);
                println!("Config keys: {}", inspection.configuration_count);
                if let Some(docs) = &inspection.docs_path {
                    println!("Docs:        {docs}");
                }
            }
            None => eprintln!("Package `{package_name}` is not installed."),
        },
    }
    Ok(())
}

fn dirs_home_config_clay_packages() -> std::path::PathBuf {
    // Prefer the platform config dir; fall back to the current directory.
    let base = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from);
    match base {
        Some(home) => home.join(".config").join("clay").join("packages"),
        None => std::path::PathBuf::from(".clay-packages"),
    }
}

#[cfg(any(unix, windows))]
fn run_server(
    endpoint: IpcEndpoint,
    configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    eprintln!("{}", LaunchDiagnostic::server_starting(&endpoint));
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = configuration_root;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(IpcServer::new(config).run())
        .map_err(|error| LaunchError::server_start_failed(endpoint, error.to_string()))?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn run_server(
    endpoint: IpcEndpoint,
    _configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    Err(format!("Clay server IPC is unsupported on this platform: {endpoint}").into())
}

fn run_perf_fixture(
    kind: FixtureKind,
    size_mib: usize,
    seed: u64,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let size_bytes = size_mib
        .checked_mul(1024 * 1024)
        .ok_or("--size-mib is too large")?;
    let spec = FixtureSpec {
        kind,
        size_bytes,
        seed,
    };
    let output = output.unwrap_or_else(|| default_fixture_path(kind, size_mib));
    let output = generate_fixture_file(&spec, &output)?;
    println!(
        "generated {} MiB {} fixture at {}",
        size_mib,
        kind.as_str(),
        output.display()
    );
    Ok(())
}

fn run_client(endpoint: IpcEndpoint, start_server_if_missing: bool) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let client_session = match runtime.block_on(client::connect(&endpoint)) {
        Ok(session) => {
            eprintln!("{}", LaunchDiagnostic::connected(&endpoint));
            Some(session)
        }
        Err(connect_error) if start_server_if_missing => {
            eprintln!(
                "{}",
                LaunchDiagnostic::auto_starting_server(&endpoint, &connect_error)
            );
            start_background_server(&endpoint)?;
            Some(runtime.block_on(connect_with_retry(&endpoint))?)
        }
        Err(connect_error) => {
            eprintln!(
                "{}",
                LaunchDiagnostic::local_fallback(&endpoint, &connect_error)
            );
            None
        }
    };

    let (editor_widget, events) = if let Some(session) = client_session {
        editor_widget_from_session(session)
    } else {
        (
            EditorWidget::default().with_status(EditorStatus::local_fallback()),
            None,
        )
    };

    run_editor(editor_widget, events, &runtime)
}

fn run_smoke_gui(
    endpoint: IpcEndpoint,
    configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let executable = std::env::current_exe()?.into_os_string();
    let mut server = ManagedServer::spawn(executable, &endpoint, configuration_root.as_deref())?;

    eprintln!("{}", LaunchDiagnostic::smoke_server_starting(&endpoint));
    let session = runtime.block_on(connect_with_retry_while(&endpoint, || server.try_wait()))?;
    eprintln!("{}", LaunchDiagnostic::connected(&endpoint));
    let (editor_widget, events) = editor_widget_from_session(session);
    let result = run_editor(editor_widget, events, &runtime);
    server.shutdown();
    result
}

fn start_background_server(endpoint: &IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?.into_os_string();
    background_server_command(executable, endpoint).spawn()?;
    Ok(())
}

struct ManagedServer {
    child: Option<Child>,
    endpoint: IpcEndpoint,
}

impl ManagedServer {
    fn spawn(
        executable: OsString,
        endpoint: &IpcEndpoint,
        configuration_root: Option<&Path>,
    ) -> Result<Self, Box<dyn Error>> {
        let child = managed_server_command(executable, endpoint, configuration_root).spawn()?;
        Ok(Self {
            child: Some(child),
            endpoint: endpoint.clone(),
        })
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, std::io::Error> {
        match self.child.as_mut() {
            Some(child) => child.try_wait(),
            None => Ok(None),
        }
    }

    fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            match child.try_wait() {
                Ok(Some(_status)) => {}
                Ok(None) => {
                    if let Err(error) = child.kill() {
                        eprintln!("failed to stop managed Clay server: {error}");
                    }
                    if let Err(error) = child.wait() {
                        eprintln!("failed to wait for managed Clay server shutdown: {error}");
                    }
                }
                Err(error) => eprintln!("failed to inspect managed Clay server: {error}"),
            }
        }
        cleanup_managed_endpoint(&self.endpoint);
    }
}

fn cleanup_managed_endpoint(endpoint: &IpcEndpoint) {
    #[cfg(unix)]
    if let Err(error) = std::fs::remove_file(endpoint.as_unix_socket_path())
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("failed to remove managed Clay socket {endpoint}: {error}");
    }

    #[cfg(not(unix))]
    let _ = endpoint;
}

impl Drop for ManagedServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn background_server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = server_command(executable, endpoint);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn managed_server_command(
    executable: OsString,
    endpoint: &IpcEndpoint,
    configuration_root: Option<&Path>,
) -> Command {
    let mut command = server_command(executable, endpoint);
    if let Some(configuration_root) = configuration_root {
        command.arg("--config-fixture").arg(
            configuration_root
                .file_name()
                .expect("fixture root has a name"),
        );
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = Command::new(executable);
    command.arg("server").arg(endpoint.as_child_arg());
    command
}

fn editor_widget_from_session(
    session: client::ClientSession,
) -> (EditorWidget, Option<mpsc::Receiver<ClientConnectionEvent>>) {
    let client::ClientSession {
        initial_state,
        edit_queue,
        events,
    } = session;
    (
        EditorWidget::with_initial_state(initial_state).with_edit_queue(edit_queue),
        Some(events),
    )
}

async fn connect_with_retry(endpoint: &IpcEndpoint) -> Result<client::ClientSession, LaunchError> {
    connect_with_retry_while(endpoint, || Ok(None)).await
}

async fn connect_with_retry_while(
    endpoint: &IpcEndpoint,
    mut check_child_exit: impl FnMut() -> Result<Option<ExitStatus>, std::io::Error>,
) -> Result<client::ClientSession, LaunchError> {
    let mut last_error = None;
    for attempt in 1..=50 {
        if let Some(status) = check_child_exit().map_err(|error| {
            LaunchError::readiness(
                endpoint.clone(),
                attempt,
                LaunchReadinessFailure::ChildStatus(error),
            )
        })? {
            return Err(LaunchError::readiness(
                endpoint.clone(),
                attempt,
                LaunchReadinessFailure::ChildExited(status),
            ));
        }

        match client::connect(endpoint).await {
            Ok(session) => return Ok(session),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }

    Err(LaunchError::readiness(
        endpoint.clone(),
        50,
        LaunchReadinessFailure::ConnectFailed(
            last_error.expect("connect retry loop always records an error"),
        ),
    ))
}

fn run_editor(
    editor_widget: EditorWidget,
    events: Option<mpsc::Receiver<ClientConnectionEvent>>,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), Box<dyn Error>> {
    let shell_widget = ClayShellWidget::single_editor(editor_widget);
    let editor_widget_id = shell_widget.editor_widget_id();
    let root_widget = NewWidget::new(shell_widget);
    let window_id = WindowId::next();
    let window_attributes = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    let event_loop = EventLoop::with_user_event().build()?;

    if let Some(events) = events {
        spawn_client_connection_event_bridge(
            runtime,
            events,
            event_loop.create_proxy(),
            window_id,
            editor_widget_id,
        );
    }

    masonry_winit::app::run_with(
        event_loop,
        vec![NewWindow::new_with_id(
            window_id,
            window_attributes,
            root_widget.erased(),
        )],
        Driver { editor_widget_id },
        default_property_set(),
    )?;

    Ok(())
}

fn spawn_client_connection_event_bridge(
    runtime: &tokio::runtime::Runtime,
    mut events: mpsc::Receiver<ClientConnectionEvent>,
    proxy: EventLoopProxy,
    window_id: WindowId,
    editor_widget_id: WidgetId,
) {
    runtime.spawn(async move {
        while let Some(event) = events.recv().await {
            eprintln!("clay client IPC event: {event:?}");
            if proxy
                .send_event(connection_event_user_event(
                    window_id,
                    editor_widget_id,
                    event,
                ))
                .is_err()
            {
                break;
            }
        }
    });
}

fn connection_event_user_event(
    window_id: WindowId,
    editor_widget_id: WidgetId,
    event: ClientConnectionEvent,
) -> MasonryUserEvent {
    MasonryUserEvent::Action(
        window_id,
        Box::new(EditorAction::ClientConnection(event)),
        editor_widget_id,
    )
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    #[cfg(not(windows))]
    use super::handle_client_ui_command;
    use super::{
        ClayCommand, ClientUiCommandResult, Driver, FixtureKind, LaunchDiagnostic,
        LaunchReadinessFailure, SelectedPathKind, background_server_command,
        client_dialog_result_to_command_result, connect_with_retry, connect_with_retry_while,
        connection_event_user_event, extract_profile_perf_flag, managed_server_command,
        parse_command,
    };
    use clay::client::{ClientBootstrapError, ClientConnectionEvent};
    use clay::editor::{EditorSurface, is_printable_text};
    use clay::ipc::default_endpoint;
    use clay::protocol::codec::CodecError;
    use masonry::core::WidgetId;
    use masonry_winit::app::{MasonryUserEvent, WindowId};

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
            vec!["smoke-gui".into()],
        ] {
            let command = parse_command(args).expect("mode parses with default endpoint");
            match command {
                ClayCommand::Auto { endpoint }
                | ClayCommand::Client { endpoint }
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

        for args in [vec![], vec!["server".into()], vec!["client".into()]] {
            let command = parse_command(args).expect("default launch mode parses");
            let endpoint = match command {
                ClayCommand::Auto { endpoint }
                | ClayCommand::Client { endpoint }
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
                if diagnostic.code == "clay.client.file_dialog.unsupported"
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
                if diagnostic.code == "clay.client.file_dialog.failed"
                    && diagnostic.message == "dialog failed"
        ));
    }

    #[test]
    fn client_copy_selection_command_routes_to_editor_widget() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "clay.editor.clientCopySelection".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(result, ClientUiCommandResult::CopySelection));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_client_open_file_dialog_command_reports_status_diagnostic() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "clay.documents.clientOpenFileDialog".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(
            result,
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
                if diagnostic.code == "clay.client.file_dialog.unsupported"
                    && diagnostic.message.contains("Windows only")
        ));
    }

    #[test]
    fn connection_event_action_is_dispatched_to_shell_editor_child() {
        let window_id = WindowId::next();
        let shell = clay::masonry_shell::ClayShellWidget::single_editor(
            clay::masonry_editor::EditorWidget::default(),
        );
        let widget_id = shell.editor_widget_id();
        let event = ClientConnectionEvent::Disconnected;

        let user_event = connection_event_user_event(window_id, widget_id, event.clone());

        match user_event {
            MasonryUserEvent::Action(action_window_id, action, action_widget_id) => {
                assert_eq!(action_window_id, window_id);
                assert_eq!(action_widget_id, widget_id);
                assert_eq!(
                    *action
                        .downcast::<clay::masonry_editor::EditorAction>()
                        .expect("connection action type"),
                    clay::masonry_editor::EditorAction::ClientConnection(event)
                );
            }
            MasonryUserEvent::AccessKit(..) => panic!("connection events must use actions"),
        }
    }

    #[test]
    fn driver_routes_editor_actions_to_shell_editor_child() {
        let editor_widget_id = WidgetId::next();
        let shell_or_source_widget_id = WidgetId::next();
        let driver = Driver { editor_widget_id };

        assert_eq!(
            driver.editor_action_target(shell_or_source_widget_id),
            editor_widget_id
        );
    }

    #[test]
    fn smoke_launch_routes_sdui_events_to_gui() {
        let window_id = WindowId::next();
        let shell = clay::masonry_shell::ClayShellWidget::single_editor(
            clay::masonry_editor::EditorWidget::default(),
        );
        let widget_id = shell.editor_widget_id();
        let event = ClientConnectionEvent::SduiSnapshot {
            client_id: 1,
            tree: clay::protocol::SduiTree {
                ui_version: 1,
                root_id: clay::protocol::SduiNodeId(1),
                nodes: vec![clay::protocol::SduiNode::new(
                    clay::protocol::SduiNodeId(1),
                    clay::protocol::SduiNodeKind::Label {
                        text: "Workspace".to_string(),
                    },
                )],
            },
        };

        let user_event = connection_event_user_event(window_id, widget_id, event.clone());

        match user_event {
            MasonryUserEvent::Action(action_window_id, action, action_widget_id) => {
                assert_eq!(action_window_id, window_id);
                assert_eq!(action_widget_id, widget_id);
                assert_eq!(
                    *action
                        .downcast::<clay::masonry_editor::EditorAction>()
                        .expect("SDUI connection action type"),
                    clay::masonry_editor::EditorAction::ClientConnection(event)
                );
            }
            MasonryUserEvent::AccessKit(..) => panic!("SDUI events must use GUI actions"),
        }
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
}
