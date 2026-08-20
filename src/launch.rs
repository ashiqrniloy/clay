//! App launch + server/client startup + window creation: the `LaunchError`/
//! `LaunchDiagnostic`/`LaunchReadinessFailure` vocabulary, the `run_*` entry
//! points (`run_server`/`run_client`/`run_restart`/`run_smoke_gui`/
//! `run_perf_fixture`/`run_package_subcommand`), the managed background server
//! lifecycle (`ManagedServer`, command builders), the linux restart helpers,
//! the `connect_with_retry*`/`editor_widget_from_session`/`run_editor`
//! window-creation path, and the `WINDOW_*` geometry constants. `Driver` is
//! constructed here; event dispatch lives in `app_driver`.

use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use masonry::core::NewWidget;
use masonry::theme::default_property_set;
use masonry_winit::app::{EventLoop, NewWindow, WindowId};
use masonry_winit::winit::dpi::LogicalSize;
use masonry_winit::winit::window::Window;
use tokio::sync::mpsc;

use clay::client::{self, ClientConnectionEvent};
use clay::ipc::IpcEndpoint;
use clay::masonry_editor::{EditorStatus, EditorWidget};
use clay::masonry_shell::ClayShellWidget;
use clay::perf::fixtures::{FixtureKind, FixtureSpec, default_fixture_path, generate_fixture_file};
use clay::protocol::{ClientId, TabRegistrySnapshot};

use crate::cli::PackageCliSubcommand;
use crate::driver::{
    Driver, RESTORE_CONFIRM_TIMEOUT, TabState, spawn_client_connection_event_bridge,
};
#[cfg(any(unix, windows))]
use clay::server::{IpcServer, ServerConfig};
use clay::shell::PersistedWindowState;

