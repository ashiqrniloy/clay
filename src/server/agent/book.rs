//! The agent session book: selection state, persistence, snapshots, and picker
//! inventory (plan 119 SC-3).
//!
//! Split out of `src/server/agent.rs` unchanged. The book is the server's
//! authority for what an agent session *is*: which provider/model/profile a
//! workspace or `(agent type, workspace root)` last selected, the live session
//! per `(agent type, workspace root)` (plan 119 SC-6), each session's
//! transcript rows, branch, context-token counter and effort level, and the
//! daemon session records loaded at resume. Persistence is the `book.json`
//! file under the agent's data dir; snapshots are the wire STATE derived from
//! it; the picker inventory is the daemon's provider/model/profile/session
//! catalog parsed into menu rows.

use super::{
    AGENT_MAX_ENTRY_TEXT_BYTES, AGENT_MAX_SNAPSHOT_ENTRIES, AgentClientCommand, AgentError,
    AgentHost, AgentMcpServerInfo, AgentModelInfo, AgentOmWorkerKind, AgentOmWorkerModel,
    AgentPickerKind, AgentProfileInfo, AgentSearchHit, AgentServerMessage, AgentSessionInfo,
    AgentSessionSnapshot, AgentSkillInfo, AgentSlashCommand, AgentTranscriptEntry,
    AgentTranscriptKind, AgentWireEvent, TabId, apply_transcript_event, json_string,
    transcript_file_for_tool, truncate_transcript_text,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Session ownership key (plan 119 SC-6, decision 2026-09-14-1705): one
/// `(agent type, workspace root)` pair owns one daemon session, whatever tab
/// looks it up. Empty agent = the daemon's default agent, empty root = a tab
/// with no workspace (the launcher landing).
pub(super) fn workspace_key(agent: Option<&str>, root: Option<&str>) -> (String, String) {
    (
        agent.unwrap_or("").to_string(),
        root.unwrap_or("").to_string(),
    )
}

#[derive(Default)]
pub(super) struct SessionBook {
    pub(super) profile: String,
    pub(super) provider: String,
    pub(super) model: String,
    /// Last used selection per workspace root (plan 109 I2): written on
    /// picker/model selections that happen while a tab is bound to the
    /// root; resolved at session creation with the global trio as fallback.
    pub(super) workspaces: HashMap<String, BookSelection>,
    /// Plan 118 task 35: last-used selection per (agent type, workspace
    /// root). Switching a tab's agent must reset the model/effort controls
    /// to *that* agent's defaults, so each agent remembers its own pick;
    /// `workspaces` keeps the pre-agent behavior (and stays the fallback).
    pub(super) agent_workspaces: HashMap<String, HashMap<String, BookSelection>>,
    /// The live session per `(agent type, workspace root)` (plan 119 SC-6).
    /// This map — not a tab — is the ownership record: every tab resolves
    /// through `session_for_workspace`, so two tabs on one folder share one
    /// session, and a tab whose root/agent changed simply resolves a
    /// different key (its old session stays resumable from `transcripts`).
    pub(super) sessions_by_workspace: HashMap<(String, String), String>,
    /// Plan 118 task 35: the agent type each session runs as. Recorded at
    /// creation (and on load/resume from the daemon record), stamped onto
    /// every transcript row the server appends, and rewritten by a switch.
    pub(super) session_agent: HashMap<String, String>,
    /// Last finished run's context-token counter per session (plan 108
    /// task 9): the snapshot's context-used-vs-window numerator.
    pub(super) context_tokens: HashMap<String, Option<u64>>,
    /// Declared thinking levels per `provider/model` (plan 109 I4), cached
    /// from the daemon's model inventory; the snapshot's `effortLevels`.
    pub(super) model_levels: HashMap<String, Vec<String>>,
    /// Active thinking level per session (plan 109 I4): the last level a
    /// prompt carried; the snapshot's `effort`.
    pub(super) effort: HashMap<String, String>,

    pub(super) transcripts: HashMap<String, Vec<AgentTranscriptEntry>>,
    pub(super) running: HashSet<String>,
    pub(super) cancelled: HashSet<String>,
    /// Git branch per session (plan 109 R2): resolved from the session's
    /// workspace root, refreshed on session/workspace change and run
    /// completion. Empty = not a repo (status row shows `—`).
    pub(super) branches: HashMap<String, String>,
    /// Workspace root each session was created/resumed against (plan 109
    /// R2): lets the run-finish republish refresh the branch without a
    /// tab reference.
    pub(super) session_root: HashMap<String, String>,
    /// Branch cache: root -> (generation read at, branch) (plan 109 R2).
    /// Same generation = run still in flight = cached read; run
    /// completion bumps the generation so the next look re-reads.
    pub(super) branch_cache: HashMap<String, (u64, String)>,
    /// Bumped on run terminal events (plan 109 R2): invalidates the
    /// branch cache so completion-time reads are fresh.
    pub(super) run_generation: u64,
}

/// Plan 109 R2: best-effort git branch for a workspace root — direct
/// `.git` reads only, never a subprocess. Handles the usual layout
/// (`.git/HEAD`) and worktree/submodule pointers (`.git` file with
/// `gitdir:`). Bounded; failure-silent (`None` = not a repo).
pub(super) fn read_git_branch(root: &Path) -> Option<String> {
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
pub(super) fn refresh_branch(
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

impl SessionBook {
    /// The live session for a `(agent type, workspace root)` pair (plan 119
    /// SC-6). Pure lookup: a changed root/agent resolves a different key and
    /// the caller creates a session for it; the previous session stays
    /// resumable from `transcripts`.
    pub(super) fn session_for_workspace(
        &self,
        agent: Option<&str>,
        root: Option<&str>,
    ) -> Option<String> {
        self.sessions_by_workspace
            .get(&workspace_key(agent, root))
            .cloned()
    }

    /// Bind `session_id` to a `(agent type, workspace root)` pair.
    pub(super) fn bind_workspace_session(
        &mut self,
        agent: Option<&str>,
        root: Option<&str>,
        session_id: &str,
    ) {
        self.sessions_by_workspace
            .insert(workspace_key(agent, root), session_id.to_string());
    }

    /// Append a transcript row, stamped with the agent that produced it.
    pub(super) fn push_row(&mut self, session_id: &str, row: AgentTranscriptEntry) {
        let agent = self.session_agent.get(session_id).cloned();
        let entries = self.transcripts.entry(session_id.to_string()).or_default();
        entries.push(row.with_agent(agent.as_deref()));
        cap_entries(entries);
    }
}

/// A persisted profile/provider/model selection (plan 109 I2): the global
/// fallback trio in `book.json`, and one entry per workspace root. The OM
/// worker bindings (plan 109 I8) ride the same entries (protocol
/// `AgentOmWorkerModel` is the shared wire shape).
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct BookSelection {
    #[serde(default)]
    pub(super) profile: String,
    #[serde(default)]
    pub(super) provider: String,
    #[serde(default)]
    pub(super) model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) om_observation: Option<AgentOmWorkerModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) om_reflection: Option<AgentOmWorkerModel>,
}

impl BookSelection {
    pub(super) fn from_book(book: &SessionBook) -> Self {
        Self {
            profile: book.profile.clone(),
            provider: book.provider.clone(),
            model: book.model.clone(),
            om_observation: None,
            om_reflection: None,
        }
    }

    pub(super) fn is_configured(&self, is_configured: &impl Fn(&str) -> bool) -> bool {
        !self.provider.is_empty() && is_configured(&self.provider)
    }
}

/// Plan 109 I8: build the daemon's `observationalMemoryWorkers` /
/// `workers` payload — `{ observation: {provider, model}|null, reflection:
/// …|null }`; `null` when both sides are absent (no key at all).
pub(super) fn json_om_workers(
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

/// Resolve the selection for (agent type, workspace root) (plan 109 I2, plan
/// 118 task 35): that agent's last-used selection for the root, else the
/// root's pre-agent entry (a migration read, so an existing book keeps its
/// pick), else the global fallback trio. Only a configured provider wins;
/// anything else falls through.
pub(super) fn stored_selection(
    book: &SessionBook,
    agent: Option<&str>,
    root: Option<&str>,
) -> Option<BookSelection> {
    match agent.map(str::trim).filter(|agent| !agent.is_empty()) {
        // A named agent reads only its own map: a switch must land on *that*
        // agent's defaults, never on another agent's last pick. A pre-agent
        // book is migrated into the shipped agent's map at load, so this does
        // not strand an existing selection.
        Some(agent) => book
            .agent_workspaces
            .get(agent)
            .and_then(|roots| root.and_then(|root| roots.get(root)))
            .cloned(),
        // No agent = the daemon's default agent (and the pre-agent path).
        None => root.and_then(|root| book.workspaces.get(root)).cloned(),
    }
}

pub(super) fn selection_for(
    book: &SessionBook,
    agent: Option<&str>,
    root: Option<&str>,
    is_configured: &impl Fn(&str) -> bool,
) -> BookSelection {
    stored_selection(book, agent, root)
        .filter(|selection| selection.is_configured(is_configured))
        .unwrap_or_else(|| BookSelection::from_book(book))
}

/// Record a selection where its own read path will find it: a named agent's
/// map, or the root map for the daemon's default agent (the pre-agent read).
pub(super) fn remember_selection(
    book: &mut SessionBook,
    agent: Option<&str>,
    root: Option<&str>,
    selection: &BookSelection,
) {
    let Some(root) = root.map(str::to_string) else {
        return;
    };
    match agent.map(str::to_string).filter(|agent| !agent.is_empty()) {
        Some(agent) => {
            book.agent_workspaces
                .entry(agent)
                .or_default()
                .insert(root, selection.clone());
        }
        None => {
            book.workspaces.insert(root, selection.clone());
        }
    }
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

pub(super) fn snapshot_from_new(value: &Value) -> AgentSessionSnapshot {
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
        // Plan 118 task 35: the daemon names the session's agent type in its
        // new/load/setAgent replies (absent for the default agent).
        agent: value
            .get("agent")
            .and_then(Value::as_str)
            .filter(|agent| !agent.is_empty())
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
        skills: Vec::new(),
    }
}

pub(super) fn snapshot_from_load(value: &Value) -> AgentSessionSnapshot {
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
    // Plan 118 task 35: the session's agent type rides the record metadata
    // and each entry's own stamp, so a resumed session keeps running (and
    // labelling its turns) as the agent that produced them.
    snapshot.agent = value
        .get("metadata")
        .and_then(|meta| meta.get("agentType"))
        .and_then(Value::as_str)
        .filter(|agent| !agent.is_empty())
        .map(str::to_string);
    if let Some(entries) = value.get("entries").and_then(Value::as_array) {
        let mut rows = Vec::new();
        for entry in entries {
            append_loaded_entry(&mut rows, entry, snapshot.agent.as_deref());
        }
        rows.truncate(AGENT_MAX_SNAPSHOT_ENTRIES);
        snapshot.entries = rows;
    }
    snapshot
}

/// Text of a persisted `tool_result` block: its `result` is the tool's raw
/// return — Prism's `{ content: [text blocks], value }` fold shape or a bare
/// string. Same projection the live `tool_output_digest` uses.
pub(super) fn loaded_tool_result_text(block: &Value) -> String {
    let Some(result) = block.get("result") else {
        return String::new();
    };
    let mut text = String::new();
    if let Some(blocks) = result.get("content").and_then(Value::as_array) {
        for entry in blocks {
            if entry.get("type").and_then(Value::as_str) == Some("text")
                && let Some(chunk) = entry.get("text").and_then(Value::as_str)
            {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(chunk);
            }
        }
    }
    if text.is_empty() {
        text = match result {
            Value::String(text) => text.clone(),
            Value::Object(map) => map
                .get("value")
                .filter(|value| !value.is_null())
                .map(|value| value.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
    }
    text
}

/// One persisted `SessionEntry` → transcript rows.
///
/// `session.load` returns store records — `{ kind, message: { role, content:
/// [blocks] }, summary }` — not the flat `{ role, content }` this parser
/// originally assumed. Reading the flat shape found no role and no text, so
/// every resume produced an empty transcript and the panel looked like the
/// click did nothing. One message can hold several rows: an assistant turn
/// interleaves text, thinking, and tool calls.
pub(super) fn append_loaded_entry(
    rows: &mut Vec<AgentTranscriptEntry>,
    entry: &Value,
    fallback_agent: Option<&str>,
) {
    // Plan 118 task 35: each entry carries the agent type that produced it
    // (the daemon stamps the record metadata on append). Rows written before
    // the stamp existed fall back to the session's agent, so a resumed
    // transcript is labelled rather than blank.
    let agent = entry
        .get("metadata")
        .and_then(|meta| meta.get("agentType"))
        .and_then(Value::as_str)
        .filter(|agent| !agent.is_empty())
        .or(fallback_agent);
    if let Some(summary) = entry
        .get("summary")
        .and_then(Value::as_str)
        .filter(|summary| !summary.is_empty())
    {
        rows.push(
            AgentTranscriptEntry::new(
                AgentTranscriptKind::Assistant,
                truncate_transcript_text(summary, AGENT_MAX_ENTRY_TEXT_BYTES),
            )
            .with_agent(agent),
        );
        return;
    }
    let Some(message) = entry.get("message") else {
        return;
    };
    let role = message.get("role").and_then(Value::as_str);
    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let Some(text) = block.get("text").and_then(Value::as_str) else {
                    continue;
                };
                if text.is_empty() {
                    continue;
                }
                rows.push(
                    AgentTranscriptEntry::new(
                        transcript_kind(role),
                        truncate_transcript_text(text, AGENT_MAX_ENTRY_TEXT_BYTES),
                    )
                    .with_agent(agent),
                );
            }
            Some("thinking") => {
                let Some(text) = block.get("text").and_then(Value::as_str) else {
                    continue;
                };
                if text.is_empty() {
                    continue;
                }
                rows.push(
                    AgentTranscriptEntry::new(
                        AgentTranscriptKind::Thinking,
                        truncate_transcript_text(text, AGENT_MAX_ENTRY_TEXT_BYTES),
                    )
                    .with_agent(agent),
                );
            }
            Some("tool_call") => {
                let name = json_string(block, &["name"]);
                if name.is_empty() {
                    continue;
                }
                let id = json_string(block, &["id"]);
                // Same row shape the live `apply_tool_event` produces, so a
                // resumed tool row sits where the live one would have.
                let text = match block.get("arguments") {
                    Some(arguments) if arguments.is_object() => {
                        format!("{name} {}", arguments)
                    }
                    _ => format!("{name} - running"),
                };
                let skill = json_string(block, &["arguments", "name"]);
                // A resumed session rebuilds the same file records the live
                // rows carried (plan 118 task 36): the persisted call keeps
                // its arguments, so the Files tab survives a resume.
                let file = block
                    .get("arguments")
                    .and_then(|arguments| transcript_file_for_tool(&name, arguments));
                rows.push(
                    AgentTranscriptEntry::new_tool(
                        truncate_transcript_text(&text, AGENT_MAX_ENTRY_TEXT_BYTES),
                        &id,
                        (name == "load_skill" && !skill.is_empty()).then_some(skill),
                        file,
                    )
                    .with_agent(agent),
                );
            }
            Some("tool_result") => {
                let id = json_string(block, &["toolCallId"]);
                let digest = loaded_tool_result_text(block);
                match rows.iter().position(|row| row.tool_call_id == id) {
                    Some(index) if !digest.is_empty() => {
                        let prior = rows[index].text.clone();
                        rows[index].text = truncate_transcript_text(
                            &format!("{prior} -> {digest}"),
                            AGENT_MAX_ENTRY_TEXT_BYTES,
                        );
                    }
                    Some(_) => {}
                    None => {
                        let name = json_string(block, &["name"]);
                        let text = if digest.is_empty() {
                            name
                        } else {
                            format!("{name} -> {digest}")
                        };
                        // No call row to project from (the transcript
                        // dropped it): name only, no file record.
                        rows.push(
                            AgentTranscriptEntry::new_tool(
                                truncate_transcript_text(&text, AGENT_MAX_ENTRY_TEXT_BYTES),
                                &id,
                                None,
                                None,
                            )
                            .with_agent(agent),
                        );
                    }
                }
            }
            // Image/audio/video/file blocks carry no transcript text; the
            // live path records none either.
            _ => {}
        }
    }
}

/// Persisted book selection: survives server restarts so a configured
/// provider/model/profile does not need re-picking. Best-effort JSON next
/// to the daemon data (sessions/credentials live there too).
pub(super) fn book_path(data_dir: &Path) -> PathBuf {
    data_dir.join("book.json")
}

pub(super) fn load_persisted_book(data_dir: &Path) -> SessionBook {
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
            // Plan 118 task 35: per-agent selections. A book written before
            // agent types existed carries only the root map, so the shipped
            // agent inherits it (a read migration, not a rewrite): the user's
            // existing pick survives without being re-picked per agent.
            if value.get("agents").is_none() && !book.workspaces.is_empty() {
                book.agent_workspaces.insert(
                    crate::server::agent_settings::DEFAULT_AGENT_TYPE.to_string(),
                    book.workspaces.clone(),
                );
            }
            if let Some(agents) = value.get("agents").and_then(Value::as_object) {
                for (agent, roots) in agents {
                    let Some(roots) = roots.as_object() else {
                        continue;
                    };
                    let mut per_agent = HashMap::new();
                    for (root, entry) in roots {
                        if let Ok(selection) =
                            serde_json::from_value::<BookSelection>(entry.clone())
                        {
                            per_agent.insert(root.clone(), selection);
                        }
                    }
                    book.agent_workspaces.insert(agent.clone(), per_agent);
                }
            }
        }
        Err(error) => {
            eprintln!("[agent] book.json unreadable: {error}");
        }
    }
    book
}

