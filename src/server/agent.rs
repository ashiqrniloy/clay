//! Core-owned `clay-agent` child. One daemon per Clay server.
//!
//! Package JavaScript never receives this type. Spawn is `Command` +
//! `env_clear`, never a shell string. Node missing is a diagnostic, not a hang.

use std::collections::HashMap;
use std::future::Future;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio::time::timeout;

use crate::protocol::{
    AGENT_DAEMON_MAX_LINE_BYTES, AGENT_MAX_ENTRY_TEXT_BYTES, AGENT_MAX_PROMPT_BYTES,
    AGENT_MAX_SNAPSHOT_ENTRIES, AgentClientCommand, AgentInventory, AgentMcpServerInfo,
    AgentModelInfo, AgentOmWorkerKind, AgentOmWorkerModel, AgentPickerItem, AgentPickerKind,
    AgentProfileInfo, AgentProviderInfo, AgentSecret, AgentServerMessage, AgentSessionInfo,
    AgentSessionSnapshot, AgentSkillInfo, AgentSlashCommand, AgentToolPhase, AgentTranscriptEntry,
    AgentTranscriptFile, AgentTranscriptKind, AgentWireEvent, ApprovalRequestKind, TabId,
    apply_transcript_event, transcript_file_for_tool, truncate_transcript_text,
};
use crate::server::agent_picker::AgentSearchHit;

// Plan 119 SC-3: the file used to hold the book, the run pipeline, the MCP
// allow-list and the daemon inventory alongside the actor itself. They are
// split into focused submodules, mechanically (no logic edits); the paths
// outside this module keep resolving through the re-exports below.
mod book;
mod mcp;
mod run;

pub(crate) use book::{AgentPickerAuth, AgentPickerInventory, AgentPickerProvider};
pub use mcp::AgentMcpAllowListEntry;

use book::{
    SessionBook, apply_book_event, book_snapshot, json_om_workers, load_persisted_book,
    read_git_branch, refresh_branch, selection_for, snapshot_from_new, workspace_key,
};
use mcp::DaemonEnvironment;
use run::{PendingApprovals, map_event};

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(5);
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const EVENT_CAPACITY: usize = 256;
/// Handler for daemon-initiated reverse-RPC requests (document reads/writes
/// and user-approval asks). Installed once by the server before the first
/// spawn; package JavaScript never sees this type.
pub type ReverseRpcHandler = Arc<
    dyn Fn(String, Value) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Debug)]
pub enum AgentError {
    NodeMissing,
    ScriptMissing,
    Spawn(io::Error),
    MissingPipe,
    FrameTooLarge { len: usize },
    Timeout,
    ChildExited,
    Rpc(String),
    ServiceStopped,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeMissing => {
                f.write_str("Node >= 22 is required for clay-agent but was not found")
            }
            Self::ScriptMissing => f.write_str("clay-agent script was not found"),
            Self::Spawn(error) => write!(f, "failed to spawn clay-agent: {error}"),
            Self::MissingPipe => f.write_str("clay-agent stdio pipe missing"),
            Self::FrameTooLarge { len } => {
                write!(f, "clay-agent frame too large ({len} bytes)")
            }
            Self::Timeout => f.write_str("clay-agent RPC timed out"),
            Self::ChildExited => f.write_str("clay-agent exited"),
            Self::Rpc(message) => f.write_str(message),
            Self::ServiceStopped => f.write_str("clay-agent host stopped"),
        }
    }
}

impl std::error::Error for AgentError {}

/// The Clay root a server without an explicit configuration root keeps its
/// agent state under: the per-user `~/.clay`, which is also the daemon's own
/// `homedir()` default and the runtime's effective root. `None` only when no
/// home is knowable.
///
/// Under `cfg(test)` a per-process temp root stands in: a unit test that
/// constructs a server without a root (most of them do) must never read or
/// write the developer's real profile — the integration suites that prove the
/// per-user resolution isolate `HOME` themselves.
fn root_less_agent_config_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        Some(std::env::temp_dir().join(format!("clay-agent-test-root-{}", std::process::id())))
    }
    #[cfg(not(test))]
    {
        crate::server::configuration::ConfigurationRuntime::default_config_root()
    }
}

#[derive(Debug, Clone)]
pub struct AgentHostConfig {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub data_dir: PathBuf,
    pub inherit_environment: Vec<String>,
    /// When true, skip spawn and return diagnostics. Used by tests that do not
    /// exercise the child.
    pub inert: bool,
    /// Server-built MCP allow-list (decision 1758). The daemon validates every
    /// entry fail-closed (canonical executable, literal argv, explicit env
    /// names); an empty list connects nothing. Package JS never supplies argv.
    pub mcp_allow_list: Vec<AgentMcpAllowListEntry>,
}

impl AgentHostConfig {
    pub(crate) fn for_server(
        configuration_root: Option<&Path>,
        workspace_root: Option<&Path>,
    ) -> Self {
        // Per-agent data dir (decision 2026-09-10-1526): runtime state lives
        // under the agent config root as `agents/coding-agent/data`. One-time
        // migration: a legacy `<config-root>/agent/` dir is renamed into
        // place (atomic within the config root) so sessions.sqlite,
        // credentials.vault, book.json and vault.passphrase follow; on
        // rename failure the legacy dir keeps serving (never orphan
        // credentials).
        //
        // Absent an explicit root the per-user Clay root (`~/.clay`) applies —
        // the same root the runtime and the daemon's own homedir() default
        // resolve to. A shared `temp_dir()` fallback used to live here, which
        // made every root-less server (the desktop's own launch, a fixture, an
        // isolated HOME, a bare `clay server`) share one book.json /
        // credentials.vault / sessions.sqlite: a stale selection from another
        // run then named a profile this run never registered, and the tab
        // could not bind a session at all. Only a missing HOME still falls
        // back to temp.
        let data_dir = configuration_root
            .map(Path::to_path_buf)
            .or_else(root_less_agent_config_root)
            .map(|root| {
                let new_dir = root.join("agents").join("coding-agent").join("data");
                let legacy_dir = root.join("agent");
                if legacy_dir.is_dir() && !new_dir.exists() {
                    if let Some(parent) = new_dir.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(error) = std::fs::rename(&legacy_dir, &new_dir) {
                        eprintln!(
                            "[agent] migrating {} -> {} failed ({error}); continuing with the legacy data dir",
                            legacy_dir.display(),
                            new_dir.display()
                        );
                        return legacy_dir;
                    }
                }
                new_dir
            })
            .unwrap_or_else(|| std::env::temp_dir().join("clay-agent"));
        // The per-agent config root follows the SERVER's configuration root
        // (plan 117 launch-test finding): a fixture or isolated HOME must
        // not fall back to the daemon's homedir() default. Absent a root,
        // no flag — the daemon keeps its homedir() default.
        let mut args = Vec::new();
        if let Some(root) = configuration_root {
            args.push("--agent-config-root".to_string());
            args.push(
                root.join("agents")
                    .join("coding-agent")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        Self {
            program: PathBuf::new(),
            args,
            data_dir,
            // The daemon spawn env_clear()s; inherit exactly what it needs:
            // HOME/USERPROFILE so its per-agent config root
            // (~/.clay/agents/coding-agent) follows the server's
            // isolated profile, PATH so MCP bare-name commands resolve.
            inherit_environment: vec![
                "HOME".to_string(),
                "USERPROFILE".to_string(),
                "PATH".to_string(),
            ],
            inert: false,
            // Plan 117: user mcp.json + repo .mcp.json merge into the
            // server-built allow-list (decision 2026-09-09-1341).
            // The shipped agent's list doubles as the initialize-time default
            // (plan 118 task 35: each agent's own list rides its session).
            mcp_allow_list: super::agent_mcp_config::build_mcp_allow_list(
                configuration_root,
                workspace_root,
                None,
            ),
        }
    }
}

enum HostCommand {
    Rpc {
        method: String,
        params: Value,
        reply: oneshot::Sender<Result<Value, AgentError>>,
    },
    Shutdown,
}

struct Running {
    commands: mpsc::Sender<HostCommand>,
}

/// Plan 118 task 35: where per-agent config roots resolve from, installed by
/// the owning server (mirrors `set_tab_registry`).
#[derive(Debug, Clone)]
struct AgentRoots {
    /// Clay data root (`~/.clay` or the explicit configuration root).
    config_root: Option<PathBuf>,
}

struct Inner {
    config: AgentHostConfig,
    events: broadcast::Sender<Arc<AgentServerMessage>>,
    // ponytail: one mutex for the child; per-session queues if prompt throughput matters
    state: Mutex<Option<Running>>,
    secrets: Arc<Mutex<Vec<String>>>,
    book: Arc<Mutex<SessionBook>>,
    reverse: Mutex<Option<ReverseRpcHandler>>,
    /// Server tab registry, installed once by the owning server (plan 109
    /// I1): the source of truth for each tab's current workspace root.
    tab_roots: Mutex<Option<Arc<Mutex<super::tab_registry::TabRegistry>>>>,
    /// Plan 118 task 35: the Clay data root + launch workspace root, installed
    /// by the server so the host can resolve per-agent config roots
    /// (`<data root>/agents/<agent type>`) and build that agent's MCP
    /// allow-list per session. `None` ⇒ the host keeps the single shipped
    /// agent (inert/test hosts).
    agent_roots: Mutex<Option<AgentRoots>>,
    /// Pending daemon-initiated approval requests, keyed by request id.
    /// Resolved by `ApprovalResolve`/`AskDecisionResolve`; dropped senders
    /// and timeouts deny fail-closed.
    approvals: Arc<Mutex<PendingApprovals>>,
    approval_seq: AtomicU64,
    /// Registration RPCs queued while the daemon is not yet running
    /// (package load entries must never spawn or block on the daemon).
    /// Drained in order right after the initialize handshake succeeds.
    pending_registrations: Arc<Mutex<Vec<HostCommand>>>,
    /// Daemon environment (plan 109 R1/R3): registered slash commands +
    /// active extensions + catalog skills, fetched once per daemon
    /// generation and invalidated on daemon death or a registration / new
    /// session / knowledge mutation. Plan 118 task 35: keyed by agent type
    /// (`""` = the default agent), because MCP outcomes follow the session's
    /// agent — a switch shows that agent's servers.
    environment: Mutex<HashMap<String, DaemonEnvironment>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentOauthStart {
    pub login_id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub authorization_url: String,
}

#[derive(Debug, Clone)]
pub(crate) enum AgentOauthPoll {
    Pending,
    Complete,
}

#[derive(Clone)]
pub struct AgentHost {
    inner: Arc<Inner>,
}

static AGENT_HOST_AUTHORITY: std::sync::OnceLock<AgentHostHandle> = std::sync::OnceLock::new();

/// Process-global authority cell for the `agent` JS ops. One clay-agent child
/// per server means one authority per process; the server installs it at
/// startup and every JS runtime worker reads the same handle. Unset → ops
/// fail closed.
#[derive(Clone)]
pub struct AgentHostHandle(Arc<AgentHost>);

impl std::fmt::Debug for AgentHostHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AgentHostHandle")
    }
}

/// A registration declaration queued before any agent host exists (hostless
/// runtimes: unit harnesses, embedded workers). Applied to the host when one
/// installs. `install_global` drains this into the host's own pending queue.
#[derive(Debug, Clone)]
pub(crate) struct PackageRegistration {
    pub method: String,
    pub params: Value,
}

static PENDING_PACKAGE_REGISTRATIONS: std::sync::Mutex<Vec<PackageRegistration>> =
    std::sync::Mutex::new(Vec::new());

const PENDING_REGISTRATION_CAP: usize = 64;

/// Queue a registration declaration with no host attached. Package load
/// entries never fail just because this runtime has no agent host: the
/// declaration applies when a host installs (see [`install_global`]).
pub(crate) fn queue_package_registration(method: &str, params: Value) -> Result<(), AgentError> {
    let mut pending = PENDING_PACKAGE_REGISTRATIONS
        .lock()
        .map_err(|_| AgentError::ServiceStopped)?;
    if pending.len() >= PENDING_REGISTRATION_CAP {
        return Err(AgentError::ServiceStopped);
    }
    pending.push(PackageRegistration {
        method: method.to_string(),
        params,
    });
    Ok(())
}

/// Drain every queued package registration (test/introspection and
/// install-time handoff).
pub(crate) fn take_pending_package_registrations() -> Vec<PackageRegistration> {
    PENDING_PACKAGE_REGISTRATIONS
        .lock()
        .map(|mut pending| pending.drain(..).collect())
        .unwrap_or_default()
}

impl AgentHostHandle {
    pub(crate) fn new(host: AgentHost) -> Self {
        Self(Arc::new(host))
    }

    /// Install the process-global authority. First install wins; later
    /// servers in the same process (tests) keep the first handle. Package
    /// registrations queued while no host existed move into the host's own
    /// pending queue, preserving order, and apply after its first
    /// initialize handshake.
    pub(crate) fn install_global(host: AgentHost) {
        let handle = Self::new(host.clone());
        let _ = AGENT_HOST_AUTHORITY.set(handle);
        // Move anything queued before the host existed into the host's own
        // pending queue, preserving order; they apply after its first
        // initialize handshake. No registration op can run before server
        // construction installs the authority, so one drain after the set
        // covers every queued entry.
        let drained = take_pending_package_registrations();
        if !drained.is_empty()
            && let Ok(mut pending) = host.inner.pending_registrations.try_lock()
        {
            for registration in drained {
                let (reply_tx, _reply_rx) = oneshot::channel();
                pending.push(HostCommand::Rpc {
                    method: registration.method,
                    params: registration.params,
                    reply: reply_tx,
                });
            }
        }
    }

