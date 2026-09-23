//! AG-UI event relay (Plan 097 Phase 10).
//!
//! The Clay agent wire union is adapted to AG-UI events in Rust
//! ([`clay::server::agent_agui`]) and delivered to subscribed webview
//! channels. Prompt/cancel/session operations deliberately reuse the existing
//! validated request path (`session_request` with `sduiAction`/`agent`
//! families); this module only owns the event stream, so there is exactly one
//! producer of AG-UI events and the webview never sees Clay-only agent
//! frames.

use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;

use clay::protocol::{AgentServerMessage, ClientId, TabId};
use clay::server::agent_agui::{self, AgUiEvent};

/// One AG-UI event tagged with the session it arrived on. The event is
/// flattened so the AG-UI `"type"` discriminator stays at the top level.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStreamEvent {
    pub client_id: ClientId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<TabId>,
    /// Session the event belongs to (plan 119 SC-6). The relay fans every
    /// agent message out to every connection, so a per-tab store can only
    /// accept its own session's traffic when the session rides the event.
    /// Absent for process-wide messages (diagnostics, agent-RPC replies) and
    /// for a tab's pre-session STATE snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub event: AgUiEvent,
}

/// Return the session tag carried by session-scoped agent messages.
///
/// This is desktop relay plumbing, not a Clay JS API: it stays private to the
/// Tauri crate while the server crate exposes only the AG-UI adapter.
fn session_of(message: &AgentServerMessage) -> Option<String> {
    match message {
        AgentServerMessage::Event { session_id, .. } => Some(session_id.clone()),
        AgentServerMessage::Snapshot(snapshot) if !snapshot.session_id.is_empty() => {
            Some(snapshot.session_id.clone())
        }
        _ => None,
    }
}

/// Fan-out of adapted AG-UI events to every subscribed webview channel.
/// Registration replaces nothing: multiple windows may observe the same
/// stream; each channel gets its own serialized copy.
#[derive(Default)]
pub struct AgentRelay {
    channels: Mutex<Vec<Channel<AgentStreamEvent>>>,
}

impl AgentRelay {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one webview channel.
    pub fn subscribe(&self, channel: Channel<AgentStreamEvent>) {
        self.channels
            .lock()
            .expect("agent relay lock poisoned")
            .push(channel);
    }

    /// Drops every registration (window closed / explicit unsubscribe).
    pub fn unsubscribe(&self) {
        self.channels
            .lock()
            .expect("agent relay lock poisoned")
            .clear();
    }

    pub fn subscriber_count(&self) -> usize {
        self.channels
            .lock()
            .expect("agent relay lock poisoned")
            .len()
    }

