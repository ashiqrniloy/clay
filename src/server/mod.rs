use crate::lock_util::LockOrRecover;

#[doc(hidden)]
pub mod agent;
pub mod agent_agui;
pub(crate) mod agent_checkpoints;
pub(crate) mod agent_documents;
pub mod agent_mcp_config;
pub(crate) mod agent_picker;
pub(crate) mod agent_settings;
mod behavior;
pub mod command_execution;
pub mod completion;
mod config_watch;
mod configuration;
mod connection;
pub(crate) mod control_center;
pub mod decorations;
pub mod diagnostics;
pub(crate) mod document;
pub(crate) mod document_analysis;
mod facades;
pub(crate) mod fanout;
pub(crate) mod folding; // FOLDING_RANGE_PAYLOAD_BUDGET_BYTES
pub(crate) mod git;
mod js_runtime;
#[doc(hidden)]
pub mod language_intelligence;
#[doc(hidden)]
pub mod language_server;
pub(crate) mod launcher;
pub(crate) mod locks;
pub(crate) mod menu_sessions;
mod ops;
pub(crate) mod output_router;
pub mod parse_coordinator;
mod runtime_reload;
mod runtime_state;
mod sdui;
pub mod syntax;
pub mod syntax_session;
mod tab_registry;
mod ui;
// OS default-browser / clipboard helpers for the agent OAuth journey
// (plan 116): open the authorization URL and copy it as a fallback.
mod open;
pub(crate) mod workspace;

use std::{
    collections::HashMap,
    error::Error,
    fmt, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::{fs, os::unix::fs::FileTypeExt, path::Path};

use tokio::{
    sync::{Mutex, broadcast},
    task::JoinSet,
};

#[cfg(test)]
use tokio::sync::oneshot;

#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
#[cfg(windows)]
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, LocalFree},
    Security::{
        ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, AddAccessAllowedAce, GetLengthSid,
        GetTokenInformation, InitializeAcl, InitializeSecurityDescriptor, PSECURITY_DESCRIPTOR,
        PSID, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SetSecurityDescriptorDacl, TOKEN_QUERY,
        TOKEN_USER, TokenUser,
    },
    System::Memory::{LPTR, LocalAlloc},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

use crate::{
    ipc::IpcEndpoint,
    packages::commands::{
        CommandCatalogue, CommandCatalogueError, CommandRegistry, RegisteredCommand,
    },
    perf::budgets::RUNTIME_STATE_BROADCAST_CAPACITY,
    protocol::{
        ClientId, DocumentAccess, DocumentId, RuntimeDiagnostic, RuntimeGenerationId,
        RuntimeStateSnapshot, ServerMessage, TabId, TabRegistrySnapshot, WorkspaceRootId,
        codec::Codec,
    },
    server::command_execution::{
        CommandExecutionDiagnostic, CommandExecutionRequest, CommandExecutionRule,
        CommandExecutionTarget, CommandExecutor, RELOAD_CONFIGURATION_COMMAND_ID,
        is_reload_command,
    },
};

use self::{
    behavior::{ActiveBehaviorManifest, BehaviorGraceState},
    config_watch::watch_configuration_root,
    configuration::ConfigurationRuntime,
    connection::handle_connection_with_analysis,
    document::DocumentState,
    js_runtime::ClayJsRuntimeService,
    language_intelligence::LanguageIntelligenceCoordinator,
    locks::ScopedLockManager,
    parse_coordinator::ParseCoordinator,
    sdui::StaticSduiState,
    workspace::WorkspaceState,
};

// Plan 133 task 3: the runtime stores/assembly live in focused submodules; the
// paths below are the historical `crate::server::…` names, kept so callers and
// the connection/tab plumbing do not churn.
pub use self::runtime_reload::{ReloadedDocumentRefresh, RuntimeReloadOutcome};
pub use self::runtime_state::ServerConfig;
pub(crate) use self::runtime_state::{
    RuntimeGeneration, RuntimeGenerationStore, apply_runtime_outputs_without_sdui,
};

#[cfg(windows)]
const ERROR_PIPE_CONNECTED: i32 = 535;

#[cfg(test)]
pub(crate) static JS_RUNTIME_TEST_LOCK: Mutex<()> = Mutex::const_new(());

