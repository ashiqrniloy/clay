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
    AGENT_DAEMON_MAX_LINE_BYTES, AGENT_MAX_PROMPT_BYTES, AGENT_MAX_SNAPSHOT_ENTRIES,
    AgentClientCommand, AgentInventory, AgentModelInfo, AgentPickerItem, AgentPickerKind,
    AgentProfileInfo, AgentProviderInfo, AgentSecret, AgentServerMessage, AgentSessionInfo,
    AgentSessionSnapshot, AgentToolPhase, AgentTranscriptEntry, AgentTranscriptKind,
    AgentWireEvent, ApprovalRequestKind, TabId, apply_transcript_event,
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
    tab_session: HashMap<TabId, String>,
    /// Last finished run's context-token counter per session (plan 108
    /// task 9): the snapshot's context-used-vs-window numerator.
    context_tokens: HashMap<String, Option<u64>>,

    transcripts: HashMap<String, Vec<AgentTranscriptEntry>>,
    running: HashSet<String>,
    cancelled: HashSet<String>,
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
    /// Pending daemon-initiated approval requests, keyed by request id.
    /// Resolved by `ApprovalResolve`/`AskDecisionResolve`; dropped senders
    /// and timeouts deny fail-closed.
    approvals: Arc<Mutex<PendingApprovals>>,
    approval_seq: AtomicU64,
    /// Registration RPCs queued while the daemon is not yet running
    /// (package load entries must never spawn or block on the daemon).
    /// Drained in order right after the initialize handshake succeeds.
    pending_registrations: Arc<Mutex<Vec<HostCommand>>>,
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

    pub(crate) async fn select_picker(&self, kind: AgentPickerKind, id: &str) {
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
        self.persist_book_selection().await;
        self.publish_book_snapshot().await;
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
        }
        let snapshot = self.snapshot_for(&session_id).await;
        let (provider, model) = {
            let book = self.inner.book.lock().await;
            (book.provider.clone(), book.model.clone())
        };
        self.dispatch(AgentClientCommand::Prompt {
            session_id,
            text: text.to_string(),
            // Run-scoped book override (plan 108 task 9): a picker switch
            // between runs applies at the next prompt without a new session.
            provider: Some(provider),
            model: Some(model),
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
        self.dispatch(AgentClientCommand::Steer {
            session_id,
            text: text.to_string(),
            soft_interrupt: false,
        });
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
        {
            let book = self.inner.book.lock().await;
            if let Some(session_id) = book.tab_session.get(&tab) {
                return Some(session_id.clone());
            }
            if book.provider.is_empty() || book.model.is_empty() {
                eprintln!(
                    "[agent] ensure_tab_session({tab:?}): empty book (provider='{}' model='{}')",
                    book.provider, book.model
                );
                return None;
            }
        }
        let (profile, provider, model) = {
            let book = self.inner.book.lock().await;
            (
                if book.profile.is_empty() {
                    "Chat".to_string()
                } else {
                    book.profile.clone()
                },
                book.provider.clone(),
                book.model.clone(),
            )
        };
        let created = self
            .run(AgentClientCommand::NewSession {
                profile,
                provider,
                model,
                workspace_root: None,
                full_autonomy: None,
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
        }
    }

    async fn unconfigured_snapshot(&self) -> AgentSessionSnapshot {
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
        }
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
                let result = self.rpc("session.new", Value::Object(params)).await?;
                Ok(AgentServerMessage::Snapshot(snapshot_from_new(&result)))
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
                Ok(AgentServerMessage::Snapshot(snapshot_from_load(&result)))
            }
            AgentClientCommand::ResumeSession { session_id } => {
                let result = self
                    .rpc("session.resume", json!({ "sessionId": session_id }))
                    .await?;
                Ok(AgentServerMessage::Snapshot(snapshot_from_new(&result)))
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
        Ok(AgentPickerInventory {
            providers: parse_picker_providers(&providers),
            models: parse_models(&models),
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
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AgentError::ServiceStopped),
            Err(_) => Err(AgentError::Timeout),
        }
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
        tokio::spawn(daemon_actor(
            child,
            stdout,
            commands_rx,
            events,
            secrets,
            book,
            reverse,
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

async fn daemon_actor(
    mut child: Child,
    stdout: tokio::process::ChildStdout,
    mut commands: mpsc::Receiver<HostCommand>,
    events: broadcast::Sender<Arc<AgentServerMessage>>,
    secrets: Arc<Mutex<Vec<String>>>,
    book: Arc<Mutex<SessionBook>>,
    reverse: Option<ReverseRpcHandler>,
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
                                {
                                    let mut book_guard = book.lock().await;
                                    apply_book_event(&mut book_guard, &message);
                                }
                                let _ = events.send(Arc::new(message));
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
        },
        "tool_execution_progress" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Progress,
            name: json_string(event, &["name"]),
            tool_call_id: json_string(event, &["toolCallId"]),
        },
        "tool_execution_finished" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Finished,
            name: json_string(event, &["result", "name"]),
            tool_call_id: json_string(event, &["result", "toolCallId"]),
        },
        "tool_execution_error" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Error,
            name: json_string(event, &["call", "name"]),
            tool_call_id: json_string(event, &["call", "id"]),
        },
        "tool_execution_blocked" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Blocked,
            name: json_string(event, &["name"]),
            tool_call_id: json_string(event, &["toolCallId"]),
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
                    Some(AgentTranscriptEntry {
                        kind: transcript_kind(entry.get("role").and_then(Value::as_str)),
                        text,
                    })
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
            if let Some(field) = value.get("provider").and_then(Value::as_str) {
                book.provider = field.to_string();
            }
            if let Some(field) = value.get("model").and_then(Value::as_str) {
                book.model = field.to_string();
            }
            if let Some(field) = value.get("profile").and_then(Value::as_str) {
                book.profile = field.to_string();
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
        "provider": book.provider,
        "model": book.model,
        "profile": book.profile,
    });
    if let Err(error) = std::fs::write(book_path(data_dir), payload.to_string()) {
        eprintln!("[agent] book.json write failed: {error}");
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
        }
        AgentWireEvent::Error { .. } => {
            book.running.remove(session_id);
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
            })
        })
        .collect()
}

fn picker_items(kind: AgentPickerKind, inventory: &AgentInventory) -> Vec<AgentPickerItem> {
    match kind {
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
                label: session.profile.clone(),
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
        host.select_picker(AgentPickerKind::Model, "model:ollama/glm-5.3")
            .await;
        host.select_picker(AgentPickerKind::Agent, "agent:coding")
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
}
