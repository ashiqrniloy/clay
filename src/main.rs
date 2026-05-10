use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
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
use clay::ipc::default_socket_path;
use clay::masonry_editor::{EditorAction, EditorWidget};
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
    Auto { socket_path: PathBuf },
    Client { socket_path: PathBuf },
    Server { socket_path: PathBuf },
}

fn main() -> Result<(), Box<dyn Error>> {
    match parse_command(std::env::args_os().skip(1).collect()) {
        ClayCommand::Server { socket_path } => run_server(socket_path),
        ClayCommand::Client { socket_path } => run_client(socket_path, false),
        ClayCommand::Auto { socket_path } => run_client(socket_path, true),
    }
}

fn parse_command(args: Vec<OsString>) -> ClayCommand {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return ClayCommand::Auto {
            socket_path: default_socket_path(),
        };
    };

    match first.to_string_lossy().as_ref() {
        "server" | "--server" => ClayCommand::Server {
            socket_path: args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_socket_path),
        },
        "client" | "--client" => ClayCommand::Client {
            socket_path: args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_socket_path),
        },
        _ => ClayCommand::Client {
            socket_path: PathBuf::from(first),
        },
    }
}

fn run_server(socket_path: PathBuf) -> Result<(), Box<dyn Error>> {
    eprintln!("clay server listening on {}", socket_path.display());
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(IpcServer::new(ServerConfig::new(socket_path)).run())?;
    Ok(())
}

fn run_client(socket_path: PathBuf, start_server_if_missing: bool) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let client_session = match runtime.block_on(client::connect(&socket_path)) {
        Ok(session) => Some(session),
        Err(connect_error) if start_server_if_missing => {
            eprintln!(
                "no Clay server available at {}; starting a background server: {connect_error}",
                socket_path.display()
            );
            start_background_server(&socket_path)?;
            Some(runtime.block_on(connect_with_retry(&socket_path))?)
        }
        Err(connect_error) => {
            eprintln!(
                "failed to connect to Clay server at {}; starting local empty client: {connect_error}",
                socket_path.display()
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

fn start_background_server(socket_path: &Path) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    Command::new(executable)
        .arg("server")
        .arg(socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(())
}

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