    /// Adapts one wire message and delivers each resulting AG-UI event to
    /// every subscriber. Dead channels are dropped so a closed window stops
    /// costing serialization work.
    pub fn deliver(
        &self,
        client_id: ClientId,
        tab_id: Option<TabId>,
        message: &AgentServerMessage,
    ) {
        let events = agent_agui::adapt_agent_message(message);
        if events.is_empty() {
            return;
        }
        let session_id = session_of(message);
        let mut channels = self.channels.lock().expect("agent relay lock poisoned");
        channels.retain(|channel| {
            events.iter().all(|event| {
                channel
                    .send(AgentStreamEvent {
                        client_id,
                        tab_id,
                        session_id: session_id.clone(),
                        event: event.clone(),
                    })
                    .is_ok()
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AgentServerMessage {
        AgentServerMessage::Diagnostic {
            code: "agent.cancelled".into(),
            message: "cancelled".into(),
        }
    }

    #[test]
    fn session_tags_only_session_scoped_messages() {
        let event = AgentServerMessage::Event {
            session_id: "sess-1".into(),
            event: clay::protocol::AgentWireEvent::Started {
                session_id: "sess-1".into(),
                run_id: "run-1".into(),
            },
        };
        assert_eq!(session_of(&event).as_deref(), Some("sess-1"));

        let mut snapshot = clay::protocol::AgentSessionSnapshot {
            effort_levels: Vec::new(),
            effort: None,
            session_id: "sess-1".into(),
            profile: String::new(),
            provider: String::new(),
            model: String::new(),
            leaf_id: None,
            agent: None,
            entries: Vec::new(),
            mcp_servers: Vec::new(),
            context_tokens: None,
            commands: Vec::new(),
            branch: String::new(),
            extensions: Vec::new(),
            skills: Vec::new(),
        };
        assert_eq!(
            session_of(&AgentServerMessage::Snapshot(snapshot.clone())).as_deref(),
            Some("sess-1")
        );
        snapshot.session_id.clear();
        assert_eq!(session_of(&AgentServerMessage::Snapshot(snapshot)), None);
        assert_eq!(session_of(&sample()), None);
    }

    #[test]
    fn deliver_without_subscribers_is_noop() {
        let relay = AgentRelay::new();
        relay.deliver(1, None, &sample());
        assert_eq!(relay.subscriber_count(), 0);
    }

    #[test]
    fn unsubscribe_clears_every_channel() {
        let relay = AgentRelay::new();
        // No real webview in unit tests; exercise bookkeeping only.
        relay.unsubscribe();
        assert_eq!(relay.subscriber_count(), 0);
    }

    #[test]
    fn stream_event_serializes_flat_with_type_discriminant() {
        let event = AgentStreamEvent {
            client_id: 3,
            tab_id: Some(9),
            session_id: Some("sess-1".into()),
            event: AgUiEvent::RunStarted {
                thread_id: "sess-1".into(),
                run_id: "run-1".into(),
            },
        };
        let json = serde_json::to_value(&event).expect("json");
        assert_eq!(json["type"], "RUN_STARTED");
        assert_eq!(json["threadId"], "sess-1");
        assert_eq!(json["clientId"], 3);
        assert_eq!(json["tabId"], 9);
        assert_eq!(json["sessionId"], "sess-1");
        let event = AgentStreamEvent {
            client_id: 3,
            tab_id: None,
            session_id: None,
            event: AgUiEvent::RunError {
                message: "x".into(),
            },
        };
        let json = serde_json::to_value(&event).expect("json");
        assert_eq!(json["type"], "RUN_ERROR");
        assert!(json.get("tabId").is_none());
        assert!(json.get("sessionId").is_none());
    }

    #[test]
    fn deliver_fans_out_to_subscribed_channels() {
        use std::sync::{Arc, Mutex as StdMutex};
        use tauri::ipc::{Channel, InvokeResponseBody};

        let relay = AgentRelay::new();
        let received: Arc<StdMutex<Vec<String>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&received);
        let channel = Channel::<AgentStreamEvent>::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                sink.lock().expect("sink lock").push(json);
            }
            Ok(())
        });
        relay.subscribe(channel);
        relay.deliver(
            5,
            None,
            &AgentServerMessage::Event {
                session_id: "sess-1".into(),
                event: clay::protocol::AgentWireEvent::Started {
                    session_id: "sess-1".into(),
                    run_id: "run-1".into(),
                },
            },
        );
        {
            let rows = received.lock().expect("sink lock");
            assert_eq!(rows.len(), 1);
            let value: serde_json::Value = serde_json::from_str(&rows[0]).expect("json body");
            assert_eq!(value["type"], "RUN_STARTED");
            assert_eq!(value["runId"], "run-1");
            assert_eq!(value["clientId"], 5);
            // Plan 119 SC-6: the session rides every session-scoped relay
            // event so a per-tab store filters by it (the client-id stamp is
            // the receiving connection, not the owner).
            assert_eq!(value["sessionId"], "sess-1");
        }
        // A process-wide message (diagnostics) carries no session tag.
        relay.deliver(5, None, &sample());
        let rows = received.lock().expect("sink lock");
        let value: serde_json::Value = serde_json::from_str(&rows[1]).expect("json body");
        assert_eq!(value["type"], "CUSTOM");
        assert!(value.get("sessionId").is_none());
    }
}
