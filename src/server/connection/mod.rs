use std::{
    collections::HashSet,
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};

use crate::protocol::{
    AgentServerMessage, ClientId, ClientMessage, DocumentId, PROTOCOL_VERSION, ProtocolErrorCode,
    RuntimeDiagnostic, ServerMessage, TabCommand, TabId, TabRegistrySnapshot, WorkspaceRootId,
    codec::{Codec, CodecError},
};

use super::{
    RuntimeGenerationStore, TabServerState,
    behavior::ActiveBehaviorManifest,
    document::DocumentState,
    language_intelligence::LanguageIntelligenceCoordinator,
    menu_sessions::ServerMenuSessions,
    output_router::OutputRouter,
    parse_coordinator::ParseCoordinator,
    sdui::StaticSduiState,
    tab_registry::TabRegistry,
    workspace::{WorkspaceError, WorkspaceState},
};

// Plan 090 task 2: connection dispatch families. The loop in this module is
// the single dispatch owner; each family module holds one coherent set of
// responsibilities. Everything stays crate-private (pub(super)); no public
// surface is created by the split.
mod documents;
mod menus;
mod runtime;
mod tabs;
mod workspace;

pub(crate) use self::documents::open_document_followup_messages;
// Family re-exports keep the module's `mod tests` on the pre-split namespace:
// tests reference moved helpers unqualified. Test-only: the coordinator itself
// calls family handlers through module paths.
#[cfg(test)]
use self::documents::*;
// menus/runtime/tabs/workspace helpers are referenced only through their
// family modules (or explicit test imports), so their globs are omitted.

/// Bounded, deduplicating runtime-diagnostic retention (Plan 060 T6, P1-8).
/// Consecutive duplicates collapse to one entry; past the capacity the oldest
/// entry drops and the drop count is retained for observability. Retention is
/// aligned with the snapshot publication cap so welcome/runtime snapshots
/// never grow past the frame budget.
#[derive(Debug, Default)]
pub(crate) struct RuntimeDiagnosticStore {
    entries: std::collections::VecDeque<RuntimeDiagnostic>,
    dropped: u64,
    live_router: Arc<std::sync::Mutex<OutputRouter<RuntimeDiagnostic>>>,
}

impl RuntimeDiagnosticStore {
    pub(crate) fn push(&mut self, diagnostic: RuntimeDiagnostic) {
        if self.entries.back() == Some(&diagnostic) {
            return;
        }
        if self.entries.len() >= crate::perf::budgets::RUNTIME_DIAGNOSTIC_CAPACITY {
            self.entries.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.entries.push_back(diagnostic);
    }

    pub(crate) fn publish(&mut self, diagnostic: RuntimeDiagnostic) {
        if self.entries.back() == Some(&diagnostic) {
            return;
        }
        self.push(diagnostic.clone());
        self.live_router
            .lock()
            .expect("runtime diagnostic router lock poisoned")
            .broadcast(&diagnostic);
    }

    pub(crate) fn live_router(&self) -> Arc<std::sync::Mutex<OutputRouter<RuntimeDiagnostic>>> {
        Arc::clone(&self.live_router)
    }

    pub(crate) fn snapshot(&self) -> Vec<RuntimeDiagnostic> {
        self.entries.iter().cloned().collect()
    }

    #[cfg(test)]
    pub(crate) fn dropped_count(&self) -> u64 {
        self.dropped
    }
}

/// Withdraws a connection's parse/analysis output subscriptions on drop so
/// every exit path (clean close, IO error, disconnect) fails closed without
/// leaking routed payloads to a recycled client identity (Plan 060 T4).
struct ConnectionOutputSubscriptions {
    parse_coordinator: ParseCoordinator,
    document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
    runtime_diagnostic_router: Arc<std::sync::Mutex<OutputRouter<RuntimeDiagnostic>>>,
    client_id: ClientId,
}

impl Drop for ConnectionOutputSubscriptions {
    fn drop(&mut self) {
        self.parse_coordinator.unsubscribe_client(self.client_id);
        self.document_analysis.unsubscribe_client(self.client_id);
        self.runtime_diagnostic_router
            .lock()
            .expect("runtime diagnostic router lock poisoned")
            .unsubscribe_client(self.client_id);
    }
}

/// Extract the legacy caller-supplied identity from any post-`Hello` message.
/// The dispatch loop compares this against the connection's handshake-assigned
/// `client_id` exactly once; downstream arms only ever see the canonical
/// connection identity (Plan 060 T4, P0-2).
fn client_message_identity(message: &ClientMessage) -> Option<ClientId> {
    match message {
        ClientMessage::Hello { .. } => None,
        ClientMessage::Edit { client_id, .. }
        | ClientMessage::EditorIntent { client_id, .. }
        | ClientMessage::RequestResync { client_id, .. }
        | ClientMessage::DecorationViewportRequest { client_id, .. }
        | ClientMessage::OpenDocument { client_id, .. }
        | ClientMessage::OpenSelectedFile { client_id, .. }
        | ClientMessage::AddSelectedWorkspaceRoot { client_id, .. }
        | ClientMessage::SaveDocument { client_id, .. }
        | ClientMessage::ReloadDocument { client_id, .. }
        | ClientMessage::GetDocumentStatus { client_id, .. }
        | ClientMessage::ListDocuments { client_id }
        | ClientMessage::SduiAction { client_id, .. }
        | ClientMessage::CommandIntent { client_id, .. }
        | ClientMessage::RuntimeGenerationInstalled { client_id, .. }
        | ClientMessage::CloseDocument { client_id, .. }
        | ClientMessage::TabCommand { client_id, .. }
        | ClientMessage::MenuQueryUpdate { client_id, .. }
        | ClientMessage::MenuBackspace { client_id, .. }
        | ClientMessage::MenuSelectionMove { client_id, .. }
        | ClientMessage::MenuActivate { client_id, .. }
        | ClientMessage::MenuCancel { client_id, .. }
        | ClientMessage::Agent { client_id, .. } => Some(*client_id),
        ClientMessage::CompletionRequest { request } => Some(request.client_id),
        ClientMessage::LanguageIntelligenceRequest { request } => Some(request.client_id),
        ClientMessage::SelectionQueryRequest { request } => Some(request.client_id),
    }
}

/// Per-message state route. Production connections must already be bound to a
/// live registry entry; test-only handlers without an `IpcServer` retain their
/// explicit bootstrap handles.
#[derive(Debug, Clone)]
struct RoutedTabState {
    tab_id: Option<TabId>,
    state: TabServerState,
}

async fn route_connection_tab_state(
    client_id: ClientId,
    reload_server: Option<&super::IpcServer>,
    bootstrap_document: &Arc<Mutex<DocumentState>>,
    bootstrap_workspace: &Arc<Mutex<WorkspaceState>>,
) -> Option<RoutedTabState> {
    if let Some(server) = reload_server {
        if let Some((tab_id, state)) = server.tab_state_for_client(client_id).await {
            return Some(RoutedTabState {
                tab_id: Some(tab_id),
                state,
            });
        }
        return server
            .unbound_bootstrap_state()
            .await
            .map(|state| RoutedTabState {
                tab_id: None,
                state,
            });
    }

    Some(RoutedTabState {
        tab_id: None,
        state: TabServerState {
            welcome: Arc::clone(bootstrap_document),
            workspace: Arc::clone(bootstrap_workspace),
            workspace_pane_visible: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        },
    })
}

fn message_requires_tab_state(message: &ClientMessage) -> bool {
    matches!(
        message,
        ClientMessage::Edit { .. }
            | ClientMessage::EditorIntent { .. }
            | ClientMessage::RequestResync { .. }
            | ClientMessage::DecorationViewportRequest { .. }
            | ClientMessage::OpenDocument { .. }
            | ClientMessage::OpenSelectedFile { .. }
            | ClientMessage::AddSelectedWorkspaceRoot { .. }
            | ClientMessage::SaveDocument { .. }
            | ClientMessage::ReloadDocument { .. }
            | ClientMessage::CloseDocument { .. }
            | ClientMessage::GetDocumentStatus { .. }
            | ClientMessage::ListDocuments { .. }
            | ClientMessage::SduiAction { .. }
            | ClientMessage::CommandIntent { .. }
            | ClientMessage::CompletionRequest { .. }
            | ClientMessage::LanguageIntelligenceRequest { .. }
            | ClientMessage::SelectionQueryRequest { .. }
            | ClientMessage::TabCommand {
                command: TabCommand::OpenWorkspace { .. },
                ..
            }
            // Phase 24.1: server-owned menu sessions are per-connection (per
            // tab) state; the intents need the bound tab server state.
            | ClientMessage::MenuQueryUpdate { .. }
            | ClientMessage::MenuBackspace { .. }
            | ClientMessage::MenuSelectionMove { .. }
            | ClientMessage::MenuActivate { .. }
            | ClientMessage::MenuCancel { .. }
    )
}

fn unbound_tab_state_error() -> ServerMessage {
    ServerMessage::Error {
        code: ProtocolErrorCode::InvalidMessage,
        message: "connection is not bound to a live tab".to_string(),
    }
}

/// Phase 24.1: bounded diagnostic for menu intents naming a session this
/// connection does not hold (stale after cancel/activate/replace, or never
/// opened). Never an error or disconnect.
fn unknown_menu_session_diagnostic(client_id: u64, session_id: u64) -> ServerMessage {
    ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic {
        severity: crate::protocol::DiagnosticSeverity::Info,
        code: "menu.unknown_session".to_string(),
        message: format!("no active menu session for id {session_id} (client {client_id})"),
    })
}

fn tab_binding_conflict_error() -> ServerMessage {
    ServerMessage::Error {
        code: ProtocolErrorCode::InvalidMessage,
        message: "connection is already bound to a different tab".to_string(),
    }
}

fn new_tab_binding_conflict_error() -> ServerMessage {
    ServerMessage::FileOperationFailed {
        code: crate::protocol::FileErrorCode::AccessDenied,
        message: "connection is already bound to a tab".to_string(),
        workspace_root_id: None,
        document_id: None,
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
async fn handle_connection<S>(
    stream: S,
    client_id: u64,
    document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    sdui: Arc<Mutex<StaticSduiState>>,
    active_theme: Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: Arc<Mutex<RuntimeDiagnosticStore>>,
    runtime_generation: RuntimeGenerationStore,
    parse_coordinator: ParseCoordinator,
    language_intelligence: LanguageIntelligenceCoordinator,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    handle_connection_with_analysis(
        stream,
        client_id,
        document,
        behavior,
        workspace,
        sdui,
        active_theme,
        runtime_diagnostics,
        runtime_generation,
        parse_coordinator,
        crate::server::completion::CompletionCoordinator::new(),
        crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
        language_intelligence,
        None,
        Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new())),
        tokio::sync::broadcast::channel(16).0,
        codec,
    )
    .await
}

#[allow(
    clippy::too_many_arguments,
    reason = "connection handler receives server-owned state explicitly instead of hiding authority in a context bag"
)]
pub(crate) async fn handle_connection_with_analysis<S>(
    stream: S,
    client_id: u64,
    document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    sdui: Arc<Mutex<StaticSduiState>>,
    active_theme: Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: Arc<Mutex<RuntimeDiagnosticStore>>,
    runtime_generation: RuntimeGenerationStore,
    parse_coordinator: ParseCoordinator,
    completion: crate::server::completion::CompletionCoordinator,
    document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
    language_intelligence: LanguageIntelligenceCoordinator,
    reload_server: Option<super::IpcServer>,
    tab_registry: Arc<Mutex<TabRegistry>>,
    tab_registry_tx: tokio::sync::broadcast::Sender<TabRegistrySnapshot>,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let cleanup_document = Arc::clone(&document);
    let cleanup_workspace = Arc::clone(&workspace);
    let cleanup_server = reload_server.clone();
    let cleanup_bound_state = Arc::new(std::sync::Mutex::new(None));
    let cleanup_parse = parse_coordinator.clone();
    let cleanup_completion = completion.clone();
    let cleanup_language_intelligence = language_intelligence.clone();
    let cleanup_document_analysis = document_analysis.clone();
    let result = handle_connection_loop(
        stream,
        client_id,
        document,
        behavior,
        workspace,
        Arc::clone(&cleanup_bound_state),
        sdui,
        active_theme,
        runtime_diagnostics,
        runtime_generation,
        parse_coordinator,
        completion,
        document_analysis,
        language_intelligence,
        reload_server,
        tab_registry,
        tab_registry_tx,
        codec,
    )
    .await;

    // A peer closing while asynchronous output is pending is a normal
    // disconnect, matching read-pump EOF/reset handling.
    let result = match result {
        Err(CodecError::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
            ) =>
        {
            Ok(())
        }
        result => result,
    };

    // A connection starts on bootstrap state for the legacy handshake, then
    // switches to its bound tab after `New`/`Reclaim`. Clean the last state
    // actually routed to this connection even when a later `Reclaim` removed
    // its registry binding before the old connection exited.
    let tracked_state = cleanup_bound_state.lock().unwrap().clone();
    let tracked_state = match tracked_state {
        Some(state) => Some(state),
        None => match cleanup_server.as_ref() {
            Some(server) => server.state_for_client(client_id).await,
            None => None,
        },
    };
    if let Some(state) = tracked_state
        && (!Arc::ptr_eq(&state.welcome, &cleanup_document)
            || !Arc::ptr_eq(&state.workspace, &cleanup_workspace))
    {
        cleanup_connection_documents(
            client_id,
            &state.welcome,
            &state.workspace,
            &cleanup_parse,
            &cleanup_completion,
            &cleanup_language_intelligence,
            &cleanup_document_analysis,
        )
        .await;
    }

    // Every exit path, including failed asynchronous server writes, releases
    // document authority and document-scoped coordinator state.
    cleanup_connection_documents(
        client_id,
        &cleanup_document,
        &cleanup_workspace,
        &cleanup_parse,
        &cleanup_completion,
        &cleanup_language_intelligence,
        &cleanup_document_analysis,
    )
    .await;
    result
}

