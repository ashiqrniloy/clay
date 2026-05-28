use std::{
    error::Error,
    ffi::OsString,
    fmt,
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
                ctx.render_root(window_id)
                    .edit_widget(widget_id, |mut widget| {
                        if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                            let changed = editor.widget.apply_connection_event(event);
                            if changed {
                                editor.ctx.request_render();
                                editor.ctx.request_accessibility_update();
                            }
                        }
                    });
            }
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

const CLI_USAGE: &str = "Usage:\n  clay\n  clay server [endpoint]\n  clay client [endpoint]\n  clay smoke-gui\n  clay <endpoint>\n\nModes:\n  clay                  Connect to the default local endpoint, start a background server if missing, then open the GUI.\n  clay server           Run a foreground server on the default local endpoint.\n  clay client           Connect to the default local endpoint, or open a local fallback GUI if missing.\n  clay smoke-gui        App-managed GUI smoke mode; starts an isolated child server, opens a client, then cleans up.\n  clay <endpoint>       Advanced debugging shorthand for 'clay client <endpoint>'.\n";

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
    match parse_command(std::env::args_os().skip(1).collect())? {
        ClayCommand::Server { endpoint } => run_server(endpoint),
        ClayCommand::Client { endpoint } => run_client(endpoint, false),
        ClayCommand::Auto { endpoint } => run_client(endpoint, true),
        ClayCommand::SmokeGui { endpoint } => run_smoke_gui(endpoint),
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
        "smoke-gui" | "smoke" | "--smoke-gui" => parse_smoke_gui_subcommand(args),
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

fn parse_smoke_gui_subcommand(
    mut args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    if let Some(extra) = args.next() {
        return Err(CliError::new(format!(
            "unexpected extra argument for 'smoke-gui': {}",
            extra.to_string_lossy()
        )));
    }

    Ok(ClayCommand::SmokeGui {
        endpoint: smoke_endpoint("gui"),
    })
}

#[cfg(any(unix, windows))]
fn run_server(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    eprintln!("{}", LaunchDiagnostic::server_starting(&endpoint));
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(IpcServer::new(ServerConfig::new(endpoint.clone())).run())
        .map_err(|error| LaunchError::server_start_failed(endpoint, error.to_string()))?;
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

fn run_smoke_gui(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let executable = std::env::current_exe()?.into_os_string();
    let mut server = ManagedServer::spawn(executable, &endpoint)?;

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
    fn spawn(executable: OsString, endpoint: &IpcEndpoint) -> Result<Self, Box<dyn Error>> {
        let child = managed_server_command(executable, endpoint).spawn()?;
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
    if let Err(error) = std::fs::remove_file(endpoint.as_unix_socket_path()) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("failed to remove managed Clay socket {endpoint}: {error}");
        }
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

fn managed_server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = server_command(executable, endpoint);
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
    let root_widget = NewWidget::new(editor_widget);
    let editor_widget_id = root_widget.id();
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
    use std::ffi::OsString;

    use super::{
        ClayCommand, LaunchDiagnostic, LaunchReadinessFailure, background_server_command,
        connect_with_retry, connect_with_retry_while, connection_event_user_event,
        managed_server_command, parse_command,
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
    fn default_server_and_clients_use_same_platform_endpoint() {
        let expected = default_endpoint();

        for args in [vec![], vec!["server".into()], vec!["client".into()]] {
            let command = parse_command(args).expect("default launch mode parses");
            let endpoint = match command {
                ClayCommand::Auto { endpoint }
                | ClayCommand::Client { endpoint }
                | ClayCommand::Server { endpoint } => endpoint,
                ClayCommand::SmokeGui { .. } => {
                    panic!("default smoke endpoint must remain isolated")
                }
                ClayCommand::Help => panic!("help should not be selected by default launch modes"),
            };
            assert_eq!(endpoint, expected);
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
        let command = managed_server_command(executable.clone(), &endpoint);

        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_owned())
                .collect::<Vec<_>>(),
            vec![OsString::from("server"), endpoint_arg]
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
    fn connection_event_action_is_dispatched_to_driver() {
        let window_id = WindowId::next();
        let widget_id = WidgetId::next();
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