pub(crate) const WINDOW_TITLE: &str = "Clay";
pub(crate) const WINDOW_WIDTH: f64 = 900.0;
pub(crate) const WINDOW_HEIGHT: f64 = 600.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchDiagnostic {
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

    pub(crate) fn local_fallback(
        endpoint: &IpcEndpoint,
        error: &client::ClientBootstrapError,
    ) -> Self {
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
pub(crate) struct LaunchError {
    endpoint: IpcEndpoint,
    pub(crate) failure: LaunchReadinessFailure,
    pub(crate) attempts: usize,
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
pub(crate) enum LaunchReadinessFailure {
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

pub(crate) fn run_package_subcommand(
    subcommand: PackageCliSubcommand,
) -> Result<(), Box<dyn Error>> {
    use clay::packages::manager::PnpmBackend;
    use clay::packages::service::PackageService;

    // Default store: ~/.config/clay/packages. The durable approval store
    // under the same root fails closed on corruption/unsafe permissions.
    let store_root = clay::packages::service::default_store_root();
    let mut service = PackageService::open(store_root, Box::new(PnpmBackend::new()))?;

    // A fresh service starts with an empty installed map. Repopulate it from
    // the package-manager store so `list`/`enable`/`disable`/`inspect`/`remove`
    // reflect packages installed by previous `clay package add` invocations.
    // `add` skips this: it installs via the backend (which re-discovers
    // internally) and a missing pnpm binary should fail at `pnpm add`, not at
    // the pre-list step.
    if !matches!(&subcommand, PackageCliSubcommand::Add { .. })
        && let Err(error) = service.refresh_installed()
        && !matches!(&subcommand, PackageCliSubcommand::Inspect { .. })
    {
        return Err(error.into());
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
        PackageCliSubcommand::Inspect { package_name } => {
            let from_store = service.inspect(&package_name);
            let store_hit = from_store.is_some();
            match from_store.or_else(|| PackageService::inspect_bundled_inventory(&package_name)) {
                Some(inspection) => {
                    println!("Package:     {}", inspection.name);
                    println!("Version:     {}", inspection.version);
                    println!("API prefix:  {}", inspection.api_prefix);
                    println!(
                        "Status:      {}",
                        if inspection.is_enabled {
                            "enabled"
                        } else if store_hit {
                            "installed"
                        } else {
                            "bundled"
                        }
                    );
                    println!("Modes:       {:?}", inspection.modes);
                    if let Some(preset) = &inspection.preset {
                        println!("Preset:      {preset}");
                    }
                    println!("Permissions: {:?}", inspection.permissions);
                    println!("Commands:    {}", inspection.command_count);
                    println!("Config keys: {}", inspection.configuration_count);
                    if !inspection.native_syntax_languages.is_empty() {
                        println!(
                            "Syntax:      {} — owned by native descriptor (FIRST_PARTY_NATIVE_GRAMMARS)",
                            inspection.native_syntax_languages.join(", ")
                        );
                    }
                    if let Some(docs) = &inspection.docs_path {
                        println!("Docs:        {docs}");
                    }
                    let adoption = match service.adoption_state(&package_name) {
                        Some(clay::packages::service::AdoptionState::Pending) => {
                            "pending adoption (cannot execute)"
                        }
                        Some(clay::packages::service::AdoptionState::Approved) => "approved",
                        Some(clay::packages::service::AdoptionState::Stale) => {
                            "stale approval (re-adopt required)"
                        }
                        Some(clay::packages::service::AdoptionState::Revoked) => "approval revoked",
                        None => "unknown",
                    };
                    println!("Adoption:    {adoption}");
                }
                None => eprintln!("Package `{package_name}` is not installed."),
            }
        }
        PackageCliSubcommand::Adopt { package_name } => {
            if service.inspect(&package_name).is_none() {
                eprintln!("Package `{package_name}` is not installed.");
                return Ok(());
            }
            let approval = service.approve_package(&package_name, "cli")?;
            println!("Adopted {} {}", approval.package, approval.resolved_version);
            println!("  capabilities: {}", approval.capabilities.join(", "));
            if !approval.processes.is_empty() {
                println!("  processes:    {}", approval.processes.join(", "));
            }
            for relation in &approval.relations {
                println!(
                    "  relation:     {} {} {}@{}",
                    relation.operation,
                    relation.package,
                    relation.extension_point,
                    relation.version
                );
            }
            for replacement in &approval.replacements {
                println!("  replaces:     {}", replacement.target);
            }
        }
        PackageCliSubcommand::Revoke { package_name } => {
            if service.inspect(&package_name).is_none() {
                eprintln!("Package `{package_name}` is not installed.");
                return Ok(());
            }
            let revoked = service.revoke_package_approval(&package_name)?;
            if service.disable(&package_name).is_ok() {
                println!("Disabled {package_name}");
            }
            if revoked {
                println!("Revoked approval for {package_name}");
            } else {
                println!("No approval recorded for {package_name}");
            }
        }
        PackageCliSubcommand::Rollback { target_name } => {
            let replacement = service.rollback_replacement(&target_name)?;
            println!("Disabled replacement {replacement}; restored {target_name}");
        }
    }
    Ok(())
}

#[cfg(any(unix, windows))]
pub(crate) fn run_server(
    endpoint: IpcEndpoint,
    configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    eprintln!("{}", LaunchDiagnostic::server_starting(&endpoint));
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = configuration_root;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async { IpcServer::try_new(config)?.run().await })
        .map_err(|error| LaunchError::server_start_failed(endpoint, error.to_string()))?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn run_server(
    endpoint: IpcEndpoint,
    _configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    Err(format!("Clay server IPC is unsupported on this platform: {endpoint}").into())
}

pub(crate) fn run_perf_fixture(
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

pub(crate) fn run_client(
    endpoint: IpcEndpoint,
    start_server_if_missing: bool,
) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    // Select persisted tab 0 before binding the bootstrap connection so the
    // server can scope its deferred InitialDocument to that workspace.
    let restore_candidate = clay::shell::load_window_state();
    let bootstrap_workspace_root = restore_candidate
        .as_ref()
        .and_then(|state| state.tabs.first())
        .filter(|tab| Path::new(&tab.workspace_root).is_dir())
        .map(|tab| tab.workspace_root.clone())
        .unwrap_or_default();

    let client_session = match runtime.block_on(client::connect_with_workspace_root(
        &endpoint,
        bootstrap_workspace_root.clone(),
    )) {
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
            Some(runtime.block_on(connect_with_workspace_root_retry(
                &endpoint,
                &bootstrap_workspace_root,
            ))?)
        }
        Err(connect_error) => {
            eprintln!(
                "{}",
                LaunchDiagnostic::local_fallback(&endpoint, &connect_error)
            );
            None
        }
    };

    let connected = client_session.is_some();

    let (client_id, editor_widget, events, initial_workspace_root) =
        if let Some(session) = client_session {
            let initial_workspace_root = session.initial_state.workspace_root.clone();
            let (client_id, editor_widget, events) = editor_widget_from_session(session);
            (client_id, editor_widget, events, initial_workspace_root)
        } else {
            (
                // Phase 22.3: the local-fallback tab has no connection; key 0
                // is never assigned by the server (ClientIds start at 1).
                0,
                EditorWidget::default().with_status(EditorStatus::local_fallback()),
                None,
                String::new(),
            )
        };

    // Phase 22.5: whole-window restore — only with a live server connection
    // (the local fallback has no registry to rebuild); missing/corrupt/legacy
    // state keeps today's bootstrap exactly.
    let restore = if connected { restore_candidate } else { None };

    run_editor(
        endpoint,
        client_id,
        editor_widget,
        events,
        initial_workspace_root,
        &runtime,
        restore,
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn run_restart(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let stopped = stop_default_linux_servers(&endpoint)?;
    if stopped > 0 {
        eprintln!("stopped {stopped} Clay server process(es)");
    }

    start_background_server(&endpoint)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    drop(runtime.block_on(connect_with_retry(&endpoint))?);
    eprintln!("Clay server restarted at {endpoint}");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn run_restart(_endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    Err(CliError::new("'restart' is currently supported only on Linux").into())
}

#[cfg(target_os = "linux")]
pub(crate) fn stop_default_linux_servers(endpoint: &IpcEndpoint) -> Result<usize, std::io::Error> {
    use std::os::unix::ffi::OsStrExt;
    use std::time::Instant;

    const STOP_TIMEOUT: Duration = Duration::from_secs(2);

    let executable = std::env::current_exe()?;
    let endpoint_arg = endpoint.as_child_arg();
    let mut pids = Vec::new();

    for entry in std::fs::read_dir("/proc")? {
        let Ok(entry) = entry else { continue };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if linux_process_is_default_server(pid, &executable, endpoint_arg.as_bytes()) {
            pids.push(pid);
        }
    }

    for &pid in &pids {
        signal_linux_process(pid, libc::SIGTERM)?;
    }

    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline
        && pids
            .iter()
            .any(|&pid| linux_process_uses_executable(pid, &executable))
    {
        std::thread::sleep(Duration::from_millis(25));
    }

    for &pid in &pids {
        if linux_process_uses_executable(pid, &executable) {
            signal_linux_process(pid, libc::SIGKILL)?;
        }
    }

    Ok(pids.len())
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_process_is_default_server(
    pid: u32,
    executable: &Path,
    endpoint: &[u8],
) -> bool {
    if !linux_process_uses_executable(pid, executable) {
        return false;
    }
    let Ok(command_line) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    linux_command_line_is_default_server(&command_line, endpoint)
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_process_uses_executable(pid: u32, executable: &Path) -> bool {
    let Ok(process_executable) = std::fs::read_link(format!("/proc/{pid}/exe")) else {
        return false;
    };
    process_executable == executable
        || process_executable
            .to_string_lossy()
            .strip_suffix(" (deleted)")
            .is_some_and(|path| Path::new(path) == executable)
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_command_line_is_default_server(command_line: &[u8], endpoint: &[u8]) -> bool {
    let mut args = command_line.split(|byte| *byte == 0);
    let _executable = args.next();
    if !matches!(args.next(), Some(b"server") | Some(b"--server")) {
        return false;
    }
    match args.next() {
        None | Some(b"") | Some(b"--config-fixture") => true,
        Some(argument) => argument == endpoint,
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn signal_linux_process(pid: u32, signal: libc::c_int) -> Result<(), std::io::Error> {
    // SAFETY: kill receives a PID discovered under /proc and a fixed signal constant.
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

pub(crate) fn run_smoke_gui(
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
    let (client_id, editor_widget, events) = editor_widget_from_session(session);
    let result = run_editor(
        endpoint,
        client_id,
        editor_widget,
        events,
        String::new(),
        &runtime,
        None,
    );
    server.shutdown();
    result
}

pub(crate) fn start_background_server(endpoint: &IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?.into_os_string();
    background_server_command(executable, endpoint).spawn()?;
    Ok(())
}

pub(crate) struct ManagedServer {
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

pub(crate) fn cleanup_managed_endpoint(endpoint: &IpcEndpoint) {
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

pub(crate) fn background_server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = server_command(executable, endpoint);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

pub(crate) fn managed_server_command(
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

pub(crate) fn server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = Command::new(executable);
    command.arg("server").arg(endpoint.as_child_arg());
    command
}

pub(crate) fn editor_widget_from_session(
    session: client::ClientSession,
) -> (
    ClientId,
    EditorWidget,
    Option<mpsc::Receiver<ClientConnectionEvent>>,
) {
    let client::ClientSession {
        initial_state,
        edit_queue,
        events,
    } = session;
    let client_id = initial_state.client_id;
    (
        client_id,
        EditorWidget::with_initial_state(initial_state).with_edit_queue(edit_queue),
        Some(events),
    )
}

pub(crate) async fn connect_with_retry(
    endpoint: &IpcEndpoint,
) -> Result<client::ClientSession, LaunchError> {
    connect_with_retry_while(endpoint, || Ok(None)).await
}

pub(crate) async fn connect_with_workspace_root_retry(
    endpoint: &IpcEndpoint,
    workspace_root: &str,
) -> Result<client::ClientSession, LaunchError> {
    let mut last_error = None;
    for _ in 1..=50 {
        match client::connect_with_workspace_root(endpoint, workspace_root.to_string()).await {
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
            last_error.expect("connect retry loop always records the last error"),
        ),
    ))
}

pub(crate) async fn connect_with_retry_while(
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

pub(crate) fn run_editor(
    endpoint: IpcEndpoint,
    client_id: ClientId,
    editor_widget: EditorWidget,
    events: Option<mpsc::Receiver<ClientConnectionEvent>>,
    initial_workspace_root: String,
    runtime: &tokio::runtime::Runtime,
    restore: Option<PersistedWindowState>,
) -> Result<(), Box<dyn Error>> {
    // Phase 22.2: a master queue clone for mounting pane document views.
    // Phase 22.3: the initial tab's queue lives in its `TabState`.
    let edit_queue = editor_widget.edit_queue_shared();
    // Phase 22.5: whole-window restore plan. Persisted tab 0 rides the
    // bootstrap connection (already connected in `run_client`); tabs 1..
    // mount sequentially inside the event loop, gated on registry
    // confirmation. A missing tab-0 workspace root falls back to today's
    // bootstrap (server root) and the rest of the window restores around it.
    let restore_active = restore.as_ref().and_then(|state| state.active_tab);
    let (first_valid, restoring) = {
        let restore_first = restore.as_ref().and_then(|state| state.tabs.first());
        (
            restore_first.is_some_and(|tab| Path::new(&tab.workspace_root).is_dir()),
            restore_first.is_some(),
        )
    };
    let mut restore_tabs = restore.map(|state| state.tabs).unwrap_or_default();
    let mut restore_queue = VecDeque::new();
    let mut restore_mounted = Vec::new();
    let mut restore_diagnostics = Vec::new();
    if !restore_tabs.is_empty() {
        let first = restore_tabs.remove(0);
        if first_valid {
            restore_mounted.push((client_id, 0, first));
        } else {
            restore_diagnostics.push(format!(
                "Restore skipped {}: workspace root is missing or not a directory",
                first.workspace_root
            ));
        }
        restore_queue = restore_tabs
            .into_iter()
            .enumerate()
            .map(|(index, tab)| (index + 1, tab))
            .collect();
    }
    let bootstrap_root = restore_mounted
        .first()
        .map(|(_, _, tab)| tab.workspace_root.clone())
        .unwrap_or_else(|| initial_workspace_root.clone());
    // The bootstrap connection binds its tab during the handshake; the
    // deferred initial document already belongs to `bootstrap_root`.
    let shell_widget = if first_valid {
        ClayShellWidget::restored_single_editor(
            client_id,
            editor_widget,
            &restore_mounted
                .first()
                .expect("first_valid mounts persisted tab 0")
                .2,
        )
    } else {
        ClayShellWidget::single_editor(client_id, editor_widget)
    };
    let editor_widget_id = shell_widget.editor_widget_id();
    let root_widget = NewWidget::new(shell_widget);
    let shell_widget_id = root_widget.id();
    let window_id = WindowId::next();
    let window_attributes = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    if let Some(events) = events {
        spawn_client_connection_event_bridge(
            runtime.handle(),
            events,
            proxy.clone(),
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
        Driver {
            editor_widget_id,
            shell_widget_id,
            window_id,
            centered_layer_id: None,
            tabs: BTreeMap::from([(
                client_id,
                TabState {
                    edit_queue,
                    pending_opens: BTreeMap::new(),
                    tab_id: None,
                    workspace_root: bootstrap_root.clone(),
                },
            )]),
            active_tab: client_id,
            registry: TabRegistrySnapshot {
                tabs: Vec::new(),
                active: None,
                revision: 0,
            },
            registry_revision: None,
            runtime: runtime.handle().clone(),
            endpoint,
            reconnect_cancel: BTreeMap::new(),
            proxy: Some(proxy),
            dialog_generation: 0,
            file_dialog_in_flight: None,
            folder_dialog_in_flight: None,
            pending_close_after_saves: None,
            tab_menu_session_id: 0,
            restore_queue,
            restore_pending: None,
            restore_mounted,
            restore_gate: restoring.then(|| (client_id, Instant::now() + RESTORE_CONFIRM_TIMEOUT)),
            restore_active,
            restore_diagnostics,
        },
        default_property_set(),
    )?;

    Ok(())
}
