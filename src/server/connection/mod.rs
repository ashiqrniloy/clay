use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};

use crate::perf::metrics::{MetricMetadata, MetricValue, SERVER_RECEIVE, global_recorder};
use crate::protocol::ViewportRenderPatch;
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

#[allow(unused_imports)]
pub(crate) use self::documents::{CachedModeActivation, ModeActivationKey};
pub(crate) use self::documents::{open_document_followup_messages, start_document_analysis};
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

/// Connection-local aggregation state for one atomic viewport request.
struct PendingViewportPatch {
    /// Scheduled parse windows still owed a terminal update.
    remaining: usize,
    patch: ViewportRenderPatch,
}

fn client_message_trace_id(message: &ClientMessage) -> Option<crate::protocol::PerformanceTraceId> {
    match message {
        ClientMessage::Edit { transaction_id, .. } => Some(*transaction_id),
        ClientMessage::ViewportRenderRequest { trace_id, .. } => *trace_id,
        _ => None,
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
        | ClientMessage::DocumentChunkRequest { client_id, .. }
        | ClientMessage::ViewportRenderRequest { client_id, .. }
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
            | ClientMessage::ViewportRenderRequest { .. }
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
    // Protocol v29 atomic viewport aggregation: one pending entry per
    // (document, request id); the newest request for a document supersedes
    // every older pending entry.
    let mut pending_viewport_patches: HashMap<
        (DocumentId, crate::protocol::ViewportRequestId),
        PendingViewportPatch,
    > = HashMap::new();
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
                    menus::write_active_menu_session_closed(
                        codec,
                        &mut stream,
                        &mut menu_sessions,
                    )
                    .await?;
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
                // Request-scoped updates aggregate into their pending patch;
                // edit-driven updates keep the per-update frames. Family-owned
                // delivery: documents.rs owns patch aggregation and batching.
                documents::deliver_parse_update(
                    codec,
                    &mut stream,
                    &mut pending_viewport_patches,
                    update,
                    client_id,
                )
                .await?;
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

        let recorder = global_recorder();
        let trace_id = client_message_trace_id(&message);
        if recorder.is_enabled()
            && let Some(trace_id) = trace_id
        {
            let metadata = match &message {
                ClientMessage::Edit {
                    document_id,
                    base_version,
                    ..
                } => MetricMetadata::document(*document_id, *base_version),
                ClientMessage::ViewportRenderRequest {
                    document_id,
                    document_version,
                    byte_start,
                    byte_end,
                    ..
                } => MetricMetadata::document(*document_id, *document_version)
                    .with_byte_count(byte_end.saturating_sub(*byte_start)),
                _ => MetricMetadata::default(),
            };
            recorder.record_with_metadata(
                SERVER_RECEIVE,
                MetricValue::Counter { amount: 1 },
                metadata.with_trace_id(Some(trace_id)),
            );
        }

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
            ClientMessage::DocumentChunkRequest {
                client_id,
                document_id,
                document_version,
                offset,
                max_bytes,
            } => {
                documents::handle_document_chunk_request(
                    codec,
                    &mut stream,
                    &document,
                    &workspace,
                    documents::DocumentChunkRequestParams {
                        client_id,
                        document_id,
                        document_version,
                        offset,
                        max_bytes,
                    },
                )
                .await?;
            }
            ClientMessage::ViewportRenderRequest {
                document_id,
                document_version,
                request_id,
                byte_start,
                byte_end,
                trace_id,
                ..
            } => {
                let scheduled = documents::handle_viewport_render_request(
                    codec,
                    &mut stream,
                    &behavior,
                    &runtime_generation,
                    &workspace,
                    &parse_coordinator,
                    client_id,
                    document_id,
                    document_version,
                    request_id,
                    byte_start,
                    byte_end,
                    trace_id,
                )
                .await?;
                // Latest request wins: a newer request for the same document
                // supersedes any still-pending older patch (protocol v29).
                documents::track_pending_viewport_request(
                    &mut pending_viewport_patches,
                    document_id,
                    document_version,
                    request_id,
                    trace_id,
                    scheduled,
                );
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
            ClientMessage::SduiAction {
                ui_version, intent, ..
            } => {
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
                    ui_version,
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
mod tests;
