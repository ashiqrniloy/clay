use std::{error::Error, ffi::OsString};
#[cfg(unix)]
use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::theme::default_property_set;
use masonry_winit::app::{AppDriver, DriverCtx, NewWindow, WindowId};
use masonry_winit::winit::dpi::LogicalSize;
use masonry_winit::winit::event_loop::EventLoop;
use masonry_winit::winit::window::Window;

#[cfg(unix)]
use clay::client;
use clay::ipc::{IpcEndpoint, default_endpoint};
use clay::masonry_editor::{EditorAction, EditorWidget};
#[cfg(unix)]
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
}

fn main() -> Result<(), Box<dyn Error>> {
    match parse_command(std::env::args_os().skip(1).collect()) {
        ClayCommand::Server { endpoint } => run_server(endpoint),
        ClayCommand::Client { endpoint } => run_client(endpoint, false),
        ClayCommand::Auto { endpoint } => run_client(endpoint, true),
    }
}

fn parse_command(args: Vec<OsString>) -> ClayCommand {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return ClayCommand::Auto {
            endpoint: default_endpoint(),
        };
    };

    match first.to_string_lossy().as_ref() {
        "server" | "--server" => ClayCommand::Server {
            endpoint: args
                .next()
                .map(IpcEndpoint::from_argument)
                .unwrap_or_else(default_endpoint),
        },
        "client" | "--client" => ClayCommand::Client {
            endpoint: args
                .next()
                .map(IpcEndpoint::from_argument)
                .unwrap_or_else(default_endpoint),
        },
        _ => ClayCommand::Client {
            endpoint: IpcEndpoint::from_argument(first),
        },
    }
}

#[cfg(unix)]
fn run_server(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    eprintln!("clay server listening on {endpoint}");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(IpcServer::new(ServerConfig::new(endpoint.as_unix_socket_path())).run())?;
    Ok(())
}

#[cfg(not(unix))]
fn run_server(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    Err(format!(
        "Clay server IPC is currently implemented only for Unix sockets; unsupported endpoint {endpoint}"
    )
    .into())
}

#[cfg(unix)]
fn run_client(endpoint: IpcEndpoint, start_server_if_missing: bool) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let socket_path = endpoint.as_unix_socket_path();
    let client_session = match runtime.block_on(client::connect(socket_path)) {
        Ok(session) => Some(session),
        Err(connect_error) if start_server_if_missing => {
            eprintln!(
                "no Clay server available at {endpoint}; starting a background server: {connect_error}"
            );
            start_background_server(&endpoint)?;
            Some(runtime.block_on(connect_with_retry(socket_path))?)
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

#[cfg(not(unix))]
fn run_client(endpoint: IpcEndpoint, _start_server_if_missing: bool) -> Result<(), Box<dyn Error>> {
    Err(format!(
        "Clay client IPC is currently implemented only for Unix sockets; unsupported endpoint {endpoint}"
    )
    .into())
}

#[cfg(unix)]
fn start_background_server(endpoint: &IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    Command::new(executable)
        .arg("server")
        .arg(endpoint.as_child_arg())
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(())
}

#[cfg(unix)]
async fn connect_with_retry(
    socket_path: &Path,
) -> Result<client::ClientSession, client::ClientBootstrapError> {
    let mut last_error = None;
    for _ in 0..50 {
        match client::connect(socket_path).await {
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

    use super::{ClayCommand, parse_command};
    use clay::editor::{EditorSurface, is_printable_text};

    #[test]
    fn parses_server_subcommand() {
        assert!(matches!(
            parse_command(vec!["server".into()]),
            ClayCommand::Server { .. }
        ));
    }

    #[test]
    fn parses_client_subcommand() {
        assert!(matches!(
            parse_command(vec!["client".into()]),
            ClayCommand::Client { .. }
        ));
    }

    #[test]
    fn parses_no_args_as_auto() {
        assert!(matches!(parse_command(vec![]), ClayCommand::Auto { .. }));
    }

    #[test]
    fn cli_parses_platform_endpoint() {
        let endpoint = "clay-test-endpoint";

        match parse_command(vec!["server".into(), endpoint.into()]) {
            ClayCommand::Server { endpoint: parsed } => {
                assert_eq!(parsed.as_child_arg(), OsString::from(endpoint));
            }
            command => panic!("expected server command, got {command:?}"),
        }

        match parse_command(vec!["client".into(), endpoint.into()]) {
            ClayCommand::Client { endpoint: parsed } => {
                assert_eq!(parsed.as_child_arg(), OsString::from(endpoint));
            }
            command => panic!("expected client command, got {command:?}"),
        }
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
