//! The bridge session: one live client connection to the Clay server, its
//! event pump, and the (re)connect state machine.
//!
//! Reconnect semantics: the old pump is aborted *before* the new handshake
//! starts and every session carries a monotonic generation number, so stale
//! stream data from a dead connection can structurally never reach the
//! webview after a reconnect. The new bootstrap installs one complete latest
//! state; nothing is merged across sessions.

use super::agent::AgentRelay;
use super::dto::{BootstrapDto, InitialDocumentDto, ThemeSnapshotDto, TypographySnapshotDto};
use super::errors::{BridgeError, MAX_REQUEST_BYTES};
use super::forwarder::{Forwarder, SinkRegistry};
use clay::client::{
    ClientConnectionEvent, ClientEditQueue, ClientInitialState, EditorEditEvent,
    connect_for_reclaim_or_new, connect_with_workspace_root,
};
use clay::protocol::{ClientId, ClientMessage, TabCommand, TabId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_TAB_SESSIONS: usize = 64;

/// Shared bridge state managed by Tauri. Construct with [`BridgeState::new`].
pub struct BridgeState {
    inner: Arc<Inner>,
}

struct Inner {
    endpoint: clay::ipc::IpcEndpoint,
    sessions: Mutex<HashMap<ClientId, LiveSession>>,
    connecting: AtomicBool,
    generation: AtomicU64,
    last_bootstrap: Mutex<Option<BootstrapDto>>,
    bootstraps: Mutex<HashMap<ClientId, BootstrapDto>>,
    active_client: Mutex<Option<ClientId>>,
    /// Per-connection tab binding observed from `TabRegistry` events.
    bindings: Arc<Mutex<HashMap<ClientId, (TabId, String)>>>,
    sinks: Arc<SinkRegistry>,
    /// AG-UI event relay for the agent stream (Phase 10).
    agent_relay: Arc<AgentRelay>,
}

struct LiveSession {
    /// Identity retained for diagnostics/tests.
    #[allow(dead_code)]
    client_id: ClientId,
    /// Held so the connection's outbound channel stays open while the session
    /// lives; dropping it (teardown) lets the connection loop wind down.
    edit_queue: ClientEditQueue,
    forwarder: Arc<Forwarder>,
    /// Aborts the event pump on takeover/teardown.
    pump_abort: tokio::task::AbortHandle,
}

impl BridgeState {
    pub fn new(endpoint: clay::ipc::IpcEndpoint) -> Self {
        Self {
            inner: Arc::new(Inner {
                endpoint,
                sessions: Mutex::new(HashMap::new()),
                connecting: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                last_bootstrap: Mutex::new(None),
                bootstraps: Mutex::new(HashMap::new()),
                active_client: Mutex::new(None),
                bindings: Arc::new(Mutex::new(HashMap::new())),
                sinks: Arc::new(SinkRegistry::new()),
                agent_relay: Arc::new(AgentRelay::new()),
            }),
        }
    }

    /// Connects to the server and installs a complete session. Returns the
    /// cached bootstrap when a live session already exists.
    pub async fn bootstrap(&self) -> Result<BootstrapDto, BridgeError> {
        if let Some(cached) = self
            .inner
            .last_bootstrap
            .lock()
            .expect("bootstrap lock")
            .clone()
            && self.is_connected()
        {
            return Ok(cached);
        }
        self.connect_fresh().await
    }

    /// Tears down any live session and connects again, reclaiming the tab we
    /// held if the server still knows it.
    pub async fn reconnect(&self) -> Result<BootstrapDto, BridgeError> {
        // `connect_inner` snapshots the first tab binding, then tears down.
        self.connect_fresh().await
    }

    async fn connect_fresh(&self) -> Result<BootstrapDto, BridgeError> {
        if self.inner.connecting.swap(true, Ordering::Acquire) {
            return Err(BridgeError::busy());
        }
        let result = self.connect_inner().await;
        self.inner.connecting.store(false, Ordering::Release);
        result
    }

    async fn connect_inner(&self) -> Result<BootstrapDto, BridgeError> {
        // Old pumps die before the new handshake: no stale events possible.
        let binding = self
            .inner
            .bindings
            .lock()
            .expect("tab lock poisoned")
            .values()
            .next()
            .cloned();
        self.teardown();
        self.handshake_and_install(binding, None).await
    }

    /// Opens an additional tab client. Does not tear down existing sessions.
    pub async fn open_tab(&self, workspace_root: String) -> Result<BootstrapDto, BridgeError> {
        if self.session_count() >= MAX_TAB_SESSIONS {
            return Err(BridgeError::forbidden("tab connection cap reached"));
        }
        if self.inner.connecting.swap(true, Ordering::Acquire) {
            return Err(BridgeError::busy());
        }
        let result = self.handshake_and_install(None, Some(workspace_root)).await;
        self.inner.connecting.store(false, Ordering::Release);
        result
    }

    pub async fn close_tab(&self, tab_id: TabId) -> Result<(), BridgeError> {
        if self.session_count() <= 1 {
            return Err(BridgeError::forbidden("the last tab cannot be closed"));
        }
        let client_id = self.client_for_tab(tab_id)?;
        self.request_on(
            Some(tab_id),
            &serde_json::to_string(&ClientMessage::TabCommand {
                client_id,
                command: TabCommand::Close { tab_id },
            })
            .expect("tab close payload"),
        )?;
        self.drop_session(client_id);
        Ok(())
    }

    pub async fn activate_tab(&self, tab_id: TabId) -> Result<(), BridgeError> {
        let client_id = self.client_for_tab(tab_id)?;
        *self.inner.active_client.lock().expect("active lock") = Some(client_id);
        self.request_on(
            Some(tab_id),
            &serde_json::to_string(&ClientMessage::TabCommand {
                client_id,
                command: TabCommand::Activate { tab_id },
            })
            .expect("tab activate payload"),
        )
    }

    pub fn accept_selected_path(
        &self,
        tab_id: Option<TabId>,
        path: std::path::PathBuf,
        workspace: bool,
    ) -> Result<(), BridgeError> {
        let queue = self.edit_queue_for(tab_id)?;
        let result = if workspace {
            queue.enqueue_add_selected_workspace_root(path)
        } else {
            queue.enqueue_open_selected_file(path)
        };
        result.map_err(|error| match error {
            tokio::sync::mpsc::error::TrySendError::Full(_) => BridgeError::queue_full(),
            other => BridgeError::invalid_request(other),
        })
    }

    fn edit_queue_for(&self, tab_id: Option<TabId>) -> Result<ClientEditQueue, BridgeError> {
        let client_id = if let Some(tab_id) = tab_id {
            self.client_for_tab(tab_id)?
        } else {
            self.inner
                .active_client
                .lock()
                .expect("active lock")
                .ok_or_else(BridgeError::not_connected)?
        };
        self.inner
            .sessions
            .lock()
            .expect("sessions lock")
            .get(&client_id)
            .map(|live| live.edit_queue.clone())
            .ok_or_else(BridgeError::not_connected)
    }

    fn session_count(&self) -> usize {
        self.inner.sessions.lock().expect("sessions lock").len()
    }

    fn client_for_tab(&self, tab_id: TabId) -> Result<ClientId, BridgeError> {
        self.inner
            .bindings
            .lock()
            .expect("tab lock poisoned")
            .iter()
            .find_map(|(client, (bound, _))| (*bound == tab_id).then_some(*client))
            .ok_or_else(BridgeError::not_connected)
    }

    fn drop_session(&self, client_id: ClientId) {
        if let Some(live) = self
            .inner
            .sessions
            .lock()
            .expect("sessions lock")
            .remove(&client_id)
        {
            live.pump_abort.abort();
            live.forwarder.stop();
            drop(live);
        }
        self.inner
            .bootstraps
            .lock()
            .expect("bootstrap lock")
            .remove(&client_id);
        self.inner
            .bindings
            .lock()
            .expect("tab lock poisoned")
            .remove(&client_id);
        let mut active = self.inner.active_client.lock().expect("active lock");
        if *active == Some(client_id) {
            *active = self
                .inner
                .sessions
                .lock()
                .expect("sessions lock")
                .keys()
                .next()
                .copied();
        }
    }

    async fn handshake_and_install(
        &self,
        reclaim: Option<(TabId, String)>,
        new_root: Option<String>,
    ) -> Result<BootstrapDto, BridgeError> {
        let endpoint = self.inner.endpoint.clone();
        let handshake = async move {
            if let Some((tab_id, workspace_root)) = reclaim {
                connect_for_reclaim_or_new(&endpoint, tab_id, workspace_root).await
            } else {
                connect_with_workspace_root(&endpoint, new_root.unwrap_or_default()).await
            }
        };
        let session = match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await {
            Err(_) => return Err(BridgeError::timeout()),
            Ok(Err(error)) => return Err(BridgeError::server_unreachable(error)),
            Ok(Ok(session)) => session,
        };

        let initial_state: ClientInitialState = session.initial_state.clone();
        let generation = self.inner.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let client_id = initial_state.client_id;

        let forwarder = Arc::new(Forwarder::spawn(Arc::clone(&self.inner.sinks)));

        // Event pump: translate validated client-layer events into envelopes
        // until aborted (takeover/teardown) or the connection ends.
        let mut pump_events = session.events;
        let pump_forwarder = Arc::clone(&forwarder);
        let pump_bindings = Arc::clone(&self.inner.bindings);
        let pump_agent_relay = Arc::clone(&self.inner.agent_relay);
        let pump = tokio::spawn(async move {
            while let Some(event) = pump_events.recv().await {
                if let ClientConnectionEvent::TabRegistry(ref snapshot) = event
                    && let Some(entry) = snapshot
                        .tabs
                        .iter()
                        .find(|entry| entry.client_id == client_id)
                {
                    pump_bindings
                        .lock()
                        .expect("tab lock poisoned")
                        .insert(client_id, (entry.tab_id, entry.workspace_root.clone()));
                }
                let tab_id = pump_bindings
                    .lock()
                    .expect("tab lock poisoned")
                    .get(&client_id)
                    .map(|(tab_id, _)| *tab_id);
                match event {
                    ClientConnectionEvent::Disconnected => {
                        pump_forwarder.push_disconnected_from(
                            "connection closed".into(),
                            Some(client_id),
                            tab_id,
                        );
                    }
                    ClientConnectionEvent::ConnectionError(detail) => {
                        pump_forwarder.push_disconnected_from(detail, Some(client_id), tab_id);
                    }
                    ClientConnectionEvent::RuntimeStateSnapshot(snapshot) => {
                        match super::dto::RuntimeSnapshotDto::resolve(*snapshot) {
                            Ok(snapshot) => {
                                pump_forwarder.push_runtime_snapshot(client_id, tab_id, snapshot)
                            }
                            Err(rejection) => {
                                pump_forwarder
                                    .push(ClientConnectionEvent::RuntimeDiagnostic(
                                        clay::protocol::RuntimeDiagnostic {
                                            severity: clay::protocol::DiagnosticSeverity::Error,
                                            code: "runtime.snapshot_rejected".into(),
                                            message: rejection,
                                        },
                                    ))
                                    .await;
                            }
                        }
                    }
                    // Resolve server theme changes through the Rust authority
                    // before they cross the bridge; raw overrides never reach
                    // the webview. A rejected (below-AA) snapshot is dropped
                    // and surfaced as a diagnostic instead of a broken theme.
                    ClientConnectionEvent::ActiveTheme(theme) => {
                        match ThemeSnapshotDto::resolve(&theme.specifier, &theme) {
                            Ok(snapshot) => pump_forwarder.push_theme_snapshot(snapshot),
                            Err(rejection) => {
                                pump_forwarder
                                    .push(ClientConnectionEvent::RuntimeDiagnostic(
                                        clay::protocol::RuntimeDiagnostic {
                                            severity: clay::protocol::DiagnosticSeverity::Error,
                                            code: "theme.rejected".into(),
                                            message: format!("theme update rejected: {rejection}"),
                                        },
                                    ))
                                    .await;
                            }
                        }
                    }
                    ClientConnectionEvent::Agent(payload) => {
                        // AG-UI adaptation happens in Rust; raw Clay agent
                        // frames never reach the webview (Phase 10).
                        pump_agent_relay.deliver(client_id, tab_id, &payload);
                    }
                    event => pump_forwarder.push_routed(client_id, tab_id, event).await,
                }
            }
        });

        let dto = BootstrapDto {
            client_id,
            tab_id: None,
            protocol_version: clay::protocol::PROTOCOL_VERSION,
            endpoint: self.inner.endpoint.to_string(),
            generation,
            active_theme: ThemeSnapshotDto::resolve(
                &initial_state.active_theme.specifier,
                &initial_state.active_theme,
            )
            .map_err(BridgeError::invalid_request)?,
            active_typography: TypographySnapshotDto::from(&initial_state.active_typography),
            initial_document: InitialDocumentDto::from_initial_state(&initial_state),
            behavior_manifest: initial_state.behavior_manifest.clone(),
        };

        *self.inner.last_bootstrap.lock().expect("bootstrap lock") = Some(dto.clone());
        self.inner
            .bootstraps
            .lock()
            .expect("bootstrap lock")
            .insert(client_id, dto.clone());
        self.inner.sessions.lock().expect("sessions lock").insert(
            client_id,
            LiveSession {
                client_id,
                edit_queue: session.edit_queue.clone(),
                forwarder,
                pump_abort: pump.abort_handle(),
            },
        );
        let mut active = self.inner.active_client.lock().expect("active lock");
        if active.is_none() {
            *active = Some(client_id);
        }
        Ok(dto)
    }

    /// Tears down every live session and clears cached bootstrap state.
    pub fn teardown(&self) {
        let sessions = std::mem::take(&mut *self.inner.sessions.lock().expect("sessions lock"));
        for live in sessions.into_values() {
            live.pump_abort.abort();
            live.forwarder.stop();
            drop(live);
        }
        *self.inner.last_bootstrap.lock().expect("bootstrap lock") = None;
        self.inner
            .bootstraps
            .lock()
            .expect("bootstrap lock")
            .clear();
        *self.inner.active_client.lock().expect("active lock") = None;
        self.inner
            .bindings
            .lock()
            .expect("tab lock poisoned")
            .clear();
    }

    /// Subscribe a sink to session events. Replaces prior subscriptions so
    /// there is exactly one live listener per webview.
    pub fn subscribe<S: super::forwarder::EventSink>(&self, sink: S) {
        self.inner.sinks.clear();
        self.inner.sinks.add(Arc::new(sink));
    }

    pub fn unsubscribe(&self) {
        self.inner.sinks.clear();
    }

    /// Registers one webview channel on the AG-UI agent relay (Phase 10).
    /// Replaces nothing: multiple windows may observe the same stream.
    pub fn subscribe_agent(&self, channel: tauri::ipc::Channel<super::agent::AgentStreamEvent>) {
        self.inner.agent_relay.subscribe(channel);
    }

    /// Drops every AG-UI agent relay registration.
    pub fn unsubscribe_agent(&self) {
        self.inner.agent_relay.unsubscribe();
    }

    /// Diagnostics: number of live AG-UI relay subscriptions.
    pub fn agent_subscriber_count(&self) -> usize {
        self.inner.agent_relay.subscriber_count()
    }

    /// True while a session is installed.
    pub fn is_connected(&self) -> bool {
        !self
            .inner
            .sessions
            .lock()
            .expect("sessions lock")
            .is_empty()
    }

    /// Diagnostics for status surfaces/tests.
    pub fn stats(&self) -> BridgeStats {
        let sessions = self.inner.sessions.lock().expect("sessions lock");
        BridgeStats {
            connected: !sessions.is_empty(),
            coalesced: sessions
                .values()
                .map(|live| live.forwarder.coalesced_count())
                .sum(),
            generation: self.inner.generation.load(Ordering::Relaxed),
        }
    }

    /// Validates, size-caps, stamps, and forwards one frontend request.
    ///
    /// `payload` is raw JSON so the byte cap applies before parsing. Edits are
    /// routed through the optimistic edit queue (version bookkeeping is
    /// bridge-owned); everything else goes verbatim with the session's
    /// server-issued identity stamped over whatever the caller supplied.
    pub fn request(&self, payload: &str) -> Result<(), BridgeError> {
        self.request_on(None, payload)
    }

    pub fn request_on(&self, tab_id: Option<TabId>, payload: &str) -> Result<(), BridgeError> {
        if payload.len() > MAX_REQUEST_BYTES {
            return Err(BridgeError::request_too_large(payload.len()));
        }
        let message: ClientMessage =
            serde_json::from_str(payload).map_err(BridgeError::invalid_request)?;

        let (client_id, edit_queue) = {
            let resolved = if let Some(tab_id) = tab_id {
                Some(self.client_for_tab(tab_id)?)
            } else {
                *self.inner.active_client.lock().expect("active lock")
            };
            let sessions = self.inner.sessions.lock().expect("sessions lock");
            let client_id = resolved
                .or_else(|| sessions.keys().next().copied())
                .ok_or_else(BridgeError::not_connected)?;
            let live = sessions
                .get(&client_id)
                .ok_or_else(BridgeError::not_connected)?;
            (live.client_id, live.edit_queue.clone())
        };

        match message {
            ClientMessage::Hello { .. } => Err(BridgeError::forbidden(
                "handshake is bridge-owned; use session_bootstrap",
            )),
            ClientMessage::Edit {
                document_id,
                operation,
                behavior_version,
                transaction_id,
                ..
            } => {
                let event = EditorEditEvent {
                    document_id,
                    base_version: 0, // reserve_pending derives the authoritative base
                    behavior_version,
                    operation,
                };
                edit_queue
                    .enqueue_edit_event(event, transaction_id)
                    .map_err(|error| match error {
                        tokio::sync::mpsc::error::TrySendError::Full(_) => {
                            BridgeError::queue_full()
                        }
                        other => BridgeError::invalid_request(other),
                    })
            }
            stamped => {
                let stamped = stamp_client_id(stamped, client_id)?;
                edit_queue
                    .enqueue_raw(stamped)
                    .map_err(|error| match error {
                        tokio::sync::mpsc::error::TrySendError::Full(_) => {
                            BridgeError::queue_full()
                        }
                        other => BridgeError::invalid_request(other),
                    })
            }
        }
    }
}

/// Diagnostics snapshot for status surfaces/tests.
#[derive(serde::Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStats {
    pub connected: bool,
    pub coalesced: u64,
    pub generation: u64,
}

