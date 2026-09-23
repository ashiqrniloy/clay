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

use crate::lock_util::LockOrRecover;
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
mod delivery;
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
        self.live_router.lock_or_recover().broadcast(&diagnostic);
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
            .lock_or_recover()
            .unsubscribe_client(self.client_id);
    }
}

/// Connection-local aggregation state for one atomic viewport request.
pub(super) struct PendingViewportPatch {
    /// Scheduled parse windows still owed a terminal update.
    remaining: usize,
    patch: ViewportRenderPatch,
}

/// Per-message dispatch context for extracted connection handlers.
///
/// Handlers borrow the connection's shared state through this struct instead of
/// repeating the loop's parameter list. Every field is a borrow, so a handler
/// cannot outlive the connection state it serves, and the authorization helpers
/// (`document_for_message`, capability consumption, tab-binding checks) stay
/// inside the handler that needs them. Built per dispatched message: the loop's
/// subscription `select!` arms keep using the connection's locals directly, and
/// `TabCommand` rebinds `document`/`workspace` through the mutable handles.
pub(super) struct ConnectionCtx<'a, S: AsyncWrite + Unpin> {
    pub(super) codec: Codec,
    pub(super) stream: &'a mut S,
    pub(super) client_id: ClientId,
    pub(super) document: &'a mut Arc<Mutex<DocumentState>>,
    pub(super) workspace: &'a mut Arc<Mutex<WorkspaceState>>,
    pub(super) behavior: &'a Arc<Mutex<ActiveBehaviorManifest>>,
    pub(super) runtime_generation: &'a RuntimeGenerationStore,
    pub(super) sdui: &'a Arc<Mutex<StaticSduiState>>,
    pub(super) parse_coordinator: &'a ParseCoordinator,
    pub(super) completion: &'a crate::server::completion::CompletionCoordinator,
    pub(super) language_intelligence: &'a LanguageIntelligenceCoordinator,
    pub(super) document_analysis: &'a crate::server::document_analysis::DocumentAnalysisCoordinator,
    pub(super) reload_server: Option<&'a super::IpcServer>,
    pub(super) file_open_capabilities: &'a mut FileOpenCapabilityPool,
    pub(super) menu_sessions: &'a mut ServerMenuSessions,
    pub(super) bound_tab_id: &'a mut Option<TabId>,
    pub(super) bound_state: &'a Arc<std::sync::Mutex<Option<TabServerState>>>,
    pub(super) tab_registry: &'a Arc<Mutex<TabRegistry>>,
    pub(super) tab_registry_tx: &'a tokio::sync::broadcast::Sender<TabRegistrySnapshot>,
    pub(super) pending_viewport_patches:
        &'a mut HashMap<(DocumentId, crate::protocol::ViewportRequestId), PendingViewportPatch>,
    pub(super) completion_tx: &'a tokio::sync::mpsc::Sender<ServerMessage>,
    pub(super) language_intelligence_tx: &'a tokio::sync::mpsc::Sender<ServerMessage>,
    pub(super) dropped_results: &'a Arc<AtomicU64>,
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
        | ClientMessage::ListAgentSettingsFiles { client_id }
        | ClientMessage::OpenAgentSettingsFile { client_id, .. }
        | ClientMessage::AddSelectedWorkspaceRoot { client_id, .. }
        | ClientMessage::SaveDocument { client_id, .. }
        | ClientMessage::ReloadDocument { client_id, .. }
        | ClientMessage::GetDocumentStatus { client_id, .. }
        | ClientMessage::ListDocuments { client_id }
        | ClientMessage::ListLauncherEntries { client_id }
        | ClientMessage::RemoveLauncherRecent { client_id, .. }
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
            | ClientMessage::OpenAgentSettingsFile { .. }
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

