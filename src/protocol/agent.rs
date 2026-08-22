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
pub const AGENT_DAEMON_MAX_LINE_BYTES: usize = 1024 * 1024;
/// One inbound MessageDelta / ThinkingDelta text slice.
pub const AGENT_DELTA_MAX_TEXT_BYTES: usize = 8 * 1024;
/// One retained transcript entry after coalescing deltas.
pub const AGENT_MAX_ENTRY_TEXT_BYTES: usize = AGENT_MAX_PROMPT_BYTES;
/// Sum of retained entry text bytes. Older entries drop first.
pub const AGENT_TRANSCRIPT_SNAPSHOT_BUDGET_BYTES: usize = 256 * 1024;

/// Secret wrapper. `Debug` never prints the value. The bytes still travel on
/// the one-shot put command so Command Centre can reach the vault.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, PartialEq, Eq)]
pub struct AgentSecret(pub String);

impl fmt::Debug for AgentSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentPickerKind {
    Provider,
    Model,
    Agent,
    ProviderSetup,
    Session,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentToolPhase {
    Started,
    Progress,
    Finished,
    Error,
    Blocked,
}

/// Compact Prism `AgentEvent` projection. Tool/permission variants stay on the
/// wire so Phase 29 does not rewrite IPC; Chat never emits them.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentWireEvent {
    Started {
        session_id: String,
        run_id: String,
    },
    Finished {
        session_id: String,
        run_id: String,
        usage: String,
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

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTranscriptKind {
    User,
    Assistant,
    Thinking,
    Error,
    Usage,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentTranscriptEntry {
    pub kind: AgentTranscriptKind,
    pub text: String,
}

impl AgentTranscriptEntry {
    pub fn new(kind: AgentTranscriptKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
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
        AgentWireEvent::Started { .. }
        | AgentWireEvent::Tool { .. }
        | AgentWireEvent::Permission { .. } => {}
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

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionSnapshot {
    pub session_id: String,
    pub profile: String,
    pub provider: String,
    pub model: String,
    pub leaf_id: Option<String>,
    pub entries: Vec<AgentTranscriptEntry>,
}

impl AgentSessionSnapshot {
    pub fn apply_event(&mut self, event: &AgentWireEvent) {
        apply_transcript_event(&mut self.entries, event);
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentProviderInfo {
    pub id: String,
    pub configured: bool,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentModelInfo {
    pub provider: String,
    pub model: String,
    pub display_name: String,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentProfileInfo {
    pub name: String,
    pub description: String,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionInfo {
    pub id: String,
    pub profile: String,
    pub updated_at: String,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentInventory {
    pub providers: Vec<AgentProviderInfo>,
    pub models: Vec<AgentModelInfo>,
    pub profiles: Vec<AgentProfileInfo>,
    pub sessions: Vec<AgentSessionInfo>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentPickerItem {
    pub id: String,
    pub label: String,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentClientCommand {
    Prompt {
        session_id: String,
        text: String,
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
    },
    LoadSession {
        session_id: String,
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
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
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
            },
        );
        assert_eq!(
            entries,
            vec![
                AgentTranscriptEntry::new(AgentTranscriptKind::Assistant, "Hello"),
                AgentTranscriptEntry::new(AgentTranscriptKind::Usage, "1 token"),
            ]
        );
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