    /// Process-global authority, or a fail-closed error naming the missing
    /// wiring. Never grants authority by existing.
    pub(crate) fn global() -> Result<Self, AgentError> {
        AGENT_HOST_AUTHORITY
            .get()
            .cloned()
            .ok_or(AgentError::ServiceStopped)
    }

    /// Forward a raw daemon RPC (e.g. `session.setAutonomy`). Errors and
    /// results are daemon-produced; callers redact before surfacing.
    pub(crate) async fn rpc(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        self.0.rpc(method, params).await
    }

    pub(crate) async fn rpc_or_queue(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, AgentError> {
        self.0.rpc_or_queue(method, params).await
    }

    #[cfg(test)]
    pub(crate) async fn take_pending_registrations(&self) -> Vec<PackageRegistration> {
        self.0.take_pending_registrations().await
    }
}

impl std::fmt::Debug for AgentHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AgentHost")
    }
}

impl AgentHost {
    pub fn new(config: AgentHostConfig) -> Self {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let book = load_persisted_book(&config.data_dir);
        Self {
            inner: Arc::new(Inner {
                config,
                events,
                state: Mutex::new(None),
                secrets: Arc::new(Mutex::new(Vec::new())),
                book: Arc::new(Mutex::new(book)),
                reverse: Mutex::new(None),
                tab_roots: Mutex::new(None),
                agent_roots: Mutex::new(None),
                environment: Mutex::new(HashMap::new()),
                approvals: Arc::new(Mutex::new(HashMap::new())),
                approval_seq: AtomicU64::new(1),
                pending_registrations: Arc::new(Mutex::new(Vec::new())),
            }),
        }
    }

    pub fn inert() -> Self {
        Self::new(AgentHostConfig {
            program: PathBuf::new(),
            args: Vec::new(),
            data_dir: PathBuf::new(),
            inherit_environment: Vec::new(),
            inert: true,
            mcp_allow_list: Vec::new(),
        })
    }