pub(super) fn persist_book(data_dir: &Path, book: &SessionBook) {
    let payload = json!({
        "fallback": BookSelection::from_book(book),
        "workspaces": book.workspaces,
        // Plan 118 task 35: per-agent last-used selections.
        "agents": book.agent_workspaces,
    });
    if let Err(error) = std::fs::write(book_path(data_dir), payload.to_string()) {
        eprintln!("[agent] book.json write failed: {error}");
    }
}

/// Book transcript snapshot for republish (plan 109 I5): same shape as
/// `snapshot_for` without facade access (the pump owns the book guard).
pub(super) fn book_snapshot(
    book: &SessionBook,
    session_id: &str,
    mcp_servers: &[AgentMcpServerInfo],
    commands: &[AgentSlashCommand],
    extensions: &[String],
    skills: &[AgentSkillInfo],
) -> AgentSessionSnapshot {
    AgentSessionSnapshot {
        session_id: session_id.to_string(),
        profile: book.profile.clone(),
        provider: book.provider.clone(),
        model: book.model.clone(),
        leaf_id: None,
        agent: book
            .session_agent
            .get(session_id)
            .cloned()
            .filter(|agent| !agent.is_empty()),
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
        skills: skills.to_vec(),
        branch: book.branches.get(session_id).cloned().unwrap_or_default(),
    }
}

