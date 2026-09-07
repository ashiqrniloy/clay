//! Core-owned `clay-agent` child. One daemon per Clay server.
//!
//! Package JavaScript never receives this type. Spawn is `Command` +
//! `env_clear`, never a shell string. Node missing is a diagnostic, not a hang.

use std::collections::{HashMap, HashSet};
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
    AGENT_MAX_SNAPSHOT_ENTRIES, AgentClientCommand, AgentInventory, AgentModelInfo,
    AgentOmWorkerKind, AgentOmWorkerModel, AgentPickerItem, AgentPickerKind, AgentProfileInfo,
    AgentProviderInfo, AgentSecret, AgentServerMessage, AgentSessionInfo, AgentSessionSnapshot,
    AgentSlashCommand, AgentToolPhase, AgentTranscriptEntry, AgentTranscriptKind, AgentWireEvent,
    ApprovalRequestKind, TabId, apply_transcript_event, truncate_transcript_text,
};
use crate::server::agent_picker::AgentSearchHit;

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(5);
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const EVENT_CAPACITY: usize = 256;
/// How long a pending user-approval request waits for a client answer
/// before failing closed (deny). Generous: a human is deciding.
const APPROVAL_WAIT: Duration = Duration::from_secs(300);
/// Upper bound on one daemon-produced approval request payload.
const MAX_APPROVAL_PAYLOAD_BYTES: usize = 16 * 1024;

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
                f.write_str("Node >= 20 is required for clay-agent but was not found")
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

/// One allow-listed MCP stdio server. Built by the server (later: config /
/// approved contribution data) — never from package JavaScript at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMcpAllowListEntry {
    pub server_id: String,
    pub command: String,
    pub args: Vec<String>,
    /// Explicit env names and literal values only; never inherited wholesale.
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
}

impl AgentMcpAllowListEntry {
    fn to_json(&self) -> Value {
        json!({
            "serverId": self.server_id,
            "command": self.command,
            "args": self.args,
            "env": self.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<std::collections::BTreeMap<_, _>>(),
            "cwd": self.cwd,
        })
    }
}