    pub(crate) fn for_server(
        configuration_root: Option<&Path>,
        workspace_root: Option<&Path>,
    ) -> Self {
        Self::new(AgentHostConfig::for_server(
            configuration_root,
            workspace_root,
        ))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<AgentServerMessage>> {
        self.inner.events.subscribe()
    }

    /// Install the reverse-RPC handler. Call once before the first spawn.
    pub fn set_reverse_handler(&self, handler: ReverseRpcHandler) {
        if let Ok(mut reverse) = self.inner.reverse.try_lock() {
            *reverse = Some(handler);
        }
    }

    /// Install the server's tab registry (plan 109 I1). The registry is the
    /// single source of truth for tab→workspace-root binding; the host only
    /// reads it (cached in-memory lookup, never a filesystem probe).
    pub(crate) fn set_tab_registry(&self, registry: Arc<Mutex<super::tab_registry::TabRegistry>>) {
        if let Ok(mut tab_roots) = self.inner.tab_roots.try_lock() {
            *tab_roots = Some(registry);
        }
    }

    /// Test seam (plan 119 SC-6): record a session's workspace root without a
    /// daemon round-trip — the field the agent tool path resolves against.
    #[cfg(test)]
    pub(crate) async fn record_session_root_for_test(&self, session_id: &str, root: &str) {
        self.inner
            .book
            .lock()
            .await
            .session_root
            .insert(session_id.to_string(), root.to_string());
    }

    /// Plan 118 task 35: install the roots per-agent config resolves from
    /// (the Clay data root). Called once by the owning server right after
    /// construction; without it the host runs the single shipped agent.
    /// The workspace root is *not* a host-level root (plan 119 SC-6): every
    /// session's `.mcp.json` comes from that session's own workspace root.
    pub(crate) fn set_agent_roots(&self, config_root: Option<PathBuf>) {
        if let Ok(mut roots) = self.inner.agent_roots.try_lock() {
            *roots = Some(AgentRoots { config_root });
        }
    }

    /// The tab's current workspace root from the registry. `None` when no
    /// registry is installed (tests) or the tab is unregistered.
    async fn tab_workspace_root(&self, tab: TabId) -> Option<String> {
        let registry = self.inner.tab_roots.lock().await.clone()?;
        let registry = registry.lock().await;
        registry
            .entry(tab)
            .map(|entry| entry.workspace_root)
            .filter(|root| !root.is_empty())
    }

    /// Same lookup with an already-optional tab (plan 109 I2 selection
    /// writes): `None` when the caller has no bound tab.
    async fn tab_workspace_root_for(&self, tab: Option<TabId>) -> Option<String> {
        self.tab_workspace_root(tab?).await
    }

    /// Plan 118 task 35: the tab's agent type from the registry (`None` when
    /// no registry is installed, the tab is unregistered, or it holds no
    /// agent). The registry accepts only names that resolve under the data
    /// root's `agents/`, so this value is always a configured agent or none.
    async fn tab_agent_type(&self, tab: TabId) -> Option<String> {
        let registry = self.inner.tab_roots.lock().await.clone()?;
        let registry = registry.lock().await;
        registry.agent_type(tab).filter(|agent| !agent.is_empty())
    }

    async fn tab_agent_type_for(&self, tab: Option<TabId>) -> Option<String> {
        self.tab_agent_type(tab?).await
    }

    /// The session bound to a tab's current `(workspace root, agent type)`
    /// (plan 119 SC-6). Sibling tabs on one workspace resolve the same
    /// session; `None` = this tab has no live session yet.
    /// Public within the server crate so the connection layer can stamp a
    /// tab's session onto its own answers (plan 119 SC-6 tab bindings).
    pub(crate) async fn tab_session_id(&self, tab: TabId) -> Option<String> {
        let root = self.tab_workspace_root(tab).await;
        let agent = self.tab_agent_type(tab).await;
        self.inner
            .book
            .lock()
            .await
            .session_for_workspace(agent.as_deref(), root.as_deref())
    }

    /// Plan 119 SC-6 (decision 2026-09-14-1705): the workspace root a session
    /// runs against — the authority for every agent tool file resolution. The
    /// live book only (recorded at creation/resume); a session the server
    /// does not know resolves to `None` so the caller fails closed instead of
    /// touching the launch root.
    pub(crate) async fn session_workspace_root(&self, session_id: &str) -> Option<String> {
        self.inner
            .book
            .lock()
            .await
            .session_root
            .get(session_id)
            .filter(|root| !root.is_empty())
            .cloned()
    }

    /// The tab's `session.new` parameters, agent-resolved: the agent's MCP
    /// allow-list (its own `mcp.json` merged with the *session's* repo
    /// `.mcp.json`, plan 119 SC-6) rides the session so one agent's servers
    /// are never granted to another, and no session inherits the launch
    /// folder's grants.
    pub(crate) async fn agent_config_root(&self, agent_type: &str) -> Option<PathBuf> {
        let roots = self.inner.agent_roots.lock().await.clone()?;
        super::launcher::resolve_agent_type(roots.config_root.as_deref(), agent_type)
    }

    /// Surface a daemon-initiated user-approval request to connected clients
    /// and wait bounded for the answer. Denies fail-closed on timeout or
    /// missing consumer. `payload_json` is daemon-produced; oversized
    /// payloads are rejected before any client sees them.
    pub fn dispatch(&self, command: AgentClientCommand) {
        let host = self.clone();
        tokio::spawn(async move {
            let message = host.run(command).await;
            let _ = host.inner.events.send(Arc::new(message));
        });
    }

    fn emit_agent(&self, message: AgentServerMessage) -> AgentServerMessage {
        let _ = self.inner.events.send(Arc::new(message.clone()));
        message
    }

    pub(crate) fn broadcast(&self, message: AgentServerMessage) {
        let _ = self.inner.events.send(Arc::new(message));
    }

    /// Workspace-scoped session search for the picker (plan 108 task 11):
    /// the query runs against the shared Phase 1 FTS index, scoped to the
    /// tab's current session workspace (decision 2201). Bounded by `limit`.
    async fn ensure_tab_session(&self, tab: TabId) -> Option<String> {
        let workspace_root = self.tab_workspace_root(tab).await;
        let agent_type = self.tab_agent_type(tab).await;
        {
            let mut book = self.inner.book.lock().await;
            // Plan 119 SC-6: the lookup is the workspace's, not the tab's — a
            // sibling tab on the same (agent, root) adopts the same session.
            if let Some(session_id) =
                book.session_for_workspace(agent_type.as_deref(), workspace_root.as_deref())
            {
                // Plan 109 R2: keep the root binding and the branch fresh —
                // cached within the run generation, re-read across runs.
                if let Some(root) = workspace_root.as_deref() {
                    book.session_root
                        .insert(session_id.clone(), root.to_string());
                    refresh_branch(&mut book, root, &session_id, read_git_branch);
                }
                return Some(session_id);
            }
        }
        // Plan 109 I2: resolve the workspace's last-used selection (global
        // trio as fallback). A configured check needs the provider inventory,
        // fetched only on actual session creation — never on the cached path.
        // Fully-empty book (no workspace entry, no global trio) short-circuits
        // without the inventory RPC.
        {
            let book = self.inner.book.lock().await;
            let has_workspace_entry = workspace_root
                .as_deref()
                .and_then(|root| book.workspaces.get(root));
            if has_workspace_entry.is_none() && (book.provider.is_empty() || book.model.is_empty())
            {
                eprintln!(
                    "[agent] ensure_tab_session({tab:?}): empty book (provider='{}' model='{}')",
                    book.provider, book.model
                );
                return None;
            }
        }
        let inventory = self.picker_inventory().await;
        let is_configured = |provider: &str| {
            inventory
                .providers
                .iter()
                .any(|candidate| candidate.id == provider && candidate.configured)
        };
        let selection = {
            let book = self.inner.book.lock().await;
            let resolved = selection_for(
                &book,
                agent_type.as_deref(),
                workspace_root.as_deref(),
                &is_configured,
            );
            if resolved.provider.is_empty() || resolved.model.is_empty() {
                eprintln!(
                    "[agent] ensure_tab_session({tab:?}): empty selection (provider='{}' model='{}')",
                    resolved.provider, resolved.model
                );
                return None;
            }
            resolved
        };
        let profile = if selection.profile.is_empty() {
            "Chat".to_string()
        } else {
            selection.profile
        };
        let created = self
            .create_session(
                &profile,
                &selection.provider,
                &selection.model,
                workspace_root.as_deref(),
                agent_type.as_deref(),
                selection.om_observation,
                selection.om_reflection,
            )
            .await;
        let snapshot = match created {
            Ok(snapshot) => snapshot,
            Err(error) => {
                eprintln!("[agent] ensure_tab_session({tab:?}): session.new failed -> {error:?}");
                return None;
            }
        };
        if snapshot.session_id.is_empty() {
            eprintln!("[agent] ensure_tab_session({tab:?}): NewSession returned empty session id");
            return None;
        }
        let mut book = self.inner.book.lock().await;
        book.bind_workspace_session(
            agent_type.as_deref(),
            workspace_root.as_deref(),
            &snapshot.session_id,
        );
        match agent_type.as_deref() {
            Some(agent) => {
                book.session_agent
                    .insert(snapshot.session_id.clone(), agent.to_string());
            }
            None => {
                book.session_agent.remove(&snapshot.session_id);
            }
        }
        book.transcripts
            .entry(snapshot.session_id.clone())
            .or_default();
        // Plan 117: a fresh session's branch must render from the first
        // snapshot — record the root (the settle refresh reads it) and read
        // the branch now, not only on later rebinds.
        if let Some(root) = workspace_root.as_deref() {
            book.session_root
                .insert(snapshot.session_id.clone(), root.to_string());
            refresh_branch(&mut book, root, &snapshot.session_id, read_git_branch);
        }
        Some(snapshot.session_id)
    }

    /// Create a daemon session (plan 118 task 35: with the tab's agent type,
    /// so the daemon resolves that agent's config root — SYSTEM.md, skills,
    /// tool caps — and connects only the servers that agent declares).
    #[allow(clippy::too_many_arguments)]
    async fn create_session(
        &self,
        profile: &str,
        provider: &str,
        model: &str,
        workspace_root: Option<&str>,
        agent_type: Option<&str>,
        om_observation: Option<AgentOmWorkerModel>,
        om_reflection: Option<AgentOmWorkerModel>,
    ) -> Result<AgentSessionSnapshot, AgentError> {
        let mut params = self.agent_session_params(agent_type, workspace_root).await;
        params.insert("profile".into(), json!(profile));
        params.insert("provider".into(), json!(provider));
        params.insert("model".into(), json!(model));
        if let Some(root) = workspace_root {
            params.insert("workspaceRoot".into(), json!(root));
        }
        // Plan 109 I8: the workspace book's OM worker defaults ride session
        // creation (per-session retention is daemon metadata).
        let om_workers = json_om_workers(om_observation, om_reflection);
        if !om_workers.is_null() {
            params.insert("observationalMemoryWorkers".into(), om_workers);
        }
        let result = self.rpc("session.new", Value::Object(params)).await?;
        Ok(self.decorate_snapshot(snapshot_from_new(&result)).await)
    }

    /// Plan 118 task 35: the agent type a session id runs as, read from the
    /// live book or (for a session this server has not seen yet) from the
    /// daemon's record metadata.
    async fn session_agent(&self, session_id: &str) -> Option<String> {
        {
            let book = self.inner.book.lock().await;
            if let Some(agent) = book.session_agent.get(session_id)
                && !agent.is_empty()
            {
                return Some(agent.clone());
            }
        }
        let loaded = self
            .rpc("session.load", json!({ "sessionId": session_id }))
            .await
            .ok()?;
        loaded
            .get("metadata")
            .and_then(|meta| meta.get("agentType"))
            .and_then(Value::as_str)
            .filter(|agent| !agent.is_empty())
            .map(str::to_string)
    }

    /// Plan 118 task 35: align the tab's live agent session with the tab's
    /// agent type. Called after the tab registry accepted a new type (or
    /// detached it — the daemon's default agent is the detach target, so a
    /// cleared type really does leave the previous agent's config).
    ///
    /// `Ok(None)` = nothing to align (the tab has no live session yet — its
    /// next interaction creates one for the tab's agent). A live session is
    /// switched in place through the daemon's `session.setAgent`, which keeps
    /// the session id and its transcript and re-reads only that agent's
    /// config; the tab's workspace is untouched. The previous agent's model
    /// pick is not carried over: the new agent's own last-used selection
    /// (else the book's fallback) resolves, so the model/effort controls reset
    /// to that agent's defaults.
    ///
    /// `previous_agent` is the type the tab carried *before* the registry
    /// accepted the new one (plan 119 SC-6): the live session is keyed by the
    /// old pair, and the switch moves that one session to the new key.
    pub async fn rebind_tab_agent(
        &self,
        tab: TabId,
        previous_agent: Option<&str>,
    ) -> Result<Option<AgentSessionSnapshot>, AgentError> {
        let agent = self.tab_agent_type(tab).await;
        let root = self.tab_workspace_root(tab).await;
        let session_id = {
            let book = self.inner.book.lock().await;
            book.session_for_workspace(previous_agent, root.as_deref())
        };
        let Some(session_id) = session_id else {
            return Ok(None);
        };
        let inventory = self.picker_inventory().await;
        let is_configured = |provider: &str| {
            inventory
                .providers
                .iter()
                .any(|candidate| candidate.id == provider && candidate.configured)
        };
        let selection = {
            let book = self.inner.book.lock().await;
            selection_for(&book, agent.as_deref(), root.as_deref(), &is_configured)
        };
        // No agent = the daemon's default agent: the params omit the name (and
        // its allow-list), so the switch lands on the shipped root rather than
        // keeping whatever the tab ran before.
        let mut params = self
            .agent_session_params(agent.as_deref(), root.as_deref())
            .await;
        params.insert("sessionId".into(), json!(session_id));
        if !selection.provider.is_empty() && !selection.model.is_empty() {
            params.insert("provider".into(), json!(selection.provider));
            params.insert("model".into(), json!(selection.model));
            // Plan 109 I8: the new agent's own OM worker defaults ride the
            // switch the same way they ride session creation.
            let om_workers = json_om_workers(selection.om_observation, selection.om_reflection);
            if !om_workers.is_null() {
                params.insert("observationalMemoryWorkers".into(), om_workers);
            }
        }
        // The daemon validates the agent type again (containment inside its
        // own `agents/` root) and fails closed; the caller then reverts the
        // registry so the tab never claims an agent its session is not using.
        let result = self.rpc("session.setAgent", Value::Object(params)).await?;
        let mut book = self.inner.book.lock().await;
        // One session, moved to the new key: the tab keeps its transcript and
        // every sibling tab on the same (agent, root) resolves the same id.
        book.sessions_by_workspace
            .remove(&workspace_key(previous_agent, root.as_deref()));
        book.bind_workspace_session(agent.as_deref(), root.as_deref(), &session_id);
        match agent.as_deref() {
            Some(agent) => {
                book.session_agent
                    .insert(session_id.clone(), agent.to_string());
            }
            None => {
                book.session_agent.remove(&session_id);
            }
        }
        if let Some(provider) = result.get("provider").and_then(Value::as_str)
            && !provider.is_empty()
        {
            book.provider = provider.to_string();
        }
        if let Some(model) = result.get("model").and_then(Value::as_str)
            && !model.is_empty()
        {
            book.model = model.to_string();
        }
        // The new agent is a new effort context: the previous level was the
        // old model's.
        book.effort.remove(&session_id);
        drop(book);
        Ok(Some(self.snapshot_for(&session_id).await))
    }

    pub(crate) async fn put_credential(
        &self,
        provider: &str,
        name: &str,
        secret: &str,
    ) -> Result<(), AgentError> {
        let message = self
            .run(AgentClientCommand::CredentialPut {
                provider: provider.to_string(),
                name: name.to_string(),
                secret: AgentSecret(secret.to_string()),
            })
            .await;
        let failed = match &message {
            AgentServerMessage::CredentialAck { stored: true, .. } => None,
            AgentServerMessage::Diagnostic { message, .. } => Some(message.clone()),
            _ => Some("credential.put failed".to_string()),
        };
        let _ = self.inner.events.send(Arc::new(message));
        match failed {
            Some(message) => Err(AgentError::Rpc(message)),
            None => Ok(()),
        }
    }

    pub(crate) async fn start_oauth(&self, provider: &str) -> Result<AgentOauthStart, AgentError> {
        let value = self
            .rpc("credential.oauthStart", json!({ "provider": provider }))
            .await?;
        Ok(AgentOauthStart {
            login_id: value
                .get("loginId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            user_code: value
                .get("userCode")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            verification_uri: value
                .get("verificationUri")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            authorization_url: value
                .get("authorizationUrl")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
    }

    pub(crate) async fn poll_oauth(&self, login_id: &str) -> Result<AgentOauthPoll, AgentError> {
        let value = self
            .rpc("credential.oauthPoll", json!({ "loginId": login_id }))
            .await?;
        match value.get("status").and_then(Value::as_str) {
            Some("complete") => Ok(AgentOauthPoll::Complete),
            _ => Ok(AgentOauthPoll::Pending),
        }
    }

    /// Forward a registration RPC, or queue it server-side when the daemon
    /// is not yet running. Package load entries call this so a load never
    /// spawns the daemon or waits on its boot: queued declarations are
    /// applied right after the next successful initialize handshake, before
    /// any later command (session ordering is FIFO through the actor).
    pub async fn rpc_or_queue(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        if self.inner.state.lock().await.is_some() {
            match self.rpc(method, params.clone()).await {
                Ok(result) => return Ok(result),
                // Daemon gone/stale (server dropped, child killed): defer the
                // declaration exactly like a not-yet-running daemon.
                Err(AgentError::ServiceStopped) => {}
                Err(error) => return Err(error),
            }
        }
        self.queue_registration(method, params).await
    }

    async fn queue_registration(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        const PENDING_REGISTRATION_CAP: usize = 64;
        let mut pending = self.inner.pending_registrations.lock().await;
        if pending.len() >= PENDING_REGISTRATION_CAP {
            return Err(AgentError::ServiceStopped);
        }
        let (reply_tx, _reply_rx) = oneshot::channel();
        pending.push(HostCommand::Rpc {
            method: method.to_string(),
            params,
            reply: reply_tx,
        });
        Ok(json!({ "queued": true }))
    }

    /// Drain the host's pending registration queue (test/introspection).
    /// Entries come back as plain declarations.
    #[cfg(test)]
    pub(crate) async fn take_pending_registrations(&self) -> Vec<PackageRegistration> {
        let pending = self.inner.pending_registrations.lock().await;
        pending
            .iter()
            .filter_map(|command| match command {
                HostCommand::Rpc { method, params, .. } => Some(PackageRegistration {
                    method: method.clone(),
                    params: params.clone(),
                }),
                HostCommand::Shutdown => None,
            })
            .collect()
    }

    /// Test/introspection accessor: queued registration count.
    pub async fn pending_registration_len(&self) -> usize {
        self.inner.pending_registrations.lock().await.len()
    }

    pub async fn rpc(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        let running = self.ensure_running().await?;
        // Plan 117: a /wiki-init prompt enables the wiki binding daemon-side
        // (slash intercept), so it needs the same environment-cache
        // invalidation the knowledge.setOptions passthrough gets. Read the
        // marker before params moves into the command.
        let wiki_init_prompt = method == "session.prompt"
            && params
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|text| text.trim_start().starts_with("/wiki-init"));
        let (reply_tx, reply_rx) = oneshot::channel();
        running
            .commands
            .send(HostCommand::Rpc {
                method: method.to_string(),
                params,
                reply: reply_tx,
            })
            .await
            .map_err(|_| AgentError::ServiceStopped)?;
        match timeout(RPC_TIMEOUT, reply_rx).await {
            Ok(Ok(result)) => {
                // Plan 109 R1/R3: a new registration or a wiki/graft
                // toggle changes the daemon environment; drop the cache
                // so the next snapshot re-fetches it.
                if matches!(
                    method,
                    "command.register" | "skill.register" | "knowledge.setOptions" | "session.new"
                ) || wiki_init_prompt
                {
                    self.inner.environment.lock().await.clear();
                }
                result
            }
            Ok(Err(_)) => {
                self.inner.environment.lock().await.clear();
                Err(AgentError::ServiceStopped)
            }
            Err(_) => {
                self.inner.environment.lock().await.clear();
                Err(AgentError::Timeout)
            }
        }
    }

    /// Plan 109 R1/R3: the daemon's registered commands + active
    /// extensions, cached per daemon generation. Failure-silent — an
    /// unavailable daemon yields empty (state merges keep prior values).
    async fn ensure_running(&self) -> Result<Running, AgentError> {
        let mut state = self.inner.state.lock().await;
        if let Some(running) = state.as_ref() {
            return Ok(Running {
                commands: running.commands.clone(),
            });
        }
        let running = self.spawn_locked().await?;
        let clone = Running {
            commands: running.commands.clone(),
        };
        *state = Some(running);
        Ok(clone)
    }

    /// The daemon actor's exit: drop the handle so the next call spawns a fresh
    /// daemon instead of sending into a channel nobody reads. Identity-checked —
    /// a respawn that raced this exit keeps its own handle. Without this, a
    /// crashed daemon left every later call to fail (or wait out `RPC_TIMEOUT`)
    /// until the server restarted, so a session could never resume.
    async fn forget_running(&self, commands: &mpsc::Sender<HostCommand>) {
        let mut state = self.inner.state.lock().await;
        let stale = state
            .as_ref()
            .is_some_and(|running| running.commands.same_channel(commands));
        if stale {
            *state = None;
        }
    }

    async fn spawn_locked(&self) -> Result<Running, AgentError> {
        let (program, args) = resolve_launch(&self.inner.config)?;
        let data_dir = &self.inner.config.data_dir;
        std::fs::create_dir_all(data_dir).map_err(AgentError::Spawn)?;
        let passphrase = load_or_create_passphrase(data_dir)?;
        self.remember_secret(&passphrase).await;

        let mut command = Command::new(&program);
        command.args(&args).env_clear();
        for name in &self.inner.config.inherit_environment {
            if let Ok(value) = std::env::var(name) {
                command.env(name, value);
            }
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                AgentError::NodeMissing
            } else {
                AgentError::Spawn(error)
            }
        })?;
        let stdout = child.stdout.take().ok_or(AgentError::MissingPipe)?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(drain_stderr(stderr));
        }

        let (commands_tx, commands_rx) = mpsc::channel(32);
        let events = self.inner.events.clone();
        let secrets = Arc::clone(&self.inner.secrets);
        let book = Arc::clone(&self.inner.book);
        let reverse = self
            .inner
            .reverse
            .try_lock()
            .ok()
            .and_then(|guard| guard.clone());
        let mcp_servers: Vec<AgentMcpServerInfo> = Vec::new();
        // Plan 109 R1/R3: the pump's settled snapshots omit the
        // environment keys (empty here; snapshot_events skips them) —
        // prompt/attach snapshots carry the list and the client's state
        // merge keeps it. No rpc on the not-yet-running channel.
        let slash_commands = Vec::new();
        let extensions = Vec::new();
        let skills = Vec::new();
        tokio::spawn(daemon_actor(
            child,
            stdout,
            commands_rx,
            self.clone(),
            commands_tx.clone(),
            events,
            secrets,
            book,
            reverse,
            mcp_servers,
            slash_commands,
            extensions,
            skills,
        ));

        let running = Running {
            commands: commands_tx,
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        let allow_list: Vec<Value> = self
            .inner
            .config
            .mcp_allow_list
            .iter()
            .map(AgentMcpAllowListEntry::to_json)
            .collect();
        running
            .commands
            .send(HostCommand::Rpc {
                method: "initialize".to_string(),
                params: json!({ "passphrase": passphrase, "mcpAllowList": allow_list }),
                reply: reply_tx,
            })
            .await
            .map_err(|_| AgentError::ServiceStopped)?;
        match timeout(INITIALIZE_TIMEOUT, reply_rx).await {
            Ok(Ok(Ok(_))) => {}
            Ok(Ok(Err(error))) => return Err(error),
            Ok(Err(_)) => return Err(AgentError::ChildExited),
            Err(_) => return Err(AgentError::Timeout),
        }
        // Apply registrations queued while the daemon was down, before any
        // later command runs (the actor processes the queue in order). A
        // dropped reply channel is fine: the queueing caller was already
        // answered, and daemon-side validation still gates every entry.
        let queued: Vec<HostCommand> = self
            .inner
            .pending_registrations
            .lock()
            .await
            .drain(..)
            .collect();
        for command in queued {
            let _ = running.commands.send(command).await;
        }
        Ok(running)
    }

    pub async fn shutdown(&self) {
        let mut state = self.inner.state.lock().await;
        if let Some(running) = state.take() {
            let _ = running.commands.send(HostCommand::Shutdown).await;
        }
    }
}

#[allow(clippy::too_many_arguments)] // pump wiring: all eight are distinct deps
async fn daemon_actor(
    mut child: Child,
    stdout: tokio::process::ChildStdout,
    mut commands: mpsc::Receiver<HostCommand>,
    host: AgentHost,
    commands_tx: mpsc::Sender<HostCommand>,
    events: broadcast::Sender<Arc<AgentServerMessage>>,
    secrets: Arc<Mutex<Vec<String>>>,
    book: Arc<Mutex<SessionBook>>,
    reverse: Option<ReverseRpcHandler>,
    mcp_servers: Vec<AgentMcpServerInfo>,
    slash_commands: Vec<AgentSlashCommand>,
    extensions: Vec<String>,
    skills: Vec<AgentSkillInfo>,
) {
    let Some(stdin) = child.stdin.take() else {
        let _ = child.kill().await;
        return;
    };
    // One writer task owns stdin so command frames and reverse-RPC responses
    // serialize without multi-branch borrows.
    let (out_tx, mut out_rx) = mpsc::channel::<Value>(64);
    let writer = tokio::spawn(async move {
        let mut stdin = stdin;
        while let Some(frame) = out_rx.recv().await {
            if write_frame(&mut stdin, &frame).await.is_err() {
                break;
            }
        }
    });
    let mut reader = BufReader::new(stdout);
    let mut pending: HashMap<u64, oneshot::Sender<Result<Value, AgentError>>> = HashMap::new();
    let next_id = AtomicU64::new(1);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(HostCommand::Rpc { method, params, reply }) => {
                        let id = next_id.fetch_add(1, Ordering::Relaxed);
                        pending.insert(id, reply);
                        let frame = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": method,
                            "params": params,
                        });
                        if out_tx.send(frame).await.is_err() {
                            fail_pending(&mut pending, AgentError::ChildExited);
                            break;
                        }
                    }
                    Some(HostCommand::Shutdown) | None => {
                        let frame = json!({
                            "jsonrpc": "2.0",
                            "id": next_id.fetch_add(1, Ordering::Relaxed),
                            "method": "shutdown",
                            "params": {},
                        });
                        let _ = out_tx.send(frame).await;
                        break;
                    }
                }
            }
            read = reader.read_until(b'\n', &mut buf) => {
                match read {
                    Ok(0) => {
                        fail_pending(&mut pending, AgentError::ChildExited);
                        break;
                    }
                    Ok(len) if len > AGENT_DAEMON_MAX_LINE_BYTES || buf.len() > AGENT_DAEMON_MAX_LINE_BYTES => {
                        // A single oversized frame must not kill the daemon
                        // connection (that strands every in-flight run):
                        // drop the frame, surface a diagnostic, keep reading.
                        buf.clear();
                        let _ = events.send(Arc::new(AgentServerMessage::Event {
                            session_id: String::new(),
                            event: AgentWireEvent::Error {
                                session_id: String::new(),
                                message: format!(
                                    "Clay dropped an oversized daemon frame ({} bytes > cap {}); the run continued.",
                                    len, AGENT_DAEMON_MAX_LINE_BYTES
                                ),
                            },
                        }));
                    }
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let secrets_now = secrets.lock().await.clone();
                        match route_daemon_line(line.trim(), &secrets_now) {
                            DaemonLine::Notification(message) => {
                                let settled_session = match &message {
                                    AgentServerMessage::Event {
                                        session_id,
                                        event:
                                            AgentWireEvent::Finished { .. }
                                            | AgentWireEvent::Error { .. },
                                        ..
                                    } => Some(session_id.clone()),
                                    _ => None,
                                };
                                let settled_snapshot = {
                                    let mut book_guard = book.lock().await;
                                    apply_book_event(&mut book_guard, &message);
                                    // Plan 109 R2: the run settled — refresh
                                    // the session's branch (a steer-free run
                                    // may still have checked out a branch
                                    // via tools) so the republish is fresh.
                                    let settled_root = settled_session
                                        .as_ref()
                                        .and_then(|session_id| {
                                            book_guard.session_root.get(session_id).cloned()
                                        });
                                    if let (Some(session_id), Some(root)) =
                                        (&settled_session, settled_root)
                                    {
                                        refresh_branch(
                                            &mut book_guard,
                                            &root,
                                            session_id,
                                            read_git_branch,
                                        );
                                    }
                                    // Server-authoritative reconciliation
                                    // (plan 109 I5): the settled run's
                                    // transcript republishes AFTER the
                                    // terminal event is forwarded below, so
                                    // the run pipeline has already closed and
                                    // the snapshot rebuilds the transcript
                                    // from the server list (usage row
                                    // included) instead of the run pipeline's
                                    // live-delta accumulation.
                                    settled_session.as_ref().map(|session_id| {
                                        book_snapshot(
                                            &book_guard,
                                            session_id,
                                            &mcp_servers,
                                            &slash_commands,
                                            &extensions,
                                            &skills,
                                        )
                                    })
                                };
                                let _ = events.send(Arc::new(message));
                                if let Some(snapshot) = settled_snapshot {
                                    let _ =
                                        events.send(Arc::new(AgentServerMessage::Snapshot(snapshot)));
                                }
                            }
                            DaemonLine::Response(id) => {
                                complete_pending(line.trim(), &mut pending, id, &secrets_now);
                            }
                            DaemonLine::ReverseRequest { id, method, params } => {
                                let out = out_tx.clone();
                                tokio::spawn(handle_reverse_request(
                                    reverse.clone(),
                                    id,
                                    method,
                                    params,
                                    out,
                                    Arc::clone(&secrets),
                                ));
                            }
                            DaemonLine::Ignore => {}
                        }
                    }
                    Err(_) => {
                        fail_pending(&mut pending, AgentError::ChildExited);
                        break;
                    }
                }
            }
        }
    }
    drop(out_tx);
    let _ = writer.await;
    let _ = child.kill().await;
    fail_pending(&mut pending, AgentError::ChildExited);
    // The daemon is gone: release the host's handle so the next call respawns
    // it (a resumed session keeps its id and root through the book).
    host.forget_running(&commands_tx).await;
}

