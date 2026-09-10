//! Bounded internal-agent-event → AG-UI adapter (Plan 097 Phase 10).
//!
//! One pure function maps the Clay agent wire union to AG-UI protocol events
//! (`@ag-ui/core` 0.0.58). The Prism daemon protocol stays internal; package
//! JavaScript never sees either side. Tool/permission variants become inert
//! `CUSTOM` payloads so a future coding agent gains display transport without
//! gaining execution authority, and credentials have no field on any variant
//! they could reach.

use serde::Serialize;
use serde_json::Value;

use crate::protocol::{
    AgentInventory, AgentServerMessage, AgentSessionSnapshot, AgentTranscriptEntry,
    AgentTranscriptKind, AgentWireEvent, ApprovalRequestKind,
};
/// AG-UI event subset Clay emits. Serde shape matches `BaseEvent` JSON from
/// `@ag-ui/core` exactly: `"type"` discriminator + camelCase fields.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgUiEvent {
    #[serde(rename = "RUN_STARTED")]
    RunStarted { thread_id: String, run_id: String },
    #[serde(rename = "RUN_FINISHED")]
    RunFinished {
        thread_id: String,
        run_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
    },
    #[serde(rename = "RUN_ERROR")]
    RunError { message: String },
    #[serde(rename = "TEXT_MESSAGE_CHUNK")]
    TextMessageChunk { message_id: String, delta: String },
    #[serde(rename = "REASONING_MESSAGE_CHUNK")]
    ReasoningMessageChunk { message_id: String, delta: String },
    #[serde(rename = "STATE_SNAPSHOT")]
    StateSnapshot { snapshot: Value },
    #[serde(rename = "MESSAGES_SNAPSHOT")]
    MessagesSnapshot { messages: Vec<Value> },
    #[serde(rename = "CUSTOM")]
    Custom { name: String, value: Value },
}

/// Stable assistant text message id for one run. Chunk expansion in
/// `@ag-ui/client` opens a new text message whenever this id changes.
fn text_message_id(run_id: &str) -> String {
    format!("clay-text-{run_id}")
}

/// Stable reasoning message id for one run.
fn reasoning_message_id(run_id: &str) -> String {
    format!("clay-reasoning-{run_id}")
}

/// One transcript entry as an AG-UI message. Error/usage entries stay visible
/// through `metadata.clayKind`; roles remain standard AG-UI roles so the
/// client's default apply pipeline keeps owning them.
fn transcript_entry_message(index: usize, entry: &AgentTranscriptEntry) -> Value {
    let id = format!("clay-entry-{index}");
    match entry.kind {
        AgentTranscriptKind::User => serde_json::json!({
            "id": id, "role": "user", "content": entry.text
        }),
        AgentTranscriptKind::Assistant => serde_json::json!({
            "id": id, "role": "assistant", "content": entry.text
        }),
        AgentTranscriptKind::Thinking => serde_json::json!({
            "id": id, "role": "reasoning", "content": entry.text
        }),
        AgentTranscriptKind::Error => serde_json::json!({
            "id": id, "role": "assistant", "content": entry.text,
            "metadata": { "clayKind": "error" }
        }),
        AgentTranscriptKind::Usage => serde_json::json!({
            "id": id, "role": "assistant", "content": entry.text,
            "metadata": { "clayKind": "usage" }
        }),
        // Plan 109 I5: standard AG-UI tool role; the box label/shape ride
        // metadata so the client stays on standard roles + clayKind. The
        // message id is the tool call id so live CUSTOM rows and snapshot
        // rows reconcile to the same box.
        AgentTranscriptKind::Tool => {
            let mut value = serde_json::json!({
                "id": if entry.tool_call_id.is_empty() {
                    id
                } else {
                    format!("clay-tool-{}", entry.tool_call_id)
                },
                "role": "tool", "content": entry.text,
            });
            let mut metadata = serde_json::json!({
                "clayKind": if entry.skill_name.is_some() { "skill" } else { "tool" },
            });
            if !entry.tool_call_id.is_empty() {
                metadata["toolCallId"] = serde_json::json!(entry.tool_call_id);
            }
            if let Some(skill) = &entry.skill_name {
                metadata["skillName"] = serde_json::json!(skill);
            }
            value["metadata"] = metadata;
            value
        }
    }
}