pub(super) fn apply_book_event(book: &mut SessionBook, message: &AgentServerMessage) {
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
        AgentWireEvent::Finished { .. } => {
            book.running.remove(session_id);
            // Occupancy books per provider turn (ContextTokens); the run
            // accumulator would double-count prior context.
            book.run_generation = book.run_generation.wrapping_add(1);
        }
        AgentWireEvent::ContextTokens {
            session_id: id,
            tokens,
            ..
        } => {
            book.context_tokens.insert(id.clone(), Some(*tokens));
        }
        AgentWireEvent::Error { .. } => {
            book.running.remove(session_id);
            book.run_generation = book.run_generation.wrapping_add(1);
        }
        _ => {}
    }
    // Plan 118 task 35: only the rows this event added are stamped with the
    // session's agent — rows from before a switch keep theirs.
    let (agent, before) = {
        let entries = book.transcripts.entry(session_id.clone()).or_default();
        (book.session_agent.get(session_id).cloned(), entries.len())
    };
    apply_transcript_event(
        book.transcripts.entry(session_id.clone()).or_default(),
        event,
    );
    if let Some(entries) = book.transcripts.get_mut(session_id) {
        for row in entries.iter_mut().skip(before) {
            *row = row.clone().with_agent(agent.as_deref());
        }
        cap_entries(entries);
    }
}