/// Overwrite every client-supplied identity with the session's server-issued
/// one. Exhaustive over variants carrying `client_id`; adding a variant
/// without updating this function is a compile error by design.
fn stamp_client_id(
    message: ClientMessage,
    client_id: ClientId,
) -> Result<ClientMessage, BridgeError> {
    Ok(match message {
        ClientMessage::Hello { .. } => {
            return Err(BridgeError::forbidden("handshake is bridge-owned"));
        }
        ClientMessage::Edit { .. } => {
            return Err(BridgeError::forbidden(
                "edits route through the optimistic queue",
            ));
        }
        ClientMessage::EditorIntent {
            document_id,
            lease_id,
            base_version,
            behavior_version,
            transaction_id,
            intent,
            ..
        } => ClientMessage::EditorIntent {
            client_id,
            document_id,
            lease_id,
            base_version,
            behavior_version,
            transaction_id,
            intent,
        },
        ClientMessage::RequestResync {
            document_id,
            known_version,
            ..
        } => ClientMessage::RequestResync {
            client_id,
            document_id,
            known_version,
        },
        ClientMessage::DecorationViewportRequest {
            document_id,
            document_version,
            byte_start,
            byte_end,
            ..
        } => ClientMessage::DecorationViewportRequest {
            client_id,
            document_id,
            document_version,
            byte_start,
            byte_end,
        },
        ClientMessage::OpenDocument {
            workspace_root_id,
            path,
            ..
        } => ClientMessage::OpenDocument {
            client_id,
            workspace_root_id,
            path,
        },
        ClientMessage::OpenSelectedFile {
            capability,
            selected_path,
            ..
        } => ClientMessage::OpenSelectedFile {
            client_id,
            capability,
            selected_path,
        },
        ClientMessage::AddSelectedWorkspaceRoot {
            capability,
            selected_path,
            ..
        } => ClientMessage::AddSelectedWorkspaceRoot {
            client_id,
            capability,
            selected_path,
        },
        ClientMessage::SaveDocument {
            document_id,
            known_version,
            ..
        } => ClientMessage::SaveDocument {
            client_id,
            document_id,
            known_version,
        },
        ClientMessage::ReloadDocument {
            document_id,
            known_version,
            force,
            ..
        } => ClientMessage::ReloadDocument {
            client_id,
            document_id,
            known_version,
            force,
        },
        ClientMessage::GetDocumentStatus { document_id, .. } => ClientMessage::GetDocumentStatus {
            client_id,
            document_id,
        },
        ClientMessage::ListDocuments { .. } => ClientMessage::ListDocuments { client_id },
        ClientMessage::SduiAction {
            ui_version, intent, ..
        } => ClientMessage::SduiAction {
            client_id,
            ui_version,
            intent,
        },
        ClientMessage::CommandIntent {
            document_id,
            behavior_version,
            command_id,
            ..
        } => ClientMessage::CommandIntent {
            client_id,
            document_id,
            behavior_version,
            command_id,
        },
        ClientMessage::CompletionRequest { mut request } => {
            request.client_id = client_id;
            ClientMessage::CompletionRequest { request }
        }
        ClientMessage::LanguageIntelligenceRequest { mut request } => {
            request.client_id = client_id;
            ClientMessage::LanguageIntelligenceRequest { request }
        }
        ClientMessage::SelectionQueryRequest { mut request } => {
            request.client_id = client_id;
            ClientMessage::SelectionQueryRequest { request }
        }
        ClientMessage::RuntimeGenerationInstalled {
            runtime_generation_id,
            ..
        } => ClientMessage::RuntimeGenerationInstalled {
            client_id,
            runtime_generation_id,
        },
        ClientMessage::CloseDocument {
            document_id, force, ..
        } => ClientMessage::CloseDocument {
            client_id,
            document_id,
            force,
        },
        ClientMessage::TabCommand { command, .. } => {
            ClientMessage::TabCommand { client_id, command }
        }
        ClientMessage::MenuQueryUpdate {
            session_id, query, ..
        } => ClientMessage::MenuQueryUpdate {
            client_id,
            session_id,
            query,
        },
        ClientMessage::MenuBackspace { session_id, .. } => ClientMessage::MenuBackspace {
            client_id,
            session_id,
        },
        ClientMessage::MenuSelectionMove {
            session_id, delta, ..
        } => ClientMessage::MenuSelectionMove {
            client_id,
            session_id,
            delta,
        },
        ClientMessage::MenuActivate {
            session_id, kind, ..
        } => ClientMessage::MenuActivate {
            client_id,
            session_id,
            kind,
        },
        ClientMessage::MenuCancel { session_id, .. } => ClientMessage::MenuCancel {
            client_id,
            session_id,
        },
        msg @ ClientMessage::Agent { .. } => msg,
    })
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn reactive_requests_cannot_forge_client_identity() {
        let completion: ClientMessage = serde_json::from_value(serde_json::json!({
            "family": "completionRequest",
            "payload": { "request": {
                "requestId": 1, "clientId": 999, "documentId": 2,
                "documentVersion": 3, "behaviorVersion": 4,
                "cursorByteOffset": 5,
                "replacementRange": { "byteStart": 0, "byteEnd": 5 },
                "trigger": "manual", "providerGeneration": 0,
                "recentCompletions": []
            }}
        }))
        .unwrap();
        let stamped = stamp_client_id(completion, 7).unwrap();
        assert_eq!(
            serde_json::to_value(stamped).unwrap()["payload"]["request"]["clientId"],
            7
        );

        let intelligence: ClientMessage = serde_json::from_value(serde_json::json!({
            "family": "languageIntelligenceRequest",
            "payload": { "request": {
                "requestId": 2, "clientId": 999, "documentId": 2,
                "documentVersion": 3, "behaviorVersion": 4,
                "cursorByteOffset": 5, "feature": "hover",
                "providerGeneration": 0
            }}
        }))
        .unwrap();
        let stamped = stamp_client_id(intelligence, 7).unwrap();
        assert_eq!(
            serde_json::to_value(stamped).unwrap()["payload"]["request"]["clientId"],
            7
        );
    }
}