/// Plan 119 SC-6: the tab's own answer to `TabState`, naming the session the
/// tab owns. The agent relay fans every session's messages out to every
/// connection, so a client can only tell whose traffic it is looking at when
/// the message says so — the requesting client id is what makes this answer
/// attributable, and the session id is what the store then filters on.
fn session_bound_message(client_id: u64, tab: u64, session_id: &str) -> AgentServerMessage {
    AgentServerMessage::AgentRpc {
        code: "session.bound".into(),
        result_json: serde_json::json!({
            "clientId": client_id,
            "tabId": tab,
            "sessionId": session_id,
        })
        .to_string(),
    }
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
    let tracked_state = cleanup_bound_state.lock_or_recover().clone();
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
        .lock_or_recover()
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
    let Some(mut file_open_capabilities) = complete_first_message_handshake(
        codec,
        &mut stream,
        client_id,
        &behavior,
        &active_theme,
        &runtime_diagnostics,
        &runtime_generation,
        &tab_registry,
        if reload_server.is_none() {
            Some((&bootstrap_document, &bootstrap_workspace, &sdui))
        } else {
            None
        },
        first_message,
    )
    .await?
    else {
        return Ok(());
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
    // Lane delivery: one family helper per lane, each returning `Flow::Close`
    // when its sender is gone (the connection is over) and `Flow::Continue`
    // otherwise. `Delivery` and the per-family policy live in `delivery`.
    macro_rules! lane {
        ($delivery:expr) => {
            match $delivery.await? {
                delivery::Flow::Continue => continue,
                delivery::Flow::Close => return Ok(()),
            }
        };
    }
    // Plan 129: build the dispatch context for one extracted arm. Arms that
    // rebind connection state (TabCommand) or index the loop's mutable maps do
    // so through the context's `&mut` fields; the subscription `select!` arms
    // above keep using the locals directly.
    macro_rules! ctx {
        () => {
            ConnectionCtx {
                codec,
                stream: &mut stream,
                client_id,
                document: &mut document,
                workspace: &mut workspace,
                behavior: &behavior,
                runtime_generation: &runtime_generation,
                sdui: &sdui,
                parse_coordinator: &parse_coordinator,
                completion: &completion,
                language_intelligence: &language_intelligence,
                document_analysis: &document_analysis,
                reload_server: reload_server.as_ref(),
                file_open_capabilities: &mut file_open_capabilities,
                menu_sessions: &mut menu_sessions,
                bound_tab_id: &mut bound_tab_id,
                bound_state: &bound_state,
                tab_registry: &tab_registry,
                tab_registry_tx: &tab_registry_tx,
                pending_viewport_patches: &mut pending_viewport_patches,
                completion_tx: &completion_tx,
                language_intelligence_tx: &language_intelligence_tx,
                dropped_results: &dropped_results,
            }
        };
    }
    loop {
        let message = match tokio::select! {
            typography = typography_updates.recv() => lane!(delivery::typography(
                codec,
                &mut stream,
                typography,
                &runtime_generation
            )),
            editor_command = editor_command_updates.recv() => {
                lane!(delivery::editor_command(codec, &mut stream, editor_command))
            }
            caret_style = caret_style_updates.recv() => lane!(delivery::caret_style(
                codec,
                &mut stream,
                caret_style,
                &runtime_generation
            )),
            layout = editor_layout_updates.recv() => lane!(delivery::editor_layout(
                codec,
                &mut stream,
                layout,
                &runtime_generation
            )),
            prefs = shell_preferences_updates.recv() => lane!(delivery::shell_preferences(
                codec,
                &mut stream,
                prefs,
                &runtime_generation
            )),
            tab_registry_update = tab_registry_updates.recv() => lane!(delivery::tab_registry(
                codec,
                &mut stream,
                tab_registry_update,
                &tab_registry
            )),
            runtime_generation_id = runtime_state_updates.recv() => lane!(delivery::runtime_generation(
                codec,
                &mut stream,
                &mut menu_sessions,
                &runtime_generation,
                client_id,
                runtime_generation_id
            )),
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
                lane!(delivery::result(codec, &mut stream, diagnostic, ServerMessage::RuntimeDiagnostic))
            }
            diagnostic = runtime_diagnostics_rx.recv() => {
                lane!(delivery::result(codec, &mut stream, diagnostic, ServerMessage::RuntimeDiagnostic))
            }
            output = analysis_rx.recv() => {
                lane!(delivery::analysis(codec, &mut stream, output))
            }
            message = completion_rx.recv() => {
                lane!(delivery::result(codec, &mut stream, message, |message| message))
            }
            message = language_intelligence_rx.recv() => {
                lane!(delivery::result(codec, &mut stream, message, |message| message))
            }
            agent_event = async {
                match agent_rx.as_mut() {
                    Some(rx) => Some(rx.recv().await),
                    None => {
                        std::future::pending::<()>().await;
                        None
                    }
                }
            } => lane!(delivery::agent(codec, &mut stream, agent_event)),
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
            *bound_state.lock_or_recover() = Some(routed.state.clone());
            document = routed.state.welcome;
            workspace = routed.state.workspace;
        }

        match message {
            ClientMessage::Edit {
                document_id,
                lease_id,
                base_version,
                behavior_version,
                transaction_id,
                operation,
                ..
            } => {
                documents::dispatch_edit_operation(
                    &mut ctx!(),
                    documents::EditOperationParams {
                        document_id,
                        lease_id,
                        base_version,
                        behavior_version,
                        transaction_id,
                        operation,
                    },
                )
                .await?;
            }
            ClientMessage::EditorIntent {
                document_id,
                lease_id,
                base_version,
                behavior_version,
                transaction_id,
                intent,
                ..
            } => {
                documents::dispatch_edit_operation(
                    &mut ctx!(),
                    documents::EditOperationParams {
                        document_id,
                        lease_id,
                        base_version,
                        behavior_version,
                        transaction_id,
                        operation: match intent {
                            crate::protocol::EditorIntent::InsertText { byte_offset, text } => {
                                crate::protocol::EditOperation::Insert { byte_offset, text }
                            }
                            crate::protocol::EditorIntent::DeleteRange { start, end } => {
                                crate::protocol::EditOperation::Delete { start, end }
                            }
                        },
                    },
                )
                .await?;
            }
            ClientMessage::RequestResync { document_id, .. } => {
                documents::handle_request_resync(&mut ctx!(), document_id).await?;
            }
            ClientMessage::DocumentChunkRequest {
                document_id,
                document_version,
                offset,
                max_bytes,
                ..
            } => {
                documents::handle_document_chunk_request(
                    &mut ctx!(),
                    documents::DocumentChunkRequestParams {
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
                let mut ctx = ctx!();
                let scheduled = documents::handle_viewport_render_request(
                    &mut ctx,
                    documents::ViewportRequestParams {
                        document_id,
                        document_version,
                        request_id,
                        byte_start,
                        byte_end,
                        trace_id,
                    },
                )
                .await?;
                // Latest request wins: a newer request for the same document
                // supersedes any still-pending older patch (protocol v29).
                documents::track_pending_viewport_request(
                    &mut *ctx.pending_viewport_patches,
                    document_id,
                    document_version,
                    request_id,
                    trace_id,
                    scheduled,
                );
            }
            ClientMessage::OpenDocument {
                workspace_root_id,
                path,
                ..
            } => {
                documents::handle_open_document(&mut ctx!(), workspace_root_id, path).await?;
            }
            ClientMessage::OpenSelectedFile {
                capability,
                selected_path,
                ..
            } => {
                workspace::handle_open_selected_file(&mut ctx!(), capability, selected_path)
                    .await?;
            }
            ClientMessage::ListAgentSettingsFiles { .. } => {
                workspace::handle_list_agent_settings_files(&mut ctx!()).await?;
            }
            ClientMessage::ListLauncherEntries { .. } => {
                workspace::handle_list_launcher_entries(&mut ctx!()).await?;
            }
            ClientMessage::RemoveLauncherRecent { index, .. } => {
                workspace::handle_remove_launcher_recent(&mut ctx!(), index).await?;
            }
            ClientMessage::OpenAgentSettingsFile { name, .. } => {
                workspace::handle_open_agent_settings_file(&mut ctx!(), name).await?;
            }
            ClientMessage::AddSelectedWorkspaceRoot {
                capability,
                selected_path,
                ..
            } => {
                workspace::handle_add_selected_workspace_root(
                    &mut ctx!(),
                    capability,
                    selected_path,
                )
                .await?;
            }
            ClientMessage::SaveDocument {
                document_id,
                known_version,
                ..
            } => {
                documents::handle_save_document(&mut ctx!(), document_id, known_version).await?;
            }
            ClientMessage::ReloadDocument {
                document_id,
                known_version,
                force,
                ..
            } => {
                documents::handle_reload_document(&mut ctx!(), document_id, known_version, force)
                    .await?;
            }
            ClientMessage::CloseDocument {
                document_id, force, ..
            } => {
                documents::handle_close_document(&mut ctx!(), document_id, force).await?;
            }
            ClientMessage::GetDocumentStatus { document_id, .. } => {
                documents::handle_get_document_status(&mut ctx!(), document_id).await?;
            }
            ClientMessage::ListDocuments { .. } => {
                documents::handle_list_documents(&mut ctx!()).await?;
            }
            ClientMessage::TabCommand { command, .. } => {
                match tabs::handle_tab_command(&mut ctx!(), command).await? {
                    tabs::TabDispatch::Continue => {}
                    tabs::TabDispatch::CloseConnection => return Ok(()),
                }
            }
            ClientMessage::MenuQueryUpdate {
                session_id,
                query,
                scope,
                ..
            } => {
                menus::handle_menu_query_update(&mut ctx!(), session_id, query, scope).await?;
            }
            ClientMessage::MenuBackspace { session_id, .. } => {
                menus::handle_menu_backspace(&mut ctx!(), session_id).await?;
            }
            ClientMessage::MenuSelectionMove {
                session_id, delta, ..
            } => {
                menus::handle_menu_selection_move(&mut ctx!(), session_id, delta).await?;
            }
            ClientMessage::MenuActivate {
                session_id, kind, ..
            } => {
                menus::handle_menu_activate(&mut ctx!(), session_id, kind).await?;
            }
            ClientMessage::MenuCancel { session_id, .. } => {
                menus::handle_menu_cancel(&mut ctx!(), session_id).await?;
            }
            ClientMessage::SduiAction {
                ui_version, intent, ..
            } => {
                runtime::handle_sdui_action(&mut ctx!(), ui_version, intent).await?;
            }
            ClientMessage::CommandIntent {
                document_id,
                behavior_version,
                command_id,
                ..
            } => {
                runtime::handle_command_intent(
                    &mut ctx!(),
                    document_id,
                    behavior_version,
                    command_id,
                )
                .await?;
            }
            ClientMessage::CompletionRequest { mut request } => {
                runtime::handle_completion_request(&mut ctx!(), &mut request).await?;
            }
            ClientMessage::LanguageIntelligenceRequest { mut request } => {
                runtime::handle_language_intelligence_request(&mut ctx!(), &mut request).await?;
            }
            ClientMessage::RuntimeGenerationInstalled {
                client_id: ack_client_id,
                runtime_generation_id,
            } => {
                runtime::handle_runtime_generation_installed(
                    &mut ctx!(),
                    ack_client_id,
                    runtime_generation_id,
                )
                .await;
            }
            ClientMessage::SelectionQueryRequest { request } => {
                documents::handle_selection_query_request(&mut ctx!(), &request).await?;
            }
            ClientMessage::Hello { .. } => {
                runtime::handle_duplicate_hello(&mut ctx!()).await?;
            }
            ClientMessage::Agent { command, .. } => {
                runtime::handle_agent_command(&mut ctx!(), command).await?;
            }
        }
    }
}
/// First-message handshake. A matching `Hello` earns the welcome snapshot,
/// the tab-registry replay, and this connection's initial file-open
/// capability; a mismatched protocol version or a non-`Hello` first message is
/// answered with one bound error and ends the connection (`None`).
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the handshake snapshots the server-owned state handles the client needs"
)]
async fn complete_first_message_handshake<S>(
    codec: Codec,
    mut stream: &mut S,
    client_id: ClientId,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    active_theme: &Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: &Arc<Mutex<RuntimeDiagnosticStore>>,
    runtime_generation: &RuntimeGenerationStore,
    tab_registry: &Arc<Mutex<TabRegistry>>,
    legacy_bootstrap: Option<(
        &Arc<Mutex<DocumentState>>,
        &Arc<Mutex<WorkspaceState>>,
        &Arc<Mutex<StaticSduiState>>,
    )>,
    first_message: ClientMessage,
) -> Result<Option<FileOpenCapabilityPool>, CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let file_open_capabilities = match first_message {
        ClientMessage::Hello {
            protocol_version,
            client_name: _,
        } if protocol_version == PROTOCOL_VERSION => {
            send_welcome_snapshot_and_manifest(
                &mut stream,
                client_id,
                behavior,
                active_theme,
                runtime_diagnostics,
                runtime_generation,
                legacy_bootstrap,
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
            return Ok(None);
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
            return Ok(None);
        }
    };
    Ok(Some(file_open_capabilities))
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

    // Handshake replay of the committed runtime snapshot (package UI, SDUI
    // tree, diagnostics). Generation broadcasts only fire on *replacement*;
    // without this, a client that connects after config load never receives
    // packageUi and pane surfaces (Coding Agent, Chat empty-tab) stay dead.
    if let Some(snapshot) = runtime_generation
        .latest_runtime_snapshot_for(client_id)
        .await
    {
        codec
            .write_server_message(
                stream,
                &ServerMessage::RuntimeStateSnapshot(Box::new(snapshot)),
            )
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
