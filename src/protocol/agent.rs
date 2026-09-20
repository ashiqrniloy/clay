//! Phase 25 agent IPC: sessions, inventory, credential intents, and a wire
//! event union that includes unused tool/permission variants for Phase 29.
//!
//! Secrets may travel once on `AgentClientCommand::CredentialPut`. They never
//! appear on `AgentServerMessage`.

use std::fmt;

use serde_json::Value;

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
    /// Last provider round's context occupancy (plan 117 token meter):
    /// prompt tokens (input + cache reads + writes) of the most recent
    /// `provider_turn_finished`. Counters only — never content.
    ContextTokens {
        session_id: String,
        run_id: String,
        tokens: u64,
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
        /// Plan 118 task 36: the file this call touches, when the tool names
        /// one (`read`/`write`/`edit`/`delete`). Server-built from the
        /// daemon's arguments, so the transcript — and the Files tab
        /// projected from it — records a path the session really handled.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<AgentTranscriptFile>,
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

/// What a tool call did to the file it named (plan 118 task 36). The verb,
/// not the view's role label: `write` reads as `created` only once the
/// session's own history says it had not seen the path — a judgement the
/// view makes, since only it sees the whole session.
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
pub enum AgentTranscriptFileOp {
    Read,
    Write,
    Edit,
    Delete,
}

impl AgentTranscriptFileOp {
    /// The file verb a coding tool maps to (`read`/`write`/`edit`/`delete`).
    /// Every other tool names no single file (search, shell, git, move), so it
    /// records no session file.
    pub fn for_tool(tool: &str) -> Option<Self> {
        match tool {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            "edit" => Some(Self::Edit),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

/// One file a session touched, as its tool call named it.
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
pub struct AgentTranscriptFile {
    pub path: String,
    pub op: AgentTranscriptFileOp,
}

/// The file a tool call touches: its verb plus the call's `path` argument.
/// `None` for tools that name no single file, or a call whose arguments carry
/// no path.
pub fn transcript_file_for_tool(tool: &str, arguments: &Value) -> Option<AgentTranscriptFile> {
    let op = AgentTranscriptFileOp::for_tool(tool)?;
    let path = arguments.get("path").and_then(Value::as_str)?.trim();
    (!path.is_empty()).then(|| AgentTranscriptFile {
        path: path.to_string(),
        op,
    })
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
    /// Plan 118 task 35: the agent type that produced this entry. One tab can
    /// change its agent (the header picker), so turns carry their producer;
    /// `None` on rows written before the field existed (or by a session with
    /// no agent), which the view renders as unlabelled.
    #[serde(default)]
    pub agent: Option<String>,
    /// Plan 118 task 36: the file this tool row touched. The Files tab is the
    /// transcript projected to its file records — same rows, same bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<AgentTranscriptFile>,
}

impl AgentTranscriptEntry {
    pub fn new(kind: AgentTranscriptKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            tool_call_id: String::new(),
            skill_name: None,
            agent: None,
            file: None,
        }
    }

    /// Stamp the entry with the agent type that produced it (`None` leaves it
    /// unlabelled). Every append path runs through here so a switch cannot
    /// leave turns attributed to the wrong agent.
    pub fn with_agent(mut self, agent: Option<&str>) -> Self {
        self.agent = agent
            .map(str::trim)
            .filter(|agent| !agent.is_empty())
            .map(str::to_string);
        self
    }

    pub(crate) fn new_tool(
        text: impl Into<String>,
        tool_call_id: &str,
        skill_name: Option<String>,
        file: Option<AgentTranscriptFile>,
    ) -> Self {
        Self {
            kind: AgentTranscriptKind::Tool,
            text: text.into(),
            tool_call_id: tool_call_id.to_string(),
            skill_name,
            agent: None,
            file,
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
        // Meter numerator only; no transcript row (the usage box rides
        // `Finished`).
        AgentWireEvent::ContextTokens { .. } => {}
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
            file,
            ..
        } => apply_tool_event(
            entries,
            *phase,
            name,
            tool_call_id,
            args_digest.as_deref(),
            output_digest.as_deref(),
            skill_name.clone(),
            file.clone(),
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
#[allow(clippy::too_many_arguments)]
fn apply_tool_event(
    entries: &mut Vec<AgentTranscriptEntry>,
    phase: AgentToolPhase,
    name: &str,
    tool_call_id: &str,
    args_digest: Option<&str>,
    output_digest: Option<&str>,
    skill_name: Option<String>,
    file: Option<AgentTranscriptFile>,
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
        if file.is_some() {
            entry.file = file;
        }
    } else {
        entries.push(AgentTranscriptEntry::new_tool(
            text,
            tool_call_id,
            skill,
            file,
        ));
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
    /// Plan 118 task 35: the agent type this session runs as (the directory
    /// name of its per-agent config root). `None` for a session created
    /// before agent types existed, or by a host with no agent roots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Tokens used in the session's current context window (plan 108
    /// task 9): last finished run's bounded counter. `None` until a run
    /// reports usage; counters only, never content.
    #[serde(default)]
    pub context_tokens: Option<u64>,
    pub entries: Vec<AgentTranscriptEntry>,
    /// Per-server MCP connect outcomes from the daemon's environment
    /// (plan 117): connected servers + tool counts + hidden-because errors.
    /// Empty on snapshots predating the first environment fetch (state
    /// merges keep the list).
    #[serde(default)]
    pub mcp_servers: Vec<AgentMcpServerInfo>,
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
    /// Catalog skills (disk-discovered + registered): bounded name and
    /// description pairs for the pinned skills card. Empty on snapshots
    /// predating the first environment fetch (state merges keep the list).
    #[serde(default)]
    pub skills: Vec<AgentSkillInfo>,
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

/// One catalog skill (disk-discovered or registered): bounded name +
/// description for the skills card on the coding surface — never
/// instructions, toolNames, or metadata.
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
pub struct AgentSkillInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// Per-server MCP connect outcome (plan 117): what the daemon actually
/// connected, for the MCP card and composer section. Carries ids, counts,
/// and hidden-because errors — never commands, args, or env.
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
pub struct AgentMcpServerInfo {
    pub server_id: String,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub tools: u32,
    #[serde(default)]
    pub error: String,
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
    /// Plan 117 follow-up: `updated_at` rendered in the daemon's local time
    /// (`YYYY-MM-DD HH:MM`) for the resume list's second line. Empty when the
    /// daemon sent none — callers fall back to the raw ISO stamp.
    #[serde(default)]
    pub updated_at_label: String,
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
        /// autonomy on — a session is autonomous unless this says `false`;
        /// decision 2026-09-20-2049).
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
    /// Plan 117: the panel's recent-sessions list — the tab's
    /// workspace-scoped, labeled resumable page. The root comes from the
    /// server's tab registry (never webview input); the result rides the
    /// generic `session.resumable` agent-RPC custom event.
    ResumableSessions,
    /// Plan 117 follow-up: the coding-agent pane's mount STATE. Nothing else
    /// emits a snapshot before the first prompt, so the git branch, skills
    /// card, and MCP card stayed empty on a freshly opened surface. Tab-
    /// resolved server-side (the workspace root comes from the tab registry,
    /// never webview input); the reply rides the normal agent broadcast as
    /// `AgentServerMessage::Snapshot` (the view's relay applies it to STATE).
    TabState,
    /// Plan 117 @-mentions: bounded workspace file listing for the composer
    /// dropdown (server-side walk inside the session's workspace root).
    WorkspaceFiles {
        session_id: String,
    },
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
                file: None,
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
    fn tool_rows_record_the_file_the_call_touched() {
        // Plan 118 task 36: the Files tab projects the transcript, so the
        // path and verb are recorded on the row the call produced — and the
        // phases that carry no arguments leave the record alone.
        let mut entries = Vec::new();
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "write".into(),
                tool_call_id: "t1".into(),
                args_digest: Some(r#"{"path":"DESIGN.md","content":"x"}"#.into()),
                output_digest: None,
                skill_name: None,
                file: Some(AgentTranscriptFile {
                    path: "DESIGN.md".into(),
                    op: AgentTranscriptFileOp::Write,
                }),
            },
        );
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Finished,
                name: "write".into(),
                tool_call_id: "t1".into(),
                args_digest: None,
                output_digest: Some("wrote 1 byte".into()),
                skill_name: None,
                file: None,
            },
        );
        assert_eq!(entries.len(), 1);
        let file = entries[0].file.as_ref().expect("file record");
        assert_eq!(file.path, "DESIGN.md");
        assert_eq!(file.op, AgentTranscriptFileOp::Write);

        // A tool call that names no file keeps the row clean.
        apply_transcript_event(
            &mut entries,
            &AgentWireEvent::Tool {
                session_id: "s".into(),
                run_id: "r".into(),
                phase: AgentToolPhase::Started,
                name: "repo_search".into(),
                tool_call_id: "t2".into(),
                args_digest: Some(r#"{"query":"x"}"#.into()),
                output_digest: None,
                skill_name: None,
                file: None,
            },
        );
        assert!(entries[1].file.is_none());
    }

    #[test]
    fn tool_file_verbs_map_and_ignore_tools_without_one_file() {
        let args = |json: &str| serde_json::from_str::<Value>(json).expect("args");
        for (tool, op) in [
            ("read", AgentTranscriptFileOp::Read),
            ("write", AgentTranscriptFileOp::Write),
            ("edit", AgentTranscriptFileOp::Edit),
            ("delete", AgentTranscriptFileOp::Delete),
        ] {
            let file = transcript_file_for_tool(tool, &args(r#"{"path":"src/main.rs"}"#))
                .unwrap_or_else(|| panic!("{tool} names a file"));
            assert_eq!(file.path, "src/main.rs");
            assert_eq!(file.op, op);
        }
        // Tools that name no single file, and calls with no path, record none.
        assert!(transcript_file_for_tool("repo_search", &args(r#"{"query":"x"}"#)).is_none());
        assert!(transcript_file_for_tool("shell", &args(r#"{"command":"ls"}"#)).is_none());
        assert!(transcript_file_for_tool("read", &args(r#"{"path":"  "}"#)).is_none());
        assert!(transcript_file_for_tool("read", &args(r#"{"path":7}"#)).is_none());
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
                file: None,
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
                file: None,
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
                file: None,
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
                file: None,
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
                file: None,
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
                file: None,
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
                file: None,
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
                file: None,
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
                file: None,
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