enum DaemonLine {
    Ignore,
    Notification(AgentServerMessage),
    Response(u64),
    ReverseRequest {
        id: Value,
        method: String,
        params: Value,
    },
}

fn route_daemon_line(line: &str, secrets: &[String]) -> DaemonLine {
    if line.is_empty() {
        return DaemonLine::Ignore;
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return DaemonLine::Ignore;
    };
    match value.get("method").and_then(Value::as_str) {
        Some("event") => match map_event(value.get("params").unwrap_or(&Value::Null), secrets) {
            Some(message) => DaemonLine::Notification(message),
            None => DaemonLine::Ignore,
        },
        Some(method) => DaemonLine::ReverseRequest {
            id: value.get("id").cloned().unwrap_or(Value::Null),
            method: method.to_string(),
            params: value.get("params").cloned().unwrap_or(Value::Null),
        },
        None => match value.get("id").and_then(Value::as_u64) {
            Some(id) => DaemonLine::Response(id),
            None => DaemonLine::Ignore,
        },
    }
}

fn complete_pending(
    line: &str,
    pending: &mut HashMap<u64, oneshot::Sender<Result<Value, AgentError>>>,
    id: u64,
    secrets: &[String],
) {
    let Some(reply) = pending.remove(&id) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        let _ = reply.send(Err(AgentError::Rpc("invalid daemon response".to_string())));
        return;
    };
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("clay-agent error");
        let _ = reply.send(Err(AgentError::Rpc(redact_text(message, secrets))));
        return;
    }
    let _ = reply.send(Ok(value.get("result").cloned().unwrap_or(Value::Null)));
}

async fn handle_reverse_request(
    reverse: Option<ReverseRpcHandler>,
    id: Value,
    method: String,
    params: Value,
    out: mpsc::Sender<Value>,
    secrets: Arc<Mutex<Vec<String>>>,
) {
    let result = match reverse {
        Some(handler) => handler(method, params).await,
        None => Err("no reverse-RPC handler installed".to_string()),
    };
    let secrets = secrets.lock().await.clone();
    let frame = match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(message) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": redact_text(&message, &secrets) },
        }),
    };
    let _ = out.send(frame).await;
}

async fn write_frame(stdin: &mut ChildStdin, value: &Value) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.len() > AGENT_DAEMON_MAX_LINE_BYTES {
        return Err(io::Error::other("clay-agent frame too large"));
    }
    bytes.push(b'\n');
    stdin.write_all(&bytes).await?;
    stdin.flush().await
}

async fn drain_stderr(stderr: tokio::process::ChildStderr) {
    let mut reader = BufReader::new(stderr);
    let mut buf = Vec::new();
    while reader
        .read_until(b'\n', &mut buf)
        .await
        .ok()
        .is_some_and(|n| n > 0)
    {
        // Passthrough: daemon-side diagnostics belong in the server log
        // (draining silently hid boot/registration failures).
        eprint!("[daemon] {}", String::from_utf8_lossy(&buf));
        buf.clear();
    }
}

fn fail_pending(
    pending: &mut HashMap<u64, oneshot::Sender<Result<Value, AgentError>>>,
    error: AgentError,
) {
    for (_, reply) in pending.drain() {
        let _ = reply.send(Err(error_clone_kind(&error)));
    }
}

fn error_clone_kind(error: &AgentError) -> AgentError {
    match error {
        AgentError::NodeMissing => AgentError::NodeMissing,
        AgentError::ScriptMissing => AgentError::ScriptMissing,
        AgentError::MissingPipe => AgentError::MissingPipe,
        AgentError::FrameTooLarge { len } => AgentError::FrameTooLarge { len: *len },
        AgentError::Timeout => AgentError::Timeout,
        AgentError::ChildExited => AgentError::ChildExited,
        AgentError::ServiceStopped => AgentError::ServiceStopped,
        AgentError::Rpc(message) => AgentError::Rpc(message.clone()),
        AgentError::Spawn(_) => AgentError::ChildExited,
    }
}

fn resolve_launch(config: &AgentHostConfig) -> Result<(PathBuf, Vec<String>), AgentError> {
    if !config.program.as_os_str().is_empty() {
        if !config.program.is_file() {
            return Err(AgentError::NodeMissing);
        }
        return Ok((config.program.clone(), config.args.clone()));
    }
    let node = resolve_node()?;
    let script = resolve_script()?;
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--data-dir".to_string(),
        config.data_dir.to_string_lossy().into_owned(),
    ];
    args.extend(config.args.clone());
    if std::env::var_os("CLAY_AGENT_MOCK").is_some() {
        args.push("--mock".to_string());
    }
    Ok((node, args))
}

fn resolve_node() -> Result<PathBuf, AgentError> {
    if let Some(value) = std::env::var_os("CLAY_NODE") {
        let path = PathBuf::from(value);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(AgentError::NodeMissing)
        };
    }
    let path_var = std::env::var_os("PATH").ok_or(AgentError::NodeMissing)?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("node");
        if candidate.is_file() {
            return std::fs::canonicalize(candidate).map_err(|_| AgentError::NodeMissing);
        }
    }
    Err(AgentError::NodeMissing)
}

fn resolve_script() -> Result<PathBuf, AgentError> {
    if let Some(value) = std::env::var_os("CLAY_AGENT_MAIN") {
        let path = PathBuf::from(value);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(AgentError::ScriptMissing)
        };
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let next_to_exe = parent.join("clay-agent/dist/main.js");
        if next_to_exe.is_file() {
            return Ok(next_to_exe);
        }
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("clay-agent/dist/main.js");
    if repo.is_file() {
        Ok(repo)
    } else {
        Err(AgentError::ScriptMissing)
    }
}

fn load_or_create_passphrase(data_dir: &Path) -> Result<String, AgentError> {
    let path = data_dir.join("vault.passphrase");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    let passphrase = random_passphrase()?;
    write_passphrase(&path, &passphrase)?;
    Ok(passphrase)
}

fn write_passphrase(path: &Path, passphrase: &str) -> Result<(), AgentError> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(AgentError::Spawn)?;
    use std::io::Write;
    file.write_all(passphrase.as_bytes())
        .map_err(AgentError::Spawn)?;
    Ok(())
}

