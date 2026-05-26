use std::{
    error::Error,
    ffi::OsString,
    process::{Command, Stdio},
    time::Duration,
};

use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::theme::default_property_set;
use masonry_winit::app::{AppDriver, DriverCtx, NewWindow, WindowId};
use masonry_winit::winit::dpi::LogicalSize;
use masonry_winit::winit::event_loop::EventLoop;
use masonry_winit::winit::window::Window;

use clay::client;
use clay::ipc::{IpcEndpoint, default_endpoint};
use clay::masonry_editor::{EditorAction, EditorWidget};
#[cfg(any(unix, windows))]
use clay::server::{IpcServer, ServerConfig};

const WINDOW_TITLE: &str = "Clay Phase 4";
const WINDOW_WIDTH: f64 = 900.0;
const WINDOW_HEIGHT: f64 = 600.0;

struct Driver {
    editor_widget_id: WidgetId,
}

impl AppDriver for Driver {
    fn on_start(&mut self, state: &mut masonry_winit::app::MasonryState<'_>) {
        for root in state.roots() {
            root.set_focus_fallback(Some(self.editor_widget_id));
        }
    }

    fn on_action(
        &mut self,
        _window_id: WindowId,
        ctx: &mut DriverCtx<'_, '_>,
        _widget_id: WidgetId,
        action: ErasedAction,
    ) {
        if action.downcast::<EditorAction>().is_ok() {
            ctx.exit();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClayCommand {
    Auto { endpoint: IpcEndpoint },
    Client { endpoint: IpcEndpoint },
    Server { endpoint: IpcEndpoint },
    SmokeGui { endpoint: IpcEndpoint },
    Help,
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

const CLI_USAGE: &str = "Usage:\n  clay\n  clay server [endpoint]\n  clay client [endpoint]\n  clay smoke-gui [endpoint]\n  clay <endpoint>\n\nModes:\n  clay                  Connect to the default local endpoint, start a background server if missing, then open the GUI.\n  clay server           Run a foreground server on the default local endpoint.\n  clay client           Connect to the default local endpoint, or open a local fallback GUI if missing.\n  clay smoke-gui        App-managed GUI smoke mode; starts from a local endpoint and is isolated/managed as implementation progresses.\n  clay <endpoint>       Advanced debugging shorthand for 'clay client <endpoint>'.\n";

fn main() -> Result<(), Box<dyn Error>> {
    match parse_command(std::env::args_os().skip(1).collect())? {
        ClayCommand::Server { endpoint } => run_server(endpoint),
        ClayCommand::Client { endpoint } => run_client(endpoint, false),
        ClayCommand::Auto { endpoint } => run_client(endpoint, true),
        ClayCommand::SmokeGui { endpoint } => run_client(endpoint, true),
        ClayCommand::Help => {
            println!("{CLI_USAGE}");
            Ok(())
        }
    }
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
        "server" | "--server" => parse_endpoint_subcommand("server", args)
            .map(|endpoint| ClayCommand::Server { endpoint }),
        "client" | "--client" => parse_endpoint_subcommand("client", args)
            .map(|endpoint| ClayCommand::Client { endpoint }),
        "smoke-gui" | "smoke" | "--smoke-gui" => parse_endpoint_subcommand("smoke-gui", args)
            .map(|endpoint| ClayCommand::SmokeGui { endpoint }),
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

#[cfg(any(unix, windows))]
fn run_server(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    eprintln!("clay server listening on {endpoint}");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(IpcServer::new(ServerConfig::new(endpoint)).run())?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn run_server(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    Err(format!("Clay server IPC is unsupported on this platform: {endpoint}").into())
}

fn run_client(endpoint: IpcEndpoint, start_server_if_missing: bool) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let client_session = match runtime.block_on(client::connect(&endpoint)) {
        Ok(session) => Some(session),
        Err(connect_error) if start_server_if_missing => {
            eprintln!(
                "no Clay server available at {endpoint}; starting a background server: {connect_error}"
            );
            start_background_server(&endpoint)?;
            Some(runtime.block_on(connect_with_retry(&endpoint))?)
        }
        Err(connect_error) => {
            eprintln!(
                "failed to connect to Clay server at {endpoint}; starting local empty client: {connect_error}"
            );
            None
        }
    };

    let editor_widget = if let Some(session) = client_session {
        let client::ClientSession {
            initial_state,
            edit_queue,
            mut events,
        } = session;
        runtime.spawn(async move {
            while let Some(event) = events.recv().await {
                eprintln!("clay client IPC event: {event:?}");
            }
        });
        EditorWidget::with_initial_state(initial_state).with_edit_queue(edit_queue)
    } else {
        EditorWidget::default()
    };

    run_editor(editor_widget)
}

fn start_background_server(endpoint: &IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?.into_os_string();
    background_server_command(executable, endpoint).spawn()?;
    Ok(())
}

fn background_server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("server")
        .arg(endpoint.as_child_arg())
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

async fn connect_with_retry(
    endpoint: &IpcEndpoint,
) -> Result<client::ClientSession, client::ClientBootstrapError> {
    let mut last_error = None;
    for _ in 0..50 {
        match client::connect(endpoint).await {
            Ok(session) => return Ok(session),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }

    Err(last_error.expect("connect retry loop always records an error"))
}

fn run_editor(editor_widget: EditorWidget) -> Result<(), Box<dyn Error>> {
    let root_widget = NewWidget::new(editor_widget);
    let editor_widget_id = root_widget.id();
    let window_attributes = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

    masonry_winit::app::run_with(
        EventLoop::with_user_event().build()?,
        vec![NewWindow::new(window_attributes, root_widget.erased())],
        Driver { editor_widget_id },
        default_property_set(),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{ClayCommand, background_server_command, parse_command};
    use clay::editor::{EditorSurface, is_printable_text};

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
                | ClayCommand::Server { endpoint }
                | ClayCommand::SmokeGui { endpoint } => assert!(!endpoint.to_string().is_empty()),
                ClayCommand::Help => panic!("help should not be selected by launch modes"),
            }
        }
    }

    #[test]
    fn cli_parses_platform_endpoint() {
        let endpoint = "clay-test-endpoint";

        match parse_command(vec!["server".into(), endpoint.into()]).expect("server endpoint parses")
        {
            ClayCommand::Server { endpoint: parsed } => {
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

        match parse_command(vec!["smoke-gui".into(), endpoint.into()])
            .expect("smoke endpoint parses")
        {
            ClayCommand::SmokeGui { endpoint: parsed } => {
                assert_eq!(parsed.as_child_arg(), OsString::from(endpoint));
            }
            command => panic!("expected smoke command, got {command:?}"),
        }
    }

    #[test]
    fn rejects_extra_cli_arguments() {
        let error = parse_command(vec!["server".into(), "one".into(), "two".into()])
            .expect_err("extra arguments should fail");
        assert!(error.to_string().contains("unexpected extra argument"));
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