pub(super) fn cap_entries(entries: &mut Vec<AgentTranscriptEntry>) {
    if entries.len() > AGENT_MAX_SNAPSHOT_ENTRIES {
        let drop = entries.len() - AGENT_MAX_SNAPSHOT_ENTRIES;
        entries.drain(..drop);
    }
}

pub(super) fn transcript_kind(role: Option<&str>) -> AgentTranscriptKind {
    match role {
        Some("user") => AgentTranscriptKind::User,
        Some("thinking" | "reasoning") => AgentTranscriptKind::Thinking,
        Some("error") => AgentTranscriptKind::Error,
        Some("usage") => AgentTranscriptKind::Usage,
        _ => AgentTranscriptKind::Assistant,
    }
}

pub(super) fn json_usage(event: &Value) -> String {
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

/// Context occupancy for the token meter (plan 117): the provider turn's
/// prompt size — input + cache reads + cache writes. Absent/unreported
/// usage yields None (the client's heuristic estimate rules instead).
pub(super) fn context_tokens(event: &Value) -> Option<u64> {
    let usage = event.get("usage")?;
    const PROMPT_FIELDS: [&str; 3] = ["inputTokens", "cacheReadTokens", "cacheWriteTokens"];
    let tokens: u64 = PROMPT_FIELDS
        .iter()
        .filter_map(|field| usage.get(*field))
        .filter_map(Value::as_u64)
        .sum();
    let reported = PROMPT_FIELDS
        .iter()
        .any(|field| usage.get(*field).is_some_and(Value::is_u64));
    reported.then_some(tokens)
}
/// Plan 109 I9: parse the workspace-scoped resumable list from
/// `session.resumable`. The label carries the store's display label,
/// falling back to the summary snippet.
pub(super) fn parse_resumable_sessions(value: &Value) -> Vec<AgentSessionInfo> {
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
                updated_at_label: item
                    .get("updatedAtLabel")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

impl AgentHost {
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
        // Plan 109 I2 + plan 118 task 35: a selection made while a tab is
        // bound to a workspace also becomes that workspace's last-used
        // selection — for the tab's own agent type (each agent keeps its
        // defaults; the root entry stays the pre-agent read).
        let root = self.tab_workspace_root_for(tab).await;
        let agent = self.tab_agent_type_for(tab).await;
        {
            let mut book = self.inner.book.lock().await;
            let selection = BookSelection::from_book(&book);
            remember_selection(&mut book, agent.as_deref(), root.as_deref(), &selection);
        }
        self.persist_book_selection().await;
        self.publish_book_snapshot(tab).await;
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
        let agent = self.tab_agent_type_for(tab).await;
        {
            let mut book = self.inner.book.lock().await;
            // Deliberately unfiltered (the pre-agent behavior): an OM worker
            // binding is written even when the entry has no configured
            // provider yet.
            let mut selection = stored_selection(&book, agent.as_deref(), root.as_deref())
                .unwrap_or_else(|| BookSelection::from_book(&book));
            match worker {
                AgentOmWorkerKind::Observation => selection.om_observation = parsed.clone(),
                AgentOmWorkerKind::Reflection => selection.om_reflection = parsed.clone(),
            }
            remember_selection(&mut book, agent.as_deref(), root.as_deref(), &selection);
        }
        self.persist_book_selection().await;
        self.publish_book_snapshot(tab).await;
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
    pub(super) async fn persist_book_selection(&self) {
        if self.inner.config.inert {
            return;
        }
        let data_dir = self.inner.config.data_dir.clone();
        let book = self.inner.book.lock().await;
        persist_book(&data_dir, &book);
    }

    pub(super) async fn ensure_default_model(&self) {
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

    /// Broadcast a book-selection change (provider / model / profile / OM
    /// worker) as STATE.
    ///
    /// The book fields are the point, but the carrier must not be a
    /// session-less snapshot when the tab already has a session: the panel
    /// merges snapshots shallowly, so an empty `session_id` + `entries` +
    /// `branch` wipes the live session, transcript and status row — the
    /// Memory/Context/Settings tabs then read "No active agent session."
    /// Publish the tab's own state instead (its triple already reflects the
    /// book write). A session-less snapshot remains correct only for a tab
    /// that genuinely has no session yet (picking a provider before the first
    /// prompt), where there is nothing to wipe.
    pub(super) async fn publish_book_snapshot(&self, tab: Option<TabId>) {
        let bound_session = match tab {
            Some(tab) => self.tab_session_id(tab).await,
            None => None,
        };
        let snapshot = match bound_session {
            Some(session_id) => self.snapshot_for(&session_id).await,
            None => self.unconfigured_snapshot().await,
        };
        let _ = self
            .inner
            .events
            .send(Arc::new(AgentServerMessage::Snapshot(snapshot)));
    }

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

    /// Plan 117: the panel's recent-sessions list — the tab's
    /// workspace-scoped, labeled resumable page. The root comes from the
    /// tab registry (server-derived, never webview input); a tab without a
    /// workspace yields an empty list (fail-closed, no cross-workspace dump).
    pub(crate) async fn resumable_for_tab(&self, tab: TabId, limit: u32) -> Vec<AgentSessionInfo> {
        let Some(root) = self.tab_workspace_root(tab).await else {
            return Vec::new();
        };
        self.resumable_sessions(&root, limit)
            .await
            .unwrap_or_default()
    }

    /// Broadcast a pre-built message to every connected view. The panel's
    /// resume path needs this: the rich load snapshot must rebind state
    /// through the relay (fire-and-forget client commands have no reply
    /// lane), while the Command Centre picker writes its reply directly.
    pub async fn search_sessions(
        &self,
        tab: TabId,
        query: &str,
        limit: u32,
    ) -> Result<Vec<AgentSearchHit>, AgentError> {
        let Some(session_id) = self.tab_session_id(tab).await else {
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

    pub(super) async fn snapshot_for(&self, session_id: &str) -> AgentSessionSnapshot {
        let agent = {
            let book = self.inner.book.lock().await;
            book.session_agent
                .get(session_id)
                .cloned()
                .filter(|agent| !agent.is_empty())
        };
        let environment = self.environment(agent.as_deref()).await;
        let book = self.inner.book.lock().await;
        AgentSessionSnapshot {
            session_id: session_id.to_string(),
            profile: book.profile.clone(),
            provider: book.provider.clone(),
            model: book.model.clone(),
            leaf_id: None,
            agent: agent.clone(),
            context_tokens: book.context_tokens.get(session_id).cloned().flatten(),
            entries: book
                .transcripts
                .get(session_id)
                .cloned()
                .unwrap_or_default(),
            mcp_servers: environment.mcp_servers,
            // Plan 109 R1/R3: completion commands + extension strip data
            // ride the same STATE snapshot; the branch comes from the
            // book's refreshed per-session read.
            commands: environment.commands,
            extensions: environment.extensions,
            skills: environment.skills,
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

    /// Plan 117 follow-up: STATE for a coding-agent pane at mount. Nothing
    /// else emitted a snapshot before the first prompt, so a freshly opened
    /// surface showed `git —`, an empty skills card, and an empty MCP card.
    /// Tab-resolved: this is the same session a prompt would create, so the
    /// daemon discovers the workspace's skills, connects its MCP servers, and
    /// the branch is read — all before the first message.
    ///
    /// Plan 108 task 8's surface rule also applies at this mount: showing the
    /// coding surface makes its profile the book's active profile. A pane
    /// restored by the layout, reached through the view switcher, or rendered
    /// by the empty-tab landing never dispatched the launch command, so its
    /// session used to run the daemon-level `Chat` default — no coding tools,
    /// no MCP servers. Only an *empty* profile is filled: a deliberate
    /// selection is never overwritten.
    pub(crate) async fn tab_state_snapshot(&self, tab: TabId) -> AgentSessionSnapshot {
        let profile_empty = self.inner.book.lock().await.profile.is_empty();
        if profile_empty
            && self
                .profile_available(crate::server::command_execution::CODING_SURFACE_PROFILE_NAME)
                .await
        {
            self.select_picker(
                crate::protocol::AgentPickerKind::Agent,
                crate::server::command_execution::CODING_SURFACE_PROFILE_ID,
                Some(tab),
            )
            .await;
        }
        if let Some(session) = self.ensure_tab_session(tab).await {
            return self.snapshot_for(&session).await;
        }
        // Unconfigured (no provider/model selected yet): no session can be
        // created, but the workspace's branch is still knowable — report it so
        // the status row is not blank.
        let mut snapshot = self.unconfigured_snapshot().await;
        snapshot.branch = self
            .tab_workspace_root(tab)
            .await
            .as_deref()
            .and_then(|root| read_git_branch(Path::new(root)))
            .unwrap_or_default();
        snapshot
    }

    pub(super) async fn unconfigured_snapshot(&self) -> AgentSessionSnapshot {
        // A fresh webview's first STATE snapshot already carries the
        // completion list + extension strip + skills card (plan 109 R1/R3).
        let environment = self.environment(None).await;
        let book = self.inner.book.lock().await;
        AgentSessionSnapshot {
            session_id: String::new(),
            profile: book.profile.clone(),
            provider: book.provider.clone(),
            model: book.model.clone(),
            leaf_id: None,
            agent: None,
            context_tokens: None,
            entries: Vec::new(),
            mcp_servers: environment.mcp_servers,
            effort_levels: Vec::new(),
            effort: None,
            commands: environment.commands,
            extensions: environment.extensions,
            skills: environment.skills,
            branch: String::new(),
        }
    }

    /// Plan 109 R1/R3: fill the daemon-environment fields on a snapshot
    /// parsed from a daemon attach/open reply (those replies carry no
    /// environment data of their own).
    pub(super) async fn decorate_snapshot(
        &self,
        mut snapshot: AgentSessionSnapshot,
    ) -> AgentSessionSnapshot {
        // Plan 118 task 35: the attach/open reply already names the session's
        // agent, so its inventories are that agent's.
        let environment = self.environment(snapshot.agent.as_deref()).await;
        // A session reply carries its own agent when the daemon recorded one;
        // otherwise the book (a resumed session) does.
        if snapshot.agent.is_none() {
            let book = self.inner.book.lock().await;
            snapshot.agent = book
                .session_agent
                .get(&snapshot.session_id)
                .cloned()
                .filter(|agent| !agent.is_empty());
        }
        snapshot.commands = environment.commands;
        snapshot.extensions = environment.extensions;
        snapshot.skills = environment.skills;
        snapshot.branch = {
            let book = self.inner.book.lock().await;
            book.branches
                .get(&snapshot.session_id)
                .cloned()
                .unwrap_or_default()
        };
        snapshot
    }
}