#[allow(
    clippy::too_many_arguments,
    reason = "connection loop receives server-owned state explicitly instead of hiding authority in a context bag"
)]
async fn handle_connection_loop<S>(
    mut stream: S,
    client_id: u64,
    mut document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    mut workspace: Arc<Mutex<WorkspaceState>>,
    bound_state: Arc<std::sync::Mutex<Option<TabServerState>>>,
    sdui: Arc<Mutex<StaticSduiState>>,
    active_theme: Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: Arc<Mutex<RuntimeDiagnosticStore>>,
    runtime_generation: RuntimeGenerationStore,
    parse_coordinator: ParseCoordinator,
    completion: crate::server::completion::CompletionCoordinator,
    document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
    language_intelligence: LanguageIntelligenceCoordinator,
    reload_server: Option<super::IpcServer>,
    tab_registry: Arc<Mutex<TabRegistry>>,
    tab_registry_tx: tokio::sync::broadcast::Sender<TabRegistrySnapshot>,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut typography_updates = runtime_generation.subscribe_typography();
    let mut runtime_state_updates = runtime_generation.subscribe_runtime_state();
    // Follow-up round (`editor-control`): advisory gated programmatic
    // editor-command execution requests. Lagged requests drop (advisory);
    // the channel survives generation replacement so one subscription
    // covers the connection's lifetime.
    let mut editor_command_updates = runtime_generation.subscribe_editor_commands().await;
    // Plan 071 caret-transport fix: runtime caret override lane. Shares the
    // editor-command channel's lifetime semantics: survives generation
    // replacement, so one subscription covers the connection; lag replays
    // the current value instead of dropping state.
    let mut caret_style_updates = runtime_generation.subscribe_caret_styles().await;
    // Phase 26: user-owned editor wrap-policy override lane. Same lifetime
    // semantics: survives generation replacement, lag replays the current
    // value. Trusted-domain only; packages cannot forge it.
    let mut editor_layout_updates = runtime_generation.subscribe_editor_layout().await;
    // Phase 22.1: shell-preferences lane. Same lifetime semantics: survives
    // generation replacement, lag replays the current value.
    let mut shell_preferences_updates = runtime_generation.subscribe_shell_preferences().await;
    // Phase 22.3: tab-registry lane. Subscribed before the handshake replay so
    // a mutation between subscribe and replay is both buffered and replayed
    // (idempotent); lag replays the current snapshot from the mutex.
    let mut tab_registry_updates = tab_registry_tx.subscribe();
    // Plan 060 T6 (P1-8): bounded per-connection result lanes. A saturated
    // lane means the client is not reading; results drop with a counter and
    // log line instead of growing memory without bound.
    let (completion_tx, mut completion_rx) = tokio::sync::mpsc::channel::<ServerMessage>(
        crate::perf::budgets::CONNECTION_RESULT_LANE_CAPACITY,
    );
    let (language_intelligence_tx, mut language_intelligence_rx) =
        tokio::sync::mpsc::channel::<ServerMessage>(
            crate::perf::budgets::CONNECTION_RESULT_LANE_CAPACITY,
        );
    let mut agent_rx = reload_server
        .as_ref()
        .map(|server| server.agent.subscribe());
    let dropped_results = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    // Plan 060 T4 (P0-3): authorized per-connection subscriptions. Parse and
    // analysis payloads route only to documents this connection opened; the
    // guard withdraws every subscription on any exit path.
    let (mut parse_updates_rx, mut parse_diagnostics_rx) =
        parse_coordinator.subscribe_client(client_id);
    let mut analysis_rx = document_analysis.subscribe_client(client_id);
    let runtime_diagnostic_router = runtime_diagnostics.lock().await.live_router();
    let mut runtime_diagnostics_rx = runtime_diagnostic_router
        .lock()
        .expect("runtime diagnostic router lock poisoned")
        .subscribe_client(client_id);
    let _subscriptions = ConnectionOutputSubscriptions {
        parse_coordinator: parse_coordinator.clone(),
        document_analysis: document_analysis.clone(),
        runtime_diagnostic_router,
        client_id,
    };
    let bootstrap_document = Arc::clone(&document);
    let bootstrap_workspace = Arc::clone(&workspace);
    let default_document_id = document.lock().await.document_id();
    parse_coordinator.subscribe_document(default_document_id, client_id);
    document_analysis.subscribe_document(default_document_id, client_id);
    let first_message = codec.read_client_message(&mut stream).await?;
    let mut file_open_capabilities = match first_message {
        ClientMessage::Hello {
            protocol_version,
            client_name: _,
        } if protocol_version == PROTOCOL_VERSION => {
            send_welcome_snapshot_and_manifest(
                &mut stream,
                client_id,
                &behavior,
                &active_theme,
                &runtime_diagnostics,
                &runtime_generation,
                if reload_server.is_none() {
                    Some((&bootstrap_document, &bootstrap_workspace, &sdui))
                } else {
                    None
                },
                codec,
            )
            .await?;
            // Phase 22.3: handshake replay of the current tab registry so a
            // fresh/reconnecting connection learns the existing tabs.
            let snapshot = tab_registry.lock().await.snapshot();
            codec
                .write_server_message(&mut stream, &ServerMessage::TabRegistry(snapshot))
                .await?;
            // ponytail: per-connection capability token. Structural authority
            // gate for single-file opens; not a hard boundary against a
            // malicious same-user client that can also complete Hello. Full
            // defense needs the long-term OS-verifiable picker exchange.
            let mut file_open_capabilities = FileOpenCapabilityPool::new();
            let initial_capability = file_open_capabilities.issue();
            codec
                .write_server_message(
                    &mut stream,
                    &ServerMessage::FileOpenCapabilityIssued {
                        token: initial_capability,
                    },
                )
                .await?;
            file_open_capabilities
        }
        ClientMessage::Hello { .. } => {
            codec
                .write_server_message(
                    &mut stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::UnsupportedProtocolVersion,
                        message: "unsupported protocol version".to_string(),
                    },
                )
                .await?;
            return Ok(());
        }
        _ => {
            codec
                .write_server_message(
                    &mut stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message: "first client message must be Hello".to_string(),
                    },
                )
                .await?;
            return Ok(());
        }
    };

    // Cancellation-safety: framed reads run in a dedicated pump task so a
    // winning select branch can never strand a partially-read frame
    // (`AsyncReadExt::read_exact` is not cancellation-safe). The loop below
    // selects only over channels; `stream` is now the single owned write half.
    let (mut reader, mut stream) = tokio::io::split(stream);
    let (incoming_tx, mut incoming_rx) =
        tokio::sync::mpsc::channel::<Result<ClientMessage, CodecError>>(64);
    let read_pump = tokio::spawn(async move {
        loop {
            match codec.read_client_message(&mut reader).await {
                Ok(message) => {
                    if incoming_tx.send(Ok(message)).await.is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = incoming_tx.send(Err(error)).await;
                    return;
                }
            }
        }
    });
    let _read_pump_guard = crate::protocol::codec::ReadPumpGuard::new(read_pump.abort_handle());

    let mut bound_tab_id = None;
    // Phase 24.1-24.3: per-connection server menu session store. One active
    // session; drops with this function, sweeping every session on any exit
    // path (no cross-connection leak). Sessions open only from the built-in
    // `controlCenter.open` / `controlCenter.openPath` command paths (task 6).
    let mut menu_sessions = ServerMenuSessions::new();
    loop {
        let message = match tokio::select! {
            typography = typography_updates.recv() => match typography {
                Ok(typography) => {
                    codec
                        .write_server_message(&mut stream, &ServerMessage::ActiveTypography(typography))
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let typography = runtime_generation.active_typography().await;
                    codec
                        .write_server_message(&mut stream, &ServerMessage::ActiveTypography(typography))
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            editor_command = editor_command_updates.recv() => match editor_command {
                Ok(request) => {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::EditorCommandRequest(Box::new(request)),
                        )
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Advisory execution requests never replay: drop and move on.
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            caret_style = caret_style_updates.recv() => match caret_style {
                Ok(style) => {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::CaretStyleOverride(style),
                        )
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // State, not advice: replay the current value.
                    let style = runtime_generation.caret_style_override().await;
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::CaretStyleOverride(style),
                        )
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            layout = editor_layout_updates.recv() => match layout {
                Ok(wrap) => {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::EditorLayoutOverride(wrap),
                        )
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // State, not advice: replay the current value.
                    let wrap = runtime_generation.editor_layout_override().await;
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::EditorLayoutOverride(wrap),
                        )
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            prefs = shell_preferences_updates.recv() => match prefs {
                Ok(preferences) => {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::ShellPreferences(preferences),
                        )
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let preferences = runtime_generation.shell_preferences().await;
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::ShellPreferences(preferences),
                        )
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            tab_registry_update = tab_registry_updates.recv() => match tab_registry_update {
                Ok(snapshot) => {
                    codec
                        .write_server_message(&mut stream, &ServerMessage::TabRegistry(snapshot))
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let snapshot = tab_registry.lock().await.snapshot();
                    codec
                        .write_server_message(&mut stream, &ServerMessage::TabRegistry(snapshot))
                        .await?;
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            runtime_generation_id = runtime_state_updates.recv() => match runtime_generation_id {
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // A command catalogue is generation-bound. Close it before
                    // replaying the replacement generation's state; activation
                    // also checks the stamp if both events race.
                    if let Some(session_id) = menu_sessions.cancel_active() {
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::TransientMenuClosed { session_id },
                            )
                            .await?;
                    }
                    // Always send the latest complete snapshot. Lagged receivers
                    // must not replay intermediate generations.
                    if let Some(snapshot) = runtime_generation
                        .latest_runtime_snapshot_for(client_id)
                        .await
                    {
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::RuntimeStateSnapshot(Box::new(snapshot)),
                            )
                            .await?;
                    }
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            // Plan 060 T4 (P0-3): parse updates arrive only for documents this
            // connection opened, over this connection's bounded subscription.
            update = parse_updates_rx.recv() => {
                if let Some(update) = update {
                    // One parse update's chunks ship in a single frame;
                    // single-chunk updates keep the plain DecorationSet wire.
                    let mut chunks = update.decoration_updates;
                    match chunks.len() {
                        0 => {}
                        1 => {
                            let set = chunks.pop().expect("length checked");
                            codec
                                .write_server_message(&mut stream, &ServerMessage::DecorationSet(set))
                                .await?;
                        }
                        _ => {
                            codec
                                .write_server_message(&mut stream, &ServerMessage::DecorationBatch(chunks))
                                .await?;
                        }
                    }
                    if let Some(set) = update.diagnostic_update {
                        codec
                            .write_server_message(&mut stream, &ServerMessage::DiagnosticSet(set))
                            .await?;
                    }
                    if let Some(set) = update.folding_update {
                        // FOLDING_RANGE_PAYLOAD_BUDGET_BYTES enforced at publish.
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::FoldingRangeSet(set),
                            )
                            .await?;
                    }
                }
                continue;
            }
            diagnostic = parse_diagnostics_rx.recv() => {
                if let Some(diagnostic) = diagnostic {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::RuntimeDiagnostic(diagnostic),
                        )
                        .await?;
                }
                continue;
            }
            diagnostic = runtime_diagnostics_rx.recv() => {
                if let Some(diagnostic) = diagnostic {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::RuntimeDiagnostic(diagnostic),
                        )
                        .await?;
                }
                continue;
            }
            output = analysis_rx.recv() => {
                if let Some(output) = output {
                    let message = match output {
                        crate::server::document_analysis::DocumentAnalysisOutput::Decorations(set) => ServerMessage::DecorationSet(set),
                        crate::server::document_analysis::DocumentAnalysisOutput::Diagnostics(set) => ServerMessage::DiagnosticSet(set),
                        crate::server::document_analysis::DocumentAnalysisOutput::Diagnostic(diagnostic) => ServerMessage::RuntimeDiagnostic(diagnostic),
                    };
                    codec.write_server_message(&mut stream, &message).await?;
                }
                continue;
            }
            message = completion_rx.recv() => {
                if let Some(message) = message {
                    codec.write_server_message(&mut stream, &message).await?;
                }
                continue;
            }
            message = language_intelligence_rx.recv() => {
                if let Some(message) = message {
                    codec.write_server_message(&mut stream, &message).await?;
                }
                continue;
            }
            agent_event = async {
                match agent_rx.as_mut() {
                    Some(rx) => Some(rx.recv().await),
                    None => {
                        std::future::pending::<()>().await;
                        None
                    }
                }
            } => {
                if let Some(Ok(payload)) = agent_event {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::Agent(Box::new((*payload).clone())),
                        )
                        .await?;
                }
                continue;
            }
            message = incoming_rx.recv() => message,
        } {
            Some(Ok(message)) => message,
            None => return Ok(()),
            Some(Err(CodecError::Io(error)))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                return Ok(());
            }
            Some(Err(error)) => return Err(error),
        };

        // Plan 060 T4 (P0-2): one pre-dispatch identity boundary. Every legacy
        // message that still carries a `client_id` must present the connection's
        // handshake-assigned identity; forged or confused IDs fail closed before
        // any dispatch arm runs.
        if let Some(message_client_id) = client_message_identity(&message)
            && message_client_id != client_id
        {
            codec
                .write_server_message(
                    &mut stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message: "client identity mismatch".to_string(),
                    },
                )
                .await?;
            continue;
        }

        if message_requires_tab_state(&message) {
            let Some(routed) = route_connection_tab_state(
                client_id,
                reload_server.as_ref(),
                &bootstrap_document,
                &bootstrap_workspace,
            )
            .await
            else {
                codec
                    .write_server_message(&mut stream, &unbound_tab_state_error())
                    .await?;
                continue;
            };
            bound_tab_id = routed.tab_id;
            *bound_state.lock().unwrap() = Some(routed.state.clone());
            document = routed.state.welcome;
            workspace = routed.state.workspace;
        }

        match message {
            ClientMessage::Edit {
                document_id,
                client_id,
                lease_id,
                base_version,
                behavior_version,
                transaction_id,
                operation,
            } => {
                documents::dispatch_edit_operation(
                    codec,
                    &mut stream,
                    &behavior,
                    &runtime_generation,
                    &document,
                    &workspace,
                    &completion,
                    &language_intelligence,
                    &document_analysis,
                    &parse_coordinator,
                    client_id,
                    document_id,
                    lease_id,
                    base_version,
                    behavior_version,
                    transaction_id,
                    operation,
                )
                .await?;
            }
            ClientMessage::EditorIntent {
                document_id,
                client_id,
                lease_id,
                base_version,
                behavior_version,
                transaction_id,
                intent,
            } => {
                let operation = match intent {
                    crate::protocol::EditorIntent::InsertText { byte_offset, text } => {
                        crate::protocol::EditOperation::Insert { byte_offset, text }
                    }
                    crate::protocol::EditorIntent::DeleteRange { start, end } => {
                        crate::protocol::EditOperation::Delete { start, end }
                    }
                };
                documents::dispatch_edit_operation(
                    codec,
                    &mut stream,
                    &behavior,
                    &runtime_generation,
                    &document,
                    &workspace,
                    &completion,
                    &language_intelligence,
                    &document_analysis,
                    &parse_coordinator,
                    client_id,
                    document_id,
                    lease_id,
                    base_version,
                    behavior_version,
                    transaction_id,
                    operation,
                )
                .await?;
            }
            ClientMessage::RequestResync {
                document_id,
                client_id,
                ..
            } => {
                documents::handle_request_resync(
                    codec,
                    &mut stream,
                    &document,
                    &workspace,
                    client_id,
                    document_id,
                )
                .await?;
            }
            ClientMessage::DecorationViewportRequest {
                document_id,
                document_version,
                byte_start,
                byte_end,
                ..
            } => {
                documents::handle_decoration_viewport_request(
                    codec,
                    &mut stream,
                    &behavior,
                    &runtime_generation,
                    &workspace,
                    &parse_coordinator,
                    client_id,
                    document_id,
                    document_version,
                    byte_start,
                    byte_end,
                )
                .await?;
            }
            ClientMessage::OpenDocument {
                client_id,
                workspace_root_id,
                path,
            } => {
                documents::handle_open_document(
                    codec,
                    &mut stream,
                    &behavior,
                    &runtime_generation,
                    &workspace,
                    &sdui,
                    &parse_coordinator,
                    &document_analysis,
                    client_id,
                    workspace_root_id,
                    path,
                )
                .await?;
            }
            ClientMessage::OpenSelectedFile {
                client_id,
                capability,
                selected_path,
            } => {
                workspace::handle_open_selected_file(
                    codec,
                    &mut stream,
                    &mut file_open_capabilities,
                    &behavior,
                    &runtime_generation,
                    &workspace,
                    &sdui,
                    &parse_coordinator,
                    &document_analysis,
                    client_id,
                    capability,
                    selected_path,
                )
                .await?;
            }
            ClientMessage::AddSelectedWorkspaceRoot {
                client_id,
                capability,
                selected_path,
            } => {
                workspace::handle_add_selected_workspace_root(
                    codec,
                    &mut stream,
                    &mut file_open_capabilities,
                    &workspace,
                    &document,
                    &sdui,
                    reload_server.as_ref(),
                    client_id,
                    capability,
                    selected_path,
                )
                .await?;
            }
            ClientMessage::SaveDocument {
                client_id,
                document_id,
                known_version,
            } => {
                documents::handle_save_document(
                    codec,
                    &mut stream,
                    &workspace,
                    client_id,
                    document_id,
                    known_version,
                )
                .await?;
            }
            ClientMessage::ReloadDocument {
                client_id,
                document_id,
                known_version,
                force,
            } => {
                documents::handle_reload_document(
                    codec,
                    &mut stream,
                    &workspace,
                    &completion,
                    &language_intelligence,
                    &document_analysis,
                    client_id,
                    document_id,
                    known_version,
                    force,
                )
                .await?;
            }
            ClientMessage::CloseDocument {
                client_id,
                document_id,
                force,
            } => {
                documents::handle_close_document(
                    codec,
                    &mut stream,
                    &workspace,
                    &parse_coordinator,
                    &completion,
                    &language_intelligence,
                    &document_analysis,
                    client_id,
                    document_id,
                    force,
                )
                .await?;
            }
            ClientMessage::GetDocumentStatus {
                client_id,
                document_id,
            } => {
                documents::handle_get_document_status(
                    codec,
                    &mut stream,
                    &workspace,
                    client_id,
                    document_id,
                )
                .await?;
            }
            ClientMessage::ListDocuments { client_id } => {
                documents::handle_list_documents(codec, &mut stream, &workspace, client_id).await?;
            }
            ClientMessage::TabCommand { client_id, command } => {
                match tabs::handle_tab_command(
                    codec,
                    &mut stream,
                    &mut menu_sessions,
                    &bound_state,
                    &mut document,
                    &mut workspace,
                    &sdui,
                    &tab_registry,
                    &tab_registry_tx,
                    reload_server.as_ref(),
                    client_id,
                    command,
                    &mut bound_tab_id,
                )
                .await?
                {
                    tabs::TabDispatch::Continue => {}
                    tabs::TabDispatch::CloseConnection => return Ok(()),
                }
            }
            ClientMessage::MenuQueryUpdate {
                client_id,
                session_id,
                query,
            } => {
                menus::handle_menu_query_update(
                    codec,
                    &mut stream,
                    &mut menu_sessions,
                    client_id,
                    session_id,
                    query,
                )
                .await?;
            }
            ClientMessage::MenuBackspace {
                client_id,
                session_id,
            } => {
                menus::handle_menu_backspace(
                    codec,
                    &mut stream,
                    &mut menu_sessions,
                    client_id,
                    session_id,
                )
                .await?;
            }
            ClientMessage::MenuSelectionMove {
                client_id,
                session_id,
                delta,
            } => {
                menus::handle_menu_selection_move(
                    codec,
                    &mut stream,
                    &mut menu_sessions,
                    client_id,
                    session_id,
                    delta,
                )
                .await?;
            }
            ClientMessage::MenuActivate {
                client_id,
                session_id,
                kind,
            } => {
                menus::handle_menu_activate(
                    codec,
                    &mut stream,
                    &mut menu_sessions,
                    &behavior,
                    &runtime_generation,
                    &document,
                    &workspace,
                    &sdui,
                    &parse_coordinator,
                    &document_analysis,
                    &tab_registry,
                    &tab_registry_tx,
                    reload_server.as_ref(),
                    client_id,
                    session_id,
                    kind,
                    bound_tab_id,
                )
                .await?;
            }
            ClientMessage::MenuCancel {
                client_id,
                session_id,
            } => {
                menus::handle_menu_cancel(
                    codec,
                    &mut stream,
                    &mut menu_sessions,
                    client_id,
                    session_id,
                )
                .await?;
            }
            ClientMessage::SduiAction { intent, .. } => {
                runtime::handle_sdui_action(
                    codec,
                    &mut stream,
                    &sdui,
                    &workspace,
                    &document,
                    &behavior,
                    &runtime_generation,
                    &parse_coordinator,
                    &document_analysis,
                    &mut menu_sessions,
                    &tab_registry,
                    reload_server.as_ref(),
                    client_id,
                    intent,
                    bound_tab_id,
                )
                .await?;
            }
            ClientMessage::CommandIntent {
                client_id,
                document_id,
                behavior_version,
                command_id,
            } => {
                runtime::handle_command_intent(
                    codec,
                    &mut stream,
                    &mut menu_sessions,
                    &behavior,
                    &runtime_generation,
                    &document,
                    &workspace,
                    &sdui,
                    &tab_registry,
                    reload_server.as_ref(),
                    client_id,
                    document_id,
                    behavior_version,
                    command_id,
                    bound_tab_id,
                )
                .await?;
            }
            ClientMessage::CompletionRequest { mut request } => {
                runtime::handle_completion_request(
                    codec,
                    &mut stream,
                    &behavior,
                    &runtime_generation,
                    &document,
                    &workspace,
                    &completion,
                    &document_analysis,
                    &completion_tx,
                    &dropped_results,
                    client_id,
                    &mut request,
                )
                .await?;
            }
            ClientMessage::LanguageIntelligenceRequest { mut request } => {
                runtime::handle_language_intelligence_request(
                    codec,
                    &mut stream,
                    &behavior,
                    &document,
                    &workspace,
                    &language_intelligence,
                    &language_intelligence_tx,
                    &dropped_results,
                    client_id,
                    &mut request,
                )
                .await?;
            }
            ClientMessage::RuntimeGenerationInstalled {
                client_id: ack_client_id,
                runtime_generation_id,
            } => {
                runtime::handle_runtime_generation_installed(
                    &runtime_generation,
                    ack_client_id,
                    client_id,
                    runtime_generation_id,
                )
                .await;
            }
            ClientMessage::SelectionQueryRequest { request } => {
                documents::handle_selection_query_request(
                    codec,
                    &mut stream,
                    &workspace,
                    &document,
                    &parse_coordinator,
                    &runtime_generation,
                    client_id,
                    &request,
                )
                .await?;
            }
            ClientMessage::Hello { .. } => {
                codec
                    .write_server_message(
                        &mut stream,
                        &ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message: "duplicate Hello message".to_string(),
                        },
                    )
                    .await?;
            }
            ClientMessage::Agent { command, .. } => {
                if let Some(server) = reload_server.as_ref() {
                    server.agent.dispatch(*command);
                } else {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::Agent(Box::new(AgentServerMessage::Diagnostic {
                                code: "agent.unavailable".to_string(),
                                message: "agent host is not attached to this connection"
                                    .to_string(),
                            })),
                        )
                        .await?;
                }
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "welcome bootstrap carries every server-owned state handle explicitly"
)]
async fn send_welcome_snapshot_and_manifest<S>(
    stream: &mut S,
    client_id: u64,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    active_theme: &Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: &Arc<Mutex<RuntimeDiagnosticStore>>,
    runtime_generation: &RuntimeGenerationStore,
    legacy_bootstrap: Option<(
        &Arc<Mutex<DocumentState>>,
        &Arc<Mutex<WorkspaceState>>,
        &Arc<Mutex<StaticSduiState>>,
    )>,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    codec
        .write_server_message(
            stream,
            &ServerMessage::Welcome {
                client_id,
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await?;

    let legacy_initial = if let Some((document, workspace, _)) = legacy_bootstrap.as_ref() {
        Some(tabs::send_tab_initial_document(stream, client_id, document, workspace, codec).await?)
    } else {
        None
    };

    let behavior_guard = behavior.lock().await;
    let mut manifest_messages = behavior_guard.document_manifest_messages();
    manifest_messages.push(behavior_guard.manifest_message());
    drop(behavior_guard);
    for message in manifest_messages {
        codec.write_server_message(stream, &message).await?;
    }

    let theme = active_theme
        .lock()
        .await
        .clone()
        .unwrap_or_else(|| crate::protocol::ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        });
    codec
        .write_server_message(stream, &ServerMessage::ActiveTheme(theme))
        .await?;
    codec
        .write_server_message(
            stream,
            &ServerMessage::ActiveTypography(runtime_generation.active_typography().await),
        )
        .await?;
    // Plan 071 caret-transport fix: deliver the current runtime caret
    // override so reconnecting/late clients see the active style. `None` is
    // the client default, so only an active override goes on the wire.
    if let Some(style) = runtime_generation.caret_style_override().await {
        codec
            .write_server_message(stream, &ServerMessage::CaretStyleOverride(Some(style)))
            .await?;
    }
    // Phase 26: deliver the current user-owned editor wrap-policy override so
    // reconnecting/late clients see the active wrap. `None` is the client
    // default (manifest wins), so only an active override goes on the wire.
    if let Some(wrap) = runtime_generation.editor_layout_override().await {
        codec
            .write_server_message(stream, &ServerMessage::EditorLayoutOverride(Some(wrap)))
            .await?;
    }
    // Phase 22.1: deliver the current shell preferences so reconnecting/late
    // clients see the active pane-focus policy.
    {
        let preferences = runtime_generation.shell_preferences().await;
        codec
            .write_server_message(stream, &ServerMessage::ShellPreferences(preferences))
            .await?;
    }

    if let (Some((_, workspace, sdui)), Some((document_id, document_version))) =
        (legacy_bootstrap.as_ref(), legacy_initial)
    {
        workspace::send_tab_file_browser_snapshot(
            stream,
            client_id,
            workspace,
            sdui,
            document_id,
            document_version,
            true,
            codec,
        )
        .await?;
    }

    let diagnostics = runtime_diagnostics.lock().await.snapshot();
    for diagnostic in diagnostics {
        codec
            .write_server_message(stream, &ServerMessage::RuntimeDiagnostic(diagnostic))
            .await?;
    }

    Ok(())
}

async fn cleanup_connection_documents(
    client_id: ClientId,
    default_document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    parse_coordinator: &ParseCoordinator,
    completion: &crate::server::completion::CompletionCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
    document_analysis: &crate::server::document_analysis::DocumentAnalysisCoordinator,
) {
    default_document.lock().await.release_access(client_id);
    let finalized = workspace
        .lock()
        .await
        .release_client_access(client_id)
        .await;
    for (document_id, version) in finalized {
        teardown_closed_document(
            document_id,
            version,
            parse_coordinator,
            completion,
            language_intelligence,
            document_analysis,
        );
    }
}

/// Final-close teardown for one document: cancel active work and drop every
/// document-keyed coordinator entry (versions, generations, analysis routes).
fn teardown_closed_document(
    document_id: DocumentId,
    version: crate::protocol::DocumentVersion,
    parse_coordinator: &ParseCoordinator,
    completion: &crate::server::completion::CompletionCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
    document_analysis: &crate::server::document_analysis::DocumentAnalysisCoordinator,
) {
    parse_coordinator.remove_document(document_id);
    completion.remove_document(document_id);
    language_intelligence.remove_document(document_id);
    document_analysis.close_document(document_id, version);
}

/// Per-connection pool of single-use file-open capability tokens.
///
/// Structural authority gate for `OpenSelectedFile`: the server mints tokens
/// and only honors an open request carrying a valid, unconsumed token. Raw
/// client-supplied paths without a token are rejected with a typed
/// `RuntimeDiagnostic`. Tokens are per-connection and single-use; they are not
/// cryptographically secret because the trust model is per-user IPC with a
/// same-user server. Full defense against a malicious same-user client requires
/// the long-term OS-verifiable picker exchange.
#[derive(Debug, Default)]
pub(crate) struct FileOpenCapabilityPool {
    valid: HashSet<String>,
}

impl FileOpenCapabilityPool {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn issue(&mut self) -> String {
        let token = next_capability_token();
        self.valid.insert(token.clone());
        token
    }

    pub(crate) fn consume(&mut self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        self.valid.remove(token)
    }
}

fn next_capability_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    format!("foc-{now:x}-{nonce:x}")
}