fn snapshot_events(snapshot: &AgentSessionSnapshot) -> Vec<AgUiEvent> {
    let mut state_value = serde_json::json!({
        "sessionId": snapshot.session_id,
        "profile": snapshot.profile,
        "provider": snapshot.provider,
        "model": snapshot.model,
        "mcpServers": snapshot.mcp_servers,
        // Context-used-vs-window numerator (plan 108 task 9):
        // bounded counter only, never transcript content.
        "contextTokens": snapshot.context_tokens,
        // Reasoning-effort control state (plan 109 I4): declared
        // levels for the model (empty = no control) + active level.
        "effortLevels": snapshot.effort_levels,
        "effort": snapshot.effort,
    });
    // Plan 109 R1/R2/R3: environment + branch ride the same snapshot when
    // known. Empty keys are OMITTED so the client's state merge keeps the
    // previous values (settled-run republishes carry no environment fetch).
    if !snapshot.commands.is_empty() {
        state_value["commands"] =
            serde_json::to_value(&snapshot.commands).unwrap_or(serde_json::Value::Null);
    }
    if !snapshot.branch.is_empty() {
        state_value["branch"] = serde_json::json!(snapshot.branch);
    }
    if !snapshot.extensions.is_empty() {
        state_value["extensions"] = serde_json::json!(snapshot.extensions);
    }
    if !snapshot.skills.is_empty() {
        state_value["skills"] =
            serde_json::to_value(&snapshot.skills).unwrap_or(serde_json::Value::Null);
    }
    let state = AgUiEvent::StateSnapshot {
        snapshot: state_value,
    };
    // Book snapshots (empty session id — published by provider/model/profile
    // switches) carry no transcript: emitting a MessagesSnapshot here would
    // replace the live transcript with an empty list and visually wipe the
    // conversation on every picker selection.
    if snapshot.session_id.is_empty() {
        return vec![state];
    }
    vec![
        AgUiEvent::MessagesSnapshot {
            messages: snapshot
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| transcript_entry_message(index, entry))
                .collect(),
        },
        state,
    ]
}

fn inventory_state(inventory: &AgentInventory) -> Value {
    // Arrays are already bounded server-side; pass them through as plain data.
    // provider/model carry the current book selection so a freshly mounted
    // webview immediately knows the configured pair (STATE_SNAPSHOT merge
    // keeps them until changed).
    serde_json::json!({
        "providers": inventory.providers,
        "models": inventory.models,
        "profiles": inventory.profiles,
        "sessions": inventory.sessions,
        "provider": inventory.provider,
        "model": inventory.model,
    })
}

/// Map one wire message to zero or more AG-UI events. Pure and total: every
/// variant is handled, unknown-free, and output size is bounded by upstream
/// caps (transcript entries, delta text bytes, inventory limits).
pub fn adapt_agent_message(message: &AgentServerMessage) -> Vec<AgUiEvent> {
    match message {
        AgentServerMessage::Snapshot(snapshot) => snapshot_events(snapshot),
        AgentServerMessage::Event { session_id, event } => adapt_wire_event(session_id, event),
        AgentServerMessage::Inventory(inventory) => vec![AgUiEvent::StateSnapshot {
            snapshot: inventory_state(inventory),
        }],
        AgentServerMessage::Picker { .. } => Vec::new(),
        AgentServerMessage::CredentialAck {
            provider,
            name,
            stored,
        } => vec![AgUiEvent::Custom {
            name: "clay.credentialAck".into(),
            value: serde_json::json!({ "provider": provider, "name": name, "stored": stored }),
        }],
        AgentServerMessage::AgentRpc { code, result_json } => vec![AgUiEvent::Custom {
            name: "clay.agentRpc".into(),
            // Daemon stores an opaque JSON string on the wire; the Context /
            // Memory tabs require `result` as an object. Leave a string only
            // if the payload is not JSON.
            value: serde_json::json!({
                "code": code,
                "result": serde_json::from_str::<Value>(result_json)
                    .unwrap_or_else(|_| Value::String(result_json.clone())),
            }),
        }],
        AgentServerMessage::ApprovalRequest {
            request_id,
            kind,
            payload_json,
        } => vec![AgUiEvent::Custom {
            name: "clay.approvalRequest".into(),
            value: serde_json::json!({
                "requestId": request_id,
                "kind": match kind {
                    ApprovalRequestKind::Mutation => "mutation",
                    ApprovalRequestKind::AskDecision => "askDecision",
                },
                "payload": payload_json,
            }),
        }],
        AgentServerMessage::Diagnostic { code, message } => vec![AgUiEvent::Custom {
            name: "clay.diagnostic".into(),
            value: serde_json::json!({ "code": code, "message": message }),
        }],
    }
}