impl AgentHostConfig {
    pub fn for_server(configuration_root: Option<&Path>) -> Self {
        let data_dir = configuration_root
            .map(|root| root.join("agent"))
            .unwrap_or_else(|| std::env::temp_dir().join("clay-agent"));
        Self {
            program: PathBuf::new(),
            args: Vec::new(),
            data_dir,
            inherit_environment: Vec::new(),
            inert: false,
            mcp_allow_list: Vec::new(),
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

#[derive(Default)]
struct SessionBook {
    profile: String,
    provider: String,
    model: String,
    /// Last used selection per workspace root (plan 109 I2): written on
    /// picker/model selections that happen while a tab is bound to the
    /// root; resolved at session creation with the global trio as fallback.
    workspaces: HashMap<String, BookSelection>,
    tab_session: HashMap<TabId, String>,
    /// Workspace root each tab's session was created against (plan 109 I1).
    /// A tab whose registry root no longer matches is rebound on its next
    /// interaction; the old session stays resumable from `transcripts`.
    tab_session_root: HashMap<TabId, String>,
    /// Last finished run's context-token counter per session (plan 108
    /// task 9): the snapshot's context-used-vs-window numerator.
    context_tokens: HashMap<String, Option<u64>>,
    /// Declared thinking levels per `provider/model` (plan 109 I4), cached
    /// from the daemon's model inventory; the snapshot's `effortLevels`.
    model_levels: HashMap<String, Vec<String>>,
    /// Active thinking level per session (plan 109 I4): the last level a
    /// prompt carried; the snapshot's `effort`.
    effort: HashMap<String, String>,

    transcripts: HashMap<String, Vec<AgentTranscriptEntry>>,
    running: HashSet<String>,
    cancelled: HashSet<String>,
    /// Git branch per session (plan 109 R2): resolved from the session's
    /// workspace root, refreshed on session/workspace change and run
    /// completion. Empty = not a repo (status row shows `—`).
    branches: HashMap<String, String>,
    /// Workspace root each session was created/resumed against (plan 109
    /// R2): lets the run-finish republish refresh the branch without a
    /// tab reference.
    session_root: HashMap<String, String>,
    /// Branch cache: root -> (generation read at, branch) (plan 109 R2).
    /// Same generation = run still in flight = cached read; run
    /// completion bumps the generation so the next look re-reads.
    branch_cache: HashMap<String, (u64, String)>,
    /// Bumped on run terminal events (plan 109 R2): invalidates the
    /// branch cache so completion-time reads are fresh.
    run_generation: u64,
}

/// Plan 109 R2: best-effort git branch for a workspace root — direct
/// `.git` reads only, never a subprocess. Handles the usual layout
/// (`.git/HEAD`) and worktree/submodule pointers (`.git` file with
/// `gitdir:`). Bounded; failure-silent (`None` = not a repo).
fn read_git_branch(root: &Path) -> Option<String> {
    let git = root.join(".git");
    let meta = std::fs::metadata(&git).ok()?;
    let head_path = if meta.is_dir() {
        git.join("HEAD")
    } else {
        // Worktree/submodule: `.git` is a `gitdir: <path>` pointer file.
        let pointer = std::fs::read_to_string(&git).ok()?;
        let target = pointer.strip_prefix("gitdir:")?.trim();
        let path = Path::new(target);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        resolved.join("HEAD")
    };
    let head = std::fs::read_to_string(head_path).ok()?;
    let head = head.trim();
    if let Some(branch) = head.strip_prefix("ref: refs/heads/") {
        // Bound the name: a branch line is never PATH_MAX long.
        let branch = branch.trim();
        if branch.is_empty() {
            return None;
        }
        return Some(branch.chars().take(80).collect());
    }
    // Detached HEAD: a short sha is the truthful readout.
    if head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(head[..7].to_string());
    }
    None
}

/// Plan 109 R2: resolve the session's branch — cached within the current
/// run generation, re-read across generations (run completion bumps it).
/// `reader` is injectable for cache-hit tests.
fn refresh_branch(
    book: &mut SessionBook,
    root: &str,
    session: &str,
    reader: fn(&Path) -> Option<String>,
) {
    let Some(root_path) = Path::new(root).to_str().map(PathBuf::from) else {
        return;
    };
    let generation = book.run_generation;
    let branch = match book.branch_cache.get(root) {
        Some((seen, branch)) if *seen == generation => branch.clone(),
        _ => {
            let branch = reader(&root_path).unwrap_or_default();
            book.branch_cache
                .insert(root.to_string(), (generation, branch.clone()));
            branch
        }
    };
    book.branches.insert(session.to_string(), branch);
}

/// Plan 109 R1: bounded parse of the daemon `environment.list` response —
/// completion names/descriptions and loaded extension names only.
fn parse_environment(value: &Value) -> (Vec<AgentSlashCommand>, Vec<String>) {
    const MAX_COMMANDS: usize = 64;
    const MAX_EXTENSIONS: usize = 8;
    let mut commands = Vec::new();
    if let Some(list) = value.get("commands").and_then(Value::as_array) {
        for command in list.iter().take(MAX_COMMANDS) {
            let Some(name) = command.get("name").and_then(Value::as_str) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            commands.push(AgentSlashCommand {
                name: name.chars().take(48).collect(),
                description: command
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .chars()
                    .take(96)
                    .collect(),
            });
        }
    }
    let mut extensions = Vec::new();
    if let Some(list) = value.get("extensions").and_then(Value::as_array) {
        for extension in list.iter().take(MAX_EXTENSIONS) {
            if let Some(name) = extension.as_str().filter(|name| !name.is_empty()) {
                extensions.push(name.chars().take(48).collect());
            }
        }
    }
    (commands, extensions)
}

impl SessionBook {
    /// The tab's session when it still matches `current_root` (the tab
    /// registry's root, empty when unknown). A root change clears the
    /// stale binding so the caller creates a session for the new root.
    fn session_for_root(&mut self, tab: TabId, current_root: Option<&str>) -> Option<String> {
        let session_id = self.tab_session.get(&tab)?.clone();
        let bound_root = self
            .tab_session_root
            .get(&tab)
            .map(String::as_str)
            .unwrap_or("");
        if bound_root == current_root.unwrap_or("") {
            return Some(session_id);
        }
        self.tab_session.remove(&tab);
        self.tab_session_root.remove(&tab);
        None
    }
}

/// A persisted profile/provider/model selection (plan 109 I2): the global
/// fallback trio in `book.json`, and one entry per workspace root. The OM
/// worker bindings (plan 109 I8) ride the same entries (protocol
/// `AgentOmWorkerModel` is the shared wire shape).
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
struct BookSelection {
    #[serde(default)]
    profile: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    om_observation: Option<AgentOmWorkerModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    om_reflection: Option<AgentOmWorkerModel>,
}

impl BookSelection {
    fn from_book(book: &SessionBook) -> Self {
        Self {
            profile: book.profile.clone(),
            provider: book.provider.clone(),
            model: book.model.clone(),
            om_observation: None,
            om_reflection: None,
        }
    }

    fn is_configured(&self, is_configured: &impl Fn(&str) -> bool) -> bool {
        !self.provider.is_empty() && is_configured(&self.provider)
    }
}

/// Plan 109 I8: build the daemon's `observationalMemoryWorkers` /
/// `workers` payload — `{ observation: {provider, model}|null, reflection:
/// …|null }`; `null` when both sides are absent (no key at all).
fn json_om_workers(
    observation: Option<AgentOmWorkerModel>,
    reflection: Option<AgentOmWorkerModel>,
) -> Value {
    if observation.is_none() && reflection.is_none() {
        return Value::Null;
    }
    let one = |model: Option<AgentOmWorkerModel>| match model {
        Some(model) => json!({ "provider": model.provider, "model": model.model }),
        None => Value::Null,
    };
    json!({
        "observation": one(observation),
        "reflection": one(reflection),
    })
}

/// Resolve the selection for a workspace root (plan 109 I2): the
/// workspace's last-used selection when it exists and its provider is
/// configured, else the global fallback trio.
fn selection_for(
    book: &SessionBook,
    root: Option<&str>,
    is_configured: &impl Fn(&str) -> bool,
) -> BookSelection {
    root.and_then(|root| book.workspaces.get(root))
        .filter(|selection| selection.is_configured(is_configured))
        .cloned()
        .unwrap_or_else(|| BookSelection::from_book(book))
}

/// Pending daemon-initiated approval requests, keyed by request id.
type PendingApprovals = HashMap<String, oneshot::Sender<Result<Value, String>>>;

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
    /// active extensions, fetched once per daemon generation and
    /// invalidated on daemon death or a registration/knowledge mutation.
    environment: Mutex<Option<(Vec<AgentSlashCommand>, Vec<String>)>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AgentPickerAuth {
    pub kind: String,
    pub name: String,
    pub credential_name: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AgentPickerProvider {
    pub id: String,
    pub configured: bool,
    pub auth: Vec<AgentPickerAuth>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AgentPickerInventory {
    pub providers: Vec<AgentPickerProvider>,
    pub models: Vec<AgentModelInfo>,
    pub profiles: Vec<AgentProfileInfo>,
    pub sessions: Vec<AgentSessionInfo>,
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
                approvals: Arc::new(Mutex::new(HashMap::new())),
                approval_seq: AtomicU64::new(1),
                pending_registrations: Arc::new(Mutex::new(Vec::new())),
                environment: Mutex::new(None),
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

    pub fn for_server(configuration_root: Option<&Path>) -> Self {
        Self::new(AgentHostConfig::for_server(configuration_root))
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

    /// Surface a daemon-initiated user-approval request to connected clients
    /// and wait bounded for the answer. Denies fail-closed on timeout or
    /// missing consumer. `payload_json` is daemon-produced; oversized
    /// payloads are rejected before any client sees them.
    pub async fn request_user_approval(
        &self,
        kind: ApprovalRequestKind,
        payload_json: &str,
    ) -> Result<Value, String> {
        self.request_user_approval_for(kind, payload_json, APPROVAL_WAIT)
            .await
    }

    async fn request_user_approval_for(
        &self,
        kind: ApprovalRequestKind,
        payload_json: &str,
        wait: Duration,
    ) -> Result<Value, String> {
        if payload_json.len() > MAX_APPROVAL_PAYLOAD_BYTES {
            return Err("approval payload exceeds the wire budget".into());
        }
        let request_id = format!(
            "approval-{}",
            self.inner.approval_seq.fetch_add(1, Ordering::Relaxed)
        );
        let (reply, answer) = oneshot::channel();
        self.inner
            .approvals
            .lock()
            .await
            .insert(request_id.clone(), reply);
        let request = Arc::new(AgentServerMessage::ApprovalRequest {
            request_id: request_id.clone(),
            kind,
            payload_json: payload_json.to_string(),
        });
        if self.inner.events.send(request).is_err() {
            self.inner.approvals.lock().await.remove(&request_id);
            return Err("no approval consumer is connected".into());
        }
        let result = match timeout(wait, answer).await {
            Ok(Ok(result)) => result,
            Ok(Err(_receiver_gone)) => Err("no approval consumer is connected".into()),
            Err(_elapsed) => Err("approval request timed out".into()),
        };
        self.inner.approvals.lock().await.remove(&request_id);
        result
    }

    /// Answer a pending approval request. Returns false for unknown/stale
    /// ids; the requester sees exactly one answer either way.
    pub async fn resolve_approval(&self, request_id: &str, result: Result<Value, String>) -> bool {
        match self.inner.approvals.lock().await.remove(request_id) {
            Some(reply) => reply.send(result).is_ok(),
            None => false,
        }
    }

    pub fn dispatch(&self, command: AgentClientCommand) {
        let host = self.clone();
        tokio::spawn(async move {
            let message = host.run(command).await;
            let _ = host.inner.events.send(Arc::new(message));
        });
    }

    pub(crate) async fn select_picker(&self, kind: AgentPickerKind, id: &str, tab: Option<TabId>) {
        {
            let mut book = self.inner.book.lock().await;
            match kind {
                AgentPickerKind::Provider => {
                    book.provider = id.strip_prefix("provider:").unwrap_or(id).to_string();
                }
                AgentPickerKind::Model => {
                    let rest = id.strip_prefix("model:").unwrap_or(id);
                    if let Some((provider, model)) = rest.split_once('/') {
                        book.provider = provider.to_string();
                        book.model = model.to_string();
                    }
                }
                AgentPickerKind::Agent => {
                    book.profile = id.strip_prefix("agent:").unwrap_or(id).to_string();
                }
                _ => {}
            }
        }
        if matches!(kind, AgentPickerKind::Provider) {
            self.ensure_default_model().await;
        }
        // Plan 109 I2: a selection made while a tab is bound to a workspace
        // also becomes that workspace's last-used selection.
        if let Some(root) = self.tab_workspace_root_for(tab).await {
            let mut book = self.inner.book.lock().await;
            let selection = BookSelection::from_book(&book);
            book.workspaces.insert(root, selection);
        }
        self.persist_book_selection().await;
        self.publish_book_snapshot().await;
    }

    /// Plan 109 I8: OM worker model selection — the same per-workspace book
    /// path as `select_picker`, plus the daemon's per-session
    /// `session.om.set` when the panel bound a session. `id` is
    /// `model:provider/model`, or empty to clear.
    pub(crate) async fn select_worker(
        &self,
        worker: AgentOmWorkerKind,
        id: &str,
        session_id: Option<String>,
        tab: Option<TabId>,
    ) {
        let parsed = id
            .strip_prefix("model:")
            .and_then(|rest| rest.split_once('/'))
            .map(|(provider, model)| AgentOmWorkerModel {
                provider: provider.to_string(),
                model: model.to_string(),
            });
        let root = self.tab_workspace_root_for(tab).await;
        {
            let mut book = self.inner.book.lock().await;
            let mut selection = root
                .as_deref()
                .and_then(|r| book.workspaces.get(r))
                .cloned()
                .unwrap_or_else(|| BookSelection::from_book(&book));
            match worker {
                AgentOmWorkerKind::Observation => selection.om_observation = parsed.clone(),
                AgentOmWorkerKind::Reflection => selection.om_reflection = parsed.clone(),
            }
            if let Some(root) = root {
                book.workspaces.insert(root, selection);
            }
        }
        self.persist_book_selection().await;
        self.publish_book_snapshot().await;
        if let Some(session_id) = session_id {
            self.dispatch(AgentClientCommand::SetOmWorkers {
                session_id,
                observation: if matches!(worker, AgentOmWorkerKind::Observation) {
                    parsed.clone()
                } else {
                    None
                },
                reflection: if matches!(worker, AgentOmWorkerKind::Reflection) {
                    parsed
                } else {
                    None
                },
            });
        }
    }

    /// Best-effort persistence of the profile/provider/model trio so a
    /// configured book survives server restarts (see load_persisted_book).
    async fn persist_book_selection(&self) {
        if self.inner.config.inert {
            return;
        }
        let data_dir = self.inner.config.data_dir.clone();
        let book = self.inner.book.lock().await;
        persist_book(&data_dir, &book);
    }

    async fn ensure_default_model(&self) {
        let provider = self.inner.book.lock().await.provider.clone();
        if provider.is_empty() {
            return;
        }
        let current = self.inner.book.lock().await.model.clone();
        let inventory = self.picker_inventory().await;
        if !current.is_empty()
            && inventory
                .models
                .iter()
                .any(|model| model.provider == provider && model.model == current)
        {
            return;
        }
        if let Some(model) = inventory
            .models
            .iter()
            .find(|model| model.provider == provider)
        {
            self.inner.book.lock().await.model = model.model.clone();
        }
    }

    async fn publish_book_snapshot(&self) {
        let snapshot = self.unconfigured_snapshot().await;
        let _ = self
            .inner
            .events
            .send(Arc::new(AgentServerMessage::Snapshot(snapshot)));
    }

    fn emit_agent(&self, message: AgentServerMessage) -> AgentServerMessage {
        let _ = self.inner.events.send(Arc::new(message.clone()));
        message
    }

    pub async fn begin_prompt(&self, tab: TabId, text: &str) -> AgentServerMessage {
        self.begin_prompt_with_effort(tab, text, None).await
    }

    /// Plan 109 I4: prompt with the session's portable thinking level
    /// (`None` keeps the current effort). The daemon fail-closes invalid
    /// level strings at its boundary.
    pub async fn begin_prompt_with_effort(
        &self,
        tab: TabId,
        text: &str,
        thinking_level: Option<String>,
    ) -> AgentServerMessage {
        if text.trim().is_empty() {
            return self.emit_agent(diagnostic("agent.empty_prompt", "empty prompt"));
        }
        if text.len() > AGENT_MAX_PROMPT_BYTES {
            return self.emit_agent(diagnostic(
                "agent.prompt_too_large",
                "prompt exceeds AGENT_MAX_PROMPT_BYTES",
            ));
        }
        let Some(session_id) = self.ensure_tab_session(tab).await else {
            return self.emit_agent(AgentServerMessage::Snapshot(
                self.unconfigured_snapshot().await,
            ));
        };
        {
            let mut book = self.inner.book.lock().await;
            let entries = book.transcripts.entry(session_id.clone()).or_default();
            entries.push(AgentTranscriptEntry::new(AgentTranscriptKind::User, text));
            cap_entries(entries);
            book.cancelled.remove(&session_id);
            book.running.insert(session_id.clone());
            // Plan 109 I4: the prompt's level becomes the session's active
            // effort; STATE echoes it until the next prompt changes it.
            if let Some(level) = &thinking_level {
                book.effort.insert(session_id.clone(), level.clone());
            }
        }
        let snapshot = self.snapshot_for(&session_id).await;
        // Run-scoped selection override (plan 108 task 9, per-workspace in
        // plan 109 I2): a picker switch between runs applies at the next
        // prompt without a new session. The workspace's last-used selection
        // resolves first; no inventory re-check here — the entry was written
        // from an already-configured picker selection.
        let workspace_root = self.tab_workspace_root(tab).await;
        let (provider, model) = {
            let book = self.inner.book.lock().await;
            let resolved = selection_for(&book, workspace_root.as_deref(), &|_| true);
            (resolved.provider, resolved.model)
        };
        self.dispatch(AgentClientCommand::Prompt {
            session_id,
            text: text.to_string(),
            provider: Some(provider),
            model: Some(model),
            thinking_level,
        });
        self.emit_agent(AgentServerMessage::Snapshot(snapshot))
    }

    pub(crate) async fn cancel_tab(&self, tab: TabId) -> AgentServerMessage {
        let session_id = self.inner.book.lock().await.tab_session.get(&tab).cloned();
        let Some(session_id) = session_id else {
            return diagnostic("agent.idle", "no running session");
        };
        {
            let mut book = self.inner.book.lock().await;
            book.cancelled.insert(session_id.clone());
            book.running.remove(&session_id);
        }
        self.dispatch(AgentClientCommand::Cancel { session_id });
        diagnostic("agent.cancelled", "cancelled")
    }

    /// Queues a mid-run user message on the tab's session (pi-parity steer,
    /// plan 108 task 9). User-initiated only: reachable solely through the
    /// validated chat intent path with the composer's text.
    pub async fn steer_tab(&self, tab: TabId, text: &str) -> AgentServerMessage {
        if text.trim().is_empty() || text.len() > AGENT_MAX_PROMPT_BYTES {
            return diagnostic("agent.steer_rejected", "empty or oversized steer");
        }
        let session_id = self.inner.book.lock().await.tab_session.get(&tab).cloned();
        let Some(session_id) = session_id else {
            return diagnostic("agent.idle", "no running session");
        };
        // Plan 109 I5: mid-run user input is visible in the transcript
        // (pi parity) — record the steer as a user-kind entry and publish
        // the snapshot so the live run reconciles around it.
        let snapshot = {
            let mut book = self.inner.book.lock().await;
            let entries = book.transcripts.entry(session_id.clone()).or_default();
            entries.push(AgentTranscriptEntry::new(AgentTranscriptKind::User, text));
            cap_entries(entries);
            self.snapshot_for(&session_id).await
        };
        self.dispatch(AgentClientCommand::Steer {
            session_id,
            text: text.to_string(),
            soft_interrupt: false,
        });
        self.emit_agent(AgentServerMessage::Snapshot(snapshot));
        diagnostic("agent.steered", "steered")
    }

    pub async fn resume_tab(
        &self,
        tab: TabId,
        session_id: &str,
        entry_id: Option<&str>,
    ) -> AgentServerMessage {
        self.inner
            .book
            .lock()
            .await
            .tab_session
            .insert(tab, session_id.to_string());
        let loaded = self
            .run(AgentClientCommand::LoadSession {
                session_id: session_id.to_string(),
                entry_id: entry_id.map(str::to_string),
            })
            .await;
        if let AgentServerMessage::Snapshot(snapshot) = &loaded {
            let mut book = self.inner.book.lock().await;
            book.transcripts
                .insert(session_id.to_string(), snapshot.entries.clone());
            if let Some(tokens) = snapshot.context_tokens {
                book.context_tokens
                    .insert(session_id.to_string(), Some(tokens));
            }
            if !snapshot.profile.is_empty() {
                book.profile = snapshot.profile.clone();
            }
            if !snapshot.provider.is_empty() {
                book.provider = snapshot.provider.clone();
            }
            if !snapshot.model.is_empty() {
                book.model = snapshot.model.clone();
            }
        }
        self.dispatch(AgentClientCommand::ResumeSession {
            session_id: session_id.to_string(),
        });
        loaded
    }

    /// Plan 109 I9: workspace-scoped resumable session list for the
    /// /resume picker. `workspace_root` comes from the tab registry
    /// (server-derived, never webview input); the daemon scopes the query
    /// and bounds the page, most-recent first.
    pub async fn resumable_sessions(
        &self,
        workspace_root: &str,
        limit: u32,
    ) -> Result<Vec<AgentSessionInfo>, AgentError> {
        let result = self
            .rpc(
                "session.resumable",
                json!({ "workspaceRoot": workspace_root, "limit": limit }),
            )
            .await?;
        Ok(parse_resumable_sessions(&result))
    }

    /// Workspace-scoped session search for the picker (plan 108 task 11):
    /// the query runs against the shared Phase 1 FTS index, scoped to the
    /// tab's current session workspace (decision 2201). Bounded by `limit`.
    pub async fn search_sessions(
        &self,
        tab: TabId,
        query: &str,
        limit: u32,
    ) -> Result<Vec<AgentSearchHit>, AgentError> {
        let session_id = {
            let book = self.inner.book.lock().await;
            book.tab_session.get(&tab).cloned()
        };
        let Some(session_id) = session_id else {
            return Ok(Vec::new());
        };
        let result = self
            .rpc(
                "session.search",
                json!({ "sessionId": session_id, "query": query, "limit": limit }),
            )
            .await?;
        let Some(hits) = result.get("hits").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        Ok(hits
            .iter()
            .filter_map(|hit| {
                let session_id = hit.get("sessionId")?.as_str()?.to_string();
                let leaf_id = hit
                    .get("leafId")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let updated_at = hit
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let label = hit
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or(&session_id)
                    .to_string();
                let snippet = hit
                    .get("snippet")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                Some(AgentSearchHit {
                    session_id,
                    leaf_id,
                    updated_at,
                    label,
                    snippet,
                })
            })
            .collect())
    }

    async fn ensure_tab_session(&self, tab: TabId) -> Option<String> {
        let workspace_root = self.tab_workspace_root(tab).await;
        {
            let mut book = self.inner.book.lock().await;
            if let Some(session_id) = book.session_for_root(tab, workspace_root.as_deref()) {
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
            let resolved = selection_for(&book, workspace_root.as_deref(), &is_configured);
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
            .run(AgentClientCommand::NewSession {
                profile,
                provider: selection.provider,
                model: selection.model,
                workspace_root,
                full_autonomy: None,
                om_observation: selection.om_observation,
                om_reflection: selection.om_reflection,
            })
            .await;
        let AgentServerMessage::Snapshot(snapshot) = created else {
            eprintln!("[agent] ensure_tab_session({tab:?}): NewSession failed -> {created:?}");
            return None;
        };
        if snapshot.session_id.is_empty() {
            eprintln!("[agent] ensure_tab_session({tab:?}): NewSession returned empty session id");
            return None;
        }
        let mut book = self.inner.book.lock().await;
        book.tab_session.insert(tab, snapshot.session_id.clone());
        book.tab_session_root
            .insert(tab, self.tab_workspace_root(tab).await.unwrap_or_default());
        book.transcripts
            .entry(snapshot.session_id.clone())
            .or_default();
        Some(snapshot.session_id)
    }

    fn mcp_server_names(&self) -> Vec<String> {
        self.inner
            .config
            .mcp_allow_list
            .iter()
            .map(|entry| entry.server_id.clone())
            .collect()
    }

    async fn snapshot_for(&self, session_id: &str) -> AgentSessionSnapshot {
        let (commands, extensions) = self.environment().await;
        let book = self.inner.book.lock().await;
        AgentSessionSnapshot {
            session_id: session_id.to_string(),
            profile: book.profile.clone(),
            provider: book.provider.clone(),
            model: book.model.clone(),
            leaf_id: None,
            context_tokens: book.context_tokens.get(session_id).cloned().flatten(),
            entries: book
                .transcripts
                .get(session_id)
                .cloned()
                .unwrap_or_default(),
            mcp_servers: self.mcp_server_names(),
            // Plan 109 R1/R3: completion commands + extension strip data
            // ride the same STATE snapshot; the branch comes from the
            // book's refreshed per-session read.
            commands,
            extensions,
            branch: book.branches.get(session_id).cloned().unwrap_or_default(),
            // Plan 109 I4: effort control state — declared levels for the
            // current model (empty = no control) and the session's active
            // level.
            effort_levels: book
                .model_levels
                .get(&format!("{}/{}", book.provider, book.model))
                .cloned()
                .unwrap_or_default(),
            effort: book.effort.get(session_id).cloned(),
        }
    }

    async fn unconfigured_snapshot(&self) -> AgentSessionSnapshot {
        // A fresh webview's first STATE snapshot already carries the
        // completion list + extension strip (plan 109 R1/R3).
        let (commands, extensions) = self.environment().await;
        let book = self.inner.book.lock().await;
        AgentSessionSnapshot {
            session_id: String::new(),
            profile: book.profile.clone(),
            provider: book.provider.clone(),
            model: book.model.clone(),
            leaf_id: None,
            context_tokens: None,
            entries: Vec::new(),
            mcp_servers: self.mcp_server_names(),
            effort_levels: Vec::new(),
            effort: None,
            commands,
            extensions,
            branch: String::new(),
        }
    }

    /// Plan 109 R1/R3: fill the daemon-environment fields on a snapshot
    /// parsed from a daemon attach/open reply (those replies carry no
    /// environment data of their own).
    async fn decorate_snapshot(&self, mut snapshot: AgentSessionSnapshot) -> AgentSessionSnapshot {
        let (commands, extensions) = self.environment().await;
        snapshot.commands = commands;
        snapshot.extensions = extensions;
        snapshot.branch = {
            let book = self.inner.book.lock().await;
            book.branches
                .get(&snapshot.session_id)
                .cloned()
                .unwrap_or_default()
        };
        snapshot
    }

    pub(crate) async fn picker_inventory(&self) -> AgentPickerInventory {
        if self.inner.config.inert {
            return AgentPickerInventory::default();
        }
        self.inventory_rich().await.unwrap_or_default()
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

    pub async fn run(&self, command: AgentClientCommand) -> AgentServerMessage {
        if self.inner.config.inert {
            return diagnostic("agent.unavailable", "agent host is not started");
        }
        match self.run_inner(command).await {
            Ok(message) => message,
            Err(error) => {
                let message = redact_text(&error.to_string(), &self.secrets().await);
                diagnostic(error_code(&error), &message)
            }
        }
    }

    async fn secrets(&self) -> Vec<String> {
        self.inner.secrets.lock().await.clone()
    }

    async fn remember_secret(&self, secret: &str) {
        if secret.is_empty() {
            return;
        }
        let mut secrets = self.inner.secrets.lock().await;
        if !secrets.iter().any(|item| item == secret) {
            secrets.push(secret.to_string());
        }
    }

    async fn run_inner(
        &self,
        command: AgentClientCommand,
    ) -> Result<AgentServerMessage, AgentError> {
        match command {
            AgentClientCommand::Prompt {
                session_id,
                text,
                provider,
                model,
                thinking_level,
            } => {
                if text.len() > AGENT_MAX_PROMPT_BYTES {
                    return Ok(diagnostic(
                        "agent.prompt_too_large",
                        "prompt exceeds AGENT_MAX_PROMPT_BYTES",
                    ));
                }
                let mut params = json!({ "sessionId": session_id, "text": text });
                if let Some(provider) = provider {
                    params["provider"] = json!(provider);
                }
                if let Some(model) = model {
                    params["model"] = json!(model);
                }
                if let Some(level) = thinking_level {
                    params["thinkingLevel"] = json!(level);
                }
                self.rpc("session.prompt", params).await?;
                // Refreshed snapshot: after a switch the state carries the
                // session's persisted provider/model so every client tab and
                // the status row agree.
                Ok(AgentServerMessage::Snapshot(
                    self.snapshot_for(&session_id).await,
                ))
            }
            AgentClientCommand::Cancel { session_id } => {
                self.rpc("session.cancel", json!({ "sessionId": session_id }))
                    .await?;
                Ok(diagnostic("agent.cancelled", "cancelled"))
            }
            AgentClientCommand::Steer {
                session_id,
                text,
                soft_interrupt,
            } => {
                self.rpc(
                    "session.steer",
                    json!({
                        "sessionId": session_id,
                        "text": text,
                        "softInterrupt": soft_interrupt,
                    }),
                )
                .await?;
                Ok(diagnostic("agent.steered", "steered"))
            }
            AgentClientCommand::NewSession {
                profile,
                provider,
                model,
                workspace_root,
                full_autonomy,
                om_observation,
                om_reflection,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("profile".into(), json!(profile));
                params.insert("provider".into(), json!(provider));
                params.insert("model".into(), json!(model));
                if let Some(root) = workspace_root {
                    params.insert("workspaceRoot".into(), json!(root));
                }
                if let Some(enabled) = full_autonomy {
                    params.insert("fullAutonomy".into(), json!(enabled));
                }
                // Plan 109 I8: the workspace book's OM worker defaults ride
                // session creation (per-session retention is daemon metadata).
                let om_workers = json_om_workers(om_observation, om_reflection);
                if !om_workers.is_null() {
                    params.insert("observationalMemoryWorkers".into(), om_workers);
                }
                let result = self.rpc("session.new", Value::Object(params)).await?;
                Ok(AgentServerMessage::Snapshot(
                    self.decorate_snapshot(snapshot_from_new(&result)).await,
                ))
            }
            AgentClientCommand::LoadSession {
                session_id,
                entry_id,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(entry) = entry_id {
                    params.insert("entryId".into(), json!(entry));
                }
                let result = self.rpc("session.load", Value::Object(params)).await?;
                Ok(AgentServerMessage::Snapshot(
                    self.decorate_snapshot(snapshot_from_load(&result)).await,
                ))
            }
            // Plan 109 I7: context inspector — the daemon's bounded,
            // redacted response rides the generic agent-RPC custom event
            // (`clay.agentRpc`), so the panel renders it without new
            // snapshot state.
            AgentClientCommand::Context {
                session_id,
                item_id,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(item) = item_id {
                    params.insert("itemId".into(), json!(item));
                }
                let result = self.rpc("session.context", Value::Object(params)).await?;
                Ok(agent_rpc("session.context", &result))
            }
            // Plan 109 I8: direct dispatch (no connection intercept) still
            // applies the daemon half; the book half rides select_worker.
            AgentClientCommand::SelectWorker {
                worker,
                id,
                session_id,
            } => {
                if let Some(session_id) = session_id {
                    let parsed = id
                        .strip_prefix("model:")
                        .and_then(|rest| rest.split_once('/'))
                        .map(|(provider, model)| AgentOmWorkerModel {
                            provider: provider.to_string(),
                            model: model.to_string(),
                        });
                    let params = json!({
                        "sessionId": session_id,
                        "workers": json_om_workers(
                            matches!(worker, AgentOmWorkerKind::Observation).then(|| parsed.clone()).flatten(),
                            matches!(worker, AgentOmWorkerKind::Reflection).then(|| parsed.clone()).flatten(),
                        ),
                    });
                    let result = self.rpc("session.om.set", params).await?;
                    Ok(agent_rpc("session.om.set", &result))
                } else {
                    Ok(diagnostic("agent.omWorkers", "no session bound"))
                }
            }
            // Plan 109 I8: per-session OM worker selection — validated +
            // persisted daemon-side; the response (with the effective
            // selection) rides the generic agent-RPC custom event.
            AgentClientCommand::SetOmWorkers {
                session_id,
                observation,
                reflection,
            } => {
                let params = json!({
                    "sessionId": session_id,
                    "workers": json_om_workers(observation, reflection),
                });
                let result = self.rpc("session.om.set", params).await?;
                Ok(agent_rpc("session.om.set", &result))
            }
            AgentClientCommand::OmActivity { session_id } => {
                let result = self
                    .rpc("session.om.activity", json!({ "sessionId": session_id }))
                    .await?;
                Ok(agent_rpc("session.om.activity", &result))
            }
            AgentClientCommand::ResumeSession { session_id } => {
                let result = self
                    .rpc("session.resume", json!({ "sessionId": session_id }))
                    .await?;
                Ok(AgentServerMessage::Snapshot(
                    self.decorate_snapshot(snapshot_from_new(&result)).await,
                ))
            }
            AgentClientCommand::DeleteSession { session_id } => {
                self.rpc("session.delete", json!({ "sessionId": session_id }))
                    .await?;
                Ok(diagnostic("agent.deleted", "deleted"))
            }
            AgentClientCommand::ListSessions => {
                Ok(AgentServerMessage::Inventory(self.inventory().await?))
            }
            AgentClientCommand::OpenPicker { kind } => {
                let inventory = self.inventory().await?;
                Ok(AgentServerMessage::Picker {
                    kind,
                    items: picker_items(kind, &inventory),
                })
            }
            AgentClientCommand::Select { kind, id } => {
                Ok(diagnostic("agent.selected", &format!("{kind:?}:{id}")))
            }
            AgentClientCommand::CredentialPut {
                provider,
                name,
                secret: AgentSecret(secret),
            } => {
                self.remember_secret(&secret).await;
                self.rpc(
                    "credential.put",
                    json!({ "provider": provider, "name": name, "secret": secret }),
                )
                .await?;
                Ok(AgentServerMessage::CredentialAck {
                    provider,
                    name,
                    stored: true,
                })
            }
            AgentClientCommand::CredentialDelete { provider, name } => {
                self.rpc(
                    "credential.delete",
                    json!({ "provider": provider, "name": name }),
                )
                .await?;
                Ok(AgentServerMessage::CredentialAck {
                    provider,
                    name,
                    stored: false,
                })
            }
            AgentClientCommand::Compact {
                session_id,
                strategy,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(name) = strategy {
                    params.insert("strategy".into(), json!(name));
                }
                self.rpc("session.compact", Value::Object(params)).await?;
                Ok(diagnostic("agent.compacted", "compacted"))
            }
            AgentClientCommand::SessionTree {
                session_id,
                method,
                entry_id,
            } => {
                let rpc_method = match method.as_str() {
                    "checkout" | "fork" | "clone" | "checkpoint" => method.clone(),
                    other => return Ok(diagnostic("agent.tree_invalid_method", other)),
                };
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                params.insert("entryId".into(), json!(entry_id));
                self.rpc(&format!("session.{rpc_method}"), Value::Object(params))
                    .await?;
                Ok(diagnostic(
                    "agent.tree_commanded",
                    &format!("{rpc_method}:{session_id} at {entry_id}"),
                ))
            }
            AgentClientCommand::SetAutonomy {
                session_id,
                enabled,
            } => {
                self.rpc(
                    "session.setAutonomy",
                    json!({ "sessionId": session_id, "enabled": enabled }),
                )
                .await?;
                Ok(diagnostic(
                    "agent.autonomy_set",
                    &format!("{session_id}: {enabled}"),
                ))
            }
            AgentClientCommand::SearchSessions {
                session_id,
                query,
                limit,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(q) = query {
                    params.insert("query".into(), json!(q));
                }
                if let Some(l) = limit {
                    params.insert("limit".into(), json!(l));
                }
                let result = self.rpc("session.search", Value::Object(params)).await?;
                Ok(agent_rpc("agent.search_result", &result))
            }
            AgentClientCommand::RunResume {
                session_id,
                run_id,
                decision_json,
            } => {
                let decision: Value = serde_json::from_str(&decision_json)
                    .map_err(|error| AgentError::Rpc(format!("invalid decision JSON: {error}")))?;
                if !decision.is_object() {
                    return Err(AgentError::Rpc(
                        "invalid decision: expected a JSON object".into(),
                    ));
                }
                // Fail-closed passthrough: the daemon re-validates every
                // field. expectedVersion may be omitted — the daemon then
                // applies its stashed suspension version.
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                params.insert("runId".into(), json!(run_id));
                for key in ["expectedVersion", "decision", "decisions"] {
                    if let Some(value) = decision.get(key) {
                        params.insert(key.into(), value.clone());
                    }
                }
                let result = self.rpc("run.resume", Value::Object(params)).await?;
                Ok(agent_rpc("agent.run_resume_result", &result))
            }
            AgentClientCommand::SkillRegister {
                name,
                description,
                instructions,
                tool_names,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("name".into(), json!(name));
                if let Some(text) = description {
                    params.insert("description".into(), json!(text));
                }
                if let Some(text) = instructions {
                    params.insert("instructions".into(), json!(text));
                }
                if !tool_names.is_empty() {
                    params.insert("toolNames".into(), json!(tool_names));
                }
                self.rpc("skill.register", Value::Object(params)).await?;
                Ok(diagnostic("agent.skill_registered", "registered"))
            }
            AgentClientCommand::CommandRegister {
                name,
                handler,
                description,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("name".into(), json!(name));
                if let Some(handler) = handler {
                    params.insert("handler".into(), json!(handler));
                }
                if let Some(text) = description {
                    params.insert("description".into(), json!(text));
                }
                self.rpc("command.register", Value::Object(params)).await?;
                Ok(diagnostic("agent.command_registered", "registered"))
            }
            AgentClientCommand::CommandDispatch {
                name,
                session_id,
                args_json,
            } => {
                let args = match args_json {
                    Some(text) => {
                        let value: Value = serde_json::from_str(&text).map_err(|error| {
                            AgentError::Rpc(format!("invalid command args JSON: {error}"))
                        })?;
                        if !value.is_object() {
                            return Err(AgentError::Rpc(
                                "invalid command args: expected a JSON object".into(),
                            ));
                        }
                        value
                    }
                    None => json!({}),
                };
                let mut params = serde_json::Map::new();
                params.insert("name".into(), json!(name));
                if let Some(session_id) = session_id {
                    params.insert("sessionId".into(), json!(session_id));
                }
                params.insert("args".into(), args);
                let result = self.rpc("command.dispatch", Value::Object(params)).await?;
                Ok(agent_rpc("agent.command_result", &result))
            }
            AgentClientCommand::ApprovalResolve {
                request_id,
                allowed,
            } => {
                let delivered = self
                    .resolve_approval(&request_id, Ok(json!({ "allowed": allowed })))
                    .await;
                Ok(diagnostic(
                    if delivered {
                        "agent.approval_resolved"
                    } else {
                        // Unknown/stale request id: nothing is mutated.
                        "agent.approval_unknown"
                    },
                    &request_id,
                ))
            }
            AgentClientCommand::AskDecisionResolve {
                request_id,
                answer_json,
            } => {
                let answer: Value = serde_json::from_str(&answer_json)
                    .map_err(|error| AgentError::Rpc(format!("invalid answer JSON: {error}")))?;
                if !answer.is_object() {
                    return Err(AgentError::Rpc(
                        "invalid answer: expected a JSON object".into(),
                    ));
                }
                let delivered = self.resolve_approval(&request_id, Ok(answer)).await;
                Ok(diagnostic(
                    if delivered {
                        "agent.ask_decision_resolved"
                    } else {
                        "agent.approval_unknown"
                    },
                    &request_id,
                ))
            }
            AgentClientCommand::RegisterProfile {
                name,
                description,
                instructions,
            } => {
                self.rpc(
                    "agentProfile.register",
                    json!({
                        "name": name,
                        "description": description,
                        "instructions": instructions,
                    }),
                )
                .await?;
                Ok(diagnostic("agent.profile_registered", &name))
            }
        }
    }

    async fn inventory(&self) -> Result<AgentInventory, AgentError> {
        let rich = self.inventory_rich().await?;
        let (provider, model) = {
            let book = self.inner.book.lock().await;
            (book.provider.clone(), book.model.clone())
        };
        Ok(AgentInventory {
            providers: rich
                .providers
                .iter()
                .map(|provider| AgentProviderInfo {
                    id: provider.id.clone(),
                    configured: provider.configured,
                })
                .collect(),
            models: rich.models,
            profiles: rich.profiles,
            sessions: rich.sessions,
            provider,
            model,
        })
    }

    async fn inventory_rich(&self) -> Result<AgentPickerInventory, AgentError> {
        let providers = self.rpc("provider.list", json!({})).await?;
        let models = self.rpc("model.list", json!({})).await?;
        let profiles = self.rpc("agentProfile.list", json!({})).await?;
        let sessions = self.rpc("session.list", json!({})).await?;
        let parsed_models = parse_models(&models);
        {
            // Cache declared thinking levels per provider/model (plan 109
            // I4) so snapshots can carry effortLevels without a daemon call.
            let mut book = self.inner.book.lock().await;
            for model in &parsed_models {
                book.model_levels.insert(
                    format!("{}/{}", model.provider, model.model),
                    model.thinking_levels.clone(),
                );
            }
        }
        Ok(AgentPickerInventory {
            providers: parse_picker_providers(&providers),
            models: parsed_models,
            profiles: parse_profiles(&profiles),
            sessions: parse_sessions(&sessions),
        })
    }

    pub async fn rpc(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        let running = self.ensure_running().await?;
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
                if matches!(method, "command.register" | "knowledge.setOptions") {
                    self.inner.environment.lock().await.take();
                }
                result
            }
            Ok(Err(_)) => {
                self.inner.environment.lock().await.take();
                Err(AgentError::ServiceStopped)
            }
            Err(_) => {
                self.inner.environment.lock().await.take();
                Err(AgentError::Timeout)
            }
        }
    }

    /// Plan 109 R1/R3: the daemon's registered commands + active
    /// extensions, cached per daemon generation. Failure-silent — an
    /// unavailable daemon yields empty (state merges keep prior values).
    async fn environment(&self) -> (Vec<AgentSlashCommand>, Vec<String>) {
        if self.inner.config.inert {
            return (Vec::new(), Vec::new());
        }
        {
            let cached = self.inner.environment.lock().await;
            if let Some((commands, extensions)) = cached.as_ref() {
                return (commands.clone(), extensions.clone());
            }
        }
        let fetched = self
            .rpc("environment.list", json!({}))
            .await
            .map(|value| parse_environment(&value))
            .unwrap_or_default();
        *self.inner.environment.lock().await = Some(fetched.clone());
        fetched
    }

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
        let mcp_servers = self.mcp_server_names();
        // Plan 109 R1/R3: the pump's settled snapshots omit the
        // environment keys (empty here; snapshot_events skips them) —
        // prompt/attach snapshots carry the list and the client's state
        // merge keeps it. No rpc on the not-yet-running channel.
        let slash_commands = Vec::new();
        let extensions = Vec::new();
        tokio::spawn(daemon_actor(
            child,
            stdout,
            commands_rx,
            events,
            secrets,
            book,
            reverse,
            mcp_servers,
            slash_commands,
            extensions,
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
    events: broadcast::Sender<Arc<AgentServerMessage>>,
    secrets: Arc<Mutex<Vec<String>>>,
    book: Arc<Mutex<SessionBook>>,
    reverse: Option<ReverseRpcHandler>,
    mcp_servers: Vec<String>,
    slash_commands: Vec<AgentSlashCommand>,
    extensions: Vec<String>,
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

fn map_event(params: &Value, secrets: &[String]) -> Option<AgentServerMessage> {
    let session_id = params
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let event = params.get("event").unwrap_or(params);
    let event_type = event.get("type").and_then(Value::as_str)?;
    let run_id = event
        .get("runId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mapped = match event_type {
        "agent_started" => AgentWireEvent::Started {
            session_id: session_id.clone(),
            run_id,
        },
        "agent_finished" => AgentWireEvent::Finished {
            session_id: session_id.clone(),
            run_id,
            usage: json_usage(event),
            context_tokens: event
                .get("usage")
                .and_then(|usage| usage.get("inputTokens"))
                .and_then(Value::as_u64),
        },
        "message_delta"
            if json_string(event, &["content", "type"]) == "thinking"
                || json_string(event, &["content", "type"]) == "reasoning" =>
        {
            AgentWireEvent::ThinkingDelta {
                session_id: session_id.clone(),
                run_id,
                text: redact_text(&json_text(event.get("content").unwrap_or(event)), secrets),
            }
        }
        "message_delta" => AgentWireEvent::MessageDelta {
            session_id: session_id.clone(),
            run_id,
            text: redact_text(&json_text(event.get("content").unwrap_or(event)), secrets),
        },
        "tool_execution_started" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Started,
            name: json_string(event, &["call", "name"]),
            tool_call_id: json_string(event, &["call", "id"]),
            args_digest: tool_args_digest(event, secrets),
            output_digest: None,
            // load_skill args are {"name": "<skill>"} (plan 109 I5).
            skill_name: skill_name_from_args(event),
        },
        "tool_execution_progress" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Progress,
            name: json_string(event, &["name"]),
            tool_call_id: json_string(event, &["toolCallId"]),
            args_digest: None,
            output_digest: None,
            skill_name: None,
        },
        "tool_execution_finished" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Finished,
            name: json_string(event, &["result", "name"]),
            tool_call_id: json_string(event, &["result", "toolCallId"]),
            args_digest: None,
            output_digest: tool_output_digest(event, secrets),
            skill_name: None,
        },
        "tool_execution_error" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Error,
            name: json_string(event, &["call", "name"]),
            tool_call_id: json_string(event, &["call", "id"]),
            args_digest: tool_args_digest(event, secrets),
            output_digest: Some(redact_text(
                &json_string(event, &["error", "message"]),
                secrets,
            ))
            .filter(|output| !output.is_empty()),
            skill_name: skill_name_from_args(event),
        },
        "tool_execution_blocked" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Blocked,
            name: json_string(event, &["name"]),
            tool_call_id: json_string(event, &["toolCallId"]),
            args_digest: None,
            output_digest: Some(redact_text(&json_string(event, &["reason"]), secrets))
                .filter(|output| !output.is_empty()),
            skill_name: None,
        },
        "permission_requested" | "permission_request" => AgentWireEvent::Permission {
            session_id: session_id.clone(),
            run_id,
            request_id: json_string(event, &["requestId"]),
            tool_name: json_string(event, &["toolName"]),
            allowed: None,
        },
        "permission_resolved" => AgentWireEvent::Permission {
            session_id: session_id.clone(),
            run_id,
            request_id: json_string(event, &["requestId"]),
            tool_name: json_string(event, &["toolName"]),
            allowed: event.get("allowed").and_then(Value::as_bool),
        },
        "event_subscriber_overflow" => AgentWireEvent::Overflow,
        // Durable-run suspension (tool approval): surface the pending
        // approval as Permission (request_id = first pending approvalId)
        // so the panel can render Allow/Deny and resume the run. Without
        // this arm the suspension fell into the catch-all below and the
        // client saw a spurious Started, leaving the run "streaming"
        // forever. Suspensions without a client-resumable approval close
        // the AG-UI run instead of parking it.
        "agent_suspended" => {
            let pending = event
                .get("interruption")
                .and_then(|interruption| interruption.get("pendingDecisions"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let tool_name = event
                .get("interruption")
                .and_then(|interruption| interruption.get("toolName"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let approval = pending
                .first()
                .and_then(|decision| decision.get("approvalId"))
                .and_then(Value::as_str)
                .map(str::to_string);
            match approval {
                Some(approval_id) if !tool_name.is_empty() => AgentWireEvent::Permission {
                    session_id: session_id.clone(),
                    run_id,
                    request_id: approval_id,
                    tool_name: tool_name.to_string(),
                    allowed: None,
                },
                _ => AgentWireEvent::Finished {
                    session_id: session_id.clone(),
                    run_id,
                    usage: String::new(),
                    context_tokens: None,
                },
            }
        }
        // Prism error events nest the details under `error`: {error: {name,
        // message, code}}; a top-level `message` string is the legacy shape.
        "error" => {
            let detail = event
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .or_else(|| event.get("message").and_then(Value::as_str))
                .unwrap_or("error");
            AgentWireEvent::Error {
                session_id: session_id.clone(),
                message: redact_text(detail, secrets),
            }
        }
        _ => AgentWireEvent::Started {
            session_id: session_id.clone(),
            run_id,
        },
    };
    Some(AgentServerMessage::Event {
        session_id,
        event: mapped,
    })
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

/// Bounded, redacted argument summary for tool rows (plan 109 I5): the
/// daemon's `call.arguments` compacted, secret-redacted, and truncated to
/// the per-entry budget before the wire. `None` when the call carries no
/// arguments object.
fn tool_args_digest(event: &Value, secrets: &[String]) -> Option<String> {
    let arguments = event.get("call").and_then(|call| call.get("arguments"))?;
    if !arguments.is_object() {
        return None;
    }
    let text = redact_text(&arguments.to_string(), secrets);
    (!text.is_empty()).then(|| truncate_transcript_text(&text, AGENT_MAX_ENTRY_TEXT_BYTES))
}

/// Bounded, redacted output excerpt for terminal tool rows (plan 109 I5):
/// finished rows project the result's text content blocks (falling back to
/// the stringified `value`); the caller handles error/blocked reasons.
fn tool_output_digest(event: &Value, secrets: &[String]) -> Option<String> {
    let result = event.get("result")?;
    let mut text = String::new();
    if let Some(blocks) = result.get("content").and_then(Value::as_array) {
        for block in blocks {
            if block.get("type").and_then(Value::as_str) == Some("text")
                && let Some(chunk) = block.get("text").and_then(Value::as_str)
            {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(chunk);
            }
        }
    }
    if text.is_empty()
        && let Some(value) = result.get("value")
        && !value.is_null()
    {
        text = value.to_string();
    }
    let text = redact_text(&text, secrets);
    (!text.is_empty()).then(|| truncate_transcript_text(&text, AGENT_MAX_ENTRY_TEXT_BYTES))
}

/// `load_skill` rows carry the loaded skill's name from the call arguments
/// (`{"name": "<skill>"}`); other tools have none (plan 109 I5).
fn skill_name_from_args(event: &Value) -> Option<String> {
    let name = json_string(event, &["call", "name"]);
    if name != "load_skill" {
        return None;
    }
    let skill = json_string(event, &["call", "arguments", "name"]);
    (!skill.is_empty()).then_some(skill)
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

fn snapshot_from_new(value: &Value) -> AgentSessionSnapshot {
    AgentSessionSnapshot {
        session_id: value
            .get("sessionId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        profile: value
            .get("profile")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        provider: value
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        leaf_id: value
            .get("leafId")
            .and_then(Value::as_str)
            .map(str::to_string),
        context_tokens: value.get("contextTokens").and_then(Value::as_u64),
        entries: Vec::new(),
        mcp_servers: Vec::new(),
        effort_levels: Vec::new(),
        effort: None,
        // R1/R2/R3: filled by `decorate_snapshot` at the attach arms.
        commands: Vec::new(),
        branch: String::new(),
        extensions: Vec::new(),
    }
}

fn snapshot_from_load(value: &Value) -> AgentSessionSnapshot {
    let mut snapshot = snapshot_from_new(value);
    if let Some(meta) = value.get("metadata") {
        if snapshot.provider.is_empty() {
            snapshot.provider = meta
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
        if snapshot.model.is_empty() {
            snapshot.model = meta
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
        if snapshot.profile.is_empty() {
            snapshot.profile = meta
                .get("profile")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
    }
    if let Some(entries) = value.get("entries").and_then(Value::as_array) {
        snapshot.entries = entries
            .iter()
            .filter_map(|entry| {
                let text = json_text(entry.get("content").unwrap_or(entry));
                if text.is_empty() {
                    None
                } else {
                    Some(AgentTranscriptEntry::new(
                        transcript_kind(entry.get("role").and_then(Value::as_str)),
                        text,
                    ))
                }
            })
            .take(AGENT_MAX_SNAPSHOT_ENTRIES)
            .collect();
    }
    snapshot
}

/// Persisted book selection: survives server restarts so a configured
/// provider/model/profile does not need re-picking. Best-effort JSON next
/// to the daemon data (sessions/credentials live there too).
fn book_path(data_dir: &Path) -> PathBuf {
    data_dir.join("book.json")
}

fn load_persisted_book(data_dir: &Path) -> SessionBook {
    let mut book = SessionBook::default();
    let Ok(raw) = std::fs::read_to_string(book_path(data_dir)) else {
        return book;
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) => {
            // v2 (plan 109 I2): { fallback: {profile, provider, model},
            // workspaces: { "<root>": {...} } }. A v1 payload (flat trio)
            // loads as the fallback entry.
            let fallback = value.get("fallback").unwrap_or(&value);
            if let Some(field) = fallback.get("provider").and_then(Value::as_str) {
                book.provider = field.to_string();
            }
            if let Some(field) = fallback.get("model").and_then(Value::as_str) {
                book.model = field.to_string();
            }
            if let Some(field) = fallback.get("profile").and_then(Value::as_str) {
                book.profile = field.to_string();
            }
            if let Some(workspaces) = value.get("workspaces").and_then(Value::as_object) {
                for (root, entry) in workspaces {
                    if let Ok(selection) = serde_json::from_value::<BookSelection>(entry.clone()) {
                        book.workspaces.insert(root.clone(), selection);
                    }
                }
            }
        }
        Err(error) => {
            eprintln!("[agent] book.json unreadable: {error}");
        }
    }
    book
}

fn persist_book(data_dir: &Path, book: &SessionBook) {
    let payload = json!({
        "fallback": BookSelection::from_book(book),
        "workspaces": book.workspaces,
    });
    if let Err(error) = std::fs::write(book_path(data_dir), payload.to_string()) {
        eprintln!("[agent] book.json write failed: {error}");
    }
}

/// Book transcript snapshot for republish (plan 109 I5): same shape as
/// `snapshot_for` without facade access (the pump owns the book guard).
fn book_snapshot(
    book: &SessionBook,
    session_id: &str,
    mcp_servers: &[String],
    commands: &[AgentSlashCommand],
    extensions: &[String],
) -> AgentSessionSnapshot {
    AgentSessionSnapshot {
        session_id: session_id.to_string(),
        profile: book.profile.clone(),
        provider: book.provider.clone(),
        model: book.model.clone(),
        leaf_id: None,
        context_tokens: book.context_tokens.get(session_id).cloned().flatten(),
        entries: book
            .transcripts
            .get(session_id)
            .cloned()
            .unwrap_or_default(),
        mcp_servers: mcp_servers.to_vec(),
        effort_levels: book
            .model_levels
            .get(&format!("{}/{}", book.provider, book.model))
            .cloned()
            .unwrap_or_default(),
        effort: book.effort.get(session_id).cloned(),
        commands: commands.to_vec(),
        extensions: extensions.to_vec(),
        branch: book.branches.get(session_id).cloned().unwrap_or_default(),
    }
}

fn apply_book_event(book: &mut SessionBook, message: &AgentServerMessage) {
    let AgentServerMessage::Event { session_id, event } = message else {
        return;
    };
    if book.cancelled.contains(session_id) && !matches!(event, AgentWireEvent::Started { .. }) {
        return;
    }
    match event {
        AgentWireEvent::Started { .. } => {
            book.cancelled.remove(session_id);
            book.running.insert(session_id.clone());
        }
        AgentWireEvent::Finished { context_tokens, .. } => {
            book.running.remove(session_id);
            if context_tokens.is_some() {
                book.context_tokens
                    .insert(session_id.clone(), *context_tokens);
            }
            // Plan 109 R2: a settled run invalidates the branch cache.
            book.run_generation = book.run_generation.wrapping_add(1);
        }
        AgentWireEvent::Error { .. } => {
            book.running.remove(session_id);
            book.run_generation = book.run_generation.wrapping_add(1);
        }
        _ => {}
    }
    apply_transcript_event(
        book.transcripts.entry(session_id.clone()).or_default(),
        event,
    );
    if let Some(entries) = book.transcripts.get_mut(session_id) {
        cap_entries(entries);
    }
}

fn cap_entries(entries: &mut Vec<AgentTranscriptEntry>) {
    if entries.len() > AGENT_MAX_SNAPSHOT_ENTRIES {
        let drop = entries.len() - AGENT_MAX_SNAPSHOT_ENTRIES;
        entries.drain(..drop);
    }
}

fn transcript_kind(role: Option<&str>) -> AgentTranscriptKind {
    match role {
        Some("user") => AgentTranscriptKind::User,
        Some("thinking" | "reasoning") => AgentTranscriptKind::Thinking,
        Some("error") => AgentTranscriptKind::Error,
        Some("usage") => AgentTranscriptKind::Usage,
        _ => AgentTranscriptKind::Assistant,
    }
}

fn json_usage(event: &Value) -> String {
    let Some(usage) = event.get("usage") else {
        return String::new();
    };
    if let Some(text) = usage.as_str() {
        return text.to_string();
    }
    let input = usage
        .get("inputTokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("outputTokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if input == 0 && output == 0 {
        String::new()
    } else {
        format!("{input} in / {output} out")
    }
}

fn parse_picker_providers(value: &Value) -> Vec<AgentPickerProvider> {
    value
        .get("providers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?.to_string();
            let auth = item
                .get("auth")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|method| {
                    Some(AgentPickerAuth {
                        kind: method.get("kind").and_then(Value::as_str)?.to_string(),
                        name: method
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        credential_name: method
                            .get("credentialName")
                            .and_then(Value::as_str)
                            .unwrap_or("apiKey")
                            .to_string(),
                    })
                })
                .collect();
            Some(AgentPickerProvider {
                configured: item
                    .get("configured")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                id,
                auth,
            })
        })
        .collect()
}

fn parse_models(value: &Value) -> Vec<AgentModelInfo> {
    value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentModelInfo {
                provider: item.get("provider").and_then(Value::as_str)?.to_string(),
                model: item.get("model").and_then(Value::as_str)?.to_string(),
                display_name: item
                    .get("displayName")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                context_window: item.get("contextWindow").and_then(Value::as_u64),
                thinking_levels: item
                    .get("thinkingLevels")
                    .and_then(Value::as_array)
                    .map(|levels| {
                        levels
                            .iter()
                            .filter_map(Value::as_str)
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn parse_profiles(value: &Value) -> Vec<AgentProfileInfo> {
    value
        .get("profiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentProfileInfo {
                name: item.get("name").and_then(Value::as_str)?.to_string(),
                description: item
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

fn parse_sessions(value: &Value) -> Vec<AgentSessionInfo> {
    value
        .get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentSessionInfo {
                id: item.get("id").and_then(Value::as_str)?.to_string(),
                profile: item
                    .get("profile")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                updated_at: item
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                label: item
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

/// Plan 109 I9: parse the workspace-scoped resumable list from
/// `session.resumable`. The label carries the store's display label,
/// falling back to the summary snippet.
fn parse_resumable_sessions(value: &Value) -> Vec<AgentSessionInfo> {
    value
        .get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let label = item
                .get("label")
                .and_then(Value::as_str)
                .filter(|label| !label.is_empty())
                .or_else(|| item.get("summary").and_then(Value::as_str))
                .unwrap_or("")
                .to_string();
            Some(AgentSessionInfo {
                id: item.get("sessionId").and_then(Value::as_str)?.to_string(),
                profile: String::new(),
                updated_at: item
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                label,
            })
        })
        .collect()
}

fn picker_items(kind: AgentPickerKind, inventory: &AgentInventory) -> Vec<AgentPickerItem> {
    match kind {
        // Plan 109 I8: OM worker models are panel dropdowns (no list).
        AgentPickerKind::OmObservation | AgentPickerKind::OmReflection => vec![],
        AgentPickerKind::Provider | AgentPickerKind::ProviderSetup => inventory
            .providers
            .iter()
            .map(|provider| AgentPickerItem {
                id: provider.id.clone(),
                label: provider.id.clone(),
            })
            .collect(),
        AgentPickerKind::Model => {
            let configured: std::collections::HashSet<&str> = inventory
                .providers
                .iter()
                .filter(|provider| provider.configured)
                .map(|provider| provider.id.as_str())
                .collect();
            inventory
                .models
                .iter()
                .filter(|model| configured.contains(model.provider.as_str()))
                .map(|model| AgentPickerItem {
                    id: format!("{}/{}", model.provider, model.model),
                    label: if model.display_name.is_empty() {
                        model.model.clone()
                    } else {
                        model.display_name.clone()
                    },
                })
                .collect()
        }
        AgentPickerKind::Agent => inventory
            .profiles
            .iter()
            .map(|profile| AgentPickerItem {
                id: profile.name.clone(),
                label: if profile.description.is_empty() {
                    profile.name.clone()
                } else {
                    profile.description.clone()
                },
            })
            .collect(),
        AgentPickerKind::Session => inventory
            .sessions
            .iter()
            .map(|session| AgentPickerItem {
                id: session.id.clone(),
                label: if session.label.is_empty() {
                    session.profile.clone()
                } else {
                    session.label.clone()
                },
            })
            .collect(),
        // Search results are query-driven (FTS), never pre-listed in the
        // state snapshot.
        AgentPickerKind::SessionSearch => Vec::new(),
    }
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
    use super::*;

    // Plan 109 R2: a counting reader exposes cache hits — same generation
    // must reuse the cached branch without re-reading.
    static BRANCH_READS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn counting_reader(root: &Path) -> Option<String> {
        BRANCH_READS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        read_git_branch(root)
    }

    static REPO_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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
        let (parsed_commands, extensions) = parse_environment(&serde_json::json!({
            "commands": commands,
            "extensions": ["wiki", "graft", "", 42],
        }));
        assert_eq!(parsed_commands.len(), 64);
        assert!(parsed_commands[0].description.len() <= 96);
        assert_eq!(extensions, vec!["wiki".to_string(), "graft".to_string()]);
        assert_eq!(
            parse_environment(&serde_json::json!({})),
            (Vec::new(), Vec::new())
        );
    }

    fn book_with_session(tab: TabId, root: &str) -> SessionBook {
        let mut book = SessionBook::default();
        book.tab_session.insert(tab, "session-1".to_string());
        book.tab_session_root.insert(tab, root.to_string());
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
            ..
        } = event
        else {
            panic!("tool event expected");
        };
        assert_eq!(args_digest.as_deref(), Some(r#"{"path":"[redacted].txt"}"#));
        assert_eq!(skill_name, None);

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
            ..
        } = event
        else {
            panic!("tool event expected");
        };
        assert_eq!(skill_name.as_deref(), Some("rust-review"));
        assert_eq!(args_digest.as_deref(), Some(r#"{"name":"rust-review"}"#));

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
    fn session_kept_while_workspace_root_matches() {
        let mut book = book_with_session(7, "/tmp/alpha");
        assert_eq!(
            book.session_for_root(7, Some("/tmp/alpha")).as_deref(),
            Some("session-1")
        );
    }

    #[test]
    fn unbound_legacy_session_survives_missing_registry() {
        // Pre-I1 entries recorded no root: keep them stable when no
        // registry resolves (never surprise-rebind legacy sessions).
        let mut book = book_with_session(7, "");
        assert_eq!(book.session_for_root(7, None).as_deref(), Some("session-1"));
        assert_eq!(
            book.session_for_root(7, Some("")).as_deref(),
            Some("session-1")
        );
    }

    #[test]
    fn bound_session_rebinds_when_registry_disappears() {
        // A root-bound session must never keep running against a root the
        // registry no longer reports: rebind (fresh create) instead.
        let mut book = book_with_session(7, "/tmp/alpha");
        assert_eq!(book.session_for_root(7, None), None);
        assert!(!book.tab_session.contains_key(&7));
        assert!(!book.tab_session_root.contains_key(&7));
    }

    #[test]
    fn workspace_change_clears_stale_binding_for_rebind() {
        let mut book = book_with_session(7, "/tmp/alpha");
        assert_eq!(book.session_for_root(7, Some("/tmp/beta")), None);
        assert!(!book.tab_session.contains_key(&7));
        assert!(!book.tab_session_root.contains_key(&7));
        // Next call is a fresh create; the old session stays resumable.
        assert!(book.transcripts.is_empty());
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
        let resolved = selection_for(&book, Some("/tmp/alpha"), &configured);
        assert_eq!(
            (resolved.provider.as_str(), resolved.model.as_str()),
            ("b", "m2")
        );
        // Unknown root and no-root fall back to the global trio.
        assert_eq!(
            selection_for(&book, Some("/tmp/other"), &configured).model,
            "global-model"
        );
        assert_eq!(
            selection_for(&book, None, &configured).model,
            "global-model"
        );
    }

    #[test]
    fn unconfigured_workspace_selection_falls_back_to_global() {
        let book = book_with_selection("/tmp/alpha", "unconfigured", "m2");
        let configured = |provider: &str| provider != "unconfigured";
        let resolved = selection_for(&book, Some("/tmp/alpha"), &configured);
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
                },
                AgentSessionInfo {
                    id: "session:s2".into(),
                    profile: "chat".into(),
                    updated_at: "t2".into(),
                    label: String::new(),
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
}
