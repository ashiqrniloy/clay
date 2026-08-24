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
    AgentTranscriptKind, AgentWireEvent,
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
    }
}

fn snapshot_events(snapshot: &AgentSessionSnapshot) -> Vec<AgUiEvent> {
    vec![
        AgUiEvent::MessagesSnapshot {
            messages: snapshot
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| transcript_entry_message(index, entry))
                .collect(),
        },
        AgUiEvent::StateSnapshot {
            snapshot: serde_json::json!({
                "sessionId": snapshot.session_id,
                "profile": snapshot.profile,
                "provider": snapshot.provider,
                "model": snapshot.model,
            }),
        },
    ]
}

fn inventory_state(inventory: &AgentInventory) -> Value {
    // Arrays are already bounded server-side; pass them through as plain data.
    serde_json::json!({
        "providers": inventory.providers,
        "models": inventory.models,
        "profiles": inventory.profiles,
        "sessions": inventory.sessions,
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
            ..
        } => vec![AgUiEvent::Custom {
            name: "clay.toolPhase".into(),
            value: serde_json::json!({
                "phase": phase,
                "name": name,
                "toolCallId": tool_call_id,
            }),
        }],
        AgentWireEvent::Permission {
            request_id,
            tool_name,
            allowed,
            ..
        } => vec![AgUiEvent::Custom {
            name: "clay.permissionRequest".into(),
            value: serde_json::json!({
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
    use super::*;
    use crate::protocol::{
        AgentModelInfo, AgentProfileInfo, AgentProviderInfo, AgentSessionInfo, AgentToolPhase,
    };

    fn sample_snapshot() -> AgentSessionSnapshot {
        AgentSessionSnapshot {
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
            },
        });
        let AgUiEvent::Custom { name, value } = &tool[0] else {
            panic!("custom expected");
        };
        assert_eq!(name, "clay.toolPhase");
        assert_eq!(value["phase"], "started");
        assert_eq!(value["toolCallId"], "t1");
        // No execution surface leaks: the custom payload has no args/result.
        assert!(value.get("arguments").is_none());
        assert!(value.get("result").is_none());
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
            }],
            profiles: vec![AgentProfileInfo {
                name: "chat".into(),
                description: "Chat".into(),
            }],
            sessions: vec![AgentSessionInfo {
                id: "sess-1".into(),
                profile: "chat".into(),
                updated_at: "2026-08-23T00:00:00Z".into(),
            }],
        };
        let events = adapt_agent_message(&AgentServerMessage::Inventory(inventory));
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], AgUiEvent::StateSnapshot { .. }));
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
