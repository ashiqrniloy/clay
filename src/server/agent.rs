//! Core-owned `clay-agent` child. One daemon per Clay server.
//!
//! Package JavaScript never receives this type. Spawn is `Command` +
//! `env_clear`, never a shell string. Node missing is a diagnostic, not a hang.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
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
    AgentWireEvent, TabId, apply_transcript_event,
};

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(5);
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const EVENT_CAPACITY: usize = 256;

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
    transcripts: HashMap<String, Vec<AgentTranscriptEntry>>,
    running: HashSet<String>,
    cancelled: HashSet<String>,
}

struct Inner {
    config: AgentHostConfig,
    events: broadcast::Sender<Arc<AgentServerMessage>>,
    // ponytail: one mutex for the child; per-session queues if prompt throughput matters
    state: Mutex<Option<Running>>,
    secrets: Arc<Mutex<Vec<String>>>,
    book: Arc<Mutex<SessionBook>>,
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

impl std::fmt::Debug for AgentHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AgentHost")
    }
}

impl AgentHost {
    pub fn new(config: AgentHostConfig) -> Self {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                config,
                events,
                state: Mutex::new(None),
                secrets: Arc::new(Mutex::new(Vec::new())),
                book: Arc::new(Mutex::new(SessionBook::default())),
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
        })
    }

    pub fn for_server(configuration_root: Option<&Path>) -> Self {
        Self::new(AgentHostConfig::for_server(configuration_root))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<AgentServerMessage>> {
        self.inner.events.subscribe()
    }

    pub fn dispatch(&self, command: AgentClientCommand) {
        let host = self.clone();
        tokio::spawn(async move {
            let message = host.run(command).await;
            let _ = host.inner.events.send(Arc::new(message));
        });
    }

    pub(crate) async fn select_picker(&self, kind: AgentPickerKind, id: &str) {
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

    pub async fn begin_prompt(&self, tab: TabId, text: &str) -> AgentServerMessage {
        if text.trim().is_empty() {
            return diagnostic("agent.empty_prompt", "empty prompt");
        }
        if text.len() > AGENT_MAX_PROMPT_BYTES {
            return diagnostic(
                "agent.prompt_too_large",
                "prompt exceeds AGENT_MAX_PROMPT_BYTES",
            );
        }
        let Some(session_id) = self.ensure_tab_session(tab).await else {
            return AgentServerMessage::Snapshot(self.unconfigured_snapshot().await);
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
        self.dispatch(AgentClientCommand::Prompt {
            session_id,
            text: text.to_string(),
        });
        AgentServerMessage::Snapshot(snapshot)
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

    pub async fn resume_tab(&self, tab: TabId, session_id: &str) -> AgentServerMessage {
        self.inner
            .book
            .lock()
            .await
            .tab_session
            .insert(tab, session_id.to_string());
        let loaded = self
            .run(AgentClientCommand::LoadSession {
                session_id: session_id.to_string(),
            })
            .await;
        if let AgentServerMessage::Snapshot(snapshot) = &loaded {
            let mut book = self.inner.book.lock().await;
            book.transcripts
                .insert(session_id.to_string(), snapshot.entries.clone());
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

    async fn ensure_tab_session(&self, tab: TabId) -> Option<String> {
        {
            let book = self.inner.book.lock().await;
            if let Some(session_id) = book.tab_session.get(&tab) {
                return Some(session_id.clone());
            }
            if book.provider.is_empty() || book.model.is_empty() {
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
            })
            .await;
        let AgentServerMessage::Snapshot(snapshot) = created else {
            return None;
        };
        if snapshot.session_id.is_empty() {
            return None;
        }
        let mut book = self.inner.book.lock().await;
        book.tab_session.insert(tab, snapshot.session_id.clone());
        book.transcripts
            .entry(snapshot.session_id.clone())
            .or_default();
        Some(snapshot.session_id)
    }

    async fn snapshot_for(&self, session_id: &str) -> AgentSessionSnapshot {
        let book = self.inner.book.lock().await;
        AgentSessionSnapshot {
            session_id: session_id.to_string(),
            profile: book.profile.clone(),
            provider: book.provider.clone(),
            model: book.model.clone(),
            leaf_id: None,
            entries: book
                .transcripts
                .get(session_id)
                .cloned()
                .unwrap_or_default(),
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
            entries: Vec::new(),
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
        let _ = self
            .run(AgentClientCommand::CredentialPut {
                provider: provider.to_string(),
                name: name.to_string(),
                secret: AgentSecret(secret.to_string()),
            })
            .await;
        Ok(())
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
            AgentClientCommand::Prompt { session_id, text } => {
                if text.len() > AGENT_MAX_PROMPT_BYTES {
                    return Ok(diagnostic(
                        "agent.prompt_too_large",
                        "prompt exceeds AGENT_MAX_PROMPT_BYTES",
                    ));
                }
                self.rpc(
                    "session.prompt",
                    json!({ "sessionId": session_id, "text": text }),
                )
                .await?;
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
            } => {
                let result = self
                    .rpc(
                        "session.new",
                        json!({ "profile": profile, "provider": provider, "model": model }),
                    )
                    .await?;
                Ok(AgentServerMessage::Snapshot(snapshot_from_new(&result)))
            }
            AgentClientCommand::LoadSession { session_id } => {
                let result = self
                    .rpc("session.load", json!({ "sessionId": session_id }))
                    .await?;
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

    async fn rpc(&self, method: &str, params: Value) -> Result<Value, AgentError> {
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
        let stdin = child.stdin.take().ok_or(AgentError::MissingPipe)?;
        let stdout = child.stdout.take().ok_or(AgentError::MissingPipe)?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(drain_stderr(stderr));
        }

        let (commands_tx, commands_rx) = mpsc::channel(32);
        let events = self.inner.events.clone();
        let secrets = Arc::clone(&self.inner.secrets);
        let book = Arc::clone(&self.inner.book);
        tokio::spawn(daemon_actor(
            child,
            stdin,
            stdout,
            commands_rx,
            events,
            secrets,
            book,
        ));

        let running = Running {
            commands: commands_tx,
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        running
            .commands
            .send(HostCommand::Rpc {
                method: "initialize".to_string(),
                params: json!({ "passphrase": passphrase }),
                reply: reply_tx,
            })
            .await
            .map_err(|_| AgentError::ServiceStopped)?;
        match timeout(INITIALIZE_TIMEOUT, reply_rx).await {
            Ok(Ok(Ok(_))) => Ok(running),
            Ok(Ok(Err(error))) => Err(error),
            Ok(Err(_)) => Err(AgentError::ChildExited),
            Err(_) => Err(AgentError::Timeout),
        }
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
    mut stdin: ChildStdin,
    stdout: tokio::process::ChildStdout,
    mut commands: mpsc::Receiver<HostCommand>,
    events: broadcast::Sender<Arc<AgentServerMessage>>,
    secrets: Arc<Mutex<Vec<String>>>,
    book: Arc<Mutex<SessionBook>>,
) {
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
                        if write_frame(&mut stdin, &frame).await.is_err() {
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
                        let _ = write_frame(&mut stdin, &frame).await;
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
                        fail_pending(&mut pending, AgentError::FrameTooLarge { len: buf.len() });
                        break;
                    }
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let secrets_now = secrets.lock().await.clone();
                        handle_daemon_line(
                            line.trim(),
                            &mut pending,
                            &events,
                            &secrets_now,
                            &book,
                        )
                        .await;
                    }
                    Err(_) => {
                        fail_pending(&mut pending, AgentError::ChildExited);
                        break;
                    }
                }
            }
        }
    }
    let _ = child.kill().await;
    fail_pending(&mut pending, AgentError::ChildExited);
}

async fn handle_daemon_line(
    line: &str,
    pending: &mut HashMap<u64, oneshot::Sender<Result<Value, AgentError>>>,
    events: &broadcast::Sender<Arc<AgentServerMessage>>,
    secrets: &[String],
    book: &Mutex<SessionBook>,
) {
    if line.is_empty() {
        return;
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if value.get("method").and_then(Value::as_str) == Some("event") {
        if let Some(message) = map_event(value.get("params").unwrap_or(&Value::Null), secrets) {
            apply_book_event(&mut *book.lock().await, &message);
            let _ = events.send(Arc::new(message));
        }
        return;
    }
    let Some(id) = value.get("id").and_then(Value::as_u64) else {
        return;
    };
    let Some(reply) = pending.remove(&id) else {
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
        "error" => AgentWireEvent::Error {
            session_id: session_id.clone(),
            message: redact_text(
                event
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("error"),
                secrets,
            ),
        },
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
        entries: Vec::new(),
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
        AgentWireEvent::Finished { .. } | AgentWireEvent::Error { .. } => {
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
    }
}

fn diagnostic(code: &str, message: &str) -> AgentServerMessage {
    AgentServerMessage::Diagnostic {
        code: code.to_string(),
        message: message.to_string(),
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