fn random_passphrase() -> Result<String, AgentError> {
    let mut bytes = [0u8; 32];
    #[cfg(unix)]
    {
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut bytes))
            .map_err(AgentError::Spawn)?;
    }
    #[cfg(not(unix))]
    {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(1);
        for (index, slot) in bytes.iter_mut().enumerate() {
            *slot = ((nanos >> ((index % 16) * 8)) as u8).wrapping_add(index as u8);
        }
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn json_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn json_string(value: &Value, path: &[&str]) -> String {
    let mut current = value;
    for key in path {
        current = match current.get(key) {
            Some(next) => next,
            None => return String::new(),
        };
    }
    current.as_str().unwrap_or("").to_string()
}

fn diagnostic(code: &str, message: &str) -> AgentServerMessage {
    AgentServerMessage::Diagnostic {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn agent_rpc(code: &str, result: &Value) -> AgentServerMessage {
    AgentServerMessage::AgentRpc {
        code: code.to_string(),
        result_json: result.to_string(),
    }
}

fn error_code(error: &AgentError) -> &'static str {
    match error {
        AgentError::NodeMissing => "agent.node_missing",
        AgentError::ScriptMissing => "agent.script_missing",
        AgentError::Timeout => "agent.timeout",
        AgentError::FrameTooLarge { .. } => "agent.frame_too_large",
        AgentError::ChildExited => "agent.exited",
        _ => "agent.error",
    }
}

fn redact_text(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        if secret.len() >= 8 {
            out = out.replace(secret, "[redacted]");
        }
    }
    out
}

#[cfg(test)]
mod tab_workspace_tests {
    use super::mcp::parse_environment;
    use super::mcp::parse_models;
    use super::*;
    use crate::protocol::AgentTranscriptFileOp;

    // Plan 109 R2: a counting reader exposes cache hits — same generation
    // must reuse the cached branch without re-reading.
    static BRANCH_READS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn counting_reader(root: &Path) -> Option<String> {
        BRANCH_READS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        read_git_branch(root)
    }

    static REPO_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// Plan 119 SC-6: a session's MCP allow-list comes from *that session's*
    /// workspace root, merged with the agent's own user file — never from a
    /// launch root the host was configured with, and never a remembered list
    /// from another session.
    #[tokio::test]
    async fn mcp_allow_list_follows_the_session_workspace_root() {
        let base = unique_temp("mcp-allow-roots");
        let config_root = base.join("config");
        std::fs::create_dir_all(config_root.join("agents").join("coding-agent")).unwrap();
        std::fs::write(
            config_root
                .join("agents")
                .join("coding-agent")
                .join("mcp.json"),
            r#"{"servers":{"user-server":{"command":"/bin/true"}}}"#,
        )
        .unwrap();
        let root_a = base.join("ws-a");
        let root_b = base.join("ws-b");
        for (dir, server) in [(&root_a, "a-server"), (&root_b, "b-server")] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(
                dir.join(".mcp.json"),
                format!(r#"{{"mcpServers":{{"{server}":{{"command":"/bin/true"}}}}}}"#),
            )
            .unwrap();
        }

        let host = AgentHost::inert();
        host.set_agent_roots(Some(config_root));
        let server_ids = |params: &serde_json::Map<String, Value>| {
            let mut ids: Vec<String> = params
                .get("mcpAllowList")
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| entry.get("serverId").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            ids.sort();
            ids
        };

        let a = host
            .agent_session_params(Some("coding-agent"), Some(root_a.to_str().unwrap()))
            .await;
        assert_eq!(server_ids(&a), vec!["a-server", "user-server"]);
        // The second workspace gets *its* list: no cross-session memory.
        let b = host
            .agent_session_params(Some("coding-agent"), Some(root_b.to_str().unwrap()))
            .await;
        assert_eq!(server_ids(&b), vec!["b-server", "user-server"]);
        // A session with no agent type still reads the session's folder (the
        // daemon's spawn-time launch list must never bind it); its user file
        // is the shipped default agent's, exactly as `build_mcp_allow_list`
        // resolves an absent agent type.
        let default_agent = host
            .agent_session_params(None, Some(root_b.to_str().unwrap()))
            .await;
        assert_eq!(server_ids(&default_agent), vec!["b-server", "user-server"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    fn unique_temp(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            REPO_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ))
    }

    fn git_repo_with_branch(name: &str) -> std::path::PathBuf {
        let dir = unique_temp("clay-branch");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(
            dir.join(".git").join("HEAD"),
            format!("ref: refs/heads/{name}\n"),
        )
        .unwrap();
        dir
    }

    #[test]
    fn read_git_branch_parses_head_ref_worktree_and_detached() {
        // Plan 109 R2: direct `.git` reads only — no subprocess.
        let repo = git_repo_with_branch("plan-109");
        assert_eq!(read_git_branch(&repo).as_deref(), Some("plan-109"));
        // Worktree pointer file: `.git` is `gitdir: <path>`; the target
        // dir carries its own HEAD (git writes the same ref there).
        let worktree = unique_temp("clay-wt");
        std::fs::create_dir_all(&worktree).unwrap();
        let wt_git = repo.join(".git").join("worktrees").join("wt");
        std::fs::create_dir_all(&wt_git).unwrap();
        std::fs::write(wt_git.join("HEAD"), "ref: refs/heads/plan-109\n").unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}/.git/worktrees/wt\n", repo.display()),
        )
        .unwrap();
        assert_eq!(read_git_branch(&worktree).as_deref(), Some("plan-109"));
        // Detached HEAD: short sha.
        std::fs::write(
            repo.join(".git").join("HEAD"),
            "3f9c2ab7719e4c0dab16e7f30b12c58d4a9e21fc\n",
        )
        .unwrap();
        assert_eq!(read_git_branch(&repo).as_deref(), Some("3f9c2ab"));
        // Not a repo: None (status row renders `—`).
        let bare = unique_temp("clay-norepo");
        std::fs::create_dir_all(&bare).unwrap();
        assert_eq!(read_git_branch(&bare), None);
    }

    #[test]
    fn refresh_branch_caches_within_generation_and_rereads_after() {
        // Plan 109 R2: cache-hit proof — two looks in one run generation
        // read the repo once; run completion bumps the generation and the
        // next look re-reads (fresh branch picked up).
        let repo = git_repo_with_branch("main");
        let mut book = SessionBook::default();
        refresh_branch(&mut book, repo.to_str().unwrap(), "s1", counting_reader);
        let first = BRANCH_READS.load(std::sync::atomic::Ordering::SeqCst);
        refresh_branch(&mut book, repo.to_str().unwrap(), "s1", counting_reader);
        assert_eq!(
            BRANCH_READS.load(std::sync::atomic::Ordering::SeqCst),
            first,
            "same generation must hit the cache"
        );
        assert_eq!(book.branches.get("s1").map(String::as_str), Some("main"));
        // Run completion: the terminal-event bump invalidates the cache.
        apply_book_event(
            &mut book,
            &AgentServerMessage::Event {
                session_id: "s1".into(),
                event: AgentWireEvent::Finished {
                    session_id: "s1".into(),
                    run_id: "r1".into(),
                    context_tokens: None,
                    usage: String::new(),
                },
            },
        );
        std::fs::write(repo.join(".git").join("HEAD"), "ref: refs/heads/topic\n").unwrap();
        refresh_branch(&mut book, repo.to_str().unwrap(), "s1", counting_reader);
        assert!(
            BRANCH_READS.load(std::sync::atomic::Ordering::SeqCst) > first,
            "new generation must re-read"
        );
        assert_eq!(book.branches.get("s1").map(String::as_str), Some("topic"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tab_state_snapshot_starts_the_session_and_carries_branch_and_environment() {
        use std::os::unix::fs::PermissionsExt;

        // Plan 117 follow-up: a fresh coding-agent pane used to get NO
        // snapshot at all (nothing emits one before the first prompt), so
        // the status row showed `git —` and the skills/MCP cards were empty.
        let script_dir = unique_temp("clay-mock-tabstate");
        std::fs::create_dir_all(&script_dir).unwrap();
        let program = script_dir.join("mock-agent");
        std::fs::write(
            &program,
            r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"ok":True}}), flush=True)
    elif method == "session.new":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":"s1","profile":params.get("profile"),"provider":params.get("provider"),"model":params.get("model")}}), flush=True)
    elif method == "agentProfile.list":
        # The daemon that can run the coding surface has its profile
        # registered (a package load entry declares it); the mount only
        # adopts a profile the daemon actually lists.
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"profiles":[
            {"name":"coding","description":"Coding agent"},
            {"name":"Chat","description":""}]}}), flush=True)
    elif method == "environment.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{
            "commands":[{"name":"/compact","description":"compact"}],
            "extensions":["@arnilo/prism-memory/graft"],
            "skills":[{"name":"repo-skill","description":"from .agents/skills"}],
            "mcpServers":[{"serverId":"graft","connected":True,"tools":6}]}}), flush=True)
    else:
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&program, perms).unwrap();

        let repo = git_repo_with_branch("feature/pane-open");
        let data_dir = unique_temp("clay-mock-tabstate-data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let host = AgentHost::new(AgentHostConfig {
            program,
            args: Vec::new(),
            data_dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        });
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, repo.to_str().unwrap().to_string());
        host.set_tab_registry(Arc::clone(&registry));
        {
            let mut book = host.inner.book.lock().await;
            book.provider = "mock".into();
            book.model = "demo".into();
        }

        let snapshot = host.tab_state_snapshot(1).await;
        assert_eq!(
            snapshot.session_id, "s1",
            "the pane mount starts the session"
        );
        // The surface's own profile, not the daemon-level Chat default: a
        // pane restored or reached through the view switcher never dispatched
        // `coding-agent.profile`, and a Chat session would have neither the
        // coding tools nor the MCP servers this snapshot carries.
        assert_eq!(
            snapshot.profile, "coding",
            "the pane mount selects the coding surface profile"
        );
        assert_eq!(snapshot.branch, "feature/pane-open");
        assert_eq!(
            snapshot.mcp_servers,
            vec![AgentMcpServerInfo {
                server_id: "graft".into(),
                connected: true,
                tools: 6,
                error: String::new(),
            }],
            "the MCP card data rides the mount snapshot"
        );
        assert_eq!(snapshot.skills.len(), 1);
        assert_eq!(snapshot.skills[0].name, "repo-skill");
        assert_eq!(snapshot.commands.len(), 1);

        // A deliberate profile choice is never overwritten by a later mount.
        {
            let mut book = host.inner.book.lock().await;
            book.profile = "custom".into();
        }
        let again = host.tab_state_snapshot(1).await;
        assert_eq!(again.profile, "custom");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tab_state_snapshot_skips_a_profile_the_daemon_does_not_have() {
        use std::os::unix::fs::PermissionsExt;

        // The mount guesses the coding surface's profile so a pane shown
        // without a launch still runs the coding tools. That guess must be
        // checked against the daemon's own profile list: a profile no package
        // registered (the package never loaded, a store without it) would
        // fail `session.new` with `Unknown agent: coding`, and the tab would
        // never bind a session — a dead pane with no error. With the profile
        // absent the book stays unselected and the daemon's built-in `Chat`
        // default serves the tab instead.
        let script_dir = unique_temp("clay-mock-tabstate-no-coding");
        std::fs::create_dir_all(&script_dir).unwrap();
        let program = script_dir.join("mock-agent");
        std::fs::write(
            &program,
            r#"#!/usr/bin/env python3
import json, os, sys
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "agentProfile.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"profiles":[
            {"name":"Chat","description":""}]}}), flush=True)
    elif method == "session.new":
        with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "requested-profile.txt"), "w") as handle:
            handle.write(str(params.get("profile")))
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":"s1","profile":params.get("profile"),"provider":params.get("provider"),"model":params.get("model")}}), flush=True)
    else:
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&program, perms).unwrap();

        let repo = git_repo_with_branch("feature/no-coding-profile");
        let data_dir = unique_temp("clay-mock-tabstate-no-coding-data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let host = AgentHost::new(AgentHostConfig {
            program,
            args: Vec::new(),
            data_dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        });
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, repo.to_str().unwrap().to_string());
        host.set_tab_registry(Arc::clone(&registry));
        {
            let mut book = host.inner.book.lock().await;
            book.provider = "mock".into();
            book.model = "demo".into();
        }

        let snapshot = host.tab_state_snapshot(1).await;
        assert_eq!(
            snapshot.session_id, "s1",
            "the tab still binds a session when the guessed profile is absent"
        );
        assert_eq!(
            std::fs::read_to_string(script_dir.join("requested-profile.txt")).unwrap(),
            "Chat",
            "the daemon's built-in default is what the session is created with"
        );
        assert!(
            host.inner.book.lock().await.profile.is_empty(),
            "an unavailable profile is never recorded as the book's selection"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn book_selection_broadcast_keeps_the_tab_session() {
        use std::os::unix::fs::PermissionsExt;

        // Plan 117 follow-up: picking an OM worker in the Memory tab (and a
        // model in the panel header, and launching the surface) broadcast the
        // session-less `unconfigured_snapshot`, whose empty session_id +
        // transcript + branch overwrote the panel's live session at the next
        // STATE merge — every right-hand tab then read "No active agent
        // session." The broadcast must carry the tab's own state.
        let script_dir = unique_temp("clay-mock-book-broadcast");
        std::fs::create_dir_all(&script_dir).unwrap();
        let program = script_dir.join("mock-agent");
        std::fs::write(
            &program,
            r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "session.new":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":"s1","profile":params.get("profile"),"provider":params.get("provider"),"model":params.get("model")}}), flush=True)
    else:
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&program, perms).unwrap();

        let repo = git_repo_with_branch("feature/om-workers");
        let data_dir = unique_temp("clay-mock-book-broadcast-data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let host = AgentHost::new(AgentHostConfig {
            program,
            args: Vec::new(),
            data_dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        });
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, repo.to_str().unwrap().to_string());
        host.set_tab_registry(Arc::clone(&registry));
        {
            let mut book = host.inner.book.lock().await;
            book.provider = "mock".into();
            book.model = "demo".into();
        }
        // The pane mount establishes the session, then a user row lands in the
        // transcript so a wipe is observable in the broadcast.
        assert_eq!(host.tab_state_snapshot(1).await.session_id, "s1");
        {
            let mut book = host.inner.book.lock().await;
            book.transcripts
                .entry("s1".to_string())
                .or_default()
                .push(AgentTranscriptEntry::new(
                    AgentTranscriptKind::User,
                    "hello",
                ));
        }

        let mut events = host.subscribe();
        host.select_worker(
            AgentOmWorkerKind::Observation,
            "model:mock/demo",
            Some("s1".to_string()),
            Some(1),
        )
        .await;

        let mut broadcast = None;
        while let Ok(event) = events.try_recv() {
            if let AgentServerMessage::Snapshot(snapshot) = &*event {
                broadcast = Some(snapshot.clone());
                break;
            }
        }
        let broadcast = broadcast.expect("a book selection must publish STATE");
        assert_eq!(
            broadcast.session_id, "s1",
            "the selection broadcast must not clear the live session"
        );
        assert_eq!(
            broadcast.entries.len(),
            1,
            "the transcript must survive the selection broadcast"
        );
        assert_eq!(broadcast.branch, "feature/om-workers");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn resumed_tab_keeps_its_session_on_the_next_prompt() {
        use std::os::unix::fs::PermissionsExt;

        // Plan 117 follow-up: `resume_tab` recorded the tab's session but not
        // its workspace root, and the tab-lookup pruned a tab whose root did
        // not match — so the prompt after a resume created a brand-new session
        // and abandoned the one the user had just opened (transcript stayed,
        // then the next turn answered into a different session). Plan 119 SC-6
        // keys it by `(agent, root)`: the resume records the root too, so agent
        // tool calls resolve against it (`session_workspace_root`).
        let script_dir = unique_temp("clay-mock-resume-root");
        std::fs::create_dir_all(&script_dir).unwrap();
        let program = script_dir.join("mock-agent");
        std::fs::write(
            &program,
            r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "session.load":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":params.get("sessionId"),"profile":"coding","metadata":{"provider":"mock","model":"demo"},"entries":[{"id":"e1","sessionId":params.get("sessionId"),"timestamp":"2026-09-03T00:00:00.000Z","kind":"message","message":{"role":"user","content":[{"type":"text","text":"old turn"}]}}]}}), flush=True)
    elif method == "session.new":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":"brand-new","profile":params.get("profile"),"provider":params.get("provider"),"model":params.get("model")}}), flush=True)
    else:
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&program, perms).unwrap();

        let repo = git_repo_with_branch("feature/resume");
        let data_dir = unique_temp("clay-mock-resume-root-data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let host = AgentHost::new(AgentHostConfig {
            program,
            args: Vec::new(),
            data_dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        });
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, repo.to_str().unwrap().to_string());
        host.set_tab_registry(Arc::clone(&registry));
        {
            let mut book = host.inner.book.lock().await;
            book.provider = "mock".into();
            book.model = "demo".into();
        }

        let loaded = host.resume_tab(1, "old-session", None).await;
        let AgentServerMessage::Snapshot(snapshot) = &loaded else {
            panic!("resume must answer with a snapshot");
        };
        assert_eq!(snapshot.session_id, "old-session");
        assert_eq!(snapshot.entries.len(), 1, "the old transcript loads");

        let bound = {
            let book = host.inner.book.lock().await;
            book.session_for_workspace(None, repo.to_str())
        };
        assert_eq!(
            bound.as_deref(),
            Some("old-session"),
            "the resumed session survives the next tab lookup"
        );
        assert_eq!(
            host.ensure_tab_session(1).await.as_deref(),
            Some("old-session"),
            "a prompt after the resume continues the resumed session"
        );
        assert_eq!(
            host.session_workspace_root("old-session").await.as_deref(),
            Some(repo.to_str().unwrap()),
            "the resumed session's tools resolve against the tab's workspace"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tab_state_snapshot_reports_the_branch_without_a_configured_provider() {
        use std::os::unix::fs::PermissionsExt;

        // No provider/model in the book: no session can be created, but the
        // status row must still be honest about the workspace's branch.
        let script_dir = unique_temp("clay-mock-tabstate-bare");
        std::fs::create_dir_all(&script_dir).unwrap();
        let program = script_dir.join("mock-agent");
        std::fs::write(
            &program,
            r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&program, perms).unwrap();

        let repo = git_repo_with_branch("feature/no-provider");
        let data_dir = unique_temp("clay-mock-tabstate-bare-data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let host = AgentHost::new(AgentHostConfig {
            program,
            args: Vec::new(),
            data_dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        });
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, repo.to_str().unwrap().to_string());
        host.set_tab_registry(Arc::clone(&registry));

        let snapshot = host.tab_state_snapshot(1).await;
        assert!(
            snapshot.session_id.is_empty(),
            "no session without a selection"
        );
        assert_eq!(snapshot.branch, "feature/no-provider");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fresh_tab_session_records_the_workspace_branch() {
        use std::os::unix::fs::PermissionsExt;

        let script_dir = unique_temp("clay-mock-daemon");
        std::fs::create_dir_all(&script_dir).unwrap();
        let program = script_dir.join("mock-agent");
        std::fs::write(
            &program,
            r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"ok":True}}), flush=True)
    elif method == "session.new":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":"s1","profile":params.get("profile"),"provider":params.get("provider"),"model":params.get("model")}}), flush=True)
    else:
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&program, perms).unwrap();

        let repo = git_repo_with_branch("feature/branch-fix");
        let data_dir = unique_temp("clay-mock-data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let host = AgentHost::new(AgentHostConfig {
            program,
            args: Vec::new(),
            data_dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        });

        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, repo.to_str().unwrap().to_string());
        host.set_tab_registry(Arc::clone(&registry));
        {
            let mut book = host.inner.book.lock().await;
            book.provider = "mock".into();
            book.model = "demo".into();
        }

        let session_id = host
            .ensure_tab_session(1)
            .await
            .expect("fresh session is created");
        {
            let book = host.inner.book.lock().await;
            assert_eq!(
                book.branches.get(&session_id).map(String::as_str),
                Some("feature/branch-fix"),
                "the creation path must record the workspace branch"
            );
        }
        let snapshot = host.snapshot_for(&session_id).await;
        assert_eq!(snapshot.branch, "feature/branch-fix");
    }

    #[test]
    fn parse_environment_bounds_commands_and_extensions() {
        // Plan 109 R1/R3: bounded completion data — count and field
        // lengths clamped server-side.
        let commands: Vec<Value> = (0..80)
            .map(|index| {
                serde_json::json!({
                    "name": format!("/cmd-{index}"),
                    "description": "d".repeat(300),
                })
            })
            .collect();
        let parsed = parse_environment(&serde_json::json!({
            "commands": commands,
            "extensions": ["wiki", "graft", "", 42],
            "skills": [
                { "name": "repo-skill", "description": "d".repeat(300) },
                { "description": "no name" },
            ],
        }));
        assert_eq!(parsed.commands.len(), 64);
        assert!(parsed.commands[0].description.len() <= 96);
        assert_eq!(
            parsed.extensions,
            vec!["wiki".to_string(), "graft".to_string()]
        );
        assert_eq!(
            parsed.skills,
            vec![AgentSkillInfo {
                name: "repo-skill".to_string(),
                description: "d".repeat(96),
            }]
        );
        assert_eq!(
            parse_environment(&serde_json::json!({})),
            DaemonEnvironment::default()
        );
    }

    fn book_with_session(agent: &str, root: &str) -> SessionBook {
        let mut book = SessionBook::default();
        book.bind_workspace_session(Some(agent), Some(root), "session-1");
        book
    }

    #[test]
    fn parse_models_parses_declared_thinking_levels() {
        // Plan 109 I4: the daemon's model.list carries thinkingLevels per
        // model (Prism thinkingLevelsForModel); the inventory parses them
        // so snapshots can carry effortLevels.
        let parsed = parse_models(&serde_json::json!({
            "models": [
                {
                    "provider": "mock",
                    "model": "reasoner",
                    "displayName": "Reasoner",
                    "thinkingLevels": ["low", "medium", "high"]
                },
                { "provider": "mock", "model": "plain", "displayName": "Plain" }
            ]
        }));
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0].thinking_levels,
            vec!["low".to_string(), "medium".to_string(), "high".to_string()]
        );
        assert!(parsed[1].thinking_levels.is_empty());
    }

    #[test]
    fn map_event_builds_bounded_redacted_tool_digests() {
        // Plan 109 I5: args summaries, output excerpts, and skill names are
        // extracted server-side from the daemon's tool events, redacted
        // against the session secrets, and bounded before the wire.
        let secrets = vec!["supersecret01".to_string()];
        let started = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "tool_execution_started",
                    "runId": "run-1",
                    "call": {
                        "id": "c1",
                        "name": "read",
                        "arguments": { "path": "supersecret01.txt" }
                    }
                }
            }),
            &secrets,
        )
        .expect("started event maps");
        let AgentServerMessage::Event { event, .. } = started else {
            panic!("event expected");
        };
        let AgentWireEvent::Tool {
            args_digest,
            skill_name,
            file,
            ..
        } = event
        else {
            panic!("tool event expected");
        };
        assert_eq!(args_digest.as_deref(), Some(r#"{"path":"[redacted].txt"}"#));
        assert_eq!(skill_name, None);
        // Plan 118 task 36: the file record is the redacted path + the verb.
        let file = file.expect("read names a file");
        assert_eq!(file.path, "[redacted].txt");
        assert_eq!(file.op, AgentTranscriptFileOp::Read);

        let skill = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "tool_execution_started",
                    "runId": "run-1",
                    "call": {
                        "id": "c2",
                        "name": "load_skill",
                        "arguments": { "name": "rust-review" }
                    }
                }
            }),
            &secrets,
        )
        .expect("skill event maps");
        let AgentServerMessage::Event { event, .. } = skill else {
            panic!("event expected");
        };
        let AgentWireEvent::Tool {
            args_digest,
            skill_name,
            file,
            ..
        } = event
        else {
            panic!("tool event expected");
        };
        assert_eq!(skill_name.as_deref(), Some("rust-review"));
        assert_eq!(args_digest.as_deref(), Some(r#"{"name":"rust-review"}"#));
        // A tool that names no file records none — the Files tab is session
        // history, never a guess.
        assert!(file.is_none());

        // Every file verb maps; search/shell/git name no single file.
        let write = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "tool_execution_started",
                    "runId": "run-1",
                    "call": {
                        "id": "c3",
                        "name": "write",
                        "arguments": { "path": "plans/118.md", "content": "x" }
                    }
                }
            }),
            &secrets,
        )
        .expect("write event maps");
        let AgentServerMessage::Event { event, .. } = write else {
            panic!("event expected");
        };
        let AgentWireEvent::Tool { file, .. } = event else {
            panic!("tool event expected");
        };
        let file = file.expect("write names a file");
        assert_eq!(file.path, "plans/118.md");
        assert_eq!(file.op, AgentTranscriptFileOp::Write);

        let search = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "tool_execution_started",
                    "runId": "run-1",
                    "call": {
                        "id": "c4",
                        "name": "repo_search",
                        "arguments": { "query": "sessionFile" }
                    }
                }
            }),
            &secrets,
        )
        .expect("search event maps");
        let AgentServerMessage::Event { event, .. } = search else {
            panic!("event expected");
        };
        let AgentWireEvent::Tool { file, .. } = event else {
            panic!("tool event expected");
        };
        assert!(file.is_none());

        let finished = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "tool_execution_finished",
                    "runId": "run-1",
                    "result": {
                        "toolCallId": "c1",
                        "name": "read",
                        "content": [
                            { "type": "text", "text": "fn main() { supersecret01 }" }
                        ]
                    }
                }
            }),
            &secrets,
        )
        .expect("finished event maps");
        let AgentServerMessage::Event { event, .. } = finished else {
            panic!("event expected");
        };
        let AgentWireEvent::Tool { output_digest, .. } = event else {
            panic!("tool event expected");
        };
        assert_eq!(output_digest.as_deref(), Some("fn main() { [redacted] }"));
    }

    #[test]
    fn provider_turn_finished_books_context_occupancy() {
        // Plan 117 token meter: the last provider round's prompt size
        // (input + cache reads + writes) books the occupancy; a turn with
        // unreported usage maps to the catch-all (no booking).
        let secrets = Vec::new();
        let mapped = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "provider_turn_finished",
                    "runId": "run-1",
                    "turn": 2,
                    "usage": {
                        "inputTokens": 10_000,
                        "outputTokens": 500,
                        "cacheReadTokens": 2_000,
                        "cacheWriteTokens": 300
                    }
                }
            }),
            &secrets,
        )
        .expect("turn event maps");
        let AgentServerMessage::Event { event, .. } = mapped else {
            panic!("event expected");
        };
        let AgentWireEvent::ContextTokens { tokens, .. } = event else {
            panic!("context tokens event expected");
        };
        // Output tokens are NOT occupancy; prompt fields sum.
        assert_eq!(tokens, 12_300);

        // Booking: the book keeps the latest turn's occupancy.
        let mut book = SessionBook::default();
        apply_book_event(
            &mut book,
            &AgentServerMessage::Event {
                session_id: "s1".into(),
                event: AgentWireEvent::ContextTokens {
                    session_id: "s1".into(),
                    run_id: "run-1".into(),
                    tokens: 12_300,
                },
            },
        );
        assert_eq!(book.context_tokens.get("s1"), Some(&Some(12_300)));

        // Unreported usage: dropped (no occupancy to book, and unknown /
        // unsupported event types never fabricate a Started).
        let mapped = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": { "type": "provider_turn_finished", "runId": "run-1" }
            }),
            &secrets,
        );
        assert!(mapped.is_none(), "unreported usage must be dropped");
    }

    #[test]
    fn map_event_drops_unknown_event_types() {
        // Prism 0.7 telemetry/lifecycle types and arbitrary unknowns must
        // not reach the wire: a fabricated Started pins the run "streaming"
        // forever (same class as the old agent_suspended bug). `subagent_*`
        // events without a childId/delegationId stay dropped too (plan 122:
        // only fully-identified subagent lifecycle maps, onto Tool).
        let secrets = Vec::new();
        for event_type in [
            "attention_compiled",
            "subagent_started",
            "subagent_stopped",
            "delegation_started",
            "delegation_finished",
            "delegation_child_event",
            "not_a_real_event",
        ] {
            let mapped = map_event(
                &serde_json::json!({
                    "sessionId": "s1",
                    "event": { "type": event_type, "runId": "run-1" }
                }),
                &secrets,
            );
            assert!(mapped.is_none(), "{event_type} must be dropped, not mapped");
        }
    }

    #[test]
    fn map_event_maps_subagent_lifecycle_onto_tool_rows() {
        // Plan 122: `subagent_started`/`subagent_stopped` (the daemon's
        // supervisor lifecycle bridge) reuse the Tool wire event — name is
        // the child id, the delegation id pairs the rows, and the stopped
        // status rides the output digest. No new AG-UI event type.
        let secrets = vec!["s3cr3t".to_string()];
        let started = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "subagent_started",
                    "childId": "test",
                    "delegationId": "supervisor-1",
                    "depth": 1
                }
            }),
            &secrets,
        )
        .expect("identified subagent_started must map");
        match started {
            AgentServerMessage::Event {
                event:
                    AgentWireEvent::Tool {
                        phase,
                        name,
                        tool_call_id,
                        ..
                    },
                ..
            } => {
                assert_eq!(phase, AgentToolPhase::Started);
                assert_eq!(name, "test");
                assert_eq!(tool_call_id, "supervisor-1");
            }
            other => panic!("subagent_started must map to Tool, got {other:?}"),
        }
        let stopped = map_event(
            &serde_json::json!({
                "sessionId": "s1",
                "event": {
                    "type": "subagent_stopped",
                    "childId": "validation",
                    "delegationId": "supervisor-2",
                    "status": "failed"
                }
            }),
            &secrets,
        )
        .expect("identified subagent_stopped must map");
        match stopped {
            AgentServerMessage::Event {
                event:
                    AgentWireEvent::Tool {
                        phase,
                        name,
                        tool_call_id,
                        output_digest,
                        ..
                    },
                ..
            } => {
                assert_eq!(phase, AgentToolPhase::Finished);
                assert_eq!(name, "validation");
                assert_eq!(tool_call_id, "supervisor-2");
                assert_eq!(output_digest.as_deref(), Some("failed"));
            }
            other => panic!("subagent_stopped must map to Tool, got {other:?}"),
        }
    }

    #[test]
    fn session_kept_while_workspace_root_matches() {
        let book = book_with_session("", "/tmp/alpha");
        assert_eq!(
            book.session_for_workspace(None, Some("/tmp/alpha"))
                .as_deref(),
            Some("session-1")
        );
    }

    #[test]
    fn sibling_tabs_on_one_workspace_share_one_session() {
        // Plan 119 SC-6: the ownership key is (agent, root), not a tab — a
        // second tab on the same folder resolves the same session instead of
        // creating a second one, and a different agent or folder does not.
        let book = book_with_session("coding-agent", "/tmp/alpha");
        assert_eq!(
            book.session_for_workspace(Some("coding-agent"), Some("/tmp/alpha"))
                .as_deref(),
            Some("session-1")
        );
        assert_eq!(
            book.session_for_workspace(Some("coding-agent"), Some("/tmp/beta")),
            None
        );
        assert_eq!(
            book.session_for_workspace(Some("other-agent"), Some("/tmp/alpha")),
            None
        );
    }

    #[test]
    fn unbound_legacy_session_survives_missing_registry() {
        // Pre-I1 entries recorded no root: keep them stable when no
        // registry resolves (never surprise-rebind legacy sessions).
        let book = book_with_session("", "");
        assert_eq!(
            book.session_for_workspace(None, None).as_deref(),
            Some("session-1")
        );
        assert_eq!(
            book.session_for_workspace(None, Some("")).as_deref(),
            Some("session-1")
        );
    }

    #[test]
    fn bound_session_rebinds_when_registry_disappears() {
        // A root-bound session must never be adopted for a lookup the
        // registry cannot back: the other key resolves nothing (fresh
        // create) while the original binding stays intact and resumable.
        let book = book_with_session("", "/tmp/alpha");
        assert_eq!(book.session_for_workspace(None, None), None);
        assert_eq!(
            book.session_for_workspace(None, Some("/tmp/alpha"))
                .as_deref(),
            Some("session-1")
        );
    }

    #[test]
    fn workspace_change_resolves_a_new_key() {
        let book = book_with_session("", "/tmp/alpha");
        assert_eq!(book.session_for_workspace(None, Some("/tmp/beta")), None);
        // Next call is a fresh create; the old session stays resumable.
        assert!(book.transcripts.is_empty());
        assert_eq!(
            book.session_for_workspace(None, Some("/tmp/alpha"))
                .as_deref(),
            Some("session-1")
        );
    }

    /// Plan 119 SC-6: two tabs on one workspace resolve one session through
    /// the host, with no daemon involvement (the cached binding path).
    #[tokio::test]
    async fn sibling_tabs_resolve_one_session_through_the_host() {
        let host = AgentHost::inert();
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        {
            let mut registry = registry.lock().await;
            registry.create_tab(1, 10, "/tmp/alpha".to_string());
            registry.create_tab(2, 11, "/tmp/alpha".to_string());
        }
        host.set_tab_registry(registry);
        {
            let mut book = host.inner.book.lock().await;
            book.bind_workspace_session(None, Some("/tmp/alpha"), "session-1");
        }
        assert_eq!(
            host.ensure_tab_session(1).await.as_deref(),
            Some("session-1")
        );
        assert_eq!(
            host.ensure_tab_session(2).await.as_deref(),
            Some("session-1")
        );
    }

    /// Plan 119 SC-6: the session's recorded root is what agent tool calls
    /// resolve against — a session the host has never bound resolves to
    /// `None` (fail closed) instead of the launch root.
    #[tokio::test]
    async fn session_workspace_root_is_the_recorded_root_only() {
        let host = AgentHost::inert();
        {
            let mut book = host.inner.book.lock().await;
            book.bind_workspace_session(None, Some("/tmp/alpha"), "session-1");
            book.session_root
                .insert("session-1".to_string(), "/tmp/alpha".to_string());
            // A workspace-less session records an empty root: no fallback.
            book.session_root
                .insert("session-2".to_string(), String::new());
        }
        assert_eq!(
            host.session_workspace_root("session-1").await.as_deref(),
            Some("/tmp/alpha")
        );
        assert_eq!(host.session_workspace_root("session-2").await, None);
        assert_eq!(host.session_workspace_root("unknown").await, None);
    }

    #[tokio::test]
    async fn host_resolves_tab_root_from_registry_and_follows_rebind() {
        let host = AgentHost::inert();
        assert_eq!(host.tab_workspace_root(3).await, None);

        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, "/tmp/alpha".to_string());
        host.set_tab_registry(Arc::clone(&registry));
        assert_eq!(
            host.tab_workspace_root(1).await.as_deref(),
            Some("/tmp/alpha")
        );

        // In-tab workspace rebind (registry open_workspace) is visible.
        registry
            .lock()
            .await
            .open_workspace(1, 1, 20, "/tmp/beta".to_string());
        assert_eq!(
            host.tab_workspace_root(1).await.as_deref(),
            Some("/tmp/beta")
        );
    }
}

