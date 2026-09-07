//! Phase 25 agent IPC: sessions, inventory, credential intents, and a wire
//! event union that includes unused tool/permission variants for Phase 29.
//!
//! Secrets may travel once on `AgentClientCommand::CredentialPut`. They never
//! appear on `AgentServerMessage`.

use std::fmt;

/// Composer/prompt payload ceiling. Larger prompts fail closed before spawn I/O.
pub const AGENT_MAX_PROMPT_BYTES: usize = 32 * 1024;
/// Server-authoritative transcript projection cap (matches clay-agent load).
pub const AGENT_MAX_SNAPSHOT_ENTRIES: usize = 200;
/// Daemon NDJSON line ceiling; same 1 MiB as the Clay codec / clay-agent.
// Matches the daemon-side frame cap (clay-agent/src/rpc.ts, 64 MiB): a local
// trusted stdio lane carrying Prism tool-result mirrors, whose per-tool
// output ceilings default to 64 MiB.
pub const AGENT_DAEMON_MAX_LINE_BYTES: usize = 64 * 1024 * 1024;
/// One inbound MessageDelta / ThinkingDelta text slice.
pub const AGENT_DELTA_MAX_TEXT_BYTES: usize = 8 * 1024;
/// One retained transcript entry after coalescing deltas.
pub const AGENT_MAX_ENTRY_TEXT_BYTES: usize = AGENT_MAX_PROMPT_BYTES;
/// Sum of retained entry text bytes. Older entries drop first.
pub const AGENT_TRANSCRIPT_SNAPSHOT_BUDGET_BYTES: usize = 256 * 1024;

/// Secret wrapper. `Debug` never prints the value. The bytes still travel on
/// the one-shot put command so Command Centre can reach the vault.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentSecret(pub String);

impl fmt::Debug for AgentSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Observational Memory worker slot (plan 109 I8).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum AgentOmWorkerKind {
    Observation,
    Reflection,
}