fn file_operation_failed(
    error: WorkspaceError,
    workspace_root_id: Option<WorkspaceRootId>,
    document_id: Option<DocumentId>,
) -> ServerMessage {
    let diagnostic = error.diagnostic();
    let message = diagnostic.to_string();
    ServerMessage::FileOperationFailed {
        code: diagnostic.code,
        message,
        workspace_root_id,
        document_id,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc, time::SystemTime};

    use crate::packages::commands::CommandRegistry;
    use crate::protocol::{KeyBindingContext, KeyCode};

    use tokio::{
        io::duplex,
        sync::Mutex,
        time::{Duration, timeout},
    };

    use super::{RuntimeDiagnosticStore, handle_connection, route_connection_tab_state};
    // Moved family helpers (Plan 090 task 2) are glob re-exported in the
    // connection module scope; the few names tests also import explicitly are
    // imported from their family modules for unambiguous unqualified use.
    use super::runtime::{
        execute_command_intent, language_intelligence_document_window_for_behavior,
        sdui_command_request, static_package_completion_result,
    };
    use super::tabs::open_workspace_for_bound_tab;
    use crate::protocol::ParseByteRange;
    use crate::server::command_execution::{CommandExecutionRequest, CommandExecutionTarget};

    fn workspace_state() -> Arc<Mutex<WorkspaceState>> {
        Arc::new(Mutex::new(WorkspaceState::new()))
    }

    fn sdui_state() -> Arc<Mutex<StaticSduiState>> {
        Arc::new(Mutex::new(StaticSduiState::for_document(1, 1)))
    }

    fn document_state() -> Arc<Mutex<DocumentState>> {
        Arc::new(Mutex::new(DocumentState::new(
            1,
            "".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )))
    }

    fn empty_sdui_state() -> Arc<Mutex<StaticSduiState>> {
        Arc::new(Mutex::new(StaticSduiState::empty_for_document(1)))
    }

    fn runtime_diagnostics() -> Arc<Mutex<RuntimeDiagnosticStore>> {
        Arc::new(Mutex::new(RuntimeDiagnosticStore::default()))
    }

    fn active_theme_state() -> Arc<Mutex<Option<crate::protocol::ActiveTheme>>> {
        Arc::new(Mutex::new(None))
    }

    fn js_runtime() -> ClayJsRuntimeService {
        ClayJsRuntimeService::default()
    }

    #[test]
    fn language_intelligence_window_uses_active_behavior_mode() {
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.manifest_id = "rust.rust".to_string();
        let behavior = ActiveBehaviorManifest::new(manifest).unwrap();
        let request = crate::protocol::LanguageIntelligenceRequest {
            request_id: 1,
            client_id: 2,
            document_id: 3,
            document_version: 4,
            behavior_version: 3,
            cursor_byte_offset: 1,
            feature: crate::protocol::LanguageIntelligenceFeature::Hover,
            provider_generation: 0,
        };

        let window = language_intelligence_document_window_for_behavior(
            &request,
            "fn main() {}",
            behavior.manifest(),
        );

        assert_eq!(window.active_mode, "rust");
    }

    #[test]
    fn language_intelligence_window_resolves_per_document_mode_layer() {
        // Phase 22.2: two documents in different modes; the window builder
        // must use each document's OWN layer's mode, not a connection-wide
        // latest.
        let mut state = ActiveBehaviorManifest::default();
        let mut markdown = BehaviorManifest::minimal_text_editing(1);
        markdown.manifest_id = "markdown.markdown".to_string();
        markdown.scope = crate::protocol::BehaviorScope::Document { document_id: 7 };
        let mut rust = BehaviorManifest::minimal_text_editing(1);
        rust.manifest_id = "rust.rust".to_string();
        rust.scope = crate::protocol::BehaviorScope::Document { document_id: 9 };
        state.publish_replacement(markdown).unwrap();
        state.publish_replacement(rust).unwrap();

        let request = |document_id| crate::protocol::LanguageIntelligenceRequest {
            request_id: 1,
            client_id: 2,
            document_id,
            document_version: 4,
            behavior_version: 3,
            cursor_byte_offset: 1,
            feature: crate::protocol::LanguageIntelligenceFeature::Hover,
            provider_generation: 0,
        };

        let markdown_window = language_intelligence_document_window_for_behavior(
            &request(7),
            "## Heading",
            state.manifest_for(7),
        );
        assert_eq!(markdown_window.active_mode, "markdown");

        let rust_window = language_intelligence_document_window_for_behavior(
            &request(9),
            "fn main() {}",
            state.manifest_for(9),
        );
        assert_eq!(rust_window.active_mode, "rust");
    }

    #[test]
    fn static_package_completion_filters_active_provider_items_by_prefix() {
        let provenance = crate::protocol::CompletionProvenance {
            package_name: "@clay/javascript".to_string(),
            package_version: "0.1.0".to_string(),
            package_prefix: "javascript".to_string(),
        };
        let provider = crate::server::completion::CompletionProviderMeta {
            id: "javascript.keywords".to_string(),
            provenance: provenance.clone(),
            priority: 0,
            exclusive: false,
            trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
                trigger_characters: vec![".".to_string()],
            },
            word_boundary: crate::server::completion::WordBoundaryRule::default(),
            items: ["function", "for", "return"]
                .into_iter()
                .map(|item| crate::protocol::CompletionItem::new(item, item, provenance.clone()))
                .collect(),
            timeout_ms: 300,
            max_items: 32,
            generation: 0,
        };
        let request = crate::protocol::CompletionRequest {
            request_id: 1,
            client_id: 2,
            document_id: 3,
            document_version: 4,
            behavior_version: 5,
            cursor_byte_offset: 2,
            replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
            trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
            provider_generation: 0,
            recent_completions: Vec::<String>::new().into_boxed_slice(),
        };

        let result =
            static_package_completion_result(&request, "javascript.javascript", "fu", &[provider])
                .unwrap();

        assert_eq!(
            result
                .items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["function"]
        );
        assert_eq!(result.provenance, provenance);
    }

    #[test]
    fn static_package_completion_equal_priority_merge_uses_shared_score() {
        let provenance = crate::protocol::CompletionProvenance {
            package_name: "@clay/rust".to_string(),
            package_version: "0.1.0".to_string(),
            package_prefix: "rust".to_string(),
        };
        let provider = |id: &str, item: crate::protocol::CompletionItem| {
            crate::server::completion::CompletionProviderMeta {
                id: id.to_string(),
                provenance: provenance.clone(),
                priority: 0,
                exclusive: false,
                trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
                    trigger_characters: vec![".".to_string()],
                },
                word_boundary: crate::server::completion::WordBoundaryRule::default(),
                items: vec![item],
                timeout_ms: 300,
                max_items: 32,
                generation: 0,
            }
        };
        let keyword = crate::protocol::CompletionItem::new("fn", "fn", provenance.clone());
        let snippet = crate::protocol::CompletionItem::new(
            "fn",
            "fn ${1:name}(${2:args}) {\n\t$0\n}",
            provenance.clone(),
        )
        .with_snippet();
        let providers = [
            provider("rust.keywords", keyword),
            provider("rust.snippets", snippet),
        ];
        let request = crate::protocol::CompletionRequest {
            request_id: 1,
            client_id: 2,
            document_id: 3,
            document_version: 4,
            behavior_version: 5,
            cursor_byte_offset: 2,
            replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
            trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
            provider_generation: 0,
            recent_completions: vec!["fn ${1:name}(${2:args}) {\n\t$0\n}".to_string()]
                .into_boxed_slice(),
        };

        let result =
            static_package_completion_result(&request, "rust.rust", "fn", &providers).unwrap();

        assert_eq!(result.items.len(), 2);
        assert_eq!(
            result.items[0].text_format,
            crate::protocol::CompletionItemTextFormat::Snippet
        );
        assert_eq!(
            result.items[1].text_format,
            crate::protocol::CompletionItemTextFormat::PlainText
        );
        assert!(result.validate().is_ok());
    }

    fn runtime_generation() -> super::RuntimeGenerationStore {
        runtime_generation_from(js_runtime())
    }

    fn runtime_generation_from(runtime: ClayJsRuntimeService) -> super::RuntimeGenerationStore {
        super::RuntimeGenerationStore {
            current: Arc::new(Mutex::new(super::super::RuntimeGeneration {
                id: 1,
                service: runtime,
                evaluation: None,
                diagnostics: Vec::new(),
            })),
            typography: super::super::ActiveTypographyState::default(),
            runtime_state: super::super::ActiveRuntimeStateFanout::default(),
            behavior_grace: super::super::behavior::BehaviorGraceState::new(),
        }
    }

    fn parse_coordinator() -> ParseCoordinator {
        ParseCoordinator::default()
    }

    fn language_intelligence_coordinator() -> LanguageIntelligenceCoordinator {
        LanguageIntelligenceCoordinator::new()
    }

    async fn load_markdown_runtime(
        runtime: &ClayJsRuntimeService,
        coordinator: &ParseCoordinator,
        behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
        sdui: &Arc<Mutex<StaticSduiState>>,
    ) {
        let evaluation = runtime
            .evaluate_controlled_module(
                r#"import { loadPackage } from "clay:packages";
import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
await loadPackage("@clay/markdown");
const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
serverActivateClassifiedMode(classification, { path: "README.md" });"#,
            )
            .await
            .expect("Markdown package load should evaluate");
        runtime
            .register_parse_handlers(coordinator, 1, &evaluation)
            .expect("Markdown parse handler should register");
        super::super::apply_runtime_outputs(&evaluation, 1, behavior, sdui).await;
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "clay-connection-workspace-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir
    }
    use crate::{
        protocol::{
            BehaviorManifest, BehaviorScope, ClientMessage, DocumentAccess, DocumentMetadata,
            EditOperation, EditRejection, FileErrorCode, PROTOCOL_VERSION, ProtocolErrorCode,
            RuntimeDiagnostic, SduiActionArgument, SduiActionIntent, SduiActionSource,
            SduiActionValue, SduiNodeId, SduiNodeKind, ServerMessage, TokenType, codec::Codec,
        },
        server::{
            behavior::ActiveBehaviorManifest, document::DocumentState,
            js_runtime::ClayJsRuntimeService,
            language_intelligence::LanguageIntelligenceCoordinator,
            parse_coordinator::ParseCoordinator, sdui::StaticSduiState, workspace::WorkspaceState,
        },
        shell::file_browser::FileBrowserState,
    };

    #[tokio::test]
    async fn sdui_actions_and_keybinding_intents_share_command_execution_path() {
        let sdui_request = sdui_command_request(&SduiActionIntent::command(
            "controlCenter.open",
            SduiActionSource::Button {
                node_id: SduiNodeId(5),
            },
        ));
        let keybinding_request = CommandExecutionRequest {
            command_id: "controlCenter.open".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
            provenance: None,
            expected_permissions: Vec::new(),
        };

        let document = document_state();
        let sdui = sdui_state();
        assert_eq!(
            execute_command_intent(
                sdui_request,
                workspace_state(),
                &document,
                &sdui,
                1,
                None,
                &CommandRegistry::new(),
            )
            .await,
            None
        );
        assert_eq!(
            execute_command_intent(
                keybinding_request,
                workspace_state(),
                &document,
                &sdui,
                1,
                None,
                &CommandRegistry::new(),
            )
            .await,
            None
        );
    }

    #[tokio::test]
    async fn reload_command_intent_uses_shared_server_reload_service() {
        let root = temp_workspace("reload-command-intent");
        fs::write(root.join("init.js"), "").unwrap();
        let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
            "reload-command-intent",
        ));
        config.configuration_root = Some(root.clone());
        let server = super::super::IpcServer::new(config);

        let response = execute_command_intent(
            CommandExecutionRequest {
                command_id: "runtime.reloadConfiguration".to_string(),
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&server.workspace),
            &server.document,
            &server.sdui,
            1,
            Some(&server),
            &CommandRegistry::new(),
        )
        .await
        .expect("reload command returns status");

        assert!(matches!(
            response,
            ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. })
                if code == "runtime.reload_succeeded"
        ));
        assert_eq!(server.runtime_generation.generation_id().await, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn settings_live_switch_persists_and_reloads_end_to_end() {
        // Plan 067 task 12: full live-switch + persistence matrix through the
        // real command executor + persist + reload path. From a clean config:
        // settings.setTheme selects Gruvbox (proving Gruvbox remains
        // selectable), persists to preferences.json, and advances the runtime
        // generation so the change applies live via reload→fanout;
        // settings.setAppearance persists appearance; settings.reset clears the
        // store and reloads; a non-bundled @clay/theme-* specifier is rejected
        // by execute_settings without advancing the generation.
        let root = temp_workspace("settings-live-switch-e2e");
        fs::write(root.join("init.js"), "").unwrap();
        let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
            "settings-live-switch-e2e",
        ));
        config.configuration_root = Some(root.clone());
        let server = super::super::IpcServer::new(config);
        let preferences = root.join("preferences.json");
        let workspace = workspace_state();
        let document = document_state();
        let sdui = sdui_state();

        let settings_registry = CommandRegistry::new();
        let settings_request = |command_id: &str, item_id: &str| {
            execute_command_intent(
                CommandExecutionRequest {
                    command_id: command_id.to_string(),
                    arguments: serde_json::json!({ "item_id": item_id }),
                    target: CommandExecutionTarget::Global,
                    provenance: None,
                    expected_permissions: Vec::new(),
                },
                Arc::clone(&workspace),
                &document,
                &sdui,
                1,
                Some(&server),
                &settings_registry,
            )
        };

        // 1. settings.setTheme selects Gruvbox Material Light (opt-in theme
        //    remains selectable), persists, and reloads live.
        let response =
            settings_request("settings.setTheme", "@clay/theme-gruvbox-material-light").await;
        assert!(
            response.is_none(),
            "settings.setTheme returns no error on success"
        );
        assert_eq!(server.runtime_generation.generation_id().await, 2);
        let persisted = fs::read_to_string(&preferences).expect("preferences.json written");
        assert!(
            persisted.contains("@clay/theme-gruvbox-material-light"),
            "preferences.json persists the selected theme: {persisted}"
        );

        // 2. settings.setAppearance persists appearance and reloads again.
        let response = settings_request("settings.setAppearance", "light").await;
        assert!(
            response.is_none(),
            "settings.setAppearance returns no error on success"
        );
        assert_eq!(server.runtime_generation.generation_id().await, 3);
        let persisted = fs::read_to_string(&preferences).expect("preferences.json updated");
        assert!(
            persisted.contains("\"appearance\"") && persisted.contains("light"),
            "preferences.json persists appearance: {persisted}"
        );

        // 3. A non-bundled @clay/theme-* specifier is rejected by execute_settings
        //    and does not advance the generation (authority denial).
        let generation_before = server.runtime_generation.generation_id().await;
        let response = settings_request("settings.setTheme", "@clay/theme-evil").await;
        assert!(
            matches!(response, Some(ServerMessage::Error { .. })),
            "non-bundled theme specifier is rejected"
        );
        assert_eq!(
            server.runtime_generation.generation_id().await,
            generation_before,
            "rejected settings intent does not reload"
        );

        // 4. settings.reset clears the persisted store and reloads.
        let response = execute_command_intent(
            CommandExecutionRequest {
                command_id: "settings.reset".to_string(),
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&workspace),
            &document,
            &sdui,
            1,
            Some(&server),
            &CommandRegistry::new(),
        )
        .await;
        assert!(
            response.is_none(),
            "settings.reset returns no error on success"
        );
        assert_eq!(
            server.runtime_generation.generation_id().await,
            generation_before + 1
        );
        let reset = fs::read_to_string(&preferences).unwrap_or_default();
        assert!(
            !reset.contains("@clay/theme-"),
            "preferences.json cleared after reset: {reset}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn package_ui_unregistered_action_is_rejected_by_command_execution() {
        let response = execute_command_intent(
            sdui_command_request(&SduiActionIntent::command(
                "markdown.missingCommand",
                SduiActionSource::Button {
                    node_id: SduiNodeId(5),
                },
            )),
            workspace_state(),
            &document_state(),
            &sdui_state(),
            1,
            None,
            &CommandRegistry::new(),
        )
        .await
        .expect("unknown package UI action returns protocol error");

        assert!(matches!(response, ServerMessage::Error { .. }));
        if let ServerMessage::Error { message, .. } = response {
            assert!(message.contains("UnknownCommand"));
        }
    }

    #[tokio::test]
    async fn workspace_directory_action_sends_refreshed_file_browser_snapshot() {
        let root = temp_workspace("navigate-snapshot");
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let workspace = workspace_state();
        let root_id = workspace.lock().await.add_root(&root).unwrap();
        let document = document_state();
        let sdui = sdui_state();
        let mut intent = SduiActionIntent::command(
            "workspace.openDirectory",
            SduiActionSource::ListItem {
                node_id: SduiNodeId(5),
                item_id: "src".to_string(),
            },
        );
        intent.arguments = vec![
            SduiActionArgument {
                name: "workspaceRootId".to_string(),
                value: SduiActionValue::U64(root_id),
            },
            SduiActionArgument {
                name: "relativePath".to_string(),
                value: SduiActionValue::String("src".to_string()),
            },
        ];

        let response = execute_command_intent(
            sdui_command_request(&intent),
            workspace,
            &document,
            &sdui,
            42,
            None,
            &CommandRegistry::new(),
        )
        .await
        .expect("directory navigation sends a snapshot");

        let ServerMessage::SduiSnapshot { client_id, tree } = response else {
            panic!("expected SduiSnapshot");
        };
        assert_eq!(client_id, 42);
        let labels: Vec<String> = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                SduiNodeKind::List { items } => {
                    Some(items.iter().map(|item| item.label.clone()).collect())
                }
                _ => None,
            })
            .unwrap();
        assert!(labels.iter().any(|label| label == "../"));
        assert!(labels.iter().any(|label| label == "main.rs"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn file_browser_action_survives_markdown_open_followup_diagnostic() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let root = temp_workspace("browser-survives-open-followup");
        fs::write(root.join("note.md"), "# note\n").unwrap();

        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let browser = FileBrowserState::from_workspace(&workspace_state_value, root_id).unwrap();
        let tree = browser.to_sdui_tree(1u64, 1u64);
        let action = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                SduiNodeKind::List { items } => items
                    .iter()
                    .find(|item| item.label == "note.md")
                    .and_then(|item| item.action.clone()),
                _ => None,
            })
            .expect("note.md file-browser action");
        let sdui = empty_sdui_state();
        sdui.lock()
            .await
            .replace_for_document_with_runtime_tree(1, tree)
            .unwrap();

        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let runtime = js_runtime();
        let coordinator = parse_coordinator();
        load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
        let metadata = DocumentMetadata {
            document_id: 2,
            version: 1,
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            dirty: false,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        };

        let messages = super::open_document_followup_messages(
            &metadata,
            "# note\n",
            &behavior,
            &sdui,
            1,
            &runtime,
            &coordinator,
        )
        .await;
        assert!(messages.iter().any(|message| {
            matches!(
                message,
                ServerMessage::BehaviorManifest(_) | ServerMessage::RuntimeDiagnostic(_)
            )
        }));
        sdui.lock()
            .await
            .validate_action(&action)
            .expect("file-browser action remains valid after open-time follow-up");

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn server_accepts_hello_and_sends_snapshot() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "Hello from server".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::Welcome {
                client_id: 99,
                protocol_version: PROTOCOL_VERSION,
            }
        );
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::InitialDocument {
                document_id: 7,
                version: 1,
                text: "Hello from server".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
                workspace_root: String::new(),
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    /// Plan 060 T4 test helpers: drain the bootstrap sequence through the
    /// always-terminal capability issue so tests start from a clean cursor.
    async fn drain_bootstrap(client: &mut tokio::io::DuplexStream, codec: Codec) -> String {
        loop {
            if let ServerMessage::FileOpenCapabilityIssued { token } =
                codec.read_server_message(client).await.unwrap()
            {
                return token;
            }
        }
    }

    struct TestConnection {
        client: tokio::io::DuplexStream,
        server_task: tokio::task::JoinHandle<Result<(), crate::protocol::codec::CodecError>>,
        codec: Codec,
        file_open_capability: String,
    }

    impl TestConnection {
        #[allow(
            clippy::too_many_arguments,
            reason = "test connection harness mirrors the server's explicit authority parameters"
        )]
        async fn connect(
            client_id: u64,
            document: Arc<Mutex<DocumentState>>,
            behavior: Arc<Mutex<ActiveBehaviorManifest>>,
            workspace: Arc<Mutex<WorkspaceState>>,
            runtime_generation: super::RuntimeGenerationStore,
            parse_coordinator: ParseCoordinator,
            document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
            language_intelligence: LanguageIntelligenceCoordinator,
        ) -> Self {
            let registry = Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new()));
            let (tab_registry_tx, _) = tokio::sync::broadcast::channel(16);
            Self::connect_with_registry(
                client_id,
                document,
                behavior,
                workspace,
                runtime_generation,
                parse_coordinator,
                document_analysis,
                language_intelligence,
                registry,
                tab_registry_tx,
            )
            .await
        }

        #[allow(
            clippy::too_many_arguments,
            reason = "test connection harness mirrors the server's explicit authority parameters"
        )]
        async fn connect_with_registry(
            client_id: u64,
            document: Arc<Mutex<DocumentState>>,
            behavior: Arc<Mutex<ActiveBehaviorManifest>>,
            workspace: Arc<Mutex<WorkspaceState>>,
            runtime_generation: super::RuntimeGenerationStore,
            parse_coordinator: ParseCoordinator,
            document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
            language_intelligence: LanguageIntelligenceCoordinator,
            tab_registry: Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
            tab_registry_tx: tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
        ) -> Self {
            let (client, server) = duplex(65536);
            let codec = Codec::default();
            let server_task = tokio::spawn(super::handle_connection_with_analysis(
                server,
                client_id,
                document,
                behavior,
                workspace,
                sdui_state(),
                active_theme_state(),
                runtime_diagnostics(),
                runtime_generation,
                parse_coordinator,
                crate::server::completion::CompletionCoordinator::new(),
                document_analysis,
                language_intelligence,
                None,
                tab_registry,
                tab_registry_tx,
                codec,
            ));
            let mut client = client;
            codec
                .write_client_message(
                    &mut client,
                    &ClientMessage::Hello {
                        protocol_version: PROTOCOL_VERSION,
                        client_name: "test-client".to_string(),
                    },
                )
                .await
                .unwrap();
            let file_open_capability = drain_bootstrap(&mut client, codec).await;
            Self {
                client,
                server_task,
                codec,
                file_open_capability,
            }
        }

        #[allow(
            clippy::too_many_arguments,
            reason = "test connection harness mirrors the production IpcServer connection wiring"
        )]
        async fn connect_with_server(client_id: u64, server: super::super::IpcServer) -> Self {
            let (client, server_stream) = duplex(65536);
            let codec = Codec::default();
            let document = Arc::clone(&server.bootstrap_state.welcome);
            let behavior = Arc::clone(&server.behavior);
            let workspace = Arc::clone(&server.bootstrap_state.workspace);
            let sdui = Arc::clone(&server.sdui);
            let active_theme = Arc::clone(&server.active_theme);
            let runtime_diagnostics = Arc::clone(&server.runtime_diagnostics);
            let runtime_generation = server.runtime_generation.clone();
            let parse_coordinator = server.parse_coordinator.clone();
            let completion = server.completion.clone();
            let document_analysis = server.document_analysis.clone();
            let language_intelligence = server.language_intelligence.clone();
            let tab_registry = Arc::clone(&server.tab_registry);
            let tab_registry_tx = server.tab_registry_tx.clone();
            let server_task = tokio::spawn(super::handle_connection_with_analysis(
                server_stream,
                client_id,
                document,
                behavior,
                workspace,
                sdui,
                active_theme,
                runtime_diagnostics,
                runtime_generation,
                parse_coordinator,
                completion,
                document_analysis,
                language_intelligence,
                Some(server),
                tab_registry,
                tab_registry_tx,
                codec,
            ));
            let mut client = client;
            codec
                .write_client_message(
                    &mut client,
                    &ClientMessage::Hello {
                        protocol_version: PROTOCOL_VERSION,
                        client_name: "test-client".to_string(),
                    },
                )
                .await
                .unwrap();
            let file_open_capability = drain_bootstrap(&mut client, codec).await;
            Self {
                client,
                server_task,
                codec,
                file_open_capability,
            }
        }

        async fn reclaim(&mut self, client_id: u64, tab_id: crate::protocol::TabId) {
            self.send(&ClientMessage::TabCommand {
                client_id,
                command: crate::protocol::TabCommand::Reclaim { tab_id },
            })
            .await;
            let mut received_initial_document = false;
            loop {
                match self.receive().await {
                    ServerMessage::InitialDocument { .. } => received_initial_document = true,
                    ServerMessage::TabRegistry(_) if received_initial_document => return,
                    ServerMessage::SduiSnapshot { .. }
                    | ServerMessage::TabRegistry(_)
                    | ServerMessage::RuntimeDiagnostic(_)
                    | ServerMessage::FileOpenCapabilityIssued { .. }
                    | ServerMessage::BehaviorManifest(_) => {}
                    other => panic!("unexpected message during tab reclaim: {other:?}"),
                }
            }
        }

        async fn open_document(
            &mut self,
            client_id: u64,
            workspace_root_id: crate::protocol::WorkspaceRootId,
            path: &str,
        ) -> (DocumentMetadata, crate::protocol::BehaviorVersion) {
            self.send(&ClientMessage::OpenDocument {
                client_id,
                workspace_root_id,
                path: path.to_string(),
            })
            .await;
            let mut behavior_version = 1;
            loop {
                match self.receive().await {
                    message @ ServerMessage::BehaviorManifest(_) => {
                        let ServerMessage::BehaviorManifest(manifest) = message else {
                            unreachable!();
                        };
                        behavior_version = manifest.behavior_version;
                    }
                    message @ ServerMessage::DocumentOpened { .. } => {
                        let ServerMessage::DocumentOpened { metadata, .. } = message else {
                            unreachable!();
                        };
                        return (metadata, behavior_version);
                    }
                    ServerMessage::RuntimeDiagnostic(_)
                    | ServerMessage::SduiSnapshot { .. }
                    | ServerMessage::TabRegistry(_) => {}
                    other => panic!("unexpected message during document open: {other:?}"),
                }
            }
        }

        async fn send(&mut self, message: &ClientMessage) {
            self.codec
                .write_client_message(&mut self.client, message)
                .await
                .unwrap();
        }

        async fn receive(&mut self) -> ServerMessage {
            self.codec
                .read_server_message(&mut self.client)
                .await
                .unwrap()
        }

        async fn close(self) {
            drop(self.client);
            self.server_task.await.unwrap().unwrap();
        }

        /// Drain open/activation follow-ups until the stream goes quiet so the
        /// next read observes the response to the next request, not a queued
        /// BehaviorManifest/decoration frame.
        async fn drain_until_quiet(&mut self) {
            while timeout(
                Duration::from_millis(50),
                self.codec.read_server_message(&mut self.client),
            )
            .await
            .is_ok()
            {}
        }

        /// Drain a bounded amount of asynchronous output. Some parser lanes
        /// can continuously publish while a test is intentionally not asserting
        /// every advisory frame.
        async fn drain_bounded(&mut self) {
            for _ in 0..32 {
                if timeout(
                    Duration::from_millis(10),
                    self.codec.read_server_message(&mut self.client),
                )
                .await
                .is_err()
                {
                    break;
                }
            }
        }

        /// Read until the response frame arrives, skipping asynchronous parse
        /// and activation output that can race a request/response exchange.
        async fn receive_response(&mut self) -> ServerMessage {
            loop {
                let frame = self.receive().await;
                if matches!(
                    frame,
                    ServerMessage::Error { .. }
                        | ServerMessage::FileOperationFailed { .. }
                        | ServerMessage::DocumentSaved { .. }
                        | ServerMessage::DocumentReloaded { .. }
                        | ServerMessage::DocumentClosed { .. }
                        | ServerMessage::DocumentStatus { .. }
                        | ServerMessage::DocumentList { .. }
                        | ServerMessage::ResyncSnapshot { .. }
                ) {
                    return frame;
                }
            }
        }
    }

    #[tokio::test]
    async fn connection_state_route_follows_reclaim_and_fails_closed() {
        let root_a = temp_workspace("route-alpha");
        let root_b = temp_workspace("route-beta");
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("connection-state-route"),
        ));
        let (alpha_snapshot, alpha_state) = server
            .create_tab_state(11, root_a.to_string_lossy().into_owned())
            .await
            .expect("alpha tab state is created");
        let (beta_snapshot, beta_state) = server
            .create_tab_state(22, root_b.to_string_lossy().into_owned())
            .await
            .expect("beta tab state is created");
        let alpha_tab = alpha_snapshot.tabs[0].tab_id;
        let beta_tab = beta_snapshot.tabs[1].tab_id;

        let alpha_route = route_connection_tab_state(
            11,
            Some(&server),
            &alpha_state.welcome,
            &alpha_state.workspace,
        )
        .await
        .expect("bound alpha route");
        assert_eq!(alpha_route.tab_id, Some(alpha_tab));
        assert!(Arc::ptr_eq(
            &alpha_route.state.workspace,
            &alpha_state.workspace
        ));
        assert!(!Arc::ptr_eq(
            &alpha_route.state.workspace,
            &beta_state.workspace
        ));

        fs::write(root_a.join("alpha.txt"), "alpha").expect("alpha file is written");
        fs::write(root_b.join("beta.txt"), "beta").expect("beta file is written");
        let alpha_root_id = alpha_state
            .workspace
            .lock()
            .await
            .directory_roots()
            .into_iter()
            .next()
            .expect("alpha root")
            .workspace_root_id;
        let beta_root_id = beta_state
            .workspace
            .lock()
            .await
            .directory_roots()
            .into_iter()
            .next()
            .expect("beta root")
            .workspace_root_id;
        let alpha_document = crate::server::workspace::open_existing_file_unlocked(
            &alpha_route.state.workspace,
            alpha_root_id,
            "alpha.txt",
            11,
        )
        .await
        .expect("alpha document opens in alpha state");
        let beta_document = crate::server::workspace::open_existing_file_unlocked(
            &beta_state.workspace,
            beta_root_id,
            "beta.txt",
            22,
        )
        .await
        .expect("beta document opens in beta state");
        let alpha_response = alpha_document.document.lock().await.apply_edit(
            alpha_document.document_id,
            11,
            alpha_document.access.lease_id(),
            1,
            1,
            crate::protocol::EditOperation::Insert {
                byte_offset: 5,
                text: "!".to_string(),
            },
        );
        assert!(matches!(alpha_response, ServerMessage::EditAck { .. }));
        let beta_document_state = beta_document.document.lock().await;
        assert_eq!(beta_document_state.version(), 1);
        assert!(!beta_document_state.is_dirty());
        assert_eq!(beta_document_state.text(), "beta");

        assert!(server.tab_registry.lock().await.reclaim(alpha_tab, 33));
        assert!(
            route_connection_tab_state(
                11,
                Some(&server),
                &alpha_state.welcome,
                &alpha_state.workspace,
            )
            .await
            .is_none()
        );
        let reclaimed_route = route_connection_tab_state(
            33,
            Some(&server),
            &alpha_state.welcome,
            &alpha_state.workspace,
        )
        .await
        .expect("reclaimed route");
        assert_eq!(reclaimed_route.tab_id, Some(alpha_tab));
        assert!(Arc::ptr_eq(
            &reclaimed_route.state.workspace,
            &alpha_state.workspace
        ));

        server.remove_tab_state(beta_tab).await;
        assert!(
            route_connection_tab_state(
                22,
                Some(&server),
                &beta_state.welcome,
                &beta_state.workspace,
            )
            .await
            .is_none()
        );

        let _ = fs::remove_dir_all(root_a);
        let _ = fs::remove_dir_all(root_b);
    }

    #[tokio::test]
    async fn cross_tab_workspace_and_document_authority_is_fail_closed() {
        let root_a = temp_workspace("authority-alpha");
        let root_b = temp_workspace("authority-beta");
        let extra_root_a = temp_workspace("authority-alpha-extra");
        fs::write(root_a.join("alpha.txt"), "alpha").unwrap();
        fs::write(root_b.join("beta.txt"), "beta").unwrap();
        fs::write(extra_root_a.join("only-alpha.txt"), "only alpha").unwrap();

        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("cross-tab-authority"),
        ));
        let (alpha_snapshot, alpha_state) = server
            .create_tab_state(11, root_a.to_string_lossy().into_owned())
            .await
            .expect("alpha tab state is created");
        let (beta_snapshot, beta_state) = server
            .create_tab_state(22, root_b.to_string_lossy().into_owned())
            .await
            .expect("beta tab state is created");
        let alpha_tab = alpha_snapshot.tabs[0].tab_id;
        let beta_tab = beta_snapshot.tabs[1].tab_id;
        let alpha_root_id = alpha_state
            .workspace
            .lock()
            .await
            .directory_roots()
            .into_iter()
            .next()
            .expect("alpha root")
            .workspace_root_id;
        let alpha_extra_root_id = alpha_state
            .workspace
            .lock()
            .await
            .add_root(&extra_root_a)
            .expect("alpha extra root");
        let beta_root_id = beta_state
            .workspace
            .lock()
            .await
            .directory_roots()
            .into_iter()
            .next()
            .expect("beta root")
            .workspace_root_id;

        let mut connection_a = TestConnection::connect_with_server(11, server.clone()).await;
        connection_a.reclaim(11, alpha_tab).await;
        let mut connection_b = TestConnection::connect_with_server(22, server.clone()).await;
        connection_b.reclaim(22, beta_tab).await;
        connection_a.drain_bounded().await;

        let (alpha_metadata, alpha_behavior_version) = connection_a
            .open_document(11, alpha_root_id, "alpha.txt")
            .await;
        let (beta_metadata, _) = connection_b
            .open_document(22, beta_root_id, "beta.txt")
            .await;
        connection_a.drain_bounded().await;
        connection_b.drain_bounded().await;
        connection_a
            .send(&ClientMessage::ListDocuments { client_id: 11 })
            .await;
        let alpha_list = connection_a.receive_response().await;
        assert!(matches!(
            alpha_list,
            ServerMessage::DocumentList { ref documents }
                if documents.len() == 1 && documents[0].document_id == alpha_metadata.document_id
        ));

        connection_b
            .send(&ClientMessage::ListDocuments { client_id: 22 })
            .await;
        let beta_list = connection_b.receive_response().await;
        assert!(matches!(
            beta_list,
            ServerMessage::DocumentList { ref documents }
                if documents.len() == 1 && documents[0].document_id == beta_metadata.document_id
        ));

        connection_a
            .send(&ClientMessage::OpenDocument {
                client_id: 11,
                workspace_root_id: beta_root_id,
                path: "beta.txt".to_string(),
            })
            .await;
        let foreign_open = connection_a.receive_response().await;
        assert!(matches!(
            foreign_open,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::NotFound,
                ..
            }
        ));

        connection_b
            .send(&ClientMessage::OpenDocument {
                client_id: 22,
                workspace_root_id: alpha_extra_root_id,
                path: "only-alpha.txt".to_string(),
            })
            .await;
        let foreign_root = connection_b.receive_response().await;
        assert!(matches!(
            foreign_root,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownWorkspaceRoot,
                ..
            }
        ));

        connection_a
            .send(&ClientMessage::RequestResync {
                client_id: 11,
                document_id: beta_metadata.document_id,
                known_version: 1,
            })
            .await;
        let foreign_resync = connection_a.receive_response().await;
        assert!(matches!(
            foreign_resync,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ));

        connection_a
            .send(&ClientMessage::GetDocumentStatus {
                client_id: 11,
                document_id: beta_metadata.document_id,
            })
            .await;
        let foreign_status = connection_a.receive_response().await;
        assert!(matches!(
            foreign_status,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ));

        connection_a
            .send(&ClientMessage::SaveDocument {
                client_id: 11,
                document_id: beta_metadata.document_id,
                known_version: beta_metadata.version,
            })
            .await;
        let foreign_save = connection_a.receive_response().await;
        assert!(matches!(
            foreign_save,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ));

        connection_a
            .send(&ClientMessage::ReloadDocument {
                client_id: 11,
                document_id: beta_metadata.document_id,
                known_version: beta_metadata.version,
                force: true,
            })
            .await;
        let foreign_reload = connection_a.receive_response().await;
        assert!(matches!(
            foreign_reload,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ));

        connection_a
            .send(&ClientMessage::CloseDocument {
                client_id: 11,
                document_id: beta_metadata.document_id,
                force: true,
            })
            .await;
        let foreign_close = connection_a.receive_response().await;
        assert!(matches!(
            foreign_close,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ));

        connection_a
            .send(&ClientMessage::Edit {
                client_id: 11,
                document_id: beta_metadata.document_id,
                lease_id: beta_metadata.lease_id,
                base_version: beta_metadata.version,
                behavior_version: alpha_behavior_version,
                transaction_id: 7,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "leak".to_string(),
                },
            })
            .await;
        let foreign_edit = connection_a.receive().await;
        assert!(matches!(
            foreign_edit,
            ServerMessage::EditRejected {
                reason: EditRejection::InvalidDocument { document_id },
                ..
            } if document_id == beta_metadata.document_id
        ));

        connection_a
            .send(&ClientMessage::OpenSelectedFile {
                client_id: 11,
                capability: connection_b.file_open_capability.clone(),
                selected_path: root_b.join("beta.txt").to_string_lossy().into_owned(),
            })
            .await;
        let capability_replenish = connection_a.receive().await;
        let capability_rejection = connection_a.receive().await;
        assert!(matches!(
            capability_replenish,
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));
        assert!(matches!(
            capability_rejection,
            ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { ref code, .. })
                if code == "client.selected_file_open.unauthorized"
        ));

        connection_b
            .send(&ClientMessage::RequestResync {
                client_id: 22,
                document_id: beta_metadata.document_id,
                known_version: beta_metadata.version,
            })
            .await;
        let beta_resync = connection_b.receive_response().await;
        assert!(matches!(
            beta_resync,
            ServerMessage::ResyncSnapshot { ref text, document_id, .. }
                if document_id == beta_metadata.document_id && text == "beta"
        ));
        connection_b
            .send(&ClientMessage::GetDocumentStatus {
                client_id: 22,
                document_id: beta_metadata.document_id,
            })
            .await;
        let beta_status = connection_b.receive_response().await;
        assert!(matches!(
            beta_status,
            ServerMessage::DocumentStatus { ref metadata }
                if metadata.document_id == beta_metadata.document_id
                    && metadata.version == beta_metadata.version
                    && !metadata.dirty
        ));

        connection_a
            .send(&ClientMessage::TabCommand {
                client_id: 11,
                command: crate::protocol::TabCommand::Reclaim { tab_id: beta_tab },
            })
            .await;
        let reclaim_foreign = connection_a.receive().await;
        assert!(matches!(
            reclaim_foreign,
            ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                ..
            }
        ));
        let registry = server.tab_registry.lock().await.snapshot();
        assert!(
            registry
                .tabs
                .iter()
                .any(|entry| entry.tab_id == alpha_tab && entry.client_id == 11)
        );
        assert!(
            registry
                .tabs
                .iter()
                .any(|entry| entry.tab_id == beta_tab && entry.client_id == 22)
        );

        connection_a.close().await;
        connection_b.close().await;
        let _ = fs::remove_dir_all(root_a);
        let _ = fs::remove_dir_all(root_b);
        let _ = fs::remove_dir_all(extra_root_a);
    }

    /// Phase 24.3: `controlCenter.openPath` opens the Path Browser through
    /// the shared Command Centre helper — from its keybinding command and
    /// from the Control Center catalogue — with the one-active-session
    /// invariant enforced on every open.
    #[tokio::test]
    async fn path_browser_opens_from_keybinding_and_control_center_catalogue() {
        let root = temp_workspace("path-browser-open");
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "# path browser").unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-open"),
        ));
        let (tab_snapshot, _) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;
        let behavior_version = server.behavior.lock().await.version();

        // Keybinding path: one seed resolution + one bounded listing, pushed
        // as the initial snapshot (the active document is the tab's welcome
        // document, so the seed falls back to the bound tab's workspace root).
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.openPath".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(first) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        assert_eq!(first.prompt, format!("Browse · {}", root.display()));
        assert_eq!(first.query, format!("{}/", root.display()));
        let names: Vec<_> = first.items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(
            names,
            vec!["src", "README.md"],
            "empty filter keeps deterministic directory-first order"
        );
        let first_path_id = first.session_id;
        assert!(first_path_id & (1 << 63) != 0, "server-owned id partition");

        // Reopening replaces the active session and reports the closed id.
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.openPath".to_string(),
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id: closed }
                if closed == first_path_id
        ));
        let ServerMessage::TransientMenuSnapshot(second) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected replacement TransientMenuSnapshot");
        };
        assert_ne!(second.session_id, first_path_id);
        let second_path_id = second.session_id;
        connection
            .send(&ClientMessage::MenuCancel {
                client_id: 11,
                session_id: second_path_id,
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id: closed }
                if closed == second_path_id
        ));

        // Control Center path: the catalogue lists "Browse Filesystem";
        // activating it closes the Control Center and opens the Path Browser
        // through the same helper.
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.open".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(control_center) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected Control Center snapshot");
        };
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: control_center.session_id,
                query: "Browse Filesystem".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(filtered) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected filtered snapshot");
        };
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].id, "controlCenter.openPath");
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: control_center.session_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id: closed }
                if closed == control_center.session_id
        ));
        let ServerMessage::TransientMenuSnapshot(from_catalogue) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected Path Browser snapshot from catalogue activation");
        };
        assert_eq!(
            from_catalogue.prompt,
            format!("Browse · {}", root.display())
        );
        assert_eq!(from_catalogue.query, format!("{}/", root.display()));

        connection.drain_bounded().await;
        connection.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3: `controlCenter.openPath` opens the Path Browser through
    /// the shared Command Centre helper — from its keybinding command and
    /// from the Control Center catalogue — with the one-active-session
    /// invariant enforced on every open.
    #[tokio::test]
    async fn path_browser_opens_with_sticky_error_for_unlistable_seed() {
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-error-seed"),
        ));
        let root = temp_workspace("path-browser-error-seed");
        let (tab_snapshot, _) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;
        // The tab root vanishes after binding: the seed still resolves to it,
        // but the bounded listing fails. The command does not fail; the
        // session opens in its sticky error state (empty items, bounded
        // status) and stays cancellable.
        let _ = fs::remove_dir_all(&root);
        let behavior_version = server.behavior.lock().await.version();

        // A missing seed does not fail the command: the session opens in its
        // sticky error state (empty items, bounded status) and stays
        // cancellable.
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.openPath".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        assert!(snapshot.prompt.starts_with("Browse · "));
        assert!(snapshot.items.is_empty(), "items suppressed under error");
        assert!(matches!(
            snapshot.status,
            crate::protocol::TransientMenuStatusData::Empty { .. }
        ));
        connection
            .send(&ClientMessage::MenuCancel {
                client_id: 11,
                session_id: snapshot.session_id,
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id: closed }
                if closed == snapshot.session_id
        ));
        connection.drain_bounded().await;
        connection.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3 (task 8): directory descend (primary activation keeps the
    /// session open and relists), empty-filter Backspace ascent, direct
    /// absolute/relative path jumps, and invalid-path recovery — all with a
    /// stable session id and exactly one snapshot per accepted transition.
    #[tokio::test]
    async fn path_browser_navigates_descend_ascend_and_direct_jump() {
        let root = temp_workspace("path-browser-navigate");
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::create_dir(root.join("c")).unwrap();
        fs::write(root.join("notes.txt"), "notes").unwrap();
        fs::write(root.join("a/a1.txt"), "a1").unwrap();
        fs::write(root.join("a/b/b1.txt"), "b1").unwrap();
        fs::write(root.join("c/c1.txt"), "c1").unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-navigate"),
        ));
        let (tab_snapshot, _) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;
        let behavior_version = server.behavior.lock().await.version();

        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.openPath".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        let path_id = snapshot.session_id;
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["a", "c", "notes.txt"]);

        // Direct relative jump: typing `c/` from the tab root relists
        // `/root/c`.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: "c/".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected relist snapshot");
        };
        assert_eq!(snapshot.session_id, path_id, "session id stays stable");
        assert_eq!(snapshot.prompt, format!("Browse · {}/c", root.display()));
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["c1.txt"]);

        // Direct absolute jump into `a/b`.
        let a_b = format!("{}/a/b/", root.display());
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: a_b.clone(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected absolute jump snapshot");
        };
        assert_eq!(snapshot.session_id, path_id);
        assert_eq!(snapshot.prompt, format!("Browse · {}/a/b", root.display()));
        assert_eq!(snapshot.query, a_b);
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["b1.txt"]);

        // Empty-filter Backspace ascends one level (relist, same session).
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected ascent snapshot");
        };
        assert_eq!(snapshot.session_id, path_id);
        assert_eq!(snapshot.prompt, format!("Browse · {}/a", root.display()));
        assert_eq!(snapshot.query, format!("{}/a/", root.display()));
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["b", "a1.txt"]);

        // Primary activation on the selected directory (`b`, index 0)
        // descends: the session stays open and relists.
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected descend snapshot");
        };
        assert_eq!(snapshot.session_id, path_id, "descend keeps the session");
        assert_eq!(snapshot.prompt, format!("Browse · {}/a/b", root.display()));
        assert_eq!(snapshot.query, format!("{}/a/b/", root.display()));
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["b1.txt"]);

        // Filter-only edits never relist (no second snapshot): typing a
        // fuzzy fragment over the listing re-scores locally.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: "b1".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected filter-only snapshot");
        };
        assert_eq!(snapshot.session_id, path_id);
        assert_eq!(snapshot.query, "b1");
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["b1.txt"]);

        // Invalid direct jump: the menu stays open with a bounded error
        // status (items suppressed), and Backspace recovers by ascending to
        // the last canonical directory.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: format!("{}/missing/", root.display()),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected error-status snapshot");
        };
        assert_eq!(snapshot.session_id, path_id);
        assert!(snapshot.items.is_empty(), "items suppressed under error");
        assert!(matches!(
            snapshot.status,
            crate::protocol::TransientMenuStatusData::Empty { .. }
        ));
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected recovery snapshot");
        };
        assert_eq!(snapshot.session_id, path_id);
        assert_eq!(snapshot.prompt, format!("Browse · {}/a", root.display()));
        assert!(!snapshot.items.is_empty(), "recovered listing reinstated");

        // Path mode browses the whole filesystem: ascents past the tab root
        // continue to the filesystem root, where Backspace is a no-op. The
        // recovery above left the session at `/root/a`.
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected tab-root ascent snapshot");
        };
        assert_eq!(snapshot.session_id, path_id);
        assert_eq!(snapshot.prompt, format!("Browse · {}", root.display()));
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected /tmp ascent snapshot");
        };
        assert_eq!(snapshot.prompt, "Browse · /tmp");
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected filesystem-root snapshot");
        };
        assert_eq!(snapshot.prompt, "Browse · /");
        assert_eq!(snapshot.query, "/");
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected root no-op snapshot");
        };
        assert_eq!(snapshot.session_id, path_id);
        assert_eq!(snapshot.query, "/", "filesystem root Backspace is a no-op");

        connection.drain_bounded().await;
        connection.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3 (task 9): primary activation on a selected file closes the
    /// path session and runs the ordinary selected-file open — the browse
    /// activation itself is the authorization event that converts to exactly
    /// one `SingleFile` grant. The grant is strictly single-file (siblings
    /// fail `OutsideRoot`), duplicate opens return the same document id with
    /// no second view, and a file that disappeared between listing and
    /// activation fails without a grant or document leak.
    #[tokio::test]
    async fn path_browser_open_file_converts_browse_to_single_file_grant() {
        let root = temp_workspace("path-browser-open-file");
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/b.txt"), "b").unwrap();
        fs::write(root.join("a.txt"), "hello").unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-open-file"),
        ));
        let (tab_snapshot, tab_state) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;

        // Open the Path Browser with a fresh behavior stamp; a stale stamp
        // (a concurrent manifest publish bumps the connection-wide version)
        // is answered with a bounded Error, so resync and retry exactly like
        // the real client would.
        async fn open_path_browser(
            connection: &mut TestConnection,
            server: &super::super::IpcServer,
        ) -> ServerMessage {
            loop {
                connection
                    .send(&ClientMessage::CommandIntent {
                        client_id: 11,
                        document_id: 1,
                        behavior_version: server.behavior.lock().await.version(),
                        command_id: "controlCenter.openPath".to_string(),
                    })
                    .await;
                loop {
                    match timeout(Duration::from_secs(5), connection.receive()).await {
                        Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                        Ok(ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        }) if message.contains("behavior version is stale") => break,
                        Ok(_) => continue,
                        Err(_) => panic!("timed out awaiting path browser snapshot"),
                    }
                }
            }
        }

        // Read the next transient-menu frame, skipping parse/analysis noise
        // that can race an exchange; labeled so failures name the phase.
        async fn recv_menu_frame(
            label: &'static str,
            connection: &mut TestConnection,
        ) -> ServerMessage {
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(
                        message @ (ServerMessage::TransientMenuSnapshot(_)
                        | ServerMessage::TransientMenuClosed { .. }),
                    ) => return message,
                    Ok(_) => continue,
                    Err(_) => panic!("{label}: timed out awaiting transient menu frame"),
                }
            }
        }

        // Open the path browser and filter down to the file (directory-first
        // order puts `sub` at index 0).
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: "a.txt".to_string(),
            })
            .await;
        let snapshot = recv_menu_frame("filter", &mut connection).await;
        let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
            panic!("filter: expected snapshot, got closed");
        };
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["a.txt"]);

        // Primary activation: session closes first, then DocumentOpened with
        // the ordinary follow-up chain (no capability token involved).
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        let closed = recv_menu_frame("close-before-open", &mut connection).await;
        let ServerMessage::TransientMenuClosed { session_id } = closed else {
            panic!("close-before-open: expected closed, got {closed:?}");
        };
        assert_eq!(session_id, path_id, "menu closes before the open response");
        let (opened_root_id, opened_document_id) =
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::DocumentOpened { metadata, text }) => {
                    assert_eq!(text, "hello");
                    assert_eq!(metadata.path, "a.txt");
                    (metadata.workspace_root_id, metadata.document_id)
                }
                Ok(other) => panic!("expected DocumentOpened, got {other:?}"),
                Err(_) => panic!("timed out awaiting DocumentOpened"),
            };
        // Follow-ups arrive after the open frame; drain them.
        connection.drain_bounded().await;

        // The browse activation became a single-file grant: opening a
        // sibling document under that root fails OutsideRoot.
        connection
            .send(&ClientMessage::OpenDocument {
                client_id: 11,
                workspace_root_id: opened_root_id,
                path: "sub/b.txt".to_string(),
            })
            .await;
        let mut saw_outside_root = false;
        for _ in 0..8 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::FileOperationFailed {
                    code: FileErrorCode::OutsideRoot,
                    workspace_root_id: Some(id),
                    document_id: None,
                    ..
                }) if id == opened_root_id => {
                    saw_outside_root = true;
                    break;
                }
                Ok(ServerMessage::DecorationSet(_))
                | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
                | Ok(ServerMessage::RuntimeDiagnostic(_))
                | Ok(ServerMessage::BehaviorManifest(_)) => {}
                Ok(other) => panic!("expected outside-root failure, got {other:?}"),
                Err(_) => panic!("timed out awaiting outside-root failure"),
            }
        }
        assert!(saw_outside_root, "single-file grant rejects siblings");
        connection.drain_bounded().await;

        // Duplicate open: reopening the same file returns the same document
        // id with no second view or grant.
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: "a.txt".to_string(),
            })
            .await;
        let snapshot = recv_menu_frame("second-filter", &mut connection).await;
        let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
            panic!("second-filter: expected snapshot, got closed");
        };
        assert_eq!(snapshot.items.len(), 1);
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        let closed = recv_menu_frame("close-duplicate", &mut connection).await;
        assert!(
            matches!(closed, ServerMessage::TransientMenuClosed { .. }),
            "close-duplicate: expected closed, got {closed:?}"
        );
        let mut duplicate_id = None;
        for _ in 0..8 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::DocumentOpened { metadata, .. }) => {
                    duplicate_id = Some(metadata.document_id);
                    break;
                }
                Ok(ServerMessage::DecorationSet(_))
                | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
                | Ok(ServerMessage::RuntimeDiagnostic(_))
                | Ok(ServerMessage::BehaviorManifest(_)) => {}
                Ok(other) => panic!("expected duplicate DocumentOpened, got {other:?}"),
                Err(_) => panic!("timed out awaiting duplicate DocumentOpened"),
            }
        }
        assert_eq!(
            duplicate_id,
            Some(opened_document_id),
            "duplicate open returns the existing document"
        );
        connection.drain_bounded().await;
        let workspace = tab_state.workspace.lock().await;
        assert!(
            workspace
                .document_canonical_path(opened_document_id)
                .is_some()
        );
        assert!(
            workspace
                .document_canonical_path(opened_document_id + 1)
                .is_none(),
            "no second document created by the duplicate open"
        );
        drop(workspace);

        // Disappeared file: the listing was taken before deletion, so the
        // activation still resolves to the stale canonical path; the open
        // fails with a bounded FileOperationFailed, the session is closed,
        // and no grant or document appears.
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: "a.txt".to_string(),
            })
            .await;
        let snapshot = recv_menu_frame("third-filter", &mut connection).await;
        let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
            panic!("third-filter: expected snapshot, got closed");
        };
        assert_eq!(snapshot.items.len(), 1, "listing predates the deletion");
        fs::remove_file(root.join("a.txt")).unwrap();
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        let closed = recv_menu_frame("close-failed-open", &mut connection).await;
        assert!(
            matches!(closed, ServerMessage::TransientMenuClosed { .. }),
            "close-failed-open: expected closed, got {closed:?}"
        );
        let mut failed = false;
        for _ in 0..8 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::FileOperationFailed { .. }) => {
                    failed = true;
                    break;
                }
                Ok(ServerMessage::DecorationSet(_))
                | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
                | Ok(ServerMessage::RuntimeDiagnostic(_))
                | Ok(ServerMessage::BehaviorManifest(_)) => {}
                Ok(other) => panic!("expected FileOperationFailed, got {other:?}"),
                Err(_) => panic!("timed out awaiting FileOperationFailed"),
            }
        }
        assert!(failed, "disappeared file fails without an open");
        connection.drain_bounded().await;
        let workspace = tab_state.workspace.lock().await;
        assert!(
            workspace
                .document_canonical_path(opened_document_id)
                .is_some()
        );
        assert!(
            workspace
                .document_canonical_path(opened_document_id + 1)
                .is_none(),
            "failed open allocates no document"
        );
        drop(workspace);

        connection.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3 (task 10): secondary activation (Alt+Enter) on a selected
    /// directory closes the path session and opens that directory as the
    /// current tab's workspace — the browse activation itself is the
    /// authorization event that converts ephemeral browse authority into a
    /// `Directory` root grant. The bound tab's registry row rebinds to the
    /// canonical directory root and the file browser refreshes to the new
    /// root; a foreign tab's row is untouched; reopening the same directory
    /// deduplicates to the same root id; secondary activation on a file
    /// rejects without mutation.
    #[tokio::test]
    async fn path_browser_workspace_open_rebinds_only_bound_tab() {
        let root = temp_workspace("path-browser-open-workspace");
        fs::create_dir(root.join("alpha")).unwrap();
        fs::create_dir(root.join("alpha/inner")).unwrap();
        fs::write(root.join("alpha/file.txt"), "hi").unwrap();
        fs::write(root.join("beta.txt"), "b").unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let alpha = fs::canonicalize(root.join("alpha")).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-open-workspace"),
        ));
        let (tab_snapshot, tab_state) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        // Make the workspace pane visible so the rebind refresh carries the
        // file-browser listing (hidden by default in fresh tab state).
        tab_state.toggle_workspace_pane();
        // A second tab owned by a foreign client must stay untouched by the
        // bound tab's workspace open.
        let (foreign_snapshot, _) = server
            .create_tab_state(7, root.to_string_lossy().into_owned())
            .await
            .expect("foreign tab state is created");
        let foreign_tab_id = foreign_snapshot
            .tabs
            .iter()
            .find(|tab| tab.client_id == 7)
            .expect("foreign tab present")
            .tab_id;
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;

        async fn open_path_browser(
            connection: &mut TestConnection,
            server: &super::super::IpcServer,
        ) -> ServerMessage {
            loop {
                connection
                    .send(&ClientMessage::CommandIntent {
                        client_id: 11,
                        document_id: 1,
                        behavior_version: server.behavior.lock().await.version(),
                        command_id: "controlCenter.openPath".to_string(),
                    })
                    .await;
                loop {
                    match timeout(Duration::from_secs(5), connection.receive()).await {
                        Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                        Ok(ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        }) if message.contains("behavior version is stale") => break,
                        Ok(_) => continue,
                        Err(_) => panic!("timed out awaiting path browser snapshot"),
                    }
                }
            }
        }

        // Seed listing: directory-first order puts `alpha` at index 0.
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;
        let names: Vec<_> = snapshot
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "beta.txt"]);

        // Alt+Enter on the selected directory: the session closes first,
        // then the bound tab rebinds to the canonical directory root and the
        // file browser refreshes to the new root's listing.
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Secondary,
            })
            .await;
        let mut closed_id = None;
        let mut registry_snapshot = None;
        let mut saw_browser_refresh = false;
        for _ in 0..8 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                    assert_eq!(session_id, path_id, "workspace open closes the session");
                    closed_id = Some(session_id);
                }
                Ok(ServerMessage::TabRegistry(snapshot)) => registry_snapshot = Some(snapshot),
                Ok(ServerMessage::SduiSnapshot { tree, .. }) => {
                    // The refresh is the file-browser tree for the new root.
                    // Other snapshots (late reclaim follow-ups with the
                    // hidden editor-only tree) are noise; keep scanning.
                    // Directory labels carry a trailing separator ("inner/").
                    let labels: Vec<String> = tree
                        .nodes
                        .iter()
                        .find_map(|node| match &node.kind {
                            SduiNodeKind::List { items } => {
                                Some(items.iter().map(|item| item.label.clone()).collect())
                            }
                            _ => None,
                        })
                        .unwrap_or_default();
                    if labels.iter().any(|label| label == "inner/")
                        && labels.iter().any(|label| label == "file.txt")
                    {
                        saw_browser_refresh = true;
                    }
                }
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting workspace rebind frames"),
            }
            if closed_id.is_some() && registry_snapshot.is_some() && saw_browser_refresh {
                break;
            }
        }
        assert_eq!(closed_id, Some(path_id));
        assert!(
            saw_browser_refresh,
            "file browser refresh for the new root never arrived"
        );
        let registry_snapshot =
            registry_snapshot.expect("TabRegistry snapshot after workspace open");
        let bound = registry_snapshot
            .tabs
            .iter()
            .find(|tab| tab.tab_id == tab_id)
            .expect("bound tab present");
        assert_eq!(bound.workspace_root, alpha.to_string_lossy().as_ref());
        let foreign = registry_snapshot
            .tabs
            .iter()
            .find(|tab| tab.tab_id == foreign_tab_id)
            .expect("foreign tab present");
        assert_eq!(
            foreign.workspace_root,
            root.to_string_lossy().as_ref(),
            "other tabs' roots are untouched"
        );
        let bound_root_id = registry_snapshot
            .tabs
            .iter()
            .find(|tab| tab.tab_id == tab_id)
            .expect("bound tab present")
            .workspace_root_id;
        connection.drain_bounded().await;

        // Reopening the same directory deduplicates to the same root id: the
        // tab's seed is now the rebound root, so ascend back to the original
        // root and activate `alpha` again.
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        assert_eq!(
            snapshot.prompt,
            format!("Browse · {}", alpha.display()),
            "seed follows the rebound tab workspace root"
        );
        let path_id = snapshot.session_id;
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected ascent snapshot");
        };
        assert_eq!(snapshot.prompt, format!("Browse · {}", root.display()));
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Secondary,
            })
            .await;
        loop {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::TabRegistry(snapshot)) => {
                    let bound = snapshot
                        .tabs
                        .iter()
                        .find(|tab| tab.tab_id == tab_id)
                        .expect("bound tab present");
                    assert_eq!(
                        bound.workspace_root_id, bound_root_id,
                        "same canonical directory deduplicates to the same root id"
                    );
                    break;
                }
                Ok(ServerMessage::TransientMenuClosed { .. }) => {}
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting deduplicated TabRegistry snapshot"),
            }
        }
        connection.drain_bounded().await;

        // Secondary activation on a file is not a workspace open: the
        // session closes and the bounded diagnostic names the rejection.
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: "file.txt".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected filter snapshot");
        };
        assert_eq!(snapshot.items.len(), 1);
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Secondary,
            })
            .await;
        let mut saw_rejection = false;
        for _ in 0..4 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::TransientMenuClosed { .. }) => {}
                Ok(ServerMessage::Error { message, .. }) => {
                    assert!(
                        message.contains("no activation"),
                        "file has no secondary activation: {message}"
                    );
                    saw_rejection = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting file-activation rejection"),
            }
        }
        assert!(saw_rejection);

        connection.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3 (task 10): secondary activation on a directory that
    /// vanished between listing and activation rejects with the bounded
    /// file-operation failure and leaves the tab's workspace root unchanged.
    #[tokio::test]
    async fn path_browser_workspace_open_rejects_vanished_directory() {
        let root = temp_workspace("path-browser-vanished-workspace");
        fs::create_dir(root.join("alpha")).unwrap();
        fs::write(root.join("beta.txt"), "b").unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-vanished-workspace"),
        ));
        let (tab_snapshot, _) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;

        async fn open_path_browser(
            connection: &mut TestConnection,
            server: &super::super::IpcServer,
        ) -> ServerMessage {
            loop {
                connection
                    .send(&ClientMessage::CommandIntent {
                        client_id: 11,
                        document_id: 1,
                        behavior_version: server.behavior.lock().await.version(),
                        command_id: "controlCenter.openPath".to_string(),
                    })
                    .await;
                loop {
                    match timeout(Duration::from_secs(5), connection.receive()).await {
                        Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                        Ok(ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        }) if message.contains("behavior version is stale") => break,
                        Ok(_) => continue,
                        Err(_) => panic!("timed out awaiting path browser snapshot"),
                    }
                }
            }
        }

        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;
        // The directory disappears after the listing installed.
        fs::remove_dir_all(root.join("alpha")).unwrap();
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Secondary,
            })
            .await;
        let mut saw_failure = false;
        for _ in 0..4 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                    assert_eq!(session_id, path_id);
                }
                Ok(ServerMessage::FileOperationFailed {
                    code: FileErrorCode::NotFound,
                    workspace_root_id: None,
                    document_id: None,
                    ..
                }) => {
                    saw_failure = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting vanished-directory failure"),
            }
        }
        assert!(saw_failure, "vanished directory fails with NotFound");

        // The tab's workspace root did not change: a fresh Path Browser still
        // seeds from the original root.
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        assert_eq!(
            snapshot.prompt,
            format!("Browse · {}", root.display()),
            "failed workspace open leaves the tab root unchanged"
        );

        connection.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3 (task 10): the shared bound-tab workspace-open helper
    /// rejects a connection with no bound tab before touching the workspace
    /// or registry.
    #[tokio::test]
    async fn open_workspace_helper_requires_bound_tab() {
        let workspace = workspace_state();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let sdui = sdui_state();
        let (registry, tab_registry_tx) = two_tab_registry();
        let messages = open_workspace_for_bound_tab(
            &workspace,
            &document,
            &sdui,
            &registry,
            &tab_registry_tx,
            None,
            99,
            None,
            PathBuf::from("/tmp/unused"),
        )
        .await;
        assert_eq!(messages.len(), 1);
        match &messages[0] {
            ServerMessage::Error { message, .. } => {
                assert!(message.contains("requires a bound tab"));
            }
            other => panic!("expected bound-tab rejection, got {other:?}"),
        }
    }

    /// Phase 24.3 (task 11): browsing alone — open, filter, descend, ascend,
    /// direct jump, cancel — never allocates a root grant or opens a
    /// document. Activation is the single grant conversion point.
    #[tokio::test]
    async fn path_browser_navigation_only_creates_no_grants() {
        let root = temp_workspace("path-browser-navigation-only");
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("README.md"), "r").unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-navigation-only"),
        ));
        let (tab_snapshot, tab_state) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let workspace = Arc::clone(&tab_state.workspace);
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;
        assert_eq!(workspace.lock().await.directory_roots().len(), 1);

        async fn open_path_browser(
            connection: &mut TestConnection,
            server: &super::super::IpcServer,
        ) -> ServerMessage {
            loop {
                connection
                    .send(&ClientMessage::CommandIntent {
                        client_id: 11,
                        document_id: 1,
                        behavior_version: server.behavior.lock().await.version(),
                        command_id: "controlCenter.openPath".to_string(),
                    })
                    .await;
                loop {
                    match timeout(Duration::from_secs(5), connection.receive()).await {
                        Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                        Ok(ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        }) if message.contains("behavior version is stale") => break,
                        Ok(_) => continue,
                        Err(_) => panic!("timed out awaiting path browser snapshot"),
                    }
                }
            }
        }

        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;

        // Filter-only edit: no filesystem work, no grant.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: "RE".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(filtered) =
            timeout(Duration::from_secs(5), connection.receive())
                .await
                .expect("filter snapshot")
        else {
            panic!("expected snapshot after filter edit");
        };
        assert_eq!(
            filtered.session_id, path_id,
            "session id stable across edits"
        );
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].label, "README.md");

        // Descend into src (primary on the directory after clearing the
        // filter): the session stays open and relists.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: String::new(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(_) =
            timeout(Duration::from_secs(5), connection.receive())
                .await
                .expect("cleared filter snapshot")
        else {
            panic!("expected snapshot after clearing the filter");
        };
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(descended) =
            timeout(Duration::from_secs(5), connection.receive())
                .await
                .expect("descend snapshot")
        else {
            panic!("expected snapshot after descend");
        };
        assert_eq!(
            descended.session_id, path_id,
            "session id stable across descend"
        );
        assert_eq!(
            descended.prompt,
            format!("Browse · {}/src", root.display()),
            "descend relists the canonical target"
        );
        assert_eq!(descended.items.len(), 1);
        assert_eq!(descended.items[0].label, "main.rs");

        // Ascend (Backspace on the empty filter) back to the tab root.
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(ascended) =
            timeout(Duration::from_secs(5), connection.receive())
                .await
                .expect("ascend snapshot")
        else {
            panic!("expected snapshot after ascend");
        };
        assert_eq!(
            ascended.prompt,
            format!("Browse · {}", root.display()),
            "empty-filter Backspace ascends to the parent"
        );

        // Direct jump to a typed absolute directory.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: path_id,
                query: format!("{}/src/", root.display()),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(jumped) =
            timeout(Duration::from_secs(5), connection.receive())
                .await
                .expect("direct jump snapshot")
        else {
            panic!("expected snapshot after direct jump");
        };
        assert_eq!(
            jumped.prompt,
            format!("Browse · {}/src", root.display()),
            "direct path edit jumps to the typed directory"
        );

        // Cancel: the session closes, still no grant or document.
        connection
            .send(&ClientMessage::MenuCancel {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuClosed { session_id } =
            timeout(Duration::from_secs(5), connection.receive())
                .await
                .expect("cancel frame")
        else {
            panic!("expected menu close");
        };
        assert_eq!(session_id, path_id);

        // Nothing but menu frames flowed, and the workspace gained no root.
        for _ in 0..16 {
            if timeout(
                Duration::from_millis(10),
                connection.codec.read_server_message(&mut connection.client),
            )
            .await
            .is_err()
            {
                break;
            }
        }
        assert_eq!(
            workspace.lock().await.directory_roots().len(),
            1,
            "browse navigation alone creates no root grants"
        );
        connection.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3 (task 11): a session id from one connection is opaque to
    /// every other connection — cross-client activation fails closed with the
    /// bounded `menu.unknown_session` diagnostic and never disturbs the
    /// owning session.
    #[tokio::test]
    async fn path_browser_cross_client_activation_denied() {
        let root = temp_workspace("path-browser-cross-client");
        fs::write(root.join("a.txt"), "a").unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-cross-client"),
        ));
        let (tab_snapshot, _) = server
            .create_tab_state(11, root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let (foreign_snapshot, _) = server
            .create_tab_state(22, root.to_string_lossy().into_owned())
            .await
            .expect("foreign tab state is created");
        let foreign_tab_id = foreign_snapshot
            .tabs
            .iter()
            .find(|tab| tab.client_id == 22)
            .expect("foreign tab present")
            .tab_id;
        let mut connection_a = TestConnection::connect_with_server(11, server.clone()).await;
        connection_a.reclaim(11, tab_id).await;
        connection_a.drain_bounded().await;
        let mut connection_b = TestConnection::connect_with_server(22, server.clone()).await;
        connection_b.reclaim(22, foreign_tab_id).await;
        connection_b.drain_bounded().await;

        async fn open_path_browser(
            connection: &mut TestConnection,
            server: &super::super::IpcServer,
        ) -> ServerMessage {
            loop {
                connection
                    .send(&ClientMessage::CommandIntent {
                        client_id: 11,
                        document_id: 1,
                        behavior_version: server.behavior.lock().await.version(),
                        command_id: "controlCenter.openPath".to_string(),
                    })
                    .await;
                loop {
                    match timeout(Duration::from_secs(5), connection.receive()).await {
                        Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                        Ok(ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        }) if message.contains("behavior version is stale") => break,
                        Ok(_) => continue,
                        Err(_) => panic!("timed out awaiting path browser snapshot"),
                    }
                }
            }
        }

        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection_a, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;

        // Client B cannot drive A's session: the id is per-connection opaque.
        connection_b
            .send(&ClientMessage::MenuActivate {
                client_id: 22,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        let mut saw_denial = false;
        for _ in 0..4 {
            match timeout(Duration::from_secs(2), connection_b.receive()).await {
                Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic {
                    code, message, ..
                })) if code == "menu.unknown_session" => {
                    assert!(message.contains(&path_id.to_string()));
                    saw_denial = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting cross-client denial"),
            }
        }
        assert!(saw_denial, "foreign session id fails closed");

        // A's session is untouched: it still cancels with the expected frame.
        connection_a
            .send(&ClientMessage::MenuCancel {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let ServerMessage::TransientMenuClosed { session_id } =
            timeout(Duration::from_secs(5), connection_a.receive())
                .await
                .expect("owner cancel frame")
        else {
            panic!("expected owner menu close");
        };
        assert_eq!(
            session_id, path_id,
            "owning connection still holds the session"
        );

        connection_a.close().await;
        connection_b.close().await;
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 24.3 (task 11): the per-connection session store survives tab
    /// rebinds and drops cleanly on disconnect.
    #[tokio::test]
    async fn path_browser_survives_tab_switch_and_disconnect() {
        let root_a = temp_workspace("path-browser-tab-switch-a");
        fs::write(root_a.join("a.txt"), "a").unwrap();
        let root_b = temp_workspace("path-browser-tab-switch-b");
        fs::write(root_b.join("b.txt"), "b").unwrap();
        let root_a = fs::canonicalize(&root_a).unwrap();
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("path-browser-tab-switch"),
        ));
        let (tab_a_snapshot, _) = server
            .create_tab_state(11, root_a.to_string_lossy().into_owned())
            .await
            .expect("tab a is created");
        let (tab_b_snapshot, _) = server
            .create_tab_state(11, root_b.to_string_lossy().into_owned())
            .await
            .expect("tab b is created");
        let tab_a = tab_a_snapshot.tabs[0].tab_id;
        let tab_b = tab_b_snapshot.tabs[1].tab_id;
        assert_ne!(tab_a, tab_b);

        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_a).await;
        connection.drain_bounded().await;

        async fn open_path_browser(
            connection: &mut TestConnection,
            server: &super::super::IpcServer,
        ) -> ServerMessage {
            loop {
                connection
                    .send(&ClientMessage::CommandIntent {
                        client_id: 11,
                        document_id: 1,
                        behavior_version: server.behavior.lock().await.version(),
                        command_id: "controlCenter.openPath".to_string(),
                    })
                    .await;
                loop {
                    match timeout(Duration::from_secs(5), connection.receive()).await {
                        Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                        Ok(ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        }) if message.contains("behavior version is stale") => break,
                        Ok(_) => continue,
                        Err(_) => panic!("timed out awaiting path browser snapshot"),
                    }
                }
            }
        }

        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;

        // Activating the second tab dismisses the session Escape-free (focus
        // loss) with the ordinary close frame; the session is then gone.
        connection
            .send(&ClientMessage::TabCommand {
                client_id: 11,
                command: crate::protocol::TabCommand::Activate { tab_id: tab_b },
            })
            .await;
        let ServerMessage::TransientMenuClosed { session_id } =
            timeout(Duration::from_secs(5), connection.receive())
                .await
                .expect("tab switch close frame")
        else {
            panic!("expected menu close on tab switch");
        };
        assert_eq!(session_id, path_id, "tab switch dismisses the session");
        connection
            .send(&ClientMessage::MenuCancel {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let mut saw_unknown = false;
        for _ in 0..4 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. }))
                    if code == "menu.unknown_session" =>
                {
                    saw_unknown = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting unknown-session diagnostic"),
            }
        }
        assert!(saw_unknown, "dismissed session is gone");

        // Disconnect sweeps the store; `close` fails the test if the
        // connection task panicked or leaked a session.
        connection.close().await;
        let _ = fs::remove_dir_all(&root_a);
        let _ = fs::remove_dir_all(&root_b);
    }

    /// Phase 24.3 (task 11): after a runtime reload bumps the generation
    /// stamp, an open session fails closed with the bounded stale-generation
    /// diagnostic instead of executing against the old generation — and the
    /// session is then gone, so cancel reports `menu.unknown_session`.
    #[tokio::test]
    async fn path_browser_activation_after_runtime_reload_fails_closed() {
        let config_root = temp_workspace("path-browser-reload-config");
        fs::write(config_root.join("init.js"), "").unwrap();
        let workspace_root = temp_workspace("path-browser-reload-workspace");
        fs::write(workspace_root.join("a.txt"), "a").unwrap();
        let workspace_root = fs::canonicalize(&workspace_root).unwrap();
        let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
            "path-browser-reload",
        ));
        config.configuration_root = Some(config_root.clone());
        let server = super::super::IpcServer::new(config);
        let (tab_snapshot, _) = server
            .create_tab_state(11, workspace_root.to_string_lossy().into_owned())
            .await
            .expect("tab state is created");
        let tab_id = tab_snapshot.tabs[0].tab_id;
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;

        async fn open_path_browser(
            connection: &mut TestConnection,
            server: &super::super::IpcServer,
        ) -> ServerMessage {
            loop {
                connection
                    .send(&ClientMessage::CommandIntent {
                        client_id: 11,
                        document_id: 1,
                        behavior_version: server.behavior.lock().await.version(),
                        command_id: "controlCenter.openPath".to_string(),
                    })
                    .await;
                loop {
                    match timeout(Duration::from_secs(5), connection.receive()).await {
                        Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                        Ok(ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        }) if message.contains("behavior version is stale") => break,
                        Ok(_) => continue,
                        Err(_) => panic!("timed out awaiting path browser snapshot"),
                    }
                }
            }
        }

        let ServerMessage::TransientMenuSnapshot(snapshot) =
            open_path_browser(&mut connection, &server).await
        else {
            panic!("expected path browser snapshot");
        };
        let path_id = snapshot.session_id;
        assert_eq!(server.runtime_generation.generation_id().await, 1);

        let outcome = server.reload_runtime_generation().await;
        assert!(
            outcome.reloaded,
            "reload succeeds with the empty init.js config"
        );
        assert_eq!(server.runtime_generation.generation_id().await, 2);

        // Activation with the old stamp fails closed: bounded diagnostic, no
        // execution, and the session is consumed like any rejected activation.
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: path_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        // Reload publishes the new generation snapshot; the loop closes the
        // active session first (the catalogue is generation-bound), exactly
        // like a tab switch dismisses on focus loss.
        let mut saw_closed = false;
        for _ in 0..8 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                    assert_eq!(session_id, path_id);
                    saw_closed = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting reload dismissal"),
            }
        }
        assert!(saw_closed, "runtime reload dismisses the active session");

        // The session is gone: cancel reports the bounded unknown-session
        // diagnostic rather than a spurious close.
        connection
            .send(&ClientMessage::MenuCancel {
                client_id: 11,
                session_id: path_id,
            })
            .await;
        let mut saw_unknown = false;
        for _ in 0..4 {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. }))
                    if code == "menu.unknown_session" =>
                {
                    saw_unknown = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting unknown-session diagnostic"),
            }
        }
        assert!(saw_unknown, "cancelled session reports unknown_session");

        connection.close().await;
        let _ = fs::remove_dir_all(&config_root);
        let _ = fs::remove_dir_all(&workspace_root);
    }

    /// Phase 24.3 (task 11): the package/generic command lane cannot open
    /// the Path Browser. Only the connection's `CommandIntent` special case
    /// and the Control Centre catalogue's `MenuActivate` special case reach
    /// the session store; the shared executor (the lane package callbacks
    /// run through) yields nothing on the wire for the built-in id.
    #[tokio::test]
    async fn package_command_lane_cannot_open_path_browser() {
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("package-lane-path-browser"),
        ));
        let response = execute_command_intent(
            CommandExecutionRequest {
                command_id: "controlCenter.openPath".to_string(),
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&server.workspace),
            &server.document,
            &server.sdui,
            1,
            Some(&server),
            &CommandRegistry::new(),
        )
        .await;
        assert!(
            response.is_none(),
            "generic execution of controlCenter.openPath must yield no message"
        );
    }

    /// Phase 24.1: menu intents naming sessions this connection does not hold
    /// are dropped with a bounded `menu.unknown_session` diagnostic — never an
    /// error or disconnect. Sessions are per-tab (per-connection), so the
    /// connection must be tab-bound first.
    #[tokio::test]
    async fn menu_intents_for_unknown_sessions_produce_bounded_diagnostics() {
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("menu-unknown-session"),
        ));
        let (snapshot, _state) = server
            .create_tab_state(
                11,
                temp_workspace("menu-unknown")
                    .to_string_lossy()
                    .into_owned(),
            )
            .await
            .expect("tab state is created");
        let tab_id = snapshot.tabs[0].tab_id;

        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, tab_id).await;
        connection.drain_bounded().await;

        for message in [
            ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: 1 << 63 | 7,
                query: "reload".to_string(),
            },
            ClientMessage::MenuSelectionMove {
                client_id: 11,
                session_id: 1 << 63 | 7,
                delta: 1,
            },
            ClientMessage::MenuActivate {
                client_id: 11,
                session_id: 1 << 63 | 7,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            },
            ClientMessage::MenuCancel {
                client_id: 11,
                session_id: 1 << 63 | 7,
            },
        ] {
            connection.send(&message).await;
            let response = connection.receive().await;
            assert!(
                matches!(
                    response,
                    ServerMessage::RuntimeDiagnostic(ref diagnostic)
                        if diagnostic.code == "menu.unknown_session"
                ),
                "unexpected response: {response:?}"
            );
        }

        // The connection is still alive and functional (never a disconnect).
        connection
            .send(&ClientMessage::ListDocuments { client_id: 11 })
            .await;
        assert!(matches!(
            connection.receive_response().await,
            ServerMessage::DocumentList { .. }
        ));
        connection.close().await;
    }

    /// Read until a transient-menu frame arrives, skipping parse/activation
    /// noise that can race a menu exchange.
    async fn receive_menu_message(connection: &mut TestConnection) -> ServerMessage {
        loop {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(
                    message @ (ServerMessage::TransientMenuSnapshot(_)
                    | ServerMessage::TransientMenuClosed { .. }),
                ) => return message,
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting transient menu frame"),
            }
        }
    }

    #[tokio::test]
    async fn control_center_opens_filters_activates_and_cancels() {
        // Plan 086 task 7: whole-workflow bound. A hang here means pending
        // session cleanup, not a slow machine (measured ~0.03s); the timeout
        // names the failure instead of waiting indefinitely.
        timeout(
            Duration::from_secs(5),
            control_center_opens_filters_activates_and_cancels_scenario(),
        )
        .await
        .expect(
            "control_center_opens_filters_activates_and_cancels exceeded its 5s whole-workflow bound; \
             look for pending session or reply-receiver cleanup",
        );
    }

    async fn control_center_opens_filters_activates_and_cancels_scenario() {
        let root = temp_workspace("control-center");
        // Hermetic configuration root (Phase 24.5, task 8): without an
        // explicit root this test fell back to the real ~/.config/clay and
        // hung whenever that directory contains an init.js (reload evaluates
        // the live user config). The sentinel typography proves the hermetic
        // root — not ambient ~/.config/clay — is the generation source: an
        // ambient fallback would load the default 20px monospace, not 21px.
        let config_root = temp_workspace("control-center-config");
        fs::write(
            config_root.join("init.js"),
            "import { setTypography } from \"clay:theme\"; setTypography({ monospace: { families: [\"MartianMono Nerd Font\", \"monospace\"], size: 21 }, proportional: { families: [\"Noto Sans\", \"sans-serif\"], size: 17 }, ui: { families: [\"system-ui\"], size: 13 } });",
        )
        .unwrap();
        let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
            "control-center-open",
        ));
        config.workspace_roots.push(root.clone());
        config.configuration_root = Some(config_root);
        let server = super::super::IpcServer::new(config);
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        let open = |behavior_version| ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        };
        let mut behavior_version = server.behavior.lock().await.version();

        // Opening replaces any active session: the first open delivers a
        // snapshot, a second open closes the old session and returns a
        // distinct new session id.
        connection.send(&open(behavior_version)).await;
        let ServerMessage::TransientMenuSnapshot(first_snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected first TransientMenuSnapshot");
        };
        let first_session_id = first_snapshot.session_id;
        connection.send(&open(behavior_version)).await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id }
                if session_id == first_session_id
        ));
        let ServerMessage::TransientMenuSnapshot(second_snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected replacement TransientMenuSnapshot");
        };
        let second_session_id = second_snapshot.session_id;
        assert_ne!(first_session_id, second_session_id);

        // A stale selection move against the replaced session is a bounded
        // diagnostic, never an error or disconnect.
        connection
            .send(&ClientMessage::MenuSelectionMove {
                client_id: 11,
                session_id: first_session_id,
                delta: 1,
            })
            .await;
        assert!(matches!(
            connection.receive().await,
            ServerMessage::RuntimeDiagnostic(ref diagnostic)
                if diagnostic.code == "menu.unknown_session"
        ));

        // Query filtering narrows the live session's items.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id: second_session_id,
                query: "reload".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(filtered) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected filtered TransientMenuSnapshot");
        };
        assert!(
            !filtered.items.is_empty()
                && filtered.items.iter().all(|item| item.id.contains("reload")),
            "filtered items must all match the query: {:?}",
            filtered
                .items
                .iter()
                .map(|item| &item.id)
                .collect::<Vec<_>>()
        );

        // Activating the selected item closes the menu and executes the
        // server command; the reload fanout (diagnostic + snapshot) arrives
        // asynchronously after the close.
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id: second_session_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id }
                if session_id == second_session_id
        ));
        let mut saw_reload_diagnostic = false;
        let mut saw_runtime_snapshot = false;
        for _ in 0..64 {
            match connection.receive().await {
                ServerMessage::RuntimeDiagnostic(ref diagnostic)
                    if diagnostic.code == "runtime.reload_succeeded" =>
                {
                    saw_reload_diagnostic = true;
                }
                ServerMessage::RuntimeStateSnapshot(snapshot) => {
                    saw_runtime_snapshot = true;
                    behavior_version = snapshot.behavior.behavior_version;
                }
                ServerMessage::BehaviorManifest(manifest) => {
                    behavior_version = manifest.behavior_version;
                }
                _ => {}
            }
            if saw_reload_diagnostic && saw_runtime_snapshot {
                break;
            }
        }
        assert!(
            saw_reload_diagnostic && saw_runtime_snapshot,
            "reload fanout must deliver the diagnostic and snapshot"
        );
        // The hermetic root (not ambient ~/.config/clay) was the reload
        // source: the sentinel typography from its init.js is now live.
        assert_eq!(
            server
                .runtime_generation
                .active_typography()
                .await
                .monospace
                .size,
            21.0,
            "reloaded generation must come from the hermetic config root"
        );

        // Reopening after the generation replacement yields a fresh session
        // id (the old generation's session is gone).
        connection.send(&open(behavior_version)).await;
        let ServerMessage::TransientMenuSnapshot(third_snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected reopened TransientMenuSnapshot");
        };
        assert_ne!(third_snapshot.session_id, second_session_id);

        // Escape (MenuCancel) closes the active session with a close frame.
        connection
            .send(&ClientMessage::MenuCancel {
                client_id: 11,
                session_id: third_snapshot.session_id,
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id }
                if session_id == third_snapshot.session_id
        ));

        // The connection is still alive and functional.
        connection
            .send(&ClientMessage::ListDocuments { client_id: 11 })
            .await;
        assert!(matches!(
            connection.receive_response().await,
            ServerMessage::DocumentList { .. }
        ));
        connection.drain_bounded().await;
        connection.close().await;
    }

    #[tokio::test]
    async fn control_center_shell_activation_sends_shell_command_request() {
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("control-center-shell"),
        ));
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        let behavior_version = server.behavior.lock().await.version();
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.open".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        let session_id = snapshot.session_id;
        assert!(
            snapshot
                .items
                .iter()
                .any(|item| item.id == "shell.clientSplitPaneVertical"),
            "shell.client* entries must appear in the Control Center listing"
        );

        // Fuzzy query narrows to exactly the shell entry.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id,
                query: "clientSplitPaneVertical".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(filtered) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected filtered TransientMenuSnapshot");
        };
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].id, "shell.clientSplitPaneVertical");

        // Activation closes the menu, then ships the narrow shell-command
        // request frame the client re-parses deny-by-default.
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id: closed }
                if closed == session_id
        ));
        assert_eq!(
            connection.receive().await,
            ServerMessage::ShellClientCommandRequest {
                command_id: "shell.clientSplitPaneVertical".to_string(),
            }
        );
        connection.drain_bounded().await;
        connection.close().await;
    }

    #[tokio::test]
    async fn menu_backspace_deletes_one_char_and_secondary_activation_matches_primary() {
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("menu-backspace-secondary"),
        ));
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        let behavior_version = server.behavior.lock().await.version();
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.open".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        let session_id = snapshot.session_id;

        // Backspace on an empty query is a bounded no-op snapshot.
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(after_empty_backspace) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        assert_eq!(after_empty_backspace.query, "");

        // Backspace deletes exactly one query character (Control Center
        // semantics; path mode overrides with ascend in task 8).
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id,
                query: "clientSplitPaneVertical".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(filtered) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected filtered TransientMenuSnapshot");
        };
        assert_eq!(filtered.items.len(), 1);
        connection
            .send(&ClientMessage::MenuBackspace {
                client_id: 11,
                session_id,
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(after_backspace) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        assert_eq!(after_backspace.query, "clientSplitPaneVertica");

        // Restore the exact query, then activate with the secondary kind:
        // the Control Center executes the same selection as primary.
        connection
            .send(&ClientMessage::MenuQueryUpdate {
                client_id: 11,
                session_id,
                query: "clientSplitPaneVertical".to_string(),
            })
            .await;
        let _ = receive_menu_message(&mut connection).await;
        connection
            .send(&ClientMessage::MenuActivate {
                client_id: 11,
                session_id,
                kind: crate::protocol::TransientMenuActivationData::Secondary,
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id: closed }
                if closed == session_id
        ));
        assert_eq!(
            connection.receive().await,
            ServerMessage::ShellClientCommandRequest {
                command_id: "shell.clientSplitPaneVertical".to_string(),
            }
        );
        connection.drain_bounded().await;
        connection.close().await;
    }

    #[tokio::test]
    async fn runtime_generation_replacement_cancels_open_control_center() {
        // Plan 086 task 7: whole-workflow bound. A hang here means the
        // replacement left a pending session or reply receiver; the timeout
        // names that instead of waiting indefinitely.
        timeout(
            Duration::from_secs(5),
            runtime_generation_replacement_cancels_open_control_center_scenario(),
        )
        .await
        .expect(
            "runtime_generation_replacement_cancels_open_control_center exceeded its 5s whole-workflow bound; \
             look for pending session or reply-receiver cleanup",
        );
    }

    async fn runtime_generation_replacement_cancels_open_control_center_scenario() {
        // Hermetic configuration root (Phase 24.5, task 8): same real-config
        // fallback hazard as control_center_opens_filters_activates_and_cancels.
        // Sentinel typography proves the hermetic root is the generation
        // source (ambient ~/.config/clay must never load).
        let config_root = temp_workspace("control-center-generation-config");
        fs::write(
            config_root.join("init.js"),
            "import { setTypography } from \"clay:theme\"; setTypography({ monospace: { families: [\"MartianMono Nerd Font\", \"monospace\"], size: 21 }, proportional: { families: [\"Noto Sans\", \"sans-serif\"], size: 17 }, ui: { families: [\"system-ui\"], size: 13 } });",
        )
        .unwrap();
        let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
            "control-center-generation",
        ));
        config.configuration_root = Some(config_root);
        let server = super::super::IpcServer::new(config);
        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        let mut behavior_version = server.behavior.lock().await.version();
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.open".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        let session_id = snapshot.session_id;

        // A direct reload while the menu is open replaces the runtime
        // generation; the broadcast cancels the open menu session before
        // replaying the replacement state.
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "runtime.reloadConfiguration".to_string(),
            })
            .await;
        let mut saw_reload_diagnostic = false;
        let mut saw_menu_close = false;
        let mut saw_runtime_snapshot = false;
        for _ in 0..64 {
            match connection.receive().await {
                ServerMessage::RuntimeDiagnostic(ref diagnostic)
                    if diagnostic.code == "runtime.reload_succeeded" =>
                {
                    saw_reload_diagnostic = true;
                }
                ServerMessage::TransientMenuClosed { session_id: closed }
                    if closed == session_id =>
                {
                    saw_menu_close = true;
                }
                ServerMessage::RuntimeStateSnapshot(snapshot) => {
                    saw_runtime_snapshot = true;
                    behavior_version = snapshot.behavior.behavior_version;
                }
                ServerMessage::BehaviorManifest(manifest) => {
                    behavior_version = manifest.behavior_version;
                }
                _ => {}
            }
            if saw_reload_diagnostic && saw_menu_close && saw_runtime_snapshot {
                break;
            }
        }
        assert!(
            saw_reload_diagnostic && saw_menu_close && saw_runtime_snapshot,
            "generation replacement must close the open menu and replay state"
        );
        // The hermetic root (not ambient ~/.config/clay) was the reload
        // source: the sentinel typography from its init.js is now live.
        assert_eq!(
            server
                .runtime_generation
                .active_typography()
                .await
                .monospace
                .size,
            21.0,
            "replaced generation must come from the hermetic config root"
        );

        // The reopened menu is stamped with the replacement generation and
        // gets a distinct session id.
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.open".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(reopened) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected reopened TransientMenuSnapshot");
        };
        assert_ne!(reopened.session_id, session_id);

        // No pending session survives the replacement: a stale selection
        // against the cancelled session id is a bounded diagnostic, never a
        // reply from a live session or a hang.
        connection
            .send(&ClientMessage::MenuSelectionMove {
                client_id: 11,
                session_id,
                delta: 1,
            })
            .await;
        assert!(matches!(
            connection.receive().await,
            ServerMessage::RuntimeDiagnostic(ref diagnostic)
                if diagnostic.code == "menu.unknown_session"
        ));
        connection.drain_bounded().await;
        connection.close().await;
    }

    #[tokio::test]
    async fn tab_switch_cancels_the_active_server_menu_session() {
        let root_a = temp_workspace("menu-tab-alpha");
        let root_b = temp_workspace("menu-tab-beta");
        let server = super::super::IpcServer::new(super::super::ServerConfig::new(
            crate::ipc::IpcEndpoint::from_argument("menu-tab-switch"),
        ));
        let (first_snapshot, _) = server
            .create_tab_state(11, root_a.to_string_lossy().into_owned())
            .await
            .expect("first tab state is created");
        let first_tab = first_snapshot.tabs[0].tab_id;
        let (second_snapshot, _) = server
            .create_tab_state(11, root_b.to_string_lossy().into_owned())
            .await
            .expect("second tab state is created");
        let second_tab = second_snapshot.tabs[0].tab_id;

        let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
        connection.reclaim(11, first_tab).await;
        connection.drain_bounded().await;
        let behavior_version = server.behavior.lock().await.version();
        connection
            .send(&ClientMessage::CommandIntent {
                client_id: 11,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.open".to_string(),
            })
            .await;
        let ServerMessage::TransientMenuSnapshot(snapshot) =
            receive_menu_message(&mut connection).await
        else {
            panic!("expected TransientMenuSnapshot");
        };
        let session_id = snapshot.session_id;

        // Switching to the second tab dismisses the open menu (Escape-free
        // dismissal on focus loss) with an explicit close frame.
        connection
            .send(&ClientMessage::TabCommand {
                client_id: 11,
                command: crate::protocol::TabCommand::Activate { tab_id: second_tab },
            })
            .await;
        assert!(matches!(
            receive_menu_message(&mut connection).await,
            ServerMessage::TransientMenuClosed { session_id: closed }
                if closed == session_id
        ));
        connection.drain_bounded().await;
        connection.close().await;
    }

    #[tokio::test]
    async fn package_command_dispatchs_through_shared_dispatcher_with_live_registry() {
        // A validated package command resolves through the live aggregated
        // registry passed by the menu-activation path (not the empty registry
        // SDUI/CommandIntent use): the dispatcher returns `None` (Accepted —
        // the JS side effect runs in the package runtime, no wire message).
        let mut registry = CommandRegistry::new();
        registry.insert_test_command(crate::packages::commands::RegisteredCommand {
            command_id: "markdown.togglePreview".to_string(),
            display_name: "Toggle Markdown Preview".to_string(),
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
            key_bindings: Vec::new(),
            permissions: vec![crate::packages::permissions::PackagePermission::ParseDocument],
            custom_properties: BTreeMap::new(),
        });
        let response = execute_command_intent(
            CommandExecutionRequest {
                command_id: "markdown.togglePreview".to_string(),
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
                provenance: None,
                expected_permissions: Vec::new(),
            },
            workspace_state(),
            &document_state(),
            &sdui_state(),
            1,
            None,
            &registry,
        )
        .await;
        assert_eq!(response, None, "validated package commands accept silently");
    }

    /// Read until a `TabRegistry` snapshot arrives (skipping unrelated
    /// frames that can race the tab-command exchange).
    async fn receive_tab_registry_snapshot(
        connection: &mut TestConnection,
    ) -> crate::protocol::TabRegistrySnapshot {
        loop {
            match timeout(Duration::from_secs(2), connection.receive()).await {
                Ok(ServerMessage::TabRegistry(snapshot)) => return snapshot,
                Ok(_) => continue,
                Err(_) => panic!("timed out awaiting TabRegistry snapshot"),
            }
        }
    }

    /// A shared registry seeded with two tabs: tab 1 bound to client 99 (the
    /// test connection's identity) and tab 2 bound to a foreign client (7).
    fn two_tab_registry() -> (
        Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
        tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
    ) {
        let mut registry = crate::server::tab_registry::TabRegistry::new();
        registry.create_tab(99, 1, "/workspaces/alpha".to_string());
        registry.create_tab(7, 2, "/workspaces/beta".to_string());
        let registry = Arc::new(Mutex::new(registry));
        let (tab_registry_tx, _) = tokio::sync::broadcast::channel(16);
        (registry, tab_registry_tx)
    }

    /// Phase 22.7 (task 3): a rejected `Close` (foreign tab) pushes the
    /// reconciling snapshot and the sender's connection keeps serving.
    #[tokio::test]
    async fn rejected_close_keeps_connection_serving() {
        let root = temp_workspace("rejected-close");
        let mut workspace_state_value = WorkspaceState::new();
        workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let runtime_generation = runtime_generation();
        let parse_coordinator = parse_coordinator();
        let document_analysis =
            crate::server::document_analysis::DocumentAnalysisCoordinator::default();
        let (registry, tab_registry_tx) = two_tab_registry();
        let mut connection = TestConnection::connect_with_registry(
            99,
            document,
            Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            Arc::clone(&workspace),
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence_coordinator(),
            Arc::clone(&registry),
            tab_registry_tx,
        )
        .await;

        // A (client 99) tries to close B's tab (tab 2): rejected.
        connection
            .send(&ClientMessage::TabCommand {
                client_id: 99,
                command: crate::protocol::TabCommand::Close { tab_id: 2 },
            })
            .await;
        let snapshot = receive_tab_registry_snapshot(&mut connection).await;
        // Registry unchanged: both tabs still bound, tab 2 still owned by 7.
        assert_eq!(snapshot.tabs.len(), 2);
        assert!(
            snapshot
                .tabs
                .iter()
                .any(|entry| entry.tab_id == 2 && entry.client_id == 7)
        );
        assert!(
            snapshot
                .tabs
                .iter()
                .any(|entry| entry.tab_id == 1 && entry.client_id == 99)
        );

        // A's next command still processes: activate its own tab succeeds and
        // pushes another snapshot (the connection never ended).
        connection
            .send(&ClientMessage::TabCommand {
                client_id: 99,
                command: crate::protocol::TabCommand::Activate { tab_id: 1 },
            })
            .await;
        let snapshot = receive_tab_registry_snapshot(&mut connection).await;
        assert_eq!(snapshot.active, Some(1));

        assert!(registry.lock().await.snapshot().tabs.len() == 2);
        connection.close().await;
    }

    /// Phase 22.7 (task 3): an accepted `Close` (own tab) still ends the
    /// connection (EOF on the client stream) and removes the tab.
    #[tokio::test]
    async fn accepted_close_still_ends_connection() {
        let root = temp_workspace("accepted-close");
        let mut workspace_state_value = WorkspaceState::new();
        workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let runtime_generation = runtime_generation();
        let parse_coordinator = parse_coordinator();
        let document_analysis =
            crate::server::document_analysis::DocumentAnalysisCoordinator::default();
        let (registry, tab_registry_tx) = two_tab_registry();
        let mut connection = TestConnection::connect_with_registry(
            99,
            document,
            Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            Arc::clone(&workspace),
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence_coordinator(),
            Arc::clone(&registry),
            tab_registry_tx,
        )
        .await;

        // A closes its own tab (tab 1): accepted, the connection ends.
        connection
            .send(&ClientMessage::TabCommand {
                client_id: 99,
                command: crate::protocol::TabCommand::Close { tab_id: 1 },
            })
            .await;
        // The server task resolves and the client stream reaches EOF.
        timeout(
            Duration::from_secs(2),
            connection.codec.read_server_message(&mut connection.client),
        )
        .await
        .expect("EOF expected within the timeout")
        .expect_err("accepted close must end the connection (EOF)");
        // The tab is gone from the shared registry; the foreign tab remains.
        let snapshot = registry.lock().await.snapshot();
        assert_eq!(snapshot.tabs.len(), 1);
        assert!(
            snapshot
                .tabs
                .iter()
                .any(|entry| entry.tab_id == 2 && entry.client_id == 7)
        );
    }

    /// Plan 060 T4 (P0-2): one pre-dispatch boundary rejects every legacy
    /// message whose `client_id` does not match the handshake-assigned
    /// connection identity. Table covers every post-Hello family.
    #[tokio::test]
    async fn forged_client_identity_is_rejected_for_every_message_family() {
        let root = temp_workspace("forged-identity");
        fs::write(root.join("note.md"), "# secret\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let runtime_generation = runtime_generation();
        let parse_coordinator = parse_coordinator();
        let document_analysis =
            crate::server::document_analysis::DocumentAnalysisCoordinator::default();
        let mut connection = TestConnection::connect(
            99,
            document,
            Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            Arc::clone(&workspace),
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence_coordinator(),
        )
        .await;

        let forged: Vec<(&str, ClientMessage)> = vec![
            (
                "Edit",
                ClientMessage::Edit {
                    document_id: 7,
                    client_id: 1,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 1,
                    transaction_id: 1,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "x".to_string(),
                    },
                },
            ),
            (
                "EditorIntent",
                ClientMessage::EditorIntent {
                    document_id: 7,
                    client_id: 1,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 1,
                    transaction_id: 2,
                    intent: crate::protocol::EditorIntent::InsertText {
                        byte_offset: 0,
                        text: "x".to_string(),
                    },
                },
            ),
            (
                "RequestResync",
                ClientMessage::RequestResync {
                    document_id: 7,
                    client_id: 1,
                    known_version: 0,
                },
            ),
            (
                "DecorationViewportRequest",
                ClientMessage::DecorationViewportRequest {
                    client_id: 1,
                    document_id: 7,
                    document_version: 1,
                    byte_start: 0,
                    byte_end: 1,
                },
            ),
            (
                "OpenDocument",
                ClientMessage::OpenDocument {
                    client_id: 1,
                    workspace_root_id: root_id,
                    path: "note.md".to_string(),
                },
            ),
            (
                "OpenSelectedFile",
                ClientMessage::OpenSelectedFile {
                    client_id: 1,
                    capability: "forged".to_string(),
                    selected_path: root.join("note.md").to_string_lossy().into_owned(),
                },
            ),
            (
                "AddSelectedWorkspaceRoot",
                ClientMessage::AddSelectedWorkspaceRoot {
                    client_id: 1,
                    capability: "forged".to_string(),
                    selected_path: root.to_string_lossy().into_owned(),
                },
            ),
            (
                "SaveDocument",
                ClientMessage::SaveDocument {
                    client_id: 1,
                    document_id: 7,
                    known_version: 1,
                },
            ),
            (
                "ReloadDocument",
                ClientMessage::ReloadDocument {
                    client_id: 1,
                    document_id: 7,
                    known_version: 1,
                    force: true,
                },
            ),
            (
                "GetDocumentStatus",
                ClientMessage::GetDocumentStatus {
                    client_id: 1,
                    document_id: 7,
                },
            ),
            (
                "ListDocuments",
                ClientMessage::ListDocuments { client_id: 1 },
            ),
            (
                "SduiAction",
                ClientMessage::SduiAction {
                    client_id: 1,
                    ui_version: 1,
                    intent: SduiActionIntent::command(
                        "controlCenter.open",
                        SduiActionSource::Button {
                            node_id: SduiNodeId(1),
                        },
                    ),
                },
            ),
            (
                "CommandIntent",
                ClientMessage::CommandIntent {
                    client_id: 1,
                    document_id: 7,
                    behavior_version: 1,
                    command_id: "controlCenter.open".to_string(),
                },
            ),
            (
                "CompletionRequest",
                ClientMessage::CompletionRequest {
                    request: crate::protocol::CompletionRequest {
                        request_id: 1,
                        client_id: 1,
                        document_id: 7,
                        document_version: 1,
                        behavior_version: 1,
                        cursor_byte_offset: 0,
                        replacement_range: crate::protocol::CompletionReplacementRange::new(0, 0),
                        trigger: crate::protocol::CompletionTrigger::Manual,
                        provider_generation: 1,
                        recent_completions: Vec::<String>::new().into_boxed_slice(),
                    },
                },
            ),
            (
                "LanguageIntelligenceRequest",
                ClientMessage::LanguageIntelligenceRequest {
                    request: crate::protocol::LanguageIntelligenceRequest {
                        request_id: 1,
                        client_id: 1,
                        document_id: 7,
                        document_version: 1,
                        behavior_version: 1,
                        cursor_byte_offset: 0,
                        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
                        provider_generation: 1,
                    },
                },
            ),
            (
                "RuntimeGenerationInstalled",
                ClientMessage::RuntimeGenerationInstalled {
                    client_id: 1,
                    runtime_generation_id: 1,
                },
            ),
        ];

        for (family, message) in forged {
            connection.send(&message).await;
            let response = connection.receive().await;
            assert!(
                matches!(
                    response,
                    ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        ..
                    }
                ),
                "forged {family} must be rejected at the identity boundary, got {response:?}"
            );
        }

        // The forged OpenDocument must have had no effect: the connection's own
        // document list stays empty, and the connection survives rejections.
        connection
            .send(&ClientMessage::ListDocuments { client_id: 99 })
            .await;
        let response = connection.receive().await;
        assert!(
            matches!(response, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
            "forged open must not register documents, got {response:?}"
        );

        connection.close().await;
        let _ = fs::remove_dir_all(root);
    }

    /// Plan 060 T4 (P0-3): two connections share the server coordinators; a
    /// parse update for a document opened by one connection never reaches the
    /// other connection's stream.
    #[tokio::test]
    async fn two_client_parse_updates_are_isolated_to_the_subscribed_connection() {
        let root = temp_workspace("parse-isolation");
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let runtime_generation = runtime_generation();
        let parse_coordinator = parse_coordinator();
        let document_analysis =
            crate::server::document_analysis::DocumentAnalysisCoordinator::default();

        let mut connection_a = TestConnection::connect(
            99,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation.clone(),
            parse_coordinator.clone(),
            document_analysis.clone(),
            language_intelligence_coordinator(),
        )
        .await;
        let mut connection_b = TestConnection::connect(
            100,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence_coordinator(),
        )
        .await;

        // A opens and edits the document; only A may see its parse output.
        connection_a
            .send(&ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            })
            .await;
        let behavior_version = loop {
            match connection_a.receive().await {
                ServerMessage::BehaviorManifest(manifest) => break manifest.behavior_version,
                ServerMessage::DocumentOpened { .. }
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. } => {}
                other => panic!("unexpected message during open: {other:?}"),
            }
        };
        connection_a.drain_until_quiet().await;
        connection_a
            .send(&ClientMessage::Edit {
                document_id: 1,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version,
                transaction_id: 900,
                operation: EditOperation::Insert {
                    byte_offset: 13,
                    text: "// owned by A\n".to_string(),
                },
            })
            .await;
        loop {
            match connection_a.receive().await {
                ServerMessage::DecorationSet(set)
                    if set.document_id == 1 && set.document_version == 2 =>
                {
                    break;
                }
                ServerMessage::EditAck { .. }
                | ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_) => {}
                other => panic!("unexpected message awaiting decorations: {other:?}"),
            }
        }

        // B never subscribed to document 1: its stream stays silent.
        let leaked = timeout(
            Duration::from_millis(150),
            connection_b
                .codec
                .read_server_message(&mut connection_b.client),
        )
        .await;
        assert!(
            leaked.is_err(),
            "unsubscribed connection must receive no parse output, got {leaked:?}"
        );

        connection_a.close().await;
        connection_b.close().await;
        let _ = fs::remove_dir_all(root);
    }

    /// Plan 060 T4 (P0-2): save requires the editable lease and validates
    /// `known_version`; status and list fail closed for documents the
    /// connection never opened.
    #[tokio::test]
    async fn save_reload_status_list_enforce_connection_owned_access() {
        let root = temp_workspace("save-access");
        fs::write(root.join("note.md"), "hello\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let runtime_generation = runtime_generation();
        let parse_coordinator = parse_coordinator();
        let document_analysis =
            crate::server::document_analysis::DocumentAnalysisCoordinator::default();

        let mut connection_a = TestConnection::connect(
            99,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation.clone(),
            parse_coordinator.clone(),
            document_analysis.clone(),
            language_intelligence_coordinator(),
        )
        .await;
        let mut connection_b = TestConnection::connect(
            100,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence_coordinator(),
        )
        .await;

        // A opens the document (editable lease).
        connection_a
            .send(&ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "note.md".to_string(),
            })
            .await;
        loop {
            match connection_a.receive().await {
                ServerMessage::DocumentOpened { .. } => break,
                ServerMessage::BehaviorManifest(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. } => {}
                other => panic!("unexpected message during A open: {other:?}"),
            }
        }
        connection_a.drain_until_quiet().await;

        // B has never opened document 1: resync, status, and list fail closed.
        connection_b
            .send(&ClientMessage::RequestResync {
                document_id: 1,
                client_id: 100,
                known_version: 0,
            })
            .await;
        let resync = connection_b.receive_response().await;
        assert!(
            matches!(
                resync,
                ServerMessage::FileOperationFailed {
                    code: FileErrorCode::UnknownDocument,
                    ..
                }
            ),
            "resync for an unopened document must not leak text, got {resync:?}"
        );
        connection_b
            .send(&ClientMessage::GetDocumentStatus {
                client_id: 100,
                document_id: 1,
            })
            .await;
        let status = connection_b.receive_response().await;
        assert!(
            matches!(
                status,
                ServerMessage::FileOperationFailed {
                    code: FileErrorCode::UnknownDocument,
                    ..
                }
            ),
            "status for an unopened document must fail closed, got {status:?}"
        );
        connection_b
            .send(&ClientMessage::ListDocuments { client_id: 100 })
            .await;
        let list = connection_b.receive_response().await;
        assert!(
            matches!(list, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
            "list must not leak another connection's documents, got {list:?}"
        );

        // B opens the same document: read-only access. Save fails closed.
        connection_b
            .send(&ClientMessage::OpenDocument {
                client_id: 100,
                workspace_root_id: root_id,
                path: "note.md".to_string(),
            })
            .await;
        loop {
            match connection_b.receive().await {
                ServerMessage::DocumentOpened { metadata, .. } => {
                    assert_eq!(metadata.access, DocumentAccess::ReadOnly);
                    break;
                }
                ServerMessage::BehaviorManifest(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. } => {}
                other => panic!("unexpected message during B open: {other:?}"),
            }
        }
        connection_b.drain_until_quiet().await;
        connection_b
            .send(&ClientMessage::SaveDocument {
                client_id: 100,
                document_id: 1,
                known_version: 1,
            })
            .await;
        let read_only_save = connection_b.receive_response().await;
        assert!(
            matches!(
                read_only_save,
                ServerMessage::FileOperationFailed {
                    code: FileErrorCode::AccessDenied,
                    ..
                }
            ),
            "read-only save must fail closed, got {read_only_save:?}"
        );

        // A saves with a future version claim: stale check fails closed.
        connection_a
            .send(&ClientMessage::SaveDocument {
                client_id: 99,
                document_id: 1,
                known_version: 99,
            })
            .await;
        let stale_save = connection_a.receive_response().await;
        assert!(
            matches!(
                stale_save,
                ServerMessage::FileOperationFailed {
                    code: FileErrorCode::StaleFileMetadata,
                    ..
                }
            ),
            "future-version save must fail closed, got {stale_save:?}"
        );

        // A saves at the current version: succeeds and clears dirty state.
        connection_a
            .send(&ClientMessage::SaveDocument {
                client_id: 99,
                document_id: 1,
                known_version: 1,
            })
            .await;
        let saved = connection_a.receive_response().await;
        assert!(
            matches!(
                saved,
                ServerMessage::DocumentSaved {
                    document_id: 1,
                    version: 1,
                    dirty: false,
                }
            ),
            "lease-holder save at the current version must succeed, got {saved:?}"
        );

        connection_a.close().await;
        connection_b.close().await;
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn stale_client_is_rejected_after_native_decoration_semantics_change() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document_state(),
            Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: 2,
                    client_name: "stale-client".to_string(),
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::Error {
                code: ProtocolErrorCode::UnsupportedProtocolVersion,
                message: "unsupported protocol version".to_string(),
            }
        );
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn live_typography_update_reaches_connection_once() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let runtime_generation = runtime_generation();
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document_state(),
            Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation.clone(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        loop {
            if matches!(
                codec.read_server_message(&mut client).await.unwrap(),
                ServerMessage::FileOpenCapabilityIssued { .. }
            ) {
                break;
            }
        }

        let mut typography = crate::protocol::ActiveTypography::default();
        typography.monospace.size = 16.0;
        runtime_generation
            .replace_typography(typography)
            .await
            .unwrap();

        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::ActiveTypography(typography)
                if typography.revision == 1 && typography.monospace.size == 16.0
        ));
        assert!(
            timeout(
                Duration::from_millis(20),
                codec.read_server_message(&mut client),
            )
            .await
            .is_err(),
            "one replacement emits one live update"
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_sends_minimal_behavior_manifest() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();

        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(1)))
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_does_not_send_default_workspace_sdui_snapshot_after_bootstrap() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "Hello from server".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            empty_sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();

        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        // Post-handshake file-open capability is always issued once.
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));
        let next = timeout(
            Duration::from_millis(25),
            codec.read_server_message(&mut client),
        )
        .await;
        assert!(next.is_err(), "unexpected default SDUI message: {next:?}");

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn client_receives_js_generated_sdui_snapshot() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            1,
            "Hello from runtime SDUI".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let sdui = sdui_state();
        {
            let runtime_tree = crate::protocol::SduiTree {
                ui_version: 1,
                root_id: crate::protocol::SduiNodeId(1),
                nodes: vec![
                    crate::protocol::SduiNode::new(
                        crate::protocol::SduiNodeId(1),
                        SduiNodeKind::Flex {
                            direction: crate::protocol::SduiFlexDirection::Row,
                            children: vec![
                                crate::protocol::SduiNodeId(2),
                                crate::protocol::SduiNodeId(3),
                            ],
                        },
                    ),
                    crate::protocol::SduiNode::new(
                        crate::protocol::SduiNodeId(2),
                        SduiNodeKind::Panel {
                            title: "Runtime".to_string(),
                            children: Vec::new(),
                        },
                    ),
                    crate::protocol::SduiNode::new(
                        crate::protocol::SduiNodeId(3),
                        SduiNodeKind::EditorView {
                            binding: crate::protocol::SduiEditorBinding {
                                document_id: 1,
                                expected_version: Some(1),
                            },
                        },
                    ),
                ],
            };
            sdui.lock()
                .await
                .replace_with_runtime_tree(runtime_tree)
                .unwrap();
        }
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            Arc::clone(&sdui),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();

        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => {
                assert!(tree.nodes.iter().any(|node| matches!(
                    &node.kind,
                    SduiNodeKind::Panel { title, .. } if title == "Runtime"
                )));
            }
            message => panic!("expected runtime SduiSnapshot, got {message:?}"),
        }

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn control_center_lists_and_activates_loaded_package_commands() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            1,
            "Hello from package commands".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let sdui = sdui_state();
        let runtime = js_runtime();
        let coordinator = parse_coordinator();
        load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            Arc::clone(&behavior),
            workspace_state(),
            Arc::clone(&sdui),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation_from(runtime),
            coordinator,
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();

        // Handshake: a runtime with loaded packages ships extra frames, so
        // read until the file-open capability, capturing the markdown mode
        // layer manifest on the way.
        let mut markdown_manifest = None;
        loop {
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::BehaviorManifest(manifest) => {
                    if manifest.manifest_id == "markdown.markdown" {
                        markdown_manifest = Some(*manifest);
                    }
                }
                ServerMessage::FileOpenCapabilityIssued { .. } => break,
                _ => {}
            }
        }
        let markdown_manifest = markdown_manifest.expect("markdown mode layer must be published");
        // The default Control Center binding survives mode activation: the
        // layer carries the Global `Ctrl+X Ctrl+P` chord from the shared
        // default commands/keymaps.
        assert!(markdown_manifest.keymaps.iter().any(|rule| {
            rule.command_id == "controlCenter.open"
                && rule.context == KeyBindingContext::Global
                && rule.sequence.len() == 2
                && rule.sequence[0].key == KeyCode::Character("x".to_string())
                && rule.sequence[0].modifiers.control
                && rule.sequence[1].key == KeyCode::Character("p".to_string())
                && rule.sequence[1].modifiers.control
        }));

        let behavior_version = behavior.lock().await.version();
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::CommandIntent {
                    client_id: 99,
                    document_id: 1,
                    behavior_version,
                    command_id: "controlCenter.open".to_string(),
                },
            )
            .await
            .unwrap();
        let snapshot = loop {
            if let ServerMessage::TransientMenuSnapshot(snapshot) =
                codec.read_server_message(&mut client).await.unwrap()
            {
                break snapshot;
            }
        };
        let session_id = snapshot.session_id;
        let toggle_preview = snapshot
            .items
            .iter()
            .find(|item| item.id == "markdown.togglePreview")
            .expect("markdown.togglePreview must be listed");
        assert!(
            snapshot
                .items
                .iter()
                .any(|item| item.id == "markdown.toggleComment"),
            "markdown.toggleComment must be listed"
        );
        let detail = toggle_preview.detail.as_deref().unwrap_or_default();
        assert!(
            detail.contains("Ctrl+Shift+M"),
            "detail must carry the effective binding: {detail}"
        );
        assert!(
            detail.contains("@clay/markdown@0.1.0"),
            "detail must carry package provenance: {detail}"
        );

        // Query narrows to exactly the preview command.
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::MenuQueryUpdate {
                    client_id: 99,
                    session_id,
                    query: "togglePreview".to_string(),
                },
            )
            .await
            .unwrap();
        let filtered = loop {
            if let ServerMessage::TransientMenuSnapshot(snapshot) =
                codec.read_server_message(&mut client).await.unwrap()
            {
                break snapshot;
            }
        };
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].id, "markdown.togglePreview");

        // Activation closes the menu and validates through the live
        // aggregated registry; the JS side effect runs in the package
        // runtime, so no wire frame follows the close.
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::MenuActivate {
                    client_id: 99,
                    session_id,
                    kind: crate::protocol::TransientMenuActivationData::Primary,
                },
            )
            .await
            .unwrap();
        loop {
            if let message @ ServerMessage::TransientMenuClosed { .. } =
                codec.read_server_message(&mut client).await.unwrap()
            {
                assert_eq!(message, ServerMessage::TransientMenuClosed { session_id });
                break;
            }
        }
        assert!(
            timeout(
                Duration::from_millis(25),
                codec.read_server_message(&mut client)
            )
            .await
            .is_err(),
            "no frame after validated package activation"
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_sends_runtime_diagnostics_after_bootstrap() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let diagnostics = Arc::new(Mutex::new(RuntimeDiagnosticStore::default()));
        diagnostics.lock().await.push(RuntimeDiagnostic::error(
            "runtime.invalid_import",
            "Only clay:* facades and relative local configuration modules are allowed.",
        ));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            Arc::clone(&diagnostics),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();

        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
                "runtime.invalid_import",
                "Only clay:* facades and relative local configuration modules are allowed.",
            ))
        );
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        let live_diagnostic =
            RuntimeDiagnostic::warning("runtime.live_update", "configuration reload failed");
        diagnostics.lock().await.publish(live_diagnostic.clone());
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::RuntimeDiagnostic(live_diagnostic)
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_acknowledges_insert_edit() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 7,
                    client_id: 99,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 1,
                    transaction_id: 123,
                    operation: EditOperation::Insert {
                        byte_offset: 2,
                        text: " Clay".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 2,
                transaction_id: 123,
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_rejects_edit_with_stale_behavior_version_without_mutating_document() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            Arc::clone(&document),
            behavior,
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 7,
                    client_id: 99,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 0,
                    transaction_id: 123,
                    operation: EditOperation::Insert {
                        byte_offset: 2,
                        text: " stale".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 123,
                reason: EditRejection::InvalidBehaviorVersion {
                    behavior_version: 0,
                    server_behavior_version: 1,
                },
            }
        );

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 7,
                    client_id: 99,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 1,
                    transaction_id: 124,
                    operation: EditOperation::Insert {
                        byte_offset: 2,
                        text: " ok".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 2,
                transaction_id: 124,
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_sends_resync_snapshot_after_request() {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "server 🦀".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::RequestResync {
                    document_id: 7,
                    client_id: 99,
                    known_version: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::ResyncSnapshot {
                document_id: 7,
                version: 1,
                text: "server 🦀".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn connection_open_document_sends_snapshot_and_manifest_without_full_document_on_edit_ack()
     {
        let root = temp_workspace("open-dispatch");
        let file = root.join("main.rs");
        fs::write(&file, "fn main() {}\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));

        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            Arc::clone(&workspace),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenDocument {
                    client_id: 99,
                    workspace_root_id: root_id,
                    path: "main.rs".to_string(),
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 1,
                    version: 1,
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    dirty: false,
                    workspace_root_id: root_id,
                    path: "main.rs".to_string(),
                },
                text: "fn main() {}\n".to_string(),
            }
        );
        let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => {
                assert_eq!(manifest.manifest_id, "rust.rust");
                assert_eq!(
                    manifest.scope,
                    crate::protocol::BehaviorScope::Document { document_id: 1 }
                );
                assert_eq!(manifest.editor_rules.tab.spaces_per_tab, 4);
                assert_eq!(
                    manifest
                        .editor_rules
                        .autocomplete_triggers
                        .iter()
                        .map(|trigger| trigger.trigger.as_str())
                        .collect::<Vec<_>>(),
                    vec![".", ":"]
                );
                manifest.behavior_version
            }
            other => panic!("expected Rust behavior manifest after open, got {other:?}"),
        };

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 1,
                    client_id: 99,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version,
                    transaction_id: 444,
                    operation: EditOperation::Insert {
                        byte_offset: 13,
                        text: "// ok\n".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        loop {
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::EditAck {
                    document_id: 1,
                    confirmed_version: 2,
                    transaction_id: 444,
                } => break,
                ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::BehaviorManifest(_) => {}
                other => panic!("expected edit acknowledgement, got {other:?}"),
            }
        }

        loop {
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::DecorationSet(set)
                    if set.document_id == 1 && set.document_version == 2 =>
                {
                    assert!(!set.spans.is_empty());
                    break;
                }
                ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::BehaviorManifest(_) => {}
                other => panic!("expected refreshed syntax decorations, got {other:?}"),
            }
        }

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::GetDocumentStatus {
                    client_id: 99,
                    document_id: 1,
                },
            )
            .await
            .unwrap();
        loop {
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::DocumentStatus {
                    metadata:
                        DocumentMetadata {
                            document_id: 1,
                            version: 2,
                            access: DocumentAccess::Editable { lease_id: 1 },
                            lease_id: Some(1),
                            dirty: true,
                            workspace_root_id: status_root_id,
                            path,
                        },
                } => {
                    assert_eq!(status_root_id, root_id);
                    assert_eq!(path, "main.rs");
                    break;
                }
                ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::BehaviorManifest(_) => {}
                other => panic!("expected document status, got {other:?}"),
            }
        }

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn file_browser_open_uses_generic_open_document_followups() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let root = temp_workspace("file-browser-open-followups");
        let selected = root.join("note.md");
        fs::write(&selected, "# Browser note\n\n- item\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        workspace_state_value.reserve_document_ids_from(2);
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));

        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let sdui = sdui_state();
        let runtime = js_runtime();
        let coordinator = parse_coordinator();

        let (client, server) = duplex(16 * 1024 * 1024);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            Arc::clone(&sdui),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation_from(runtime),
            coordinator,
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let tree = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => tree,
            message => panic!("expected file browser SduiSnapshot, got {message:?}"),
        };
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();
        let action = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                SduiNodeKind::List { items } => items
                    .iter()
                    .find(|item| item.label == "note.md")
                    .and_then(|item| item.action.clone()),
                _ => None,
            })
            .expect("note.md file-browser action");

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::SduiAction {
                    client_id: 99,
                    ui_version: tree.ui_version,
                    intent: action,
                },
            )
            .await
            .unwrap();

        let (opened_version, opened_lease_id) =
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::DocumentOpened { metadata, text } => {
                    assert_eq!(metadata.document_id, 2);
                    assert_eq!(metadata.workspace_root_id, root_id);
                    assert_eq!(metadata.path, "note.md");
                    assert_eq!(text, "# Browser note\n\n- item\n");
                    let DocumentAccess::Editable { lease_id } = metadata.access else {
                        panic!("file-browser opener must receive editable access");
                    };
                    (metadata.version, lease_id)
                }
                message => panic!("expected file-browser DocumentOpened, got {message:?}"),
            };
        let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => {
                assert_eq!(manifest.manifest_id, "markdown.markdown");
                assert!(matches!(
                    manifest.scope,
                    BehaviorScope::Document { document_id: 2 }
                ));
                manifest.behavior_version
            }
            message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
        };

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 2,
                    client_id: 99,
                    lease_id: Some(opened_lease_id),
                    base_version: opened_version,
                    behavior_version,
                    transaction_id: 7,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "!".to_string(),
                    },
                },
            )
            .await
            .unwrap();
        loop {
            match timeout(
                Duration::from_secs(1),
                codec.read_server_message(&mut client),
            )
            .await
            .expect("opened-file edit acknowledgement timed out")
            .unwrap()
            {
                ServerMessage::EditAck {
                    document_id: 2,
                    confirmed_version,
                    transaction_id: 7,
                } => {
                    assert_eq!(confirmed_version, opened_version + 1);
                    break;
                }
                ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::BehaviorManifest(_) => {}
                message => panic!("expected opened-file EditAck, got {message:?}"),
            }
        }

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(selected);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn multi_chunk_parse_update_ships_as_single_decoration_batch() {
        let root = temp_workspace("decoration-batch");
        let file = root.join("main.rs");
        // Well past one 128-byte authority chunk.
        let source = "fn main() { let value = 1; }\n".repeat(16);
        fs::write(&file, &source).unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));

        let (client, server) = duplex(64 * 1024);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            Arc::clone(&workspace),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        for _ in 0..8 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenDocument {
                    client_id: 99,
                    workspace_root_id: root_id,
                    path: "main.rs".to_string(),
                },
            )
            .await
            .unwrap();
        // DocumentOpened, BehaviorManifest, replenished capability.
        let _opened = codec.read_server_message(&mut client).await.unwrap();
        let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
            message => panic!("expected behavior manifest, got {message:?}"),
        };
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        // Register/cache the native handler with one edit, then request the
        // whole visible region so this test isolates multi-chunk wire batching
        // from the edit's expected one-chunk incremental update.
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 1,
                    client_id: 99,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version,
                    transaction_id: 555,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "// batch\n".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        let mut confirmed_version = None;
        let mut edit_update_seen = false;
        let mut viewport_requested = false;
        let mut single_set_frames = 0usize;
        let batch = loop {
            let message = timeout(
                Duration::from_secs(2),
                codec.read_server_message(&mut client),
            )
            .await
            .expect("decoration batch timed out")
            .unwrap();
            match message {
                ServerMessage::DecorationBatch(chunks)
                    if viewport_requested && chunks[0].document_version == 2 =>
                {
                    break chunks;
                }
                ServerMessage::DecorationBatch(chunks)
                    if !viewport_requested && chunks[0].document_version == 2 =>
                {
                    edit_update_seen = true;
                }
                ServerMessage::EditAck {
                    confirmed_version: version,
                    ..
                } => confirmed_version = Some(version),
                ServerMessage::DecorationSet(set)
                    if set.document_id == 1 && set.document_version == 2 =>
                {
                    if viewport_requested {
                        single_set_frames += 1;
                    } else {
                        edit_update_seen = true;
                    }
                }
                ServerMessage::DecorationSet(_)
                | ServerMessage::DecorationBatch(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_) => {}
                message => panic!("expected decoration batch, got {message:?}"),
            }
            if !viewport_requested
                && edit_update_seen
                && let Some(document_version) = confirmed_version
            {
                codec
                    .write_client_message(
                        &mut client,
                        &ClientMessage::DecorationViewportRequest {
                            client_id: 99,
                            document_id: 1,
                            document_version,
                            byte_start: 0,
                            byte_end: (source.len() + "// batch\n".len()) as u64,
                        },
                    )
                    .await
                    .unwrap();
                viewport_requested = true;
            }
        };

        assert!(
            batch.len() > 1,
            "multi-chunk window must batch, got {} chunks",
            batch.len()
        );
        assert!(batch.iter().all(|set| set.document_id == 1));
        assert!(
            batch
                .windows(2)
                .all(|pair| pair[0].viewport_byte_start <= pair[1].viewport_byte_start),
            "batch chunks arrive in viewport-key order"
        );
        assert!(batch.iter().all(|set| !set.spans.is_empty()));
        assert_eq!(
            single_set_frames, 0,
            "batched parse update must not fan out per-chunk frames"
        );

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn selected_markdown_file_publishes_manifest_and_decorations() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let root = temp_workspace("selected-markdown-runtime");
        let selected = root.join("note.md");
        fs::write(
            &selected,
            "# Opened note\n\n- item with `code`\n\n**strong** and *emphasis*\n",
        )
        .unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        workspace_state_value.reserve_document_ids_from(2);
        let workspace = Arc::new(Mutex::new(workspace_state_value));

        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let sdui = sdui_state();
        let runtime = js_runtime();
        let coordinator = parse_coordinator();

        let (client, server) = duplex(16 * 1024 * 1024);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            Arc::clone(&sdui),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation_from(runtime),
            coordinator,
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { token } => token,
            message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
        };

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenSelectedFile {
                    client_id: 99,
                    capability: capability_token,
                    selected_path: selected.to_string_lossy().into_owned(),
                },
            )
            .await
            .unwrap();

        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DocumentOpened { metadata, text } => {
                assert_eq!(metadata.document_id, 2);
                assert_eq!(metadata.path, "note.md");
                assert_eq!(
                    text,
                    "# Opened note\n\n- item with `code`\n\n**strong** and *emphasis*\n"
                );
            }
            message => panic!("expected selected Markdown DocumentOpened, got {message:?}"),
        }
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => {
                assert_eq!(manifest.manifest_id, "markdown.markdown");
                assert!(matches!(
                    manifest.scope,
                    BehaviorScope::Document { document_id: 2 }
                ));
                assert!(
                    manifest
                        .commands
                        .iter()
                        .any(|command| { command.command_id == "markdown.togglePreview" })
                );
            }
            message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
        }
        // Server re-issues one pending capability after the open attempt; parse
        // decorations are scheduled in the background instead of blocking open.
        // Phase 22.2: the follow-up also carries the connection-wide manifest
        // after the document's mode layer; consume it before the capability.
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(_) => {}
            message => panic!("expected trailing global manifest, got {message:?}"),
        }
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));

        // Selected-file activation publishes behavior only on the open path;
        // optional package UI panels stay opt-in, and highlights arrive later
        // through the parse coordinator rather than before the replenished
        // capability.

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(selected);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn default_init_js_load_package_powers_selected_markdown_open() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let config_root = temp_workspace("default-init-loadpackage");
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#,
        )
        .unwrap();
        let runtime = js_runtime();
        let coordinator = parse_coordinator();
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let sdui = empty_sdui_state();
        let evaluation = runtime
            .load_configuration_from_root(config_root.clone())
            .await
            .expect("default init.js loadPackage should evaluate");
        runtime
            .register_parse_handlers(&coordinator, 1, &evaluation)
            .expect("init.js loadPackage should register parse handler");
        super::super::apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;
        assert_eq!(runtime.evaluation_count(), 1);

        let metadata = DocumentMetadata {
            document_id: 2,
            version: 1,
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            dirty: false,
            workspace_root_id: 1,
            path: "note.md".to_string(),
        };
        let messages = super::open_document_followup_messages(
            &metadata,
            "# Loaded from init.js\n",
            &behavior,
            &sdui,
            1,
            &runtime,
            &coordinator,
        )
        .await;

        assert_eq!(
            runtime.evaluation_count(),
            2,
            "open should classify/activate on the persistent runtime without a fresh per-open runtime"
        );
        assert!(messages.iter().any(|message| matches!(
            message,
            ServerMessage::BehaviorManifest(manifest)
                if manifest.manifest_id == "markdown.markdown"
                    && matches!(manifest.scope, BehaviorScope::Document { document_id: 2 })
        )));
        assert!(messages.iter().all(|message| {
            !matches!(message, ServerMessage::DecorationSet(set) if set.document_id == 2)
        }));
        let update = timeout(Duration::from_secs(1), coordinator.next_update())
            .await
            .unwrap()
            .unwrap();
        let set = update
            .decoration_updates
            .into_iter()
            .next()
            .expect("background markdown decorations");
        assert_eq!(set.document_id, 2);
        assert!(
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::Heading1)
        );
        assert!(
            set.spans
                .iter()
                .all(|span| span.provenance.package_version == "builtin"),
            "open Markdown decorations must come from compiled Tier 1 grammar, not parser.js"
        );
        let _ = fs::remove_file(config_root.join("init.js"));
        let _ = fs::remove_dir(config_root);
    }

    #[tokio::test]
    async fn native_windows_schedule_once_for_each_first_party_language() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let config_root = temp_workspace("viewport-native-decoration");
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/rust");
            await loadPackage("@clay/typescript");
            await loadPackage("@clay/javascript");
            await loadPackage("@clay/markdown");
            "#,
        )
        .unwrap();
        let runtime = js_runtime();
        let coordinator = parse_coordinator();
        let evaluation = runtime
            .load_configuration_from_root(config_root.clone())
            .await
            .expect("language configuration evaluates");

        for (document_id, path, package_prefix, start_marker, text) in [
            (
                19,
                "main.rs",
                "rust",
                "fn value150",
                (0..300)
                    .map(|line| format!("fn value{line}() -> usize {{ {line} }}\n"))
                    .collect::<String>(),
            ),
            (
                20,
                "main.ts",
                "typescript",
                "const value150",
                (0..300)
                    .map(|line| format!("const value{line}: number = {line};\n"))
                    .collect::<String>(),
            ),
            (
                21,
                "main.js",
                "javascript",
                "const value150",
                (0..300)
                    .map(|line| format!("const value{line} = {line};\n"))
                    .collect::<String>(),
            ),
            (
                22,
                "notes.md",
                "markdown",
                "LAST CODE LINE",
                format!(
                    "```text\n{}LAST CODE LINE\n```\n\nPlain prose after fence.\n",
                    "code inside fence\n".repeat(300)
                ),
            ),
        ] {
            let metadata = DocumentMetadata {
                document_id,
                version: 1,
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
                dirty: false,
                workspace_root_id: 1,
                path: path.to_string(),
            };
            let (meta, policy) = runtime
                .register_native_syntax_handler(
                    &coordinator,
                    1,
                    &evaluation,
                    path,
                    package_prefix,
                    package_prefix,
                )
                .expect("native handler registration succeeds")
                .expect("native handler selected");
            assert_eq!(
                runtime.registered_native_syntax_handler(1, path),
                Some((meta.clone(), policy))
            );
            super::schedule_parse_window(
                &coordinator,
                &metadata,
                &text,
                1,
                &meta.package_prefix,
                &meta.mode_id,
                policy,
                ParseByteRange::new(0, text.len() as u64),
            )
            .expect("opening viewport schedules");
            let opening_end = text
                .len()
                .min(policy.max_window_bytes as usize)
                .min(crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
                as u64;
            let update = tokio::select! {
                update = coordinator.next_update() => update.expect("opening native update"),
                diagnostic = coordinator.next_diagnostic() => {
                    panic!("opening viewport parse failed: {:?}", diagnostic)
                }
            };
            assert_eq!(
                (update.viewport.start, update.viewport.end),
                (0, opening_end),
                "{path}"
            );
            assert!(!update.decoration_updates.is_empty(), "{path}");
            assert!(
                update
                    .decoration_updates
                    .iter()
                    .any(|set| !set.spans.is_empty()),
                "{path}"
            );

            let start = text.find(start_marker).expect("middle line marker") as u64;
            super::schedule_parse_window(
                &coordinator,
                &metadata,
                &text,
                1,
                &meta.package_prefix,
                &meta.mode_id,
                policy,
                ParseByteRange::new(start, text.len() as u64),
            )
            .expect("nonzero viewport schedules");
            let update = tokio::select! {
                update = coordinator.next_update() => update.expect("nonzero native update"),
                diagnostic = coordinator.next_diagnostic() => {
                    panic!("nonzero viewport parse failed: {:?}", diagnostic)
                }
            };
            let requested_end = (start
                + crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES as u64)
                .min(start + policy.max_window_bytes)
                .min(text.len() as u64);
            assert!(update.viewport.start <= start, "{path}");
            assert!(update.viewport.end >= requested_end, "{path}");
            assert!(!update.decoration_updates.is_empty(), "{path}");
            assert!(
                update.decoration_updates.iter().all(|set| set
                    .spans
                    .iter()
                    .all(|span| span.byte_start >= set.viewport_byte_start)),
                "{path}"
            );
            if path == "notes.md" {
                let prose = text.find("Plain prose after fence.").unwrap() as u64;
                assert!(
                    update
                        .decoration_updates
                        .iter()
                        .any(|set| set
                            .spans
                            .iter()
                            .any(|span| span.token_type == TokenType::Paragraph
                                && span.byte_start <= prose
                                && span.byte_end > prose))
                );
                assert!(
                    !update
                        .decoration_updates
                        .iter()
                        .any(|set| set
                            .spans
                            .iter()
                            .any(|span| span.token_type == TokenType::CodeBlock
                                && span.byte_start <= prose
                                && span.byte_end > prose))
                );
            }
        }

        let _ = fs::remove_file(config_root.join("init.js"));
        let _ = fs::remove_dir(config_root);
    }

    #[test]
    fn connection_has_no_markdown_specific_open_runtime_branch() {
        let source = include_str!("mod.rs");
        for (left, right) in [
            ("evaluate_", "markdown_open"),
            ("create_", "markdown_open_runtime_root"),
            ("unique_", "markdown_open_runtime_root"),
            ("markdown_", "open_init_source"),
            ("is_", "markdown_path"),
        ] {
            let removed = format!("{left}{right}");
            assert!(
                !source.contains(&removed),
                "connection.rs must not contain removed mode-specific helper `{removed}`"
            );
        }
    }

    #[tokio::test]
    async fn open_document_renders_before_background_parse_completes() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let mut text = "# Top\n\n".to_string();
        text.push_str(&"a".repeat(80 * 1024));
        text.push_str("\n# Outside initial window\n");
        let metadata = DocumentMetadata {
            document_id: 2,
            version: 1,
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            dirty: false,
            workspace_root_id: 1,
            path: "large.md".to_string(),
        };
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let sdui = empty_sdui_state();
        let runtime = js_runtime();
        let coordinator = parse_coordinator();
        load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
        let activation = super::classify_open_document(
            1,
            &runtime,
            &coordinator,
            &metadata,
            &text,
            &behavior,
            &sdui,
        )
        .await
        .expect("loaded package should classify markdown path");

        let immediate =
            super::schedule_open_parse(&coordinator, &metadata, &text, &behavior, &activation)
                .await
                .expect("open parse should schedule");
        assert!(
            immediate.is_none(),
            "open follow-up must not wait for parse output"
        );

        let native_window = crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES;
        let update = timeout(Duration::from_secs(1), coordinator.next_update())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(update.document_id, 2);
        assert_eq!(
            (update.viewport.start, update.viewport.end),
            (0, native_window as u64)
        );
        assert!(update.decoration_updates.iter().any(|set| {
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::Heading1)
        }));
    }

    #[tokio::test]
    async fn connection_open_selected_file_sends_snapshot_and_single_file_grant() {
        let root = temp_workspace("selected-dispatch");
        let selected = root.join("note.md");
        let sibling = root.join("sibling.md");
        fs::write(&selected, "# selected\n").unwrap();
        fs::write(&sibling, "# sibling\n").unwrap();
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            Arc::clone(&workspace),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { token } => token,
            message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
        };

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenSelectedFile {
                    client_id: 99,
                    capability: capability_token,
                    selected_path: selected.to_string_lossy().into_owned(),
                },
            )
            .await
            .unwrap();

        let selected_root_id = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DocumentOpened { metadata, text } => {
                assert_eq!(metadata.document_id, 1);
                assert_eq!(metadata.version, 1);
                assert_eq!(metadata.access, DocumentAccess::Editable { lease_id: 1 });
                assert_eq!(metadata.path, "note.md");
                assert_eq!(text, "# selected\n");
                metadata.workspace_root_id
            }
            message => panic!("expected selected DocumentOpened, got {message:?}"),
        };
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::BehaviorManifest(_)
        ));
        loop {
            if matches!(
                codec.read_server_message(&mut client).await.unwrap(),
                ServerMessage::FileOpenCapabilityIssued { .. }
            ) {
                break;
            }
        }

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenDocument {
                    client_id: 99,
                    workspace_root_id: selected_root_id,
                    path: sibling.to_string_lossy().into_owned(),
                },
            )
            .await
            .unwrap();
        loop {
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::FileOperationFailed {
                    code: FileErrorCode::OutsideRoot,
                    workspace_root_id: Some(id),
                    document_id: None,
                    ..
                } if id == selected_root_id => break,
                ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_) => {}
                other => panic!("expected outside-root failure, got {other:?}"),
            }
        }

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(selected);
        let _ = fs::remove_file(sibling);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn connection_add_selected_workspace_root_sends_file_browser_snapshot() {
        let root = temp_workspace("selected-folder-dispatch");
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            Arc::clone(&document),
            behavior,
            Arc::clone(&workspace),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { token } => token,
            message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
        };

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::AddSelectedWorkspaceRoot {
                    client_id: 99,
                    capability: capability_token,
                    selected_path: root.to_string_lossy().into_owned(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::SduiSnapshot { client_id: 99, .. }
        ));
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));
        assert_eq!(workspace.lock().await.list_root_metadata().len(), 1);

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(root.join("main.rs"));
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn connection_add_selected_workspace_root_rejects_stale_capability() {
        let root = temp_workspace("selected-folder-stale");
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            Arc::clone(&workspace),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::AddSelectedWorkspaceRoot {
                    client_id: 99,
                    capability: "stale".to_string(),
                    selected_path: root.to_string_lossy().into_owned(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::RuntimeDiagnostic(diagnostic)
                if diagnostic.code == "client.selected_folder_open.unauthorized"
        ));
        assert!(workspace.lock().await.list_root_metadata().is_empty());

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn file_io_errors_are_typed_protocol_failures() {
        let root = temp_workspace("typed-errors");
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));

        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace,
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();
        let _active_theme = codec.read_server_message(&mut client).await.unwrap();
        let _active_typography = codec.read_server_message(&mut client).await.unwrap();
        let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
        let _capability = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenDocument {
                    client_id: 99,
                    workspace_root_id: root_id,
                    path: "missing.txt".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::NotFound,
                workspace_root_id: Some(id),
                document_id: None,
                ..
            } if id == root_id
        ));

        let invalid_utf8 = root.join("invalid.txt");
        fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenDocument {
                    client_id: 99,
                    workspace_root_id: root_id,
                    path: "invalid.txt".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::InvalidUtf8,
                workspace_root_id: Some(id),
                document_id: None,
                ..
            } if id == root_id
        ));

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn server_rejects_invalid_frame_without_panic() {
        let (mut client, server) = duplex(4096);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));

        tokio::io::AsyncWriteExt::write_all(&mut client, &[0, 0, 0, 4, 0xde, 0xad, 0xbe, 0xef])
            .await
            .unwrap();
        drop(client);

        let result = server_task.await.unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn fragmented_client_frame_survives_concurrent_server_write() {
        use std::time::Duration;
        use tokio::io::AsyncWriteExt;

        let (mut client, server) = duplex(4096);
        let codec = Codec::default();
        let runtime_generation = runtime_generation();
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document_state(),
            Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace_state(),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation.clone(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        for _ in 0..9 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }

        // Drip-feed a client frame start, then fire a typography broadcast so a
        // server write wins the select race mid-frame. The read pump must keep
        // frame alignment regardless of the interleaving.
        let frame = codec
            .encode_client_message(&ClientMessage::ListDocuments { client_id: 99 })
            .unwrap();
        let split = 6;
        client.write_all(&frame[..split]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut typography = crate::protocol::ActiveTypography::default();
        typography.monospace.size += 1.0;
        runtime_generation
            .replace_typography(typography)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        client.write_all(&frame[split..]).await.unwrap();

        let mut saw_typography = false;
        let mut saw_status = false;
        for _ in 0..4 {
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::ActiveTypography(_) => saw_typography = true,
                ServerMessage::DocumentList { .. } => saw_status = true,
                other => panic!("unexpected message during fragmented read: {other:?}"),
            }
            if saw_typography && saw_status {
                break;
            }
        }
        assert!(saw_typography && saw_status);

        // A second full request proves the stream stayed aligned.
        codec
            .write_client_message(&mut client, &ClientMessage::ListDocuments { client_id: 99 })
            .await
            .unwrap();
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::DocumentList { .. }
        ));

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn open_selected_file_without_capability_is_rejected_with_diagnostic() {
        let root = temp_workspace("selected-unauthorized");
        let target = root.join("secret.md");
        fs::write(&target, "# secret\n").unwrap();
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let server_task = tokio::spawn(handle_connection(
            server,
            99,
            document,
            behavior,
            Arc::clone(&workspace),
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation(),
            parse_coordinator(),
            language_intelligence_coordinator(),
            codec,
        ));
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        // Consume handshake noise and the post-handshake capability so it is no
        // longer pending.
        loop {
            if matches!(
                codec.read_server_message(&mut client).await.unwrap(),
                ServerMessage::FileOpenCapabilityIssued { .. }
            ) {
                break;
            }
        }

        // Raw path with no valid capability: server must reject and must NOT
        // open or grant the file.
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenSelectedFile {
                    client_id: 99,
                    capability: String::new(),
                    selected_path: target.to_string_lossy().into_owned(),
                },
            )
            .await
            .unwrap();
        // Re-issued pending capability first, then the rejection diagnostic.
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::RuntimeDiagnostic(diagnostic) => {
                assert_eq!(diagnostic.code, "client.selected_file_open.unauthorized");
            }
            message => panic!("expected unauthorized RuntimeDiagnostic, got {message:?}"),
        }
        // No document was registered for the rejected path.
        assert!(workspace.lock().await.document_handle(1).is_none());

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(target);
        let _ = fs::remove_dir(root);
    }

    /// CloseDocument: final-holder close acknowledges and tears down the
    /// document; a shared document survives until the last holder closes
    /// (Plan 060 T6, P1-4).
    #[tokio::test]
    async fn close_document_acknowledges_and_tears_down_final_document() {
        let root = temp_workspace("close-document");
        fs::write(root.join("note.md"), "hello\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let runtime_generation = runtime_generation();
        let parse_coordinator = parse_coordinator();
        let document_analysis =
            crate::server::document_analysis::DocumentAnalysisCoordinator::default();

        let mut connection_a = TestConnection::connect(
            99,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation.clone(),
            parse_coordinator.clone(),
            document_analysis.clone(),
            language_intelligence_coordinator(),
        )
        .await;
        let mut connection_b = TestConnection::connect(
            100,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence_coordinator(),
        )
        .await;

        // A opens the shared document, then B opens it too.
        connection_a
            .send(&ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "note.md".to_string(),
            })
            .await;
        connection_a.drain_until_quiet().await;
        connection_b
            .send(&ClientMessage::OpenDocument {
                client_id: 100,
                workspace_root_id: root_id,
                path: "note.md".to_string(),
            })
            .await;
        connection_b.drain_until_quiet().await;

        // A closes: not the final holder, document survives for B.
        connection_a
            .send(&ClientMessage::CloseDocument {
                client_id: 99,
                document_id: 1,
                force: false,
            })
            .await;
        let closed_a = connection_a.receive_response().await;
        assert!(
            matches!(
                closed_a,
                ServerMessage::DocumentClosed {
                    document_id: 1,
                    closed: false
                }
            ),
            "non-final close must report closed=false, got {closed_a:?}"
        );
        connection_b
            .send(&ClientMessage::GetDocumentStatus {
                client_id: 100,
                document_id: 1,
            })
            .await;
        let status_b = connection_b.receive_response().await;
        assert!(
            matches!(status_b, ServerMessage::DocumentStatus { .. }),
            "remaining holder must still see the document, got {status_b:?}"
        );
        // A no longer has access.
        connection_a
            .send(&ClientMessage::GetDocumentStatus {
                client_id: 99,
                document_id: 1,
            })
            .await;
        let status_a = connection_a.receive_response().await;
        assert!(
            matches!(
                status_a,
                ServerMessage::FileOperationFailed {
                    code: FileErrorCode::UnknownDocument,
                    ..
                }
            ),
            "closed connection must lose access, got {status_a:?}"
        );

        // B closes: final holder, document is torn down.
        connection_b
            .send(&ClientMessage::CloseDocument {
                client_id: 100,
                document_id: 1,
                force: false,
            })
            .await;
        let closed_b = connection_b.receive_response().await;
        assert!(
            matches!(
                closed_b,
                ServerMessage::DocumentClosed {
                    document_id: 1,
                    closed: true
                }
            ),
            "final close must report closed=true, got {closed_b:?}"
        );
        assert!(workspace.lock().await.document_handle(1).is_none());

        let _ = fs::remove_file(root.join("note.md"));
        let _ = fs::remove_dir(root);
    }

    /// Disconnect releases every access grant; documents with no remaining
    /// holders are removed from the workspace registry (Plan 060 T6, P1-4).
    #[tokio::test]
    async fn disconnect_finalizes_documents_with_no_remaining_holders() {
        let root = temp_workspace("disconnect-finalize");
        fs::write(root.join("note.md"), "hello\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));

        let connection = TestConnection::connect(
            99,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation(),
            parse_coordinator(),
            crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
            language_intelligence_coordinator(),
        )
        .await;
        let mut connection = connection;
        connection
            .send(&ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "note.md".to_string(),
            })
            .await;
        connection.drain_until_quiet().await;
        assert!(workspace.lock().await.document_handle(1).is_some());

        // Disconnect: the server task exits and finalizes the document.
        drop(connection.client);
        connection.server_task.await.unwrap().unwrap();
        assert!(
            workspace.lock().await.document_handle(1).is_none(),
            "disconnect must finalize documents with no remaining holders"
        );

        let _ = fs::remove_file(root.join("note.md"));
        let _ = fs::remove_dir(root);
    }

    /// Phase 22.6 (plan 077 task 6): a reconnected/reclaimed tab regains
    /// only its own grants. Tab A's disconnect releases every document
    /// grant; a fresh connection re-opening one of the tab's documents
    /// inherits nothing — the tab's other document stays unknown until
    /// explicitly re-opened.
    #[tokio::test]
    async fn reconnected_tab_regains_only_its_own_reopened_grants() {
        let root = temp_workspace("tab-reclaim-grants");
        fs::write(root.join("note.md"), "hello\n").unwrap();
        fs::write(root.join("second.md"), "second\n").unwrap();
        let mut workspace_state_value = WorkspaceState::new();
        let root_id = workspace_state_value.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(workspace_state_value));
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "scratch".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let runtime_generation = runtime_generation();
        let parse_coordinator = parse_coordinator();
        let document_analysis =
            crate::server::document_analysis::DocumentAnalysisCoordinator::default();

        // Tab A's connection (99) opens both of the tab's documents.
        let mut connection_a = TestConnection::connect(
            99,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation.clone(),
            parse_coordinator.clone(),
            document_analysis.clone(),
            language_intelligence_coordinator(),
        )
        .await;
        let first_document =
            open_document_until_opened(&mut connection_a, 99, root_id, "note.md").await;
        let second_document =
            open_document_until_opened(&mut connection_a, 99, root_id, "second.md").await;
        assert_eq!(first_document, 1);
        assert_eq!(second_document, 2);
        connection_a.drain_until_quiet().await;
        assert!(workspace.lock().await.document_handle(1).is_some());
        assert!(workspace.lock().await.document_handle(2).is_some());

        // Disconnect: every grant is released and both documents finalize.
        connection_a.close().await;
        assert!(
            workspace.lock().await.document_handle(1).is_none(),
            "disconnect must release the tab's first document grant"
        );
        assert!(
            workspace.lock().await.document_handle(2).is_none(),
            "disconnect must release the tab's second document grant"
        );

        // The reconnected tab (fresh connection 101) inherits nothing.
        let mut connection_c = TestConnection::connect(
            101,
            Arc::clone(&document),
            Arc::clone(&behavior),
            Arc::clone(&workspace),
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence_coordinator(),
        )
        .await;
        connection_c
            .send(&ClientMessage::ListDocuments { client_id: 101 })
            .await;
        let list = connection_c.receive_response().await;
        assert!(
            matches!(list, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
            "reconnected tab must inherit no grants, got {list:?}"
        );

        // Re-opening the tab's own document grants only the new connection:
        // the old grant was finalized, so the file re-opens as a fresh
        // document with a fresh lease, not as a restored one.
        let reopened = open_document_until_opened(&mut connection_c, 101, root_id, "note.md").await;
        assert_ne!(
            reopened, first_document,
            "a finalized grant must not be re-attached; re-open is a fresh grant"
        );
        connection_c.drain_until_quiet().await;
        connection_c
            .send(&ClientMessage::GetDocumentStatus {
                client_id: 101,
                document_id: second_document,
            })
            .await;
        let status = connection_c.receive_response().await;
        assert!(
            matches!(
                status,
                ServerMessage::FileOperationFailed {
                    code: FileErrorCode::UnknownDocument,
                    ..
                }
            ),
            "the tab's second document stays ungranted until re-opened, got {status:?}"
        );
        connection_c
            .send(&ClientMessage::ListDocuments { client_id: 101 })
            .await;
        let list = connection_c.receive_response().await;
        assert!(
            matches!(list, ServerMessage::DocumentList { ref documents }
                if documents.len() == 1 && documents[0].document_id == reopened),
            "re-opened grant is the only grant, got {list:?}"
        );

        connection_c.close().await;
        let _ = fs::remove_file(root.join("note.md"));
        let _ = fs::remove_file(root.join("second.md"));
        let _ = fs::remove_dir(root);
    }

    /// Open `path` and return the granted document id, skipping open-time
    /// follow-up noise (behavior manifest, diagnostics, SDUI snapshot).
    async fn open_document_until_opened(
        connection: &mut TestConnection,
        client_id: u64,
        root_id: crate::protocol::WorkspaceRootId,
        path: &str,
    ) -> crate::protocol::DocumentId {
        connection
            .send(&ClientMessage::OpenDocument {
                client_id,
                workspace_root_id: root_id,
                path: path.to_string(),
            })
            .await;
        loop {
            match connection.receive().await {
                ServerMessage::DocumentOpened { metadata, .. } => return metadata.document_id,
                ServerMessage::BehaviorManifest(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. } => {}
                other => panic!("unexpected message during open: {other:?}"),
            }
        }
    }

    /// Runtime-diagnostic retention: consecutive duplicates collapse, the
    /// deque never exceeds its capacity, and drops are counted (Plan 060 T6,
    /// P1-8).
    #[test]
    fn runtime_diagnostic_store_deduplicates_and_bounds() {
        let mut store = RuntimeDiagnosticStore::default();
        let duplicate = RuntimeDiagnostic::warning("test.dup", "same");
        store.push(duplicate.clone());
        store.push(duplicate);
        assert_eq!(
            store.snapshot().len(),
            1,
            "consecutive duplicate must collapse"
        );
        assert_eq!(store.dropped_count(), 0);

        for index in 0..crate::perf::budgets::RUNTIME_DIAGNOSTIC_CAPACITY + 8 {
            store.push(RuntimeDiagnostic::warning(
                "test.flood",
                format!("diagnostic {index}"),
            ));
        }
        let snapshot = store.snapshot();
        assert_eq!(
            snapshot.len(),
            crate::perf::budgets::RUNTIME_DIAGNOSTIC_CAPACITY,
            "retention must stay within the snapshot cap"
        );
        // 41 total entries (1 duplicate survivor + 40 flood) minus the 32
        // retained = 9 dropped; "diagnostic 8" is the oldest survivor.
        assert_eq!(store.dropped_count(), 9);
        assert_eq!(
            snapshot.first().map(|d| d.message.as_str()),
            Some("diagnostic 8"),
            "oldest entries drop first"
        );
    }
    #[test]
    fn sdui_command_request_forwards_list_item_id_as_argument() {
        let intent = SduiActionIntent {
            command_id: "settings.setTheme".to_string(),
            source: SduiActionSource::ListItem {
                node_id: SduiNodeId(7),
                item_id: "@clay/theme-modus-vivendi".to_string(),
            },
            arguments: Vec::new(),
        };
        let request = sdui_command_request(&intent);
        assert_eq!(request.command_id, "settings.setTheme");
        assert_eq!(
            request.arguments,
            serde_json::json!({ "item_id": "@clay/theme-modus-vivendi" })
        );
    }

    #[test]
    fn sdui_command_request_forwards_button_node_id_as_argument() {
        let intent = SduiActionIntent {
            command_id: "settings.close".to_string(),
            source: SduiActionSource::Button {
                node_id: SduiNodeId(42),
            },
            arguments: Vec::new(),
        };
        let request = sdui_command_request(&intent);
        assert_eq!(request.arguments, serde_json::json!({ "node_id": "42" }));
    }

    #[test]
    fn sdui_command_request_preserves_explicit_arguments() {
        let intent = SduiActionIntent {
            command_id: "workspace.openFile".to_string(),
            source: SduiActionSource::Button {
                node_id: SduiNodeId(1),
            },
            arguments: vec![SduiActionArgument {
                name: "path".to_string(),
                value: SduiActionValue::String("/tmp/a.md".to_string()),
            }],
        };
        let request = sdui_command_request(&intent);
        // Explicit arguments are preserved; source node_id is added additively.
        assert_eq!(
            request.arguments,
            serde_json::json!({ "path": "/tmp/a.md", "node_id": "1" })
        );
    }
}