#[cfg(test)]
mod approval_tests {
    use super::book::BookSelection;
    use super::book::book_path;
    use super::book::parse_resumable_sessions;
    use super::book::persist_book;
    use super::book::remember_selection;
    use super::mcp::picker_items;
    use super::*;
    use crate::protocol::ApprovalRequestKind;

    #[tokio::test]
    async fn approval_resolve_round_trips_to_the_waiting_requester() {
        let host = AgentHost::inert();
        let _subscriber = host.subscribe();
        let waiter = {
            let host = host.clone();
            tokio::spawn(async move {
                host.request_user_approval_for(
                    ApprovalRequestKind::Mutation,
                    r#"{"kind":"write"}"#,
                    Duration::from_secs(5),
                )
                .await
            })
        };
        // Let the requester register before resolving.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            host.resolve_approval("approval-1", Ok(json!({ "allowed": true })))
                .await
        );
        let answer = waiter.await.expect("task joins");
        assert_eq!(answer.expect("allowed"), json!({ "allowed": true }));
        // One answer only: a second resolve for the same id is stale.
        assert!(
            !host
                .resolve_approval("approval-1", Ok(json!({ "allowed": false })))
                .await
        );
    }

    #[tokio::test]
    async fn approval_timeout_and_missing_consumer_deny_fail_closed() {
        let host = AgentHost::inert();
        // No subscriber: the request fails immediately (no consumer path).
        let denied = host
            .request_user_approval_for(
                ApprovalRequestKind::Mutation,
                r#"{"kind":"write"}"#,
                Duration::from_secs(5),
            )
            .await
            .expect_err("no consumer denies");
        assert!(denied.contains("consumer"));
        // With a subscriber but no answer: bounded wait denies.
        let _subscriber = host.subscribe();
        let denied = host
            .request_user_approval_for(
                ApprovalRequestKind::Mutation,
                r#"{"kind":"write"}"#,
                Duration::from_millis(20),
            )
            .await
            .expect_err("timeout denies");
        assert!(denied.contains("timed out"));
        // A resolution for the expired request is stale, not an error.
        assert!(
            !host
                .resolve_approval("approval-1", Ok(json!({ "allowed": true })))
                .await
        );
    }

    #[tokio::test]
    async fn approval_reject_reaches_the_daemon_as_a_denial() {
        let host = AgentHost::inert();
        let _subscriber = host.subscribe();
        let waiter = {
            let host = host.clone();
            tokio::spawn(async move {
                host.request_user_approval_for(
                    ApprovalRequestKind::AskDecision,
                    r#"{"question":"Which?"}"#,
                    Duration::from_secs(5),
                )
                .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            host.resolve_approval("approval-1", Err("denied by user".into()))
                .await
        );
        let answer = waiter.await.expect("task joins");
        assert_eq!(answer.expect_err("denied"), "denied by user");
    }

    #[tokio::test]
    async fn book_selection_survives_host_recreation() {
        let dir = std::env::temp_dir().join(format!("clay-book-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create data dir");
        let config = AgentHostConfig {
            program: std::path::PathBuf::new(),
            args: Vec::new(),
            data_dir: dir.clone(),
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        };
        let host = AgentHost::new(config.clone());
        host.select_picker(AgentPickerKind::Model, "model:ollama/glm-5.3", None)
            .await;
        host.select_picker(AgentPickerKind::Agent, "agent:coding", None)
            .await;
        drop(host);
        assert!(dir.join("book.json").is_file(), "book.json written");

        let restored = AgentHost::new(config);
        let book = restored.inner.book.lock().await;
        assert_eq!(book.provider, "ollama");
        assert_eq!(book.model, "glm-5.3");
        assert_eq!(book.profile, "coding");

        // Garbage book.json falls back to defaults without panicking.
        std::fs::write(dir.join("book.json"), "not json").expect("write garbage");
        let clean = load_persisted_book(&dir);
        assert_eq!(clean.provider, "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_book_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("clay-book-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create data dir");
        dir
    }

    #[test]
    fn v1_book_json_loads_as_the_fallback_entry() {
        let dir = temp_book_dir("v1");
        std::fs::write(
            dir.join("book.json"),
            r#"{"provider":"ollama","model":"glm-5.3","profile":"coding"}"#,
        )
        .expect("write v1 book");
        let book = load_persisted_book(&dir);
        assert_eq!(book.provider, "ollama");
        assert_eq!(book.model, "glm-5.3");
        assert_eq!(book.profile, "coding");
        assert!(book.workspaces.is_empty(), "v1 has no workspace entries");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v2_book_json_round_trips_workspaces() {
        let dir = temp_book_dir("v2");
        std::fs::write(
            dir.join("book.json"),
            r#"{
                "fallback": {"provider":"a","model":"m1","profile":"Chat"},
                "workspaces": {
                    "/tmp/alpha": {"provider":"b","model":"m2","profile":"coding"}
                }
            }"#,
        )
        .expect("write v2 book");
        let book = load_persisted_book(&dir);
        assert_eq!(book.provider, "a");
        let alpha = book.workspaces.get("/tmp/alpha").expect("workspace entry");
        assert_eq!(alpha.provider, "b");
        assert_eq!(alpha.model, "m2");
        // Re-persist and reload: shape and values survive a round trip.
        persist_book(&dir, &book);
        let reloaded = load_persisted_book(&dir);
        assert_eq!(
            reloaded
                .workspaces
                .get("/tmp/alpha")
                .map(|s| (s.provider.as_str(), s.model.as_str())),
            Some(("b", "m2"))
        );
        assert_eq!(reloaded.provider, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn per_agent_selections_resolve_and_remember_each_agent() {
        // Plan 118 task 35: each agent type keeps its own last-used selection
        // for a workspace, so switching an agent resets the model/effort
        // controls to *that* agent's defaults — while the root-keyed entry
        // stays the pre-agent read (migration) and the global trio the
        // fallback.
        let mut book = book_with_selection("/tmp/alpha", "root-agent-provider", "root-model");
        let configured = |_: &str| true;
        // No per-agent entry for a named agent: it does NOT inherit the root
        // entry's pick — it starts from the global fallback, so switching an
        // agent lands on that agent's own defaults.
        assert_eq!(
            selection_for(&book, Some("reviewer"), Some("/tmp/alpha"), &configured).model,
            "global-model"
        );
        // A per-agent pick wins for that agent only.
        let reviewer = BookSelection {
            profile: String::new(),
            provider: "reviewer-provider".to_string(),
            model: "reviewer-model".to_string(),
            om_observation: None,
            om_reflection: None,
        };
        remember_selection(&mut book, Some("reviewer"), Some("/tmp/alpha"), &reviewer);
        assert_eq!(
            selection_for(&book, Some("reviewer"), Some("/tmp/alpha"), &configured).model,
            "reviewer-model"
        );
        assert_eq!(
            selection_for(&book, Some("coding-agent"), Some("/tmp/alpha"), &configured).model,
            "global-model",
            "another agent keeps its own (empty) entry, not the reviewer's pick"
        );
        // A named agent's write does not touch the root map (the daemon's
        // default agent's own read path).
        assert_eq!(
            book.workspaces.get("/tmp/alpha").map(|s| s.model.as_str()),
            Some("root-model")
        );
        // The default agent (no name) still reads and writes the root map.
        let default = BookSelection {
            profile: String::new(),
            provider: "default-provider".to_string(),
            model: "default-model".to_string(),
            om_observation: None,
            om_reflection: None,
        };
        remember_selection(&mut book, None, Some("/tmp/alpha"), &default);
        assert_eq!(
            selection_for(&book, None, Some("/tmp/alpha"), &configured).model,
            "default-model"
        );
        // An agent-named write without a workspace records nothing to resolve.
        remember_selection(&mut book, Some("reviewer"), None, &reviewer);
        assert_eq!(book.agent_workspaces["reviewer"].len(), 1);
    }

    #[test]
    fn book_round_trips_per_agent_selections_and_reads_v2_books() {
        let dir = std::env::temp_dir().join(format!("clay-book-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut book = book_with_selection("/tmp/alpha", "b", "m2");
        let reviewer = BookSelection {
            profile: String::new(),
            provider: "p".to_string(),
            model: "m".to_string(),
            om_observation: None,
            om_reflection: None,
        };
        remember_selection(&mut book, Some("reviewer"), Some("/tmp/alpha"), &reviewer);
        persist_book(&dir, &book);
        let reloaded = load_persisted_book(&dir);
        let per_agent = reloaded
            .agent_workspaces
            .get("reviewer")
            .and_then(|roots| roots.get("/tmp/alpha"))
            .expect("per-agent entry survives the round trip");
        assert_eq!(per_agent.model, "m");
        // A book written before agent types existed has no `agents` map; the
        // root entry then serves every agent (migration read).
        std::fs::write(
            book_path(&dir),
            r#"{"fallback":{"provider":"a","model":"b"},"workspaces":{"/tmp/legacy":{"provider":"c","model":"d"}}}"#,
        )
        .unwrap();
        let legacy = load_persisted_book(&dir);
        assert_eq!(
            selection_for(&legacy, Some("coding-agent"), Some("/tmp/legacy"), &|_| {
                true
            })
            .model,
            "d",
            "a pre-agent book migrates into the shipped agent's map"
        );
        assert_eq!(
            selection_for(&legacy, Some("reviewer"), Some("/tmp/legacy"), &|_| true).model,
            "b",
            "a brand-new agent type starts from the global fallback"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn book_with_selection(root: &str, provider: &str, model: &str) -> SessionBook {
        let mut book = SessionBook {
            provider: "global-provider".to_string(),
            model: "global-model".to_string(),
            ..SessionBook::default()
        };
        book.workspaces.insert(
            root.to_string(),
            BookSelection {
                profile: String::new(),
                provider: provider.to_string(),
                model: model.to_string(),
                om_observation: None,
                om_reflection: None,
            },
        );
        book
    }

    #[test]
    fn workspace_selection_wins_over_global_fallback() {
        let book = book_with_selection("/tmp/alpha", "b", "m2");
        let configured = |provider: &str| provider != "unconfigured";
        let resolved = selection_for(&book, None, Some("/tmp/alpha"), &configured);
        assert_eq!(
            (resolved.provider.as_str(), resolved.model.as_str()),
            ("b", "m2")
        );
        // Unknown root and no-root fall back to the global trio.
        assert_eq!(
            selection_for(&book, None, Some("/tmp/other"), &configured).model,
            "global-model"
        );
        assert_eq!(
            selection_for(&book, None, None, &configured).model,
            "global-model"
        );
    }

    #[test]
    fn unconfigured_workspace_selection_falls_back_to_global() {
        let book = book_with_selection("/tmp/alpha", "unconfigured", "m2");
        let configured = |provider: &str| provider != "unconfigured";
        let resolved = selection_for(&book, None, Some("/tmp/alpha"), &configured);
        assert_eq!(resolved.provider, "global-provider");
        assert_eq!(resolved.model, "global-model");
    }

    #[tokio::test]
    async fn picker_selection_with_bound_tab_writes_workspace_key() {
        let host = AgentHost::inert();
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, "/tmp/alpha".to_string());
        host.set_tab_registry(Arc::clone(&registry));
        host.select_picker(AgentPickerKind::Model, "model:b/m2", Some(1))
            .await;
        let book = host.inner.book.lock().await;
        let alpha = book.workspaces.get("/tmp/alpha").expect("workspace entry");
        assert_eq!((alpha.provider.as_str(), alpha.model.as_str()), ("b", "m2"));
        // A selection without a bound tab writes only the global fallback.
        drop(book);
        host.select_picker(AgentPickerKind::Model, "model:c/m3", None)
            .await;
        let book = host.inner.book.lock().await;
        assert_eq!(book.provider, "c");
        assert_eq!(book.workspaces.get("/tmp/alpha").unwrap().provider, "b");
    }

    // Plan 109 I8: OM worker model selection rides the same per-workspace
    // book as the session model, with its own fields, and the bindings
    // survive a persist/reload round trip.
    #[tokio::test]
    async fn om_worker_selection_writes_workspace_fields_and_round_trips() {
        let host = AgentHost::inert();
        let registry = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
        registry
            .lock()
            .await
            .create_tab(1, 10, "/tmp/om-ws".to_string());
        host.set_tab_registry(Arc::clone(&registry));
        host.select_worker(
            AgentOmWorkerKind::Observation,
            "model:p/m-obs",
            None,
            Some(1),
        )
        .await;
        host.select_worker(
            AgentOmWorkerKind::Reflection,
            "model:p/m-refl",
            None,
            Some(1),
        )
        .await;
        {
            let book = host.inner.book.lock().await;
            let alpha = book.workspaces.get("/tmp/om-ws").expect("workspace entry");
            let observation = alpha.om_observation.as_ref().expect("observation set");
            assert_eq!(observation.provider, "p");
            assert_eq!(observation.model, "m-obs");
            let reflection = alpha.om_reflection.as_ref().expect("reflection set");
            assert_eq!(reflection.provider, "p");
            assert_eq!(reflection.model, "m-refl");
        }
        // An empty id clears the worker binding.
        host.select_worker(AgentOmWorkerKind::Observation, "", None, Some(1))
            .await;
        {
            let book = host.inner.book.lock().await;
            let alpha = book.workspaces.get("/tmp/om-ws").unwrap();
            assert!(alpha.om_observation.is_none(), "cleared");
            assert!(alpha.om_reflection.is_some(), "reflection untouched");
        }
        // Bindings survive a persist/reload round trip.
        drop(host);
        let dir = std::env::temp_dir().join(format!("clay-om-book-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create data dir");
        let config = AgentHostConfig {
            program: std::path::PathBuf::new(),
            args: Vec::new(),
            data_dir: dir.clone(),
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        };
        let persistent = AgentHost::new(config);
        // Re-apply the selections against a real (non-inert) host so
        // persist_book_selection writes book.json, then reload.
        {
            let registry2 = Arc::new(Mutex::new(super::super::tab_registry::TabRegistry::new()));
            registry2
                .lock()
                .await
                .create_tab(1, 10, "/tmp/om-ws".to_string());
            persistent.set_tab_registry(Arc::clone(&registry2));
        }
        persistent
            .select_worker(
                AgentOmWorkerKind::Observation,
                "model:p/m-obs",
                None,
                Some(1),
            )
            .await;
        drop(persistent);
        let book = load_persisted_book(&dir);
        let alpha = book.workspaces.get("/tmp/om-ws").expect("workspace entry");
        let observation = alpha
            .om_observation
            .as_ref()
            .expect("om_observation persisted");
        assert_eq!(observation.provider, "p");
        assert_eq!(observation.model, "m-obs");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_om_workers_maps_bindings_to_daemon_payload() {
        // Both absent: no key at all (session.new omits the field).
        assert_eq!(json_om_workers(None, None), Value::Null);
        let observation = Some(AgentOmWorkerModel {
            provider: "p".into(),
            model: "m".into(),
        });
        // One side set: the other side is an explicit null (clears).
        assert_eq!(
            json_om_workers(observation.clone(), None),
            json!({"observation": {"provider": "p", "model": "m"}, "reflection": null})
        );
        assert_eq!(
            json_om_workers(None, observation),
            json!({"observation": null, "reflection": {"provider": "p", "model": "m"}})
        );
    }

    // Plan 109 I9: the resumable list parses safe display fields, with
    // the label falling back to the summary snippet.
    #[test]
    fn parse_resumable_sessions_prefers_label_over_summary() {
        let value = json!({
            "sessions": [
                {"sessionId": "s1", "updatedAt": "t2", "label": "Fix the bug",
                 "summary": "snip", "metadata": {"workspaceRoot": "/ws/a"}},
                {"sessionId": "s2", "updatedAt": "t1", "summary": "fallback title"},
                {"updatedAt": "t0"}
            ]
        });
        let parsed = parse_resumable_sessions(&value);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "s1");
        assert_eq!(parsed[0].label, "Fix the bug");
        assert_eq!(parsed[1].id, "s2");
        assert_eq!(parsed[1].label, "fallback title");
        assert!(parsed[1].profile.is_empty());
    }

    #[test]
    fn session_picker_items_prefer_display_label() {
        let inventory = AgentInventory {
            providers: Vec::new(),
            models: Vec::new(),
            profiles: vec![AgentProfileInfo {
                name: "chat".into(),
                description: "Chat".into(),
            }],
            sessions: vec![
                AgentSessionInfo {
                    id: "session:s1".into(),
                    profile: "chat".into(),
                    updated_at: "t1".into(),
                    label: "Fix the bug".into(),
                    updated_at_label: String::new(),
                },
                AgentSessionInfo {
                    id: "session:s2".into(),
                    profile: "chat".into(),
                    updated_at: "t2".into(),
                    label: String::new(),
                    updated_at_label: String::new(),
                },
            ],
            provider: String::new(),
            model: String::new(),
        };
        let items = picker_items(AgentPickerKind::Session, &inventory);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "Fix the bug");
        assert_eq!(items[1].label, "chat", "falls back to the profile");
    }

    #[tokio::test]
    async fn broadcast_delivers_to_subscribed_views() {
        // Plan 117: the panel's resume/list path — the reply must reach the
        // view relay (fire-and-forget client commands have no reply lane).
        let dir = std::env::temp_dir().join(format!("clay-broadcast-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create data dir");
        let host = AgentHost::new(AgentHostConfig {
            program: std::path::PathBuf::new(),
            args: Vec::new(),
            data_dir: dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
        });
        let mut rx = host.subscribe();
        host.broadcast(AgentServerMessage::AgentRpc {
            code: "session.resumable".into(),
            result_json: "{\"sessions\":[]}".into(),
        });
        let delivered = rx.recv().await.expect("broadcast delivered");
        assert!(
            matches!(&*delivered, AgentServerMessage::AgentRpc { code, .. } if code == "session.resumable")
        );
    }
}
#[cfg(test)]
mod data_dir_migration_tests {
    use super::*;

    fn scratch_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-agent-datadir-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("scratch root");
        root
    }

    #[test]
    fn for_server_moves_a_legacy_agent_data_dir_under_the_per_agent_root() {
        let root = scratch_root("migrate");
        let legacy = root.join("agent");
        std::fs::create_dir_all(&legacy).expect("legacy data dir");
        std::fs::write(legacy.join("sessions.sqlite"), "db").expect("sessions");
        std::fs::write(legacy.join("credentials.vault"), "vault").expect("vault");

        let config = AgentHostConfig::for_server(Some(&root), None);
        assert_eq!(
            config.data_dir,
            root.join("agents").join("coding-agent").join("data")
        );
        assert!(
            !legacy.exists(),
            "legacy data dir renamed into the per-agent root"
        );
        assert!(
            config.data_dir.join("sessions.sqlite").is_file(),
            "sessions follow the data dir"
        );
        // The per-agent config root is surfaced via --agent-config-root;
        // --data-dir is appended later by resolve_launch at spawn time.
        let flag = config
            .args
            .iter()
            .position(|a| a == "--agent-config-root")
            .expect("--agent-config-root flag present");
        assert_eq!(
            config.args[flag + 1],
            root.join("agents").join("coding-agent").to_string_lossy()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn for_server_keeps_a_fresh_root_and_an_existing_data_dir() {
        let root = scratch_root("fresh");
        let config = AgentHostConfig::for_server(Some(&root), None);
        assert_eq!(
            config.data_dir,
            root.join("agents").join("coding-agent").join("data")
        );
        assert!(!config.data_dir.exists(), "nothing created eagerly");

        // Existing data dir (migration already done) is left untouched.
        let data = root.join("agents").join("coding-agent").join("data");
        std::fs::create_dir_all(&data).expect("data dir");
        std::fs::write(data.join("book.json"), "{}").expect("book");
        let again = AgentHostConfig::for_server(Some(&root), None);
        assert_eq!(again.data_dir, data);
        assert!(data.join("book.json").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn for_server_without_a_configuration_root_keeps_agent_state_off_the_shared_temp_dir() {
        // No explicit root: the per-user `~/.clay` applies in a real server
        // (the same root the daemon's homedir() default resolves to), *not* a
        // shared temp dir — one temp dir for every root-less server meant one
        // book.json, credentials.vault, and sessions.sqlite shared across
        // profiles and runs, so a stale selection named a profile the run
        // never registered and the tab could not bind a session. Unit tests
        // (cfg(test)) resolve a per-process root instead, so no test touches
        // the developer's profile; `tests/agent_session_isolation.rs` proves
        // the per-user path end-to-end with an isolated HOME.
        let config = AgentHostConfig::for_server(None, None);
        let expected_root = if cfg!(test) {
            std::env::temp_dir().join(format!("clay-agent-test-root-{}", std::process::id()))
        } else {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .expect("a home")
                .join(".clay")
        };
        assert_eq!(
            config.data_dir,
            expected_root
                .join("agents")
                .join("coding-agent")
                .join("data"),
            "root-less agent state follows the per-user (or per-process test) Clay root"
        );
        assert_ne!(
            config.data_dir,
            std::env::temp_dir().join("clay-agent"),
            "never the old shared temp dir"
        );
    }
}