fn adapt_wire_event(session_id: &str, event: &AgentWireEvent) -> Vec<AgUiEvent> {
    match event {
        AgentWireEvent::Started { run_id, .. } => vec![AgUiEvent::RunStarted {
            thread_id: session_id.to_string(),
            run_id: run_id.clone(),
        }],
        AgentWireEvent::Finished { run_id, usage, .. } => vec![AgUiEvent::RunFinished {
            thread_id: session_id.to_string(),
            run_id: run_id.clone(),
            result: Some(serde_json::json!({ "usage": usage })),
        }],
        // Plan 117 token meter: the meter numerator rides a custom event so
        // the strip updates live per provider turn (snapshots only flow on
        // bind/switch/settle). Counters only — never content.
        AgentWireEvent::ContextTokens { tokens, .. } => vec![AgUiEvent::Custom {
            name: "clay.contextTokens".into(),
            value: serde_json::json!({ "tokens": tokens }),
        }],
        AgentWireEvent::MessageDelta { run_id, text, .. } => vec![AgUiEvent::TextMessageChunk {
            message_id: text_message_id(run_id),
            delta: text.clone(),
        }],
        AgentWireEvent::ThinkingDelta { run_id, text, .. } => {
            vec![AgUiEvent::ReasoningMessageChunk {
                message_id: reasoning_message_id(run_id),
                delta: text.clone(),
            }]
        }
        AgentWireEvent::Tool {
            phase,
            name,
            tool_call_id,
            args_digest,
            output_digest,
            skill_name,
            ..
        } => {
            // Plan 109 I5: bounded payload digests + skill name ride the
            // custom row event; the client evolves one transcript box per
            // tool call from it (the server transcript stays authoritative
            // and reconciles at snapshot boundaries).
            let mut value = serde_json::json!({
                "phase": phase,
                "name": name,
                "toolCallId": tool_call_id,
            });
            if let Some(args) = args_digest {
                value["argsDigest"] = serde_json::json!(args);
            }
            if let Some(output) = output_digest {
                value["outputDigest"] = serde_json::json!(output);
            }
            if let Some(skill) = skill_name {
                value["skillName"] = serde_json::json!(skill);
            }
            vec![AgUiEvent::Custom {
                name: "clay.toolPhase".into(),
                value,
            }]
        }
        AgentWireEvent::Permission {
            session_id,
            run_id,
            request_id,
            tool_name,
            allowed,
        } => vec![AgUiEvent::Custom {
            name: "clay.permissionRequest".into(),
            value: serde_json::json!({
                "sessionId": session_id,
                "runId": run_id,
                "requestId": request_id,
                "toolName": tool_name,
                "allowed": allowed,
            }),
        }],
        AgentWireEvent::Overflow => vec![AgUiEvent::Custom {
            name: "clay.overflow".into(),
            value: serde_json::json!({}),
        }],
        AgentWireEvent::Error { message, .. } => vec![AgUiEvent::RunError {
            message: message.clone(),
        }],
    }
}

/// True when the diagnostic should stop the streaming indicator (native
/// parity: "cancelled" clears running; empty submits are silent no-ops).
pub fn diagnostic_is_terminal(code: &str, message: &str) -> bool {
    code == "agent.cancelled" || message == "cancelled" || message == "empty prompt"
}

#[cfg(test)]
mod tests {
    use crate::protocol::{AgentMcpServerInfo, AgentSkillInfo, AgentSlashCommand};

    use super::*;
    use crate::protocol::{
        AgentModelInfo, AgentProfileInfo, AgentProviderInfo, AgentSessionInfo, AgentToolPhase,
    };