/// A worker model binding (`provider/model`), distinct from the session
/// model (decision 2158).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentOmWorkerModel {
    pub provider: String,
    pub model: String,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentPickerKind {
    Provider,
    Model,
    Agent,
    ProviderSetup,
    Session,
    /// Workspace-scoped `session.search` picker (plan 108 task 11): the
    /// query runs against the shared Phase 1 FTS index, not a local filter.
    SessionSearch,
    /// Observational Memory worker model pickers (plan 109 I8): the same
    /// model-listing logic as the session model, retained per workspace
    /// (book) and per session (daemon record metadata).
    OmObservation,
    OmReflection,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentToolPhase {
    Started,
    Progress,
    Finished,
    Error,
    Blocked,
}

/// Compact Prism `AgentEvent` projection. Tool/permission variants stay on the
/// wire so Phase 29 does not rewrite IPC; Chat never emits them.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentWireEvent {
    Started {
        session_id: String,
        run_id: String,
    },
    Finished {
        session_id: String,
        run_id: String,
        usage: String,
        /// Bounded counters (plan 108 task 9): tokens used by the finished
        /// run in the session's current context window, for the status-row
        /// context-used-vs-window readout. Counters only — never content.
        #[serde(default)]
        context_tokens: Option<u64>,
    },
    MessageDelta {
        session_id: String,
        run_id: String,
        text: String,
    },
    ThinkingDelta {
        session_id: String,
        run_id: String,
        text: String,
    },
    Tool {
        session_id: String,
        run_id: String,
        phase: AgentToolPhase,
        name: String,
        tool_call_id: String,
        /// Bounded, redacted argument summary (plan 109 I5). Server-built
        /// from the daemon's `call.arguments`; never trusted from the client.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args_digest: Option<String>,
        /// Bounded, redacted output excerpt / error reason (plan 109 I5).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_digest: Option<String>,
        /// Skill name for `load_skill` rows (plan 109 I5).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skill_name: Option<String>,
    },
    Permission {
        session_id: String,
        run_id: String,
        request_id: String,
        tool_name: String,
        allowed: Option<bool>,
    },
    Overflow,
    Error {
        session_id: String,
        message: String,
    },
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentTranscriptKind {
    User,
    Assistant,
    Thinking,
    Error,
    Usage,
    /// Bounded tool-call rows (plan 109 I5): args summary + output excerpt
    /// in one entry per `tool_call_id`, evolved in place per phase.
    Tool,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentTranscriptEntry {
    pub kind: AgentTranscriptKind,
    pub text: String,
    /// Tool rows only (plan 109 I5): the call id that groups phase events
    /// into one evolving entry. Empty for non-tool entries.
    #[serde(default)]
    pub tool_call_id: String,
    /// `load_skill` rows carry the loaded skill's name (plan 109 I5).
    #[serde(default)]
    pub skill_name: Option<String>,
}

impl AgentTranscriptEntry {
    pub fn new(kind: AgentTranscriptKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            tool_call_id: String::new(),
            skill_name: None,
        }
    }

    pub(crate) fn new_tool(
        text: impl Into<String>,
        tool_call_id: &str,
        skill_name: Option<String>,
    ) -> Self {
        Self {
            kind: AgentTranscriptKind::Tool,
            text: text.into(),
            tool_call_id: tool_call_id.to_string(),
            skill_name,
        }
    }
}

/// Append a wire event to a bounded transcript. Tool/permission variants are
/// ignored so Chat stays crash-free when they appear.
pub fn apply_transcript_event(entries: &mut Vec<AgentTranscriptEntry>, event: &AgentWireEvent) {
    match event {
        AgentWireEvent::MessageDelta { text, .. } => {
            append_delta(entries, AgentTranscriptKind::Assistant, text);
        }
        AgentWireEvent::ThinkingDelta { text, .. } => {
            append_delta(entries, AgentTranscriptKind::Thinking, text);
        }
        AgentWireEvent::Error { message, .. } => {
            push_entry(entries, AgentTranscriptKind::Error, message);
        }
        AgentWireEvent::Finished { usage, .. } => {
            if !usage.is_empty() {
                push_entry(entries, AgentTranscriptKind::Usage, usage);
            }
        }
        AgentWireEvent::Overflow => {
            push_entry(entries, AgentTranscriptKind::Error, "event overflow");
        }
        AgentWireEvent::Tool {
            phase,
            name,
            tool_call_id,
            args_digest,
            output_digest,
            skill_name,
            ..
        } => apply_tool_event(
            entries,
            *phase,
            name,
            tool_call_id,
            args_digest.as_deref(),
            output_digest.as_deref(),
            skill_name.clone(),
        ),
        AgentWireEvent::Started { .. } | AgentWireEvent::Permission { .. } => {}
    }
}

fn append_delta(entries: &mut Vec<AgentTranscriptEntry>, kind: AgentTranscriptKind, text: &str) {
    let text = truncate_bytes(text, AGENT_DELTA_MAX_TEXT_BYTES);
    if text.is_empty() {
        return;
    }
    if let Some(last) = entries.last_mut()
        && last.kind == kind
    {
        last.text.push_str(text);
        truncate_in_place(&mut last.text, AGENT_MAX_ENTRY_TEXT_BYTES);
        cap_snapshot(entries);
        return;
    }
    push_entry(entries, kind, text);
}

fn push_entry(entries: &mut Vec<AgentTranscriptEntry>, kind: AgentTranscriptKind, text: &str) {
    entries.push(AgentTranscriptEntry::new(
        kind,
        truncate_bytes(text, AGENT_MAX_ENTRY_TEXT_BYTES),
    ));
    cap_snapshot(entries);
}

/// Tool rows evolve one entry per `tool_call_id` (plan 109 I5): the started
/// phase records the args summary, terminal phases append the output excerpt
/// in place. Progress rows are transient and carry no content.
fn apply_tool_event(
    entries: &mut Vec<AgentTranscriptEntry>,
    phase: AgentToolPhase,
    name: &str,
    tool_call_id: &str,
    args_digest: Option<&str>,
    output_digest: Option<&str>,
    skill_name: Option<String>,
) {
    if tool_call_id.is_empty() || matches!(phase, AgentToolPhase::Progress) {
        return;
    }
    let existing = entries
        .iter_mut()
        .find(|entry| entry.tool_call_id == tool_call_id);
    let skill = skill_name.filter(|skill| !skill.is_empty());
    let text = match phase {
        AgentToolPhase::Started => match args_digest.filter(|args| !args.is_empty()) {
            Some(args) => format!("{name} {args}"),
            None => format!("{name} - running"),
        },
        AgentToolPhase::Finished | AgentToolPhase::Error | AgentToolPhase::Blocked => {
            let prior = existing
                .as_ref()
                .map(|entry| {
                    entry
                        .text
                        .strip_suffix(" - running")
                        .unwrap_or(&entry.text)
                        .to_string()
                })
                .unwrap_or_else(|| name.to_string());
            let suffix = match output_digest.filter(|output| !output.is_empty()) {
                Some(output) => format!(" -> {output}"),
                None => String::new(),
            };
            format!("{prior}{suffix}")
        }
        AgentToolPhase::Progress => unreachable!("progress rows return above"),
    };
    let text = truncate_bytes(&text, AGENT_MAX_ENTRY_TEXT_BYTES);
    if let Some(entry) = existing {
        entry.text = text.to_string();
        if skill.is_some() {
            entry.skill_name = skill;
        }
    } else {
        entries.push(AgentTranscriptEntry::new_tool(text, tool_call_id, skill));
        cap_snapshot(entries);
    }
}

fn cap_snapshot(entries: &mut Vec<AgentTranscriptEntry>) {
    if entries.len() > AGENT_MAX_SNAPSHOT_ENTRIES {
        let drop = entries.len() - AGENT_MAX_SNAPSHOT_ENTRIES;
        entries.drain(..drop);
    }
    while snapshot_text_bytes(entries) > AGENT_TRANSCRIPT_SNAPSHOT_BUDGET_BYTES
        && !entries.is_empty()
    {
        entries.remove(0);
    }
}

fn snapshot_text_bytes(entries: &[AgentTranscriptEntry]) -> usize {
    entries.iter().map(|entry| entry.text.len()).sum()
}

/// Public bounded-truncation for server-side digest builders (plan 109 I5):
/// same char-boundary-safe cut the transcript uses internally.
pub fn truncate_transcript_text(text: &str, max: usize) -> String {
    truncate_bytes(text, max).to_string()
}

fn truncate_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn truncate_in_place(text: &mut String, max: usize) {
    if text.len() <= max {
        return;
    }
    let keep = truncate_bytes(text, max).len();
    text.truncate(keep);
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionSnapshot {
    pub session_id: String,
    pub profile: String,
    pub provider: String,
    pub model: String,
    pub leaf_id: Option<String>,
    /// Tokens used in the session's current context window (plan 108
    /// task 9): last finished run's bounded counter. `None` until a run
    /// reports usage; counters only, never content.
    #[serde(default)]
    pub context_tokens: Option<u64>,
    pub entries: Vec<AgentTranscriptEntry>,
    /// Server-built MCP allow-list server ids (decision 1758), for the
    /// Coding Agent extension strip chrome. Names only — never commands,
    /// args, or env.
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    /// Declared portable thinking levels for the session's model, ascending
    /// (plan 109 I4). Empty for non-reasoning / undeclared models — no
    /// effort control.
    #[serde(default)]
    pub effort_levels: Vec<String>,
    /// Active thinking level for the session (plan 109 I4): the last level
    /// a prompt carried. `None` until set; the status row shows it.
    #[serde(default)]
    pub effort: Option<String>,
    /// Daemon-registered slash commands (plan 109 R1), the composer
    /// completion's single source. Bounded server-side; empty on snapshots
    /// that predate the first environment fetch (state merges keep the
    /// previous list).
    #[serde(default)]
    pub commands: Vec<AgentSlashCommand>,
    /// Workspace git branch (plan 109 R2): a bounded direct read of the
    /// workspace's `.git` HEAD — no subprocess. Empty = not a repo or not
    /// yet read; the status row renders `—`.
    #[serde(default)]
    pub branch: String,
    /// Active daemon extensions (plan 109 R3): loaded opt-in extensions
    /// (wiki, graft). Empty = none report; the strip omits the segment.
    #[serde(default)]
    pub extensions: Vec<String>,
}

/// One daemon-registered slash command (plan 109 R1): bounded completion
/// data — names and descriptions only, never handlers or args shapes.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentSlashCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

impl AgentSessionSnapshot {
    pub fn apply_event(&mut self, event: &AgentWireEvent) {
        apply_transcript_event(&mut self.entries, event);
    }
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderInfo {
    pub id: String,
    pub configured: bool,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentModelInfo {
    pub provider: String,
    pub model: String,
    pub display_name: String,
    /// Bounded context-window size in tokens when the model registry
    /// reports one (plan 108 task 9): status-row context-used-vs-window.
    #[serde(default)]
    pub context_window: Option<u64>,
    /// Declared portable thinking levels, ascending (plan 109 I4), from the
    /// daemon's Prism registry metadata. Empty = no effort control.
    #[serde(default)]
    pub thinking_levels: Vec<String>,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfileInfo {
    pub name: String,
    pub description: String,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionInfo {
    pub id: String,
    pub profile: String,
    pub updated_at: String,
    /// Plan 109 I9: display label (first user-message summary) for the
    /// workspace-scoped resume list. Empty when the store has no label —
    /// pickers fall back to the profile name.
    #[serde(default)]
    pub label: String,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentInventory {
    pub providers: Vec<AgentProviderInfo>,
    pub models: Vec<AgentModelInfo>,
    pub profiles: Vec<AgentProfileInfo>,
    pub sessions: Vec<AgentSessionInfo>,
    /// Current book selection so a freshly mounted webview learns the
    /// configured provider/model without waiting for a picker event.
    pub provider: String,
    pub model: String,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct AgentPickerItem {
    pub id: String,
    pub label: String,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentClientCommand {
    Prompt {
        session_id: String,
        text: String,
        /// Deferred plan 108 task 9: run-scoped provider/model override from
        /// the core pickers. `None` keeps the session's current provider or
        /// model; the daemon validates, remembers, and persists the switch.
        #[serde(default)]
        provider: Option<String>,
        #[serde(default)]
        model: Option<String>,
        /// Plan 109 I4: portable thinking level for this run. `None` keeps
        /// the session's current effort; the daemon fail-closes invalid
        /// strings at its boundary (Prism `parseThinkingLevel`).
        #[serde(default)]
        thinking_level: Option<String>,
    },
    Cancel {
        session_id: String,
    },
    Steer {
        session_id: String,
        text: String,
        soft_interrupt: bool,
    },
    NewSession {
        profile: String,
        provider: String,
        model: String,
        /// Deferred Phase 1 passthrough: daemon-side workspace/autonomy
        /// parameters. `None` keeps the daemon defaults (process cwd root,
        /// autonomy off).
        workspace_root: Option<String>,
        full_autonomy: Option<bool>,
        /// Observational Memory worker model defaults for the new session
        /// (plan 109 I8): the workspace book's last selection, `None` when
        /// unset (workers stay off via requireExplicitModel).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        om_observation: Option<AgentOmWorkerModel>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        om_reflection: Option<AgentOmWorkerModel>,
    },
    LoadSession {
        session_id: String,
        /// Open the transcript at a specific tree entry (search-result
        /// opens, plan 108 task 11). `None` loads the default tail.
        entry_id: Option<String>,
    },
    /// Plan 109 I7: live context inspector. `item_id: None` fetches the
    /// categorized list (bounded, redacted); `Some(id)` fetches one item's
    /// full redacted content for the drawer detail.
    Context {
        session_id: String,
        #[serde(default)]
        item_id: Option<String>,
    },
    ResumeSession {
        session_id: String,
    },
    DeleteSession {
        session_id: String,
    },
    ListSessions,
    OpenPicker {
        kind: AgentPickerKind,
    },
    Select {
        kind: AgentPickerKind,
        id: String,
    },
    /// Plan 109 I8: OM worker model selection (Observation/Reflection).
    /// Applied through the same book path as `Select` (per-workspace
    /// retention) plus the daemon's per-session `session.om.set`.
    SelectWorker {
        worker: AgentOmWorkerKind,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    /// Plan 109 I8: the Observational Memory tab's read model — worker
    /// selection + bounded observer activity log (drops included).
    OmActivity {
        session_id: String,
    },
    /// Plan 109 I8: apply one worker's model binding to a session (the
    /// daemon-side half of `SelectWorker`; the book half is server-side).
    SetOmWorkers {
        session_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        observation: Option<AgentOmWorkerModel>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reflection: Option<AgentOmWorkerModel>,
    },
    CredentialPut {
        provider: String,
        name: String,
        secret: AgentSecret,
    },
    CredentialDelete {
        provider: String,
        name: String,
    },
    RegisterProfile {
        name: String,
        description: String,
        instructions: String,
    },
    Compact {
        session_id: String,
        strategy: Option<String>,
    },
    SetAutonomy {
        session_id: String,
        enabled: bool,
    },
    SearchSessions {
        session_id: String,
        query: Option<String>,
        limit: Option<u32>,
    },
    RunResume {
        session_id: String,
        run_id: String,
        /// JSON-encoded resume decision (`{"kind":"approve"}` or a decision
        /// batch). A string keeps the rkyv wire derivation trivial; the daemon
        /// parses and validates it fail-closed.
        decision_json: String,
    },
    SessionTree {
        session_id: String,
        method: String,
        entry_id: String,
    },
    /// Forwarded to daemon `skill.register`. Typed fields; the daemon
    /// validates fail-closed (duplicate names error).
    SkillRegister {
        name: String,
        description: Option<String>,
        instructions: Option<String>,
        tool_names: Vec<String>,
    },
    /// Forwarded to daemon `command.register`. `handler` names a host-side
    /// command driver (e.g. "steer"); the daemon rejects unknown names.
    CommandRegister {
        name: String,
        handler: Option<String>,
        description: Option<String>,
    },
    /// Forwarded to daemon `command.dispatch`. Args are an arbitrary JSON
    /// object (JSON string keeps the rkyv derivation trivial, same as
    /// `RunResume`); the daemon validates fail-closed.
    CommandDispatch {
        name: String,
        session_id: Option<String>,
        args_json: Option<String>,
    },
    /// Resolves a pending `approval.request` mutation gate. Missing/stale
    /// request ids fail closed with a diagnostic.
    ApprovalResolve {
        request_id: String,
        allowed: bool,
    },
    /// Resolves a pending `approval.askUserDecision` request. The answer is
    /// a JSON object (`selectedId`/`selectedIds`/`customText` XOR union);
    /// the daemon validates the shape fail-closed.
    AskDecisionResolve {
        request_id: String,
        answer_json: String,
    },
}

/// Which user-approval surface a daemon-initiated request needs.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalRequestKind {
    /// `approval.request`: allow/deny a gated mutation (out-of-root write,
    /// shell metacharacter, host write outside the workspace).
    Mutation,
    /// `approval.askUserDecision`: the `ask_user_decision` tool waiting for
    /// a structured answer.
    AskDecision,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentServerMessage {
    Snapshot(AgentSessionSnapshot),
    Event {
        session_id: String,
        event: AgentWireEvent,
    },
    Inventory(AgentInventory),
    Picker {
        kind: AgentPickerKind,
        items: Vec<AgentPickerItem>,
    },
    CredentialAck {
        provider: String,
        name: String,
        stored: bool,
    },
    /// Generic daemon RPC result projection (session.search, run.resume).
    /// `result_json` is daemon-produced JSON already redacted daemon-side; the
    /// server treats it as opaque and never logs it.
    AgentRpc {
        code: String,
        result_json: String,
    },
    /// Daemon-initiated user-approval request awaiting an
    /// `ApprovalResolve`/`AskDecisionResolve` answer. `payload_json` is
    /// daemon-produced and already bounded/validated daemon-side; the server
    /// treats it as opaque and never logs it.
    ApprovalRequest {
        request_id: String,
        kind: ApprovalRequestKind,
        payload_json: String,
    },
    Diagnostic {
        code: String,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_append_same_kind_and_ignore_tools() {
        let mut entries = Vec::new();
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::MessageDelta {
                session_id: "s".into(),
                run_id: "r".into(),
                text: "Hel".into(),
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::MessageDelta {
                session_id: "s".into(),
                run_id: "r".into(),
                text: "lo".into(),
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "read".into(),
                tool_call_id: "t".into(),
                args_digest: None,
                output_digest: None,
                skill_name: None,
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Permission {
                session_id: "s".into(),
                run_id: "r".into(),
                request_id: "p".into(),
                tool_name: "write".into(),
                allowed: None,
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Finished {
                session_id: "s".into(),
                run_id: "r".into(),
                usage: "1 token".into(),
                context_tokens: None,
            },
        );
        // Plan 109 I5: the started tool phase upserts a bounded tool row;
        // the permission event stays invisible.
        assert_eq!(
            entries[0],
            AgentTranscriptEntry::new(AgentTranscriptKind::Assistant, "Hello")
        );
        assert_eq!(entries[1].kind, AgentTranscriptKind::Tool);
        assert_eq!(entries[1].tool_call_id, "t");
        assert_eq!(entries[1].text, "read - running");
        assert_eq!(
            entries[2],
            AgentTranscriptEntry::new(AgentTranscriptKind::Usage, "1 token")
        );
    }

    #[test]
    fn tool_rows_evolve_one_entry_per_call_with_bounded_digests() {
        // Plan 109 I5: started -> finished evolves the row in place (args
        // kept, output excerpt appended); progress rows are transient.
        let mut entries = Vec::new();
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "read".into(),
                tool_call_id: "t1".into(),
                args_digest: Some(String::from(r#"{"path":"src/main.rs"}"#)),
                output_digest: None,
                skill_name: None,
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Progress,
                name: "read".into(),
                tool_call_id: "t1".into(),
                args_digest: None,
                output_digest: None,
                skill_name: None,
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Finished,
                name: "read".into(),
                tool_call_id: "t1".into(),
                args_digest: None,
                output_digest: Some("fn main()".into()),
                skill_name: None,
            },
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, AgentTranscriptKind::Tool);
        assert_eq!(entries[0].tool_call_id, "t1");
        assert_eq!(
            entries[0].text,
            r#"read {"path":"src/main.rs"} -> fn main()"#
        );

        // Error rows carry the failure reason on the same evolving row.
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "write".into(),
                tool_call_id: "t2".into(),
                args_digest: Some(String::from(r#"{"path":"out.txt"}"#)),
                output_digest: None,
                skill_name: None,
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Error,
                name: "write".into(),
                tool_call_id: "t2".into(),
                args_digest: None,
                output_digest: Some("permission denied".into()),
                skill_name: None,
            },
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[1].text,
            r#"write {"path":"out.txt"} -> permission denied"#
        );

        // Skill rows carry the skill name.
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "load_skill".into(),
                tool_call_id: "t3".into(),
                args_digest: Some(String::from(r#"{"name":"rust-review"}"#)),
                output_digest: None,
                skill_name: Some("rust-review".into()),
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Finished,
                name: "load_skill".into(),
                tool_call_id: "t3".into(),
                args_digest: None,
                output_digest: Some("Loaded skill".into()),
                skill_name: None,
            },
        );
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[2].skill_name.as_deref(), Some("rust-review"));
        assert_eq!(
            entries[2].text,
            r#"load_skill {"name":"rust-review"} -> Loaded skill"#
        );

        // Oversized digests stay within the per-entry budget.
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "read".into(),
                tool_call_id: "t4".into(),
                args_digest: Some("x".repeat(AGENT_MAX_ENTRY_TEXT_BYTES + 64)),
                output_digest: None,
                skill_name: None,
            },
        );
        assert!(entries[3].text.len() <= AGENT_MAX_ENTRY_TEXT_BYTES);

        // Empty tool call ids never produce rows.
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "read".into(),
                tool_call_id: String::new(),
                args_digest: Some(String::from(r#"{} "#)),
                output_digest: None,
                skill_name: None,
            },
        );
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn cancelled_flag_is_caller_owned_and_error_appends() {
        let mut entries = Vec::new();
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Error {
                session_id: "s".into(),
                message: "cancelled".into(),
            },
        );
        assert_eq!(entries[0].kind, AgentTranscriptKind::Error);
    }

    #[test]
    fn transcript_caps_delta_entry_and_snapshot_bytes() {
        let mut entries = Vec::new();
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::MessageDelta {
                session_id: "s".into(),
                run_id: "r".into(),
                text: "x".repeat(AGENT_DELTA_MAX_TEXT_BYTES + 32),
            },
        );
        assert_eq!(entries[0].text.len(), AGENT_DELTA_MAX_TEXT_BYTES);

        for _ in 0..(AGENT_MAX_ENTRY_TEXT_BYTES / AGENT_DELTA_MAX_TEXT_BYTES) {
            apply_transcript_event(
                &mut entries,
                &AgentWireEvent::MessageDelta {
                    session_id: "s".into(),
                    run_id: "r".into(),
                    text: "y".repeat(AGENT_DELTA_MAX_TEXT_BYTES),
                },
            );
        }
        assert_eq!(entries[0].text.len(), AGENT_MAX_ENTRY_TEXT_BYTES);

        for index in 0..40 {
            apply_transcript_event(
                &mut entries,
                &AgentWireEvent::Error {
                    session_id: "s".into(),
                    message: format!("{index}-{}", "z".repeat(8 * 1024)),
                },
            );
        }
        let bytes: usize = entries.iter().map(|entry| entry.text.len()).sum();
        assert!(bytes <= AGENT_TRANSCRIPT_SNAPSHOT_BUDGET_BYTES);
        assert!(entries.len() <= AGENT_MAX_SNAPSHOT_ENTRIES);
    }
}