/// Server-owned content state for one stable tab identity. The empty welcome
/// document is separate from file-backed documents in the tab workspace; the
/// client renders its Clay-owned entry surface until a real document opens.
#[derive(Debug, Clone)]
pub(crate) struct TabServerState {
    pub(crate) welcome: Arc<Mutex<DocumentState>>,
    pub(crate) workspace: Arc<Mutex<WorkspaceState>>,
    /// Per-tab shell state. The workspace tree is visible by default — the
    /// desktop empty pane offers Open File/Folder and must show the browser
    /// after an explicit folder open; `workspace.toggleFileBrowser` flips it.
    /// Reconnect/reclaim keeps the tab's current choice.
    pub(crate) workspace_pane_visible: Arc<AtomicBool>,
}

impl TabServerState {
    fn from_workspace(workspace: WorkspaceState, allocator: Arc<AtomicU64>) -> Self {
        let welcome_id = allocator.fetch_add(1, Ordering::Relaxed);
        let workspace = workspace.with_document_id_allocator(allocator);
        Self {
            welcome: Arc::new(Mutex::new(DocumentState::new(
                welcome_id,
                String::new(),
                DocumentAccess::Editable { lease_id: 1 },
            ))),
            workspace: Arc::new(Mutex::new(workspace)),
            workspace_pane_visible: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn workspace_pane_visible(&self) -> bool {
        self.workspace_pane_visible.load(Ordering::Relaxed)
    }

    pub(crate) fn toggle_workspace_pane(&self) -> bool {
        !self
            .workspace_pane_visible
            .fetch_xor(true, Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct IpcServer {
    config: ServerConfig,
    codec: Codec,
    /// Pre-bind state used only by the first connection's legacy handshake;
    /// once `TabCommand::New` succeeds, tab content lives in `tab_states`.
    bootstrap_state: TabServerState,
    tab_states: Arc<Mutex<HashMap<TabId, TabServerState>>>,
    document_id_allocator: Arc<AtomicU64>,
    bootstrap_consumed: Arc<AtomicBool>,
    #[cfg(test)]
    /// Compatibility aliases for existing connection tests that construct or
    /// inspect the pre-bind bootstrap state directly.
    document: Arc<Mutex<DocumentState>>,
    #[cfg(test)]
    workspace: Arc<Mutex<WorkspaceState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,

    sdui: Arc<Mutex<StaticSduiState>>,
    /// Resolved active theme snapshot (Plan 046 task 7 `setTheme`) shipped to the
    /// client during the welcome handshake. `None` = Clay default theme.
    active_theme: Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    /// Phase 102 resolved active UI design-system snapshot.
    active_design_system: Arc<Mutex<crate::shell::design_system::ActiveDesignSystem>>,
    /// Plan 112 resolved active icon-pack snapshot. `None` = bundled Regular
    /// safety subset (host fallback) is active.
    active_icon_pack: Arc<Mutex<Option<crate::shell::icons::ActiveIconPack>>>,
    runtime_diagnostics: Arc<Mutex<connection::RuntimeDiagnosticStore>>,
    /// Active-connection ceiling: each accepted connection must hold one
    /// permit for its lifetime; excess connections are refused at accept time
    /// instead of spawning unbounded tasks (Plan 060 T6, P1-10).
    connection_permits: Arc<tokio::sync::Semaphore>,
    parse_coordinator: ParseCoordinator,
    completion: crate::server::completion::CompletionCoordinator,
    document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
    language_intelligence: LanguageIntelligenceCoordinator,
    runtime_generation: RuntimeGenerationStore,
    scoped_locks: ScopedLockManager,
    reload_attempt: Arc<Mutex<()>>,
    next_client_id: Arc<AtomicU64>,
    /// Phase 22.7: `ClientId`s with a live connection task. The TTL sweep
    /// never expires entries bound to a live client — presence outranks
    /// tab-idleness (document editing never touches the registry). Inserted
    /// at spawn, removed when the task ends.
    live_clients: Arc<std::sync::Mutex<std::collections::HashSet<crate::protocol::ClientId>>>,
    /// Phase 22.3: server-authoritative in-memory tab registry (order, active
    /// tab, per-tab workspace + client binding) plus its broadcast lane. Every
    /// connection subscribes at spawn and forwards `ServerMessage::TabRegistry`
    /// snapshots to its stream; lagged receivers resync from the mutex.
    tab_registry: Arc<Mutex<tab_registry::TabRegistry>>,
    tab_registry_tx: broadcast::Sender<TabRegistrySnapshot>,
    /// One clay-agent child per server. Lazy-spawned on first agent command.
    pub(crate) agent: agent::AgentHost,
    /// Test-only barrier that parks candidate evaluation until the test releases
    /// it. Production builds omit this field entirely.
    #[cfg(test)]
    reload_barrier: ReloadCandidateBarrier,
}

/// Parks a reload candidate between attempt-lock acquisition and configuration
/// evaluation so tests can prove ordinary edits continue without waiting.
#[cfg(test)]
#[derive(Clone, Default)]
struct ReloadCandidateBarrier {
    inner: Arc<Mutex<ReloadCandidateBarrierState>>,
}

#[cfg(test)]
impl std::fmt::Debug for ReloadCandidateBarrier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReloadCandidateBarrier")
    }
}

#[cfg(test)]
#[derive(Default)]
struct ReloadCandidateBarrierState {
    entered: Option<oneshot::Sender<()>>,
    release: Option<oneshot::Receiver<()>>,
}

#[cfg(test)]
impl ReloadCandidateBarrier {
    async fn arm(&self) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *self.inner.lock().await = ReloadCandidateBarrierState {
            entered: Some(entered_tx),
            release: Some(release_rx),
        };
        (entered_rx, release_tx)
    }

    async fn wait_if_armed(&self) {
        let (entered, release) = {
            let mut state = self.inner.lock().await;
            (state.entered.take(), state.release.take())
        };
        if let Some(entered) = entered {
            let _ = entered.send(());
        }
        if let Some(release) = release {
            let _ = release.await;
        }
    }
}

impl IpcServer {
    async fn new_tab_state(
        &self,
        workspace_root: PathBuf,
        first_tab: bool,
    ) -> Result<(TabServerState, WorkspaceRootId), workspace::WorkspaceError> {
        if first_tab && !self.bootstrap_consumed.load(Ordering::Acquire) {
            let bootstrap_root_id =
                if let Ok(canonical_root) = std::fs::canonicalize(&workspace_root) {
                    self.bootstrap_state
                        .workspace
                        .lock()
                        .await
                        .directory_roots()
                        .into_iter()
                        .find(|root| root.canonical_path == canonical_root)
                        .map(|root| root.workspace_root_id)
                } else {
                    None
                };
            if let Some(root_id) = bootstrap_root_id {
                return Ok((self.bootstrap_state.clone(), root_id));
            }
        }

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(workspace_root)?;
        Ok((
            TabServerState::from_workspace(workspace, Arc::clone(&self.document_id_allocator)),
            root_id,
        ))
    }

    /// Create and register one tab-owned workspace/document state. The first
    /// tab reuses the validated bootstrap state when it selects the startup
    /// root; later tabs always receive fresh state.
    pub(crate) async fn create_tab_state(
        &self,
        client_id: ClientId,
        workspace_root: String,
    ) -> Result<(TabRegistrySnapshot, TabServerState), workspace::WorkspaceError> {
        let workspace_root = if workspace_root.is_empty() {
            self.bootstrap_state
                .workspace
                .lock()
                .await
                .list_root_metadata()
                .first()
                .map(|root| root.display_path.clone())
                .unwrap_or_default()
        } else {
            workspace_root
        };
        let mut registry = self.tab_registry.lock().await;
        let first_tab = self.tab_states.lock().await.is_empty();
        let (state, root_id) = self
            .new_tab_state(PathBuf::from(&workspace_root), first_tab)
            .await?;
        // Launcher (plan 118 Part D): a tab that opens on a folder was an
        // explicit open, so it leads the recents list. Best-effort.
        if !workspace_root.is_empty() {
            launcher::record_recent_workspace(
                self.config.configuration_root.as_deref(),
                Path::new(&workspace_root),
            )
            .await;
        }
        let tab_id = registry.create_tab(client_id, root_id, workspace_root);
        let state_for_connection = state.clone();
        self.tab_states.lock().await.insert(tab_id, state);
        self.bootstrap_consumed.store(true, Ordering::Release);
        Ok((registry.snapshot(), state_for_connection))
    }

    pub(crate) async fn ensure_tab_state(
        &self,
        tab_id: TabId,
        workspace_root: PathBuf,
    ) -> Result<(), workspace::WorkspaceError> {
        if self.tab_states.lock().await.contains_key(&tab_id) {
            return Ok(());
        }
        let (state, _) = self.new_tab_state(workspace_root, false).await?;
        self.tab_states.lock().await.entry(tab_id).or_insert(state);
        Ok(())
    }

    pub(crate) async fn tab_state(&self, tab_id: TabId) -> Option<TabServerState> {
        self.tab_states.lock().await.get(&tab_id).cloned()
    }

    pub(crate) async fn tab_state_for_client(
        &self,
        client_id: ClientId,
    ) -> Option<(TabId, TabServerState)> {
        let tab_id = self.tab_registry.lock().await.tab_for_client(client_id)?;
        let state = self.tab_state(tab_id).await?;
        Some((tab_id, state))
    }

    /// The pre-bind state is available only while the registry is empty and
    /// bootstrap has not been consumed. After that point, an unbound or stale
    /// connection must not fall back to another tab's bootstrap handles.
    pub(crate) async fn unbound_bootstrap_state(&self) -> Option<TabServerState> {
        if self.bootstrap_consumed.load(Ordering::Acquire)
            || !self.tab_registry.lock().await.snapshot().tabs.is_empty()
        {
            return None;
        }
        Some(self.bootstrap_state.clone())
    }

    /// Agent settings page (plan 117): the server-resolved daemon config
    /// root (`<configuration root>/agents/<agent type>`). The webview never
    /// supplies paths for this surface; the server owns the resolution.
    ///
    /// Plan 118 task 35: the root follows the *tab's* agent type (from the
    /// registry, which stores only names the server validated), so the
    /// Settings tab lists the files of the agent the tab actually runs.
    /// `None` (no bound tab, or a tab with no agent) reads the shipped
    /// default agent.
    pub(crate) async fn agent_settings_root(&self, client_id: ClientId) -> Option<PathBuf> {
        let agent_type = {
            let registry = self.tab_registry.lock().await;
            registry
                .tab_for_client(client_id)
                .and_then(|tab| registry.agent_type(tab))
        };
        agent_settings::agent_config_root_for(
            self.config.configuration_root.as_deref(),
            agent_type.as_deref(),
        )
    }

    /// Launcher (plan 118 Part D): the configured Clay root whose recents
    /// store and `agents/` folder the start surface lists. `None` means the
    /// per-user default (`~/.clay`), resolved by the launcher module.
    pub(crate) fn configuration_root(&self) -> Option<PathBuf> {
        self.config.configuration_root.clone()
    }

    /// Plan 136 task 7: record the live runtime generation's per-lane JS
    /// runtime metrics into the developer perf recorder. Called once by the
    /// server's report-time path (`src/launch.rs`, the CLI's SIGTERM hook)
    /// before the summary is written; a no-op recorder makes this free outside
    /// the harness. Public because that hook lives in the binary crate.
    pub async fn record_lane_metrics(&self, recorder: &crate::perf::metrics::PerfRecorder) {
        self.runtime_generation.record_lane_metrics(recorder).await;
    }

    pub(crate) async fn state_for_client(&self, client_id: ClientId) -> Option<TabServerState> {
        self.tab_state_for_client(client_id)
            .await
            .map(|(_, state)| state)
    }

    pub(crate) async fn remove_tab_state(&self, tab_id: TabId) {
        self.tab_states.lock().await.remove(&tab_id);
    }

    fn effective_configuration_root(&self) -> Option<PathBuf> {
        let root = if let Some(root) = &self.config.configuration_root {
            root.clone()
        } else {
            let root = ConfigurationRuntime::default_config_root()?;
            if !root.join("init.js").is_file() {
                return None;
            }
            root
        };
        let root = std::fs::canonicalize(root).ok()?;
        root.is_dir().then_some(root)
    }

    fn spawn_configuration_watcher(&self, tasks: &mut JoinSet<()>) {
        let Some(root) = self.effective_configuration_root() else {
            return;
        };
        let server = self.clone();
        tasks.spawn(async move {
            watch_configuration_root(root, move || {
                let server = server.clone();
                async move {
                    let _ = server.reload_runtime_generation().await;
                }
            })
            .await;
        });
    }

    #[cfg(unix)]
    pub async fn run(self) -> Result<(), ServerError> {
        let listener = bind_unix_listener(self.config.endpoint.as_unix_socket_path())?;
        self.load_default_configuration().await;
        self.accept_unix_loop(listener).await
    }

    #[cfg(unix)]
    async fn accept_unix_loop(self, listener: UnixListener) -> Result<(), ServerError> {
        let mut connections = JoinSet::new();
        self.spawn_configuration_watcher(&mut connections);
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let (stream, _address) = accepted.map_err(ServerError::Accept)?;
                    self.spawn_connection(stream, &mut connections);
                }
                Some(joined) = connections.join_next() => {
                    if let Err(error) = joined {
                        eprintln!("clay server connection task failed: {error}");
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    pub async fn run(self) -> Result<(), ServerError> {
        self.config
            .endpoint
            .validate_windows_named_pipe()
            .map_err(ServerError::InvalidEndpoint)?;
        self.load_default_configuration().await;
        let mut connections = JoinSet::new();
        self.spawn_configuration_watcher(&mut connections);
        loop {
            let pipe = create_named_pipe_server(self.config.endpoint.as_windows_named_pipe())?;
            tokio::select! {
                connected = connect_named_pipe_server(pipe) => {
                    let stream = connected.map_err(ServerError::Accept)?;
                    self.spawn_connection(stream, &mut connections);
                }
                Some(joined) = connections.join_next() => {
                    if let Err(error) = joined {
                        eprintln!("clay server connection task failed: {error}");
                    }
                }
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub async fn run(self) -> Result<(), ServerError> {
        Err(ServerError::InvalidEndpoint(format!(
            "Clay IPC is unsupported on this platform: {}",
            self.config.endpoint
        )))
    }

    fn spawn_connection<S>(&self, stream: S, connections: &mut JoinSet<()>)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let Ok(permit) = self.connection_permits.clone().try_acquire_owned() else {
            eprintln!(
                "clay server: active connection limit ({}) reached; refusing connection",
                crate::perf::budgets::MAX_ACTIVE_CONNECTIONS
            );
            drop(stream);
            return;
        };
        let client_id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
        let document = Arc::clone(&self.bootstrap_state.welcome);
        let behavior = Arc::clone(&self.behavior);
        let workspace = Arc::clone(&self.bootstrap_state.workspace);
        let sdui = Arc::clone(&self.sdui);
        let active_theme = Arc::clone(&self.active_theme);
        let runtime_diagnostics = Arc::clone(&self.runtime_diagnostics);
        let runtime_generation = self.runtime_generation.clone();
        let parse_coordinator = self.parse_coordinator.clone();
        let completion = self.completion.clone();
        let document_analysis = self.document_analysis.clone();
        let language_intelligence = self.language_intelligence.clone();
        let reload_server = IpcServer::clone(self);
        let tab_registry = Arc::clone(&self.tab_registry);
        let tab_registry_tx = self.tab_registry_tx.clone();
        // Phase 22.7: the TTL sweep runs on connection arrival and departure
        // (see `sweep_expired_tabs`), so these clones live with the task.
        // Liveness travels with them too: a connected client's tab never
        // expires, even when tab-idle beyond the TTL.
        let sweep_registry = Arc::clone(&self.tab_registry);
        let sweep_states = Arc::clone(&self.tab_states);
        let sweep_tx = self.tab_registry_tx.clone();
        let sweep_live = Arc::clone(&self.live_clients);
        sweep_live.lock_or_recover().insert(client_id);
        let codec = self.codec;
        connections.spawn(async move {
            // The permit lives exactly as long as the connection task.
            let _permit = permit;
            // Liveness leaves when the task ends (panic-safe via drop); the
            // client's own departure sweep still sees it live — its entries
            // were just touched, and anything truly abandoned falls to the
            // next sweep.
            let _live_guard = LiveClientGuard {
                set: Arc::clone(&sweep_live),
                client_id,
            };
            // Arrival: expire abandoned tabs before this connection's
            // handshake so reclaimed entries were genuinely within the TTL.
            sweep_expired_tabs(&sweep_registry, &sweep_states, &sweep_tx, &sweep_live).await;
            if let Err(error) = handle_connection_with_analysis(
                stream,
                client_id,
                document,
                behavior,
                workspace,
                sdui,
                active_theme,
                runtime_diagnostics,
                runtime_generation,
                parse_coordinator,
                completion,
                document_analysis,
                language_intelligence,
                Some(reload_server),
                tab_registry,
                tab_registry_tx,
                codec,
            )
            .await
            {
                eprintln!("clay server connection {client_id} closed with error: {error}");
            }
            // Departure: a connection that died without closing its tab(s)
            // leaves abandoned entries for the next sweep.
            sweep_expired_tabs(&sweep_registry, &sweep_states, &sweep_tx, &sweep_live).await;
        });
    }
}

/// Phase 22.7: remove registry entries idle beyond `REGISTRY_TAB_TTL`
/// (abandoned tabs whose bound connection never returned) and push a
/// reconciling snapshot when anything was removed. Entries bound to a
/// currently connected client (`live_clients`) NEVER expire — the
/// connection task is the liveness proof, and tab-idle presence (document
/// editing never touches the registry) must not lose a live client's tab.
/// Runs on connection arrival and departure — O(n) over ≤
/// `MAX_ACTIVE_CONNECTIONS` entries, never per message — so the registry
/// stays bounded in long-running servers without a background task.
async fn sweep_expired_tabs(
    tab_registry: &Mutex<tab_registry::TabRegistry>,
    tab_states: &Mutex<HashMap<TabId, TabServerState>>,
    tab_registry_tx: &broadcast::Sender<TabRegistrySnapshot>,
    live_clients: &std::sync::Mutex<std::collections::HashSet<crate::protocol::ClientId>>,
) {
    let snapshot = {
        let mut registry = tab_registry.lock().await;
        let live = live_clients.lock_or_recover().clone();
        let removed = registry.sweep_expired(
            std::time::Instant::now(),
            crate::perf::budgets::REGISTRY_TAB_TTL,
            &live,
        );
        if !removed.is_empty() {
            let mut states = tab_states.lock().await;
            for tab_id in &removed {
                states.remove(tab_id);
            }
        }
        (!removed.is_empty()).then(|| registry.snapshot())
    };
    if let Some(snapshot) = snapshot {
        let _ = tab_registry_tx.send(snapshot);
    }
}

/// Phase 22.7: removes its client from the live set when the connection
/// task ends (drop is panic-safe).
struct LiveClientGuard {
    set: Arc<std::sync::Mutex<std::collections::HashSet<crate::protocol::ClientId>>>,
    client_id: crate::protocol::ClientId,
}

impl Drop for LiveClientGuard {
    fn drop(&mut self) {
        self.set.lock_or_recover().remove(&self.client_id);
    }
}

impl IpcServer {}

#[cfg(unix)]
fn bind_unix_listener(socket_path: &Path) -> Result<UnixListener, ServerError> {
    validate_socket_path(socket_path)?;
    remove_stale_socket(socket_path)?;
    let listener = UnixListener::bind(socket_path).map_err(ServerError::Bind)?;
    restrict_unix_socket_permissions(socket_path)?;
    Ok(listener)
}

#[cfg(unix)]
fn restrict_unix_socket_permissions(socket_path: &Path) -> Result<(), ServerError> {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    let permissions = Permissions::from_mode(0o600);
    fs::set_permissions(socket_path, permissions).map_err(ServerError::EndpointPermissions)
}

#[cfg(unix)]
fn validate_socket_path(socket_path: &Path) -> Result<(), ServerError> {
    if socket_path.as_os_str().is_empty() {
        return Err(ServerError::InvalidEndpoint(
            "socket path must not be empty".to_string(),
        ));
    }

    let Some(parent) = socket_path.parent() else {
        return Err(ServerError::InvalidEndpoint(
            "socket path must have a parent directory".to_string(),
        ));
    };
    let metadata = fs::metadata(parent).map_err(ServerError::EndpointDirectory)?;
    if !metadata.is_dir() {
        return Err(ServerError::InvalidEndpoint(format!(
            "socket parent {} is not a directory",
            parent.display()
        )));
    }

    validate_parent_directory_ownership(parent, &metadata)?;
    validate_parent_directory_mode(parent, &metadata)?;

    Ok(())
}

/// Reject endpoint parent directories that are group- or world-writable: any
/// local user could then replace or pre-create the socket. The one sanctioned
/// exception is a sticky shared directory (`/tmp`-style, mode `0o1777`),
/// where the sticky bit prevents non-owners from removing or replacing the
/// socket entry (Plan 060 T6, P1-10).
#[cfg(unix)]
fn validate_parent_directory_mode(
    parent: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ServerError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode() & 0o1777;
    if mode & 0o022 != 0 && mode & 0o1000 == 0 {
        return Err(ServerError::InvalidEndpoint(format!(
            "socket parent {} is group/world-writable (mode {mode:04o}) without the sticky bit; \
             refusing to create an IPC endpoint another local user could replace",
            parent.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_parent_directory_ownership(
    parent: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ServerError> {
    use std::os::unix::fs::MetadataExt;

    let dir_uid = metadata.uid();
    // SAFETY: getuid has no failure mode and is async-signal-safe.
    let process_uid = unsafe { libc::getuid() };
    if dir_uid != process_uid {
        return Err(ServerError::EndpointOwnership(format!(
            "socket parent {} is owned by uid {}, but this process runs as uid {}. \
             Refusing to create an IPC endpoint in a directory not owned by the current user.",
            parent.display(),
            dir_uid,
            process_uid
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn remove_stale_socket(socket_path: &Path) -> Result<(), ServerError> {
    let Ok(metadata) = fs::symlink_metadata(socket_path) else {
        return Ok(());
    };

    if metadata.file_type().is_socket() {
        // A socket file is only stale when nothing listens on it. If a live
        // server answers the probe, refuse to replace it: rebinding would
        // steal the endpoint path, orphaning the running server and every
        // tab connected to it (the old listener keeps a deleted inode).
        if unix_socket_has_live_listener(socket_path) {
            return Err(ServerError::EndpointInUse(
                socket_path.display().to_string(),
            ));
        }
        fs::remove_file(socket_path).map_err(ServerError::RemoveStaleSocket)?;
        return Ok(());
    }

    Err(ServerError::InvalidEndpoint(format!(
        "refusing to replace non-socket path {}",
        socket_path.display()
    )))
}

/// Probe-connect a Unix socket: `true` when a listener accepts. The probe
/// connection closes immediately; the live server tolerates the handshake
/// EOF (same path the client readiness probe uses).
#[cfg(unix)]
fn unix_socket_has_live_listener(socket_path: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket_path).is_ok()
}

#[cfg(windows)]
fn create_named_pipe_server(pipe_name: &str) -> Result<NamedPipeServer, ServerError> {
    let mut security =
        CurrentUserSecurityAttributes::new().map_err(ServerError::InvalidEndpoint)?;
    // SAFETY: `security.attributes` points at heap-allocated descriptor/ACL
    // owned by `security`, which outlives this synchronous CreateNamedPipe call.
    unsafe {
        ServerOptions::new()
            .create_with_security_attributes_raw(
                pipe_name,
                &mut security.attributes as *mut _ as *mut std::ffi::c_void,
            )
            .map_err(ServerError::Bind)
    }
    // `security` drops here, freeing the descriptor and ACL after the pipe is created.
}

#[cfg(windows)]
struct CurrentUserSecurityAttributes {
    token_user: windows::Win32::Foundation::HLOCAL,
    acl: windows::Win32::Foundation::HLOCAL,
    #[allow(dead_code)]
    security_descriptor: Box<SECURITY_DESCRIPTOR>,
    attributes: SECURITY_ATTRIBUTES,
}

#[cfg(windows)]
impl CurrentUserSecurityAttributes {
    fn new() -> Result<Self, String> {
        // Standard access-mask constants; the `windows` crate does not expose
        // GENERIC_ALL as a standalone constant in this version.
        const GENERIC_ALL: u32 = 0x1000_0000;
        const SECURITY_DESCRIPTOR_REVISION: u32 = 1;

        unsafe {
            // Open the current process token to read the user SID.
            let mut token = HANDLE(std::ptr::null_mut());
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|error| format!("OpenProcessToken failed: {error}"))?;

            // Query the size required for TokenUser.
            let mut required = 0u32;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut required);

            // Allocate the TokenUser buffer.
            let token_user = LocalAlloc(LPTR, required as usize).map_err(|error| {
                let _ = CloseHandle(token);
                format!("LocalAlloc failed for token user buffer: {error}")
            })?;
            if token_user.0.is_null() {
                let _ = CloseHandle(token);
                return Err("LocalAlloc returned null for token user buffer".to_string());
            }

            // Read TokenUser.
            if let Err(error) = GetTokenInformation(
                token,
                TokenUser,
                Some(token_user.0),
                required,
                &mut required,
            ) {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                return Err(format!("GetTokenInformation failed: {error}"));
            }

            let user = &*(token_user.0 as *const TOKEN_USER);
            let user_sid: PSID = user.User.Sid;

            // Build a DACL containing one ACE that grants the current user full access.
            let sid_length = GetLengthSid(user_sid);
            let acl_size = std::mem::size_of::<ACL>() as u32
                + sid_length
                + (std::mem::size_of::<ACCESS_ALLOWED_ACE>() as u32)
                - (std::mem::size_of::<u32>() as u32);
            let acl = LocalAlloc(LPTR, acl_size as usize).map_err(|error| {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                format!("LocalAlloc failed for ACL: {error}")
            })?;
            if acl.0.is_null() {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                return Err("LocalAlloc returned null for ACL".to_string());
            }

            InitializeAcl(acl.0 as *mut ACL, acl_size, ACL_REVISION).map_err(|error| {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                let _ = LocalFree(Some(acl));
                format!("InitializeAcl failed: {error}")
            })?;

            // GENERIC_ALL is broader than needed for a pipe, but it mirrors the
            // creator-owner rights the default DACL grants. For a least-privilege
            // refinement, use FILE_GENERIC_READ | FILE_GENERIC_WRITE.
            AddAccessAllowedAce(acl.0 as *mut ACL, ACL_REVISION, GENERIC_ALL, user_sid).map_err(
                |error| {
                    let _ = CloseHandle(token);
                    let _ = LocalFree(Some(token_user));
                    let _ = LocalFree(Some(acl));
                    format!("AddAccessAllowedAce failed: {error}")
                },
            )?;

            let _ = CloseHandle(token);

            // Build a security descriptor owning the DACL.
            let mut security_descriptor = Box::new(std::mem::zeroed());
            let sd_ptr =
                PSECURITY_DESCRIPTOR(&mut *security_descriptor as *mut _ as *mut std::ffi::c_void);
            InitializeSecurityDescriptor(sd_ptr, SECURITY_DESCRIPTOR_REVISION).map_err(
                |error| {
                    let _ = LocalFree(Some(token_user));
                    let _ = LocalFree(Some(acl));
                    format!("InitializeSecurityDescriptor failed: {error}")
                },
            )?;

            SetSecurityDescriptorDacl(sd_ptr, true, Some(acl.0 as *mut ACL), false).map_err(
                |error| {
                    let _ = LocalFree(Some(token_user));
                    let _ = LocalFree(Some(acl));
                    format!("SetSecurityDescriptorDacl failed: {error}")
                },
            )?;

            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd_ptr.0,
                bInheritHandle: false.into(),
            };

            Ok(Self {
                token_user,
                acl,
                security_descriptor,
                attributes,
            })
        }
    }
}

#[cfg(windows)]
impl Drop for CurrentUserSecurityAttributes {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(self.token_user));
            let _ = LocalFree(Some(self.acl));
            // self.security_descriptor is freed by Box::drop.
        }
    }
}

#[cfg(windows)]
async fn connect_named_pipe_server(pipe: NamedPipeServer) -> io::Result<NamedPipeServer> {
    match pipe.connect().await {
        Ok(()) => Ok(pipe),
        Err(error) if error.raw_os_error() == Some(ERROR_PIPE_CONNECTED) => Ok(pipe),
        Err(error) => Err(error),
    }
}

#[derive(Debug)]
pub enum ServerError {
    InvalidEndpoint(String),
    EndpointDirectory(io::Error),
    EndpointOwnership(String),
    EndpointPermissions(io::Error),
    EndpointInUse(String),
    RemoveStaleSocket(io::Error),
    Bind(io::Error),
    Accept(io::Error),
    InvalidWorkspaceRoot(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint(message) => write!(formatter, "invalid IPC endpoint: {message}"),
            Self::EndpointDirectory(error) => {
                write!(
                    formatter,
                    "failed to inspect IPC endpoint directory: {error}"
                )
            }
            Self::EndpointOwnership(message) => {
                write!(
                    formatter,
                    "IPC endpoint directory ownership check failed: {message}"
                )
            }
            Self::EndpointPermissions(error) => {
                write!(
                    formatter,
                    "failed to restrict IPC endpoint permissions: {error}"
                )
            }
            Self::RemoveStaleSocket(error) => {
                write!(formatter, "failed to remove stale socket: {error}")
            }
            Self::EndpointInUse(endpoint) => {
                write!(
                    formatter,
                    "IPC endpoint {endpoint} already has a running Clay server"
                )
            }
            Self::Bind(error) => write!(formatter, "failed to bind IPC endpoint: {error}"),
            Self::Accept(error) => write!(formatter, "failed to accept IPC connection: {error}"),
            Self::InvalidWorkspaceRoot(message) => {
                write!(formatter, "invalid workspace root: {message}")
            }
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EndpointDirectory(error)
            | Self::EndpointPermissions(error)
            | Self::RemoveStaleSocket(error)
            | Self::Bind(error)
            | Self::Accept(error) => Some(error),
            Self::InvalidEndpoint(_)
            | Self::EndpointOwnership(_)
            | Self::EndpointInUse(_)
            | Self::InvalidWorkspaceRoot(_) => None,
        }
    }
}

#[cfg(test)]
mod runtime_generation_tests;
#[cfg(test)]
mod runtime_outputs_tests;
#[cfg(test)]
mod tab_server_state_tests;

#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, windows))]
mod windows_tests;