    fn sample_snapshot() -> AgentSessionSnapshot {
        AgentSessionSnapshot {
            effort_levels: Vec::new(),
            effort: None,
            session_id: "sess-1".into(),
            profile: "chat".into(),
            provider: "mock".into(),
            model: "mock-mini".into(),
            leaf_id: None,
            entries: vec![
                AgentTranscriptEntry::new(AgentTranscriptKind::User, "hi"),
                AgentTranscriptEntry::new(AgentTranscriptKind::Thinking, "pondering"),
                AgentTranscriptEntry::new(AgentTranscriptKind::Assistant, "hello"),
                AgentTranscriptEntry::new(AgentTranscriptKind::Error, "boom"),
                AgentTranscriptEntry::new(AgentTranscriptKind::Usage, "12 tokens"),
            ],
            mcp_servers: Vec::new(),
            context_tokens: None,
            commands: Vec::new(),
            branch: String::new(),
            extensions: Vec::new(),
            skills: Vec::new(),
        }
    }

    #[test]
    fn snapshot_maps_to_messages_and_state() {
        let events = adapt_agent_message(&AgentServerMessage::Snapshot(sample_snapshot()));
        assert_eq!(events.len(), 2);
        let AgUiEvent::MessagesSnapshot { messages } = &events[0] else {
            panic!("messages snapshot expected");
        };
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "reasoning");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[3]["metadata"]["clayKind"], "error");
        assert_eq!(messages[4]["metadata"]["clayKind"], "usage");
        let AgUiEvent::StateSnapshot { snapshot } = &events[1] else {
            panic!("state snapshot expected");
        };
        assert_eq!(snapshot["provider"], "mock");
        assert_eq!(snapshot["model"], "mock-mini");
        assert_eq!(snapshot["mcpServers"], serde_json::json!([]));
    }

    #[test]
    fn book_snapshot_without_session_emits_state_only() {
        // Book snapshots (provider/model/profile switches) carry no transcript;
        // a MessagesSnapshot here would wipe the live transcript on every
        // picker selection.
        let mut snapshot = sample_snapshot();
        snapshot.session_id = String::new();
        let events = snapshot_events(&snapshot);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], AgUiEvent::StateSnapshot { .. }));
    }

    #[test]
    fn state_snapshot_carries_mcp_server_outcomes() {
        // Plan 117: the MCP card + composer section read per-server connect
        // outcomes. Ids/counts/errors only — commands/args/env never reach
        // the client.
        let mut snapshot = sample_snapshot();
        snapshot.mcp_servers = vec![
            AgentMcpServerInfo {
                server_id: "files".into(),
                connected: true,
                tools: 3,
                error: String::new(),
            },
            AgentMcpServerInfo {
                server_id: "search".into(),
                connected: false,
                tools: 0,
                error: "spawn failed".into(),
            },
        ];
        let events = snapshot_events(&snapshot);
        let AgUiEvent::StateSnapshot { snapshot } = &events[1] else {
            panic!("state snapshot expected");
        };
        assert_eq!(
            snapshot["mcpServers"],
            serde_json::json!(
                [
                    {"serverId": "files", "connected": true, "tools": 3, "error": ""},
                    {"serverId": "search", "connected": false, "tools": 0, "error": "spawn failed"}
                ]
            )
        );
    }

    #[test]
    fn state_snapshot_carries_effort_levels_and_active_level() {
        // Plan 109 I4: STATE carries the model's declared levels (empty =
        // no control) and the session's active level.
        let mut snapshot = sample_snapshot();
        snapshot.effort_levels = vec!["low".into(), "medium".into(), "high".into()];
        snapshot.effort = Some("medium".into());
        let events = snapshot_events(&snapshot);
        let AgUiEvent::StateSnapshot { snapshot } = &events[1] else {
            panic!("state snapshot expected");
        };
        assert_eq!(
            snapshot["effortLevels"],
            serde_json::json!(["low", "medium", "high"])
        );
        assert_eq!(snapshot["effort"], serde_json::json!("medium"));
        // Non-reasoning model: empty levels and no active level.
        let snapshot = sample_snapshot();
        let events = snapshot_events(&snapshot);
        let AgUiEvent::StateSnapshot { snapshot } = &events[1] else {
            panic!("state snapshot expected");
        };
        assert_eq!(snapshot["effortLevels"], serde_json::json!([]));
        assert_eq!(snapshot["effort"], serde_json::json!(null));
    }

    #[test]
    fn run_lifecycle_maps_with_chunk_ids_stable_per_run() {
        let started = AgentWireEvent::Started {
            session_id: "sess-1".into(),
            run_id: "run-9".into(),
        };
        let delta = AgentWireEvent::MessageDelta {
            session_id: "sess-1".into(),
            run_id: "run-9".into(),
            text: "Hel".into(),
        };
        let thinking = AgentWireEvent::ThinkingDelta {
            session_id: "sess-1".into(),
            run_id: "run-9".into(),
            text: "hmm".into(),
        };
        let finished = AgentWireEvent::Finished {
            session_id: "sess-1".into(),
            run_id: "run-9".into(),
            usage: "12 tokens".into(),
            context_tokens: Some(12),
        };
        let events: Vec<AgUiEvent> = [&started, &delta, &thinking, &finished]
            .iter()
            .flat_map(|event| {
                adapt_agent_message(&AgentServerMessage::Event {
                    session_id: "sess-1".into(),
                    event: (*event).clone(),
                })
            })
            .collect();
        assert_eq!(
            events,
            vec![
                AgUiEvent::RunStarted {
                    thread_id: "sess-1".into(),
                    run_id: "run-9".into()
                },
                AgUiEvent::TextMessageChunk {
                    message_id: "clay-text-run-9".into(),
                    delta: "Hel".into()
                },
                AgUiEvent::ReasoningMessageChunk {
                    message_id: "clay-reasoning-run-9".into(),
                    delta: "hmm".into()
                },
                AgUiEvent::RunFinished {
                    thread_id: "sess-1".into(),
                    run_id: "run-9".into(),
                    result: Some(serde_json::json!({ "usage": "12 tokens" }))
                },
            ]
        );
    }

    #[test]
    fn error_maps_to_run_error_and_tools_stay_inert_customs() {
        let error = adapt_agent_message(&AgentServerMessage::Event {
            session_id: "sess-1".into(),
            event: AgentWireEvent::Error {
                session_id: "sess-1".into(),
                message: "provider unreachable".into(),
            },
        });
        assert_eq!(
            error,
            vec![AgUiEvent::RunError {
                message: "provider unreachable".into()
            }]
        );
        let tool = adapt_agent_message(&AgentServerMessage::Event {
            session_id: "sess-1".into(),
            event: AgentWireEvent::Tool {
                session_id: "sess-1".into(),
                run_id: "run-9".into(),
                phase: AgentToolPhase::Started,
                name: "read".into(),
                tool_call_id: "t1".into(),
                args_digest: Some("{\"path\":\"src/main.rs\"}".into()),
                output_digest: None,
                skill_name: None,
            },
        });
        let AgUiEvent::Custom { name, value } = &tool[0] else {
            panic!("custom expected");
        };
        assert_eq!(name, "clay.toolPhase");
        assert_eq!(value["phase"], "started");
        assert_eq!(value["toolCallId"], "t1");
        // Plan 109 I5: the bounded args digest rides the row event.
        assert_eq!(value["argsDigest"], r#"{"path":"src/main.rs"}"#);
        // No execution surface leaks: the custom payload has no raw args/result.
        assert!(value.get("arguments").is_none());
        assert!(value.get("result").is_none());
    }

    #[test]
    fn tool_transcript_entries_map_to_standard_tool_role_messages() {
        // Plan 109 I5: bounded tool rows render through the standard AG-UI
        // tool role with clayKind metadata; load_skill rows carry the skill
        // name and reconcile to the same message id as the live CUSTOM row.
        let mut snapshot = sample_snapshot();
        snapshot.entries.push(AgentTranscriptEntry::new_tool(
            "read {\"path\":\"src/main.rs\"} -> fn main()",
            "t1",
            None,
        ));
        snapshot.entries.push(AgentTranscriptEntry::new_tool(
            "load_skill {\"name\":\"rust-review\"} -> Loaded skill",
            "t2",
            Some("rust-review".into()),
        ));
        let events = adapt_agent_message(&AgentServerMessage::Snapshot(snapshot));
        let AgUiEvent::MessagesSnapshot { messages } = &events[0] else {
            panic!("messages snapshot expected");
        };
        let tool = messages
            .iter()
            .find(|message| message["id"] == "clay-tool-t1")
            .expect("tool row");
        assert_eq!(tool["role"], "tool");
        assert_eq!(tool["metadata"]["clayKind"], "tool");
        let skill = messages
            .iter()
            .find(|message| message["id"] == "clay-tool-t2")
            .expect("skill row");
        assert_eq!(skill["metadata"]["clayKind"], "skill");
        assert_eq!(skill["metadata"]["skillName"], "rust-review");
    }

    #[test]
    fn agent_rpc_parses_result_json_into_an_object() {
        let events = adapt_agent_message(&AgentServerMessage::AgentRpc {
            code: "session.context".into(),
            result_json: r#"{"sessionId":"s1","version":1,"categories":[]}"#.into(),
        });
        let AgUiEvent::Custom { name, value } = &events[0] else {
            panic!("custom expected");
        };
        assert_eq!(name, "clay.agentRpc");
        assert_eq!(value["code"], "session.context");
        assert!(
            value["result"].is_object(),
            "result must be an object, not a JSON string"
        );
        assert_eq!(value["result"]["sessionId"], "s1");
        assert!(value["result"]["categories"].is_array());
    }

    #[test]
    fn approval_request_becomes_one_custom_event_with_no_answer_surface() {
        let events = adapt_agent_message(&AgentServerMessage::ApprovalRequest {
            request_id: "approval-7".into(),
            kind: ApprovalRequestKind::Mutation,
            payload_json: r#"{"kind":"write"}"#.into(),
        });
        let AgUiEvent::Custom { name, value } = &events[0] else {
            panic!("custom expected");
        };
        assert_eq!(name, "clay.approvalRequest");
        assert_eq!(value["requestId"], "approval-7");
        assert_eq!(value["kind"], "mutation");
        // Payload stays opaque daemon-produced JSON; no decision field exists
        // on the request direction.
        assert_eq!(value["payload"], r#"{"kind":"write"}"#);
        assert!(value.get("allowed").is_none());
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn picker_is_dropped_and_inventory_becomes_one_state_snapshot() {
        let picker = adapt_agent_message(&AgentServerMessage::Picker {
            kind: crate::protocol::AgentPickerKind::Model,
            items: vec![crate::protocol::AgentPickerItem {
                id: "m".into(),
                label: "Mock".into(),
            }],
        });
        assert!(picker.is_empty());
        let inventory = AgentInventory {
            providers: vec![AgentProviderInfo {
                id: "mock".into(),
                configured: false,
            }],
            models: vec![AgentModelInfo {
                provider: "mock".into(),
                model: "mock-mini".into(),
                display_name: "Mock Mini".into(),
                context_window: None,
                thinking_levels: Vec::new(),
            }],
            profiles: vec![AgentProfileInfo {
                name: "chat".into(),
                description: "Chat".into(),
            }],
            sessions: vec![AgentSessionInfo {
                id: "sess-1".into(),
                profile: "chat".into(),
                updated_at: "2026-08-23T00:00:00Z".into(),
                label: String::new(),
                updated_at_label: String::new(),
            }],
            provider: "mock".into(),
            model: "mock-mini".into(),
        };
        let events = adapt_agent_message(&AgentServerMessage::Inventory(inventory));
        assert_eq!(events.len(), 1);
        let AgUiEvent::StateSnapshot { snapshot } = &events[0] else {
            panic!("expected state snapshot");
        };
        assert_eq!(
            snapshot["models"][0]["contextWindow"],
            serde_json::Value::Null
        );

        // Snapshot events carry the context-used counter (plan 108 task 9).
        let snapshot = AgentSessionSnapshot {
            context_tokens: Some(12),
            ..sample_snapshot()
        };
        let events = adapt_agent_message(&AgentServerMessage::Snapshot(snapshot));
        let state = events
            .iter()
            .find_map(|event| match event {
                AgUiEvent::StateSnapshot { snapshot } => Some(snapshot.clone()),
                _ => None,
            })
            .expect("state snapshot");
        assert_eq!(state["contextTokens"], 12);
    }

    #[test]
    fn context_tokens_rides_a_custom_event() {
        // Plan 117 token meter: the per-turn occupancy reaches the client
        // live as a bounded-counter custom event (no transcript row).
        let events = adapt_agent_message(&AgentServerMessage::Event {
            session_id: "sess-1".into(),
            event: AgentWireEvent::ContextTokens {
                session_id: "sess-1".into(),
                run_id: "run-9".into(),
                tokens: 220_000,
            },
        });
        assert_eq!(
            events,
            vec![AgUiEvent::Custom {
                name: "clay.contextTokens".into(),
                value: serde_json::json!({ "tokens": 220_000 }),
            }]
        );
    }

    #[test]
    fn snapshot_state_carries_environment_only_when_known() {
        // Plan 109 R1/R2/R3: completion commands, the git branch, and the
        // extension list ride STATE when present; empty values are omitted
        // so the client's merge keeps the last known good ones.
        let plain = sample_snapshot();
        let events = adapt_agent_message(&AgentServerMessage::Snapshot(plain));
        let state = events
            .iter()
            .find_map(|event| match event {
                AgUiEvent::StateSnapshot { snapshot } => Some(snapshot.clone()),
                _ => None,
            })
            .expect("state snapshot");
        assert!(state.get("commands").is_none());
        assert!(state.get("branch").is_none());
        assert!(state.get("extensions").is_none());
        assert!(state.get("skills").is_none());

        let full = AgentSessionSnapshot {
            commands: vec![AgentSlashCommand {
                name: "/new".into(),
                description: "Start a fresh session.".into(),
            }],
            branch: "main".into(),
            extensions: vec!["wiki".into()],
            skills: vec![AgentSkillInfo {
                name: "create-plan".into(),
                description: "Numbered plan documents.".into(),
            }],
            ..sample_snapshot()
        };
        let events = adapt_agent_message(&AgentServerMessage::Snapshot(full));
        let state = events
            .iter()
            .find_map(|event| match event {
                AgUiEvent::StateSnapshot { snapshot } => Some(snapshot.clone()),
                _ => None,
            })
            .expect("state snapshot");
        assert_eq!(state["commands"][0]["name"], "/new");
        assert_eq!(
            state["commands"][0]["description"],
            "Start a fresh session."
        );
        assert_eq!(state["branch"], "main");
        assert_eq!(state["extensions"][0], "wiki");
        assert_eq!(state["skills"][0]["name"], "create-plan");
        assert_eq!(
            state["skills"][0]["description"],
            "Numbered plan documents."
        );
    }

    #[test]
    fn ag_ui_json_shape_matches_protocol_names() {
        let json = serde_json::to_value(AgUiEvent::TextMessageChunk {
            message_id: "m".into(),
            delta: "x".into(),
        })
        .expect("serialize");
        assert_eq!(json["type"], "TEXT_MESSAGE_CHUNK");
        assert_eq!(json["messageId"], "m");
        let json = serde_json::to_value(AgUiEvent::RunStarted {
            thread_id: "t".into(),
            run_id: "r".into(),
        })
        .expect("serialize");
        assert_eq!(json["threadId"], "t");
        assert_eq!(json["runId"], "r");
        let json = serde_json::to_value(AgUiEvent::MessagesSnapshot { messages: vec![] })
            .expect("serialize");
        assert_eq!(json["type"], "MESSAGES_SNAPSHOT");
        let json = serde_json::to_value(AgUiEvent::Custom {
            name: "n".into(),
            value: serde_json::json!({}),
        })
        .expect("serialize");
        assert_eq!(json["type"], "CUSTOM");
        assert_eq!(json["name"], "n");
    }

    #[test]
    fn terminal_diagnostics_match_native_parity() {
        assert!(diagnostic_is_terminal("agent.cancelled", "cancelled"));
        assert!(!diagnostic_is_terminal("agent.idle", "no running session"));
        assert!(diagnostic_is_terminal("agent.empty_prompt", "empty prompt"));
    }
}
