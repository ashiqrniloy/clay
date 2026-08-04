use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};

use crate::{
    packages::commands::CommandRegistry,
    perf::budgets::{
        COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
        INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
    },
    protocol::{
        ClientId, ClientMessage, CompletionProvenance, CompletionRequest, CompletionResultSet,
        CompletionStatus, CompletionTrigger, DocumentId, DocumentMetadata,
        LanguageIntelligenceFeature, LanguageIntelligencePayload, LanguageIntelligenceResult,
        LanguageIntelligenceStatus, PROTOCOL_VERSION, ParseByteRange, ParseInputEdit, ParsePolicy,
        ParseWindowSnapshot, ProtocolErrorCode, RuntimeDiagnostic, SduiActionArgument,
        SduiActionIntent, SduiActionSource, SduiActionValue, SelectionQueryRange,
        SelectionQueryResult, ServerMessage, WorkspaceRootId,
        codec::{Codec, CodecError},
        completion::estimated_result_payload_bytes,
    },
};

use super::{
    RuntimeGenerationStore,
    behavior::{ActiveBehaviorManifest, BehaviorVersionDecision},
    command_execution::{CommandExecutionRequest, CommandExecutionTarget, CommandExecutor},
    completion::{CompletionProviderMeta, apply_exclusive_suppression},
    document::DocumentState,
    js_runtime::ClayJsRuntimeService,
    language_intelligence::{
        LanguageIntelligenceCoordinator, LanguageIntelligenceCoordinatorError,
        LanguageIntelligenceDocumentWindow,
    },
    parse_coordinator::{ParseCoordinator, ParseCoordinatorError, ParseScheduleRequest},
    sdui::{StaticSduiState, sdui_action_response},
    workspace::{
        WorkspaceError, WorkspaceState, open_existing_file_unlocked, open_selected_file_unlocked,
        reload_document_unlocked, save_document_unlocked,
    },
};
use crate::shell::file_browser::FileBrowserState;

/// Bounded, deduplicating runtime-diagnostic retention (Plan 060 T6, P1-8).
/// Consecutive duplicates collapse to one entry; past the capacity the oldest
/// entry drops and the drop count is retained for observability. Retention is
/// aligned with the snapshot publication cap so welcome/runtime snapshots
/// never grow past the frame budget.
#[derive(Debug, Default)]
pub(crate) struct RuntimeDiagnosticStore {
    entries: std::collections::VecDeque<RuntimeDiagnostic>,
    dropped: u64,
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
    client_id: ClientId,
}

impl Drop for ConnectionOutputSubscriptions {
    fn drop(&mut self) {
        self.parse_coordinator.unsubscribe_client(self.client_id);
        self.document_analysis.unsubscribe_client(self.client_id);
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
        | ClientMessage::CloseDocument { client_id, .. } => Some(*client_id),
        ClientMessage::CompletionRequest { request } => Some(request.client_id),
        ClientMessage::LanguageIntelligenceRequest { request } => Some(request.client_id),
        ClientMessage::SelectionQueryRequest { request } => Some(request.client_id),
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
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let cleanup_document = Arc::clone(&document);
    let cleanup_workspace = Arc::clone(&workspace);
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
        sdui,
        active_theme,
        runtime_diagnostics,
        runtime_generation,
        parse_coordinator,
        completion,
        document_analysis,
        language_intelligence,
        reload_server,
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
    let dropped_results = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    // Plan 060 T4 (P0-3): authorized per-connection subscriptions. Parse and
    // analysis payloads route only to documents this connection opened; the
    // guard withdraws every subscription on any exit path.
    let (mut parse_updates_rx, mut parse_diagnostics_rx) =
        parse_coordinator.subscribe_client(client_id);
    let mut analysis_rx = document_analysis.subscribe_client(client_id);
    let _subscriptions = ConnectionOutputSubscriptions {
        parse_coordinator: parse_coordinator.clone(),
        document_analysis: document_analysis.clone(),
        client_id,
    };
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
                &document,
                &behavior,
                &workspace,
                &sdui,
                &active_theme,
                &runtime_diagnostics,
                &runtime_generation,
                codec,
            )
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
            runtime_generation_id = runtime_state_updates.recv() => match runtime_generation_id {
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
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
                dispatch_edit_operation(
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
                dispatch_edit_operation(
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
                known_version: _,
            } => {
                // Plan 060 T4 (P0-2): resync returns full document text, so a
                // guessed workspace document id must fail closed instead of
                // leaking another connection's payload. The default document
                // is authorized at welcome.
                let authorized = {
                    let default_id = document.lock().await.document_id();
                    if document_id == default_id {
                        Some(Arc::clone(&document))
                    } else {
                        let workspace = workspace.lock().await;
                        match workspace.document_handle(document_id) {
                            Some(handle) if handle.lock().await.has_access(client_id) => {
                                Some(handle)
                            }
                            _ => None,
                        }
                    }
                };
                let Some(target_document) = authorized else {
                    let response = file_operation_failed(
                        crate::server::workspace::WorkspaceError::UnknownDocument { document_id },
                        None,
                        Some(document_id),
                    );
                    codec.write_server_message(&mut stream, &response).await?;
                    continue;
                };
                let response = {
                    let document = target_document.lock().await;
                    document.resync_snapshot_message_for_client(document_id, client_id)
                };
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::DecorationViewportRequest {
                client_id: _,
                document_id,
                document_version,
                byte_start,
                byte_end,
            } => {
                if byte_start > byte_end {
                    continue;
                }
                let (metadata, target_document) = {
                    let workspace = workspace.lock().await;
                    let Ok(metadata) = workspace.document_metadata(document_id, client_id).await
                    else {
                        continue;
                    };
                    let Some(target_document) = workspace.document_handle(document_id) else {
                        continue;
                    };
                    (metadata, target_document)
                };
                if metadata.version != document_version {
                    continue;
                }
                let text = target_document.lock().await.text();
                let runtime = runtime_generation.current().await;
                let Some((meta, policy)) = runtime
                    .service
                    .registered_native_syntax_handler(runtime.id, &metadata.path)
                else {
                    continue;
                };
                if let Err(diagnostic) = schedule_parse_window(
                    &parse_coordinator,
                    &metadata,
                    &text,
                    behavior.lock().await.version(),
                    &meta.package_prefix,
                    &meta.mode_id,
                    policy,
                    ParseByteRange::new(byte_start, byte_end),
                ) {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::RuntimeDiagnostic(diagnostic),
                        )
                        .await?;
                }
            }
            ClientMessage::OpenDocument {
                client_id,
                workspace_root_id,
                path,
            } => {
                let response =
                    open_document_response(&workspace, workspace_root_id, path, client_id).await;
                write_document_open_response(
                    &codec,
                    &mut stream,
                    response,
                    &behavior,
                    &runtime_generation,
                    &workspace,
                    &sdui,
                    &parse_coordinator,
                    &document_analysis,
                    client_id,
                )
                .await?;
            }
            ClientMessage::OpenSelectedFile {
                client_id,
                capability,
                selected_path,
            } => {
                let authorized = file_open_capabilities.consume(&capability);
                // Replenish one pending token regardless of outcome so a
                // legitimate client can retry or open another file.
                let replenish = ServerMessage::FileOpenCapabilityIssued {
                    token: file_open_capabilities.issue(),
                };
                if !authorized {
                    codec.write_server_message(&mut stream, &replenish).await?;
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::RuntimeDiagnostic(
                                RuntimeDiagnostic::error(
                                    "clay.client.selected_file_open.unauthorized",
                                    "OpenSelectedFile requires a valid server-issued file-open capability token.",
                                ),
                            ),
                        )
                        .await?;
                    continue;
                }
                let response =
                    open_selected_file_response(&workspace, selected_path, client_id).await;
                write_document_open_response(
                    &codec,
                    &mut stream,
                    response,
                    &behavior,
                    &runtime_generation,
                    &workspace,
                    &sdui,
                    &parse_coordinator,
                    &document_analysis,
                    client_id,
                )
                .await?;
                codec.write_server_message(&mut stream, &replenish).await?;
            }
            ClientMessage::AddSelectedWorkspaceRoot {
                client_id,
                capability,
                selected_path,
            } => {
                let authorized = file_open_capabilities.consume(&capability);
                let replenish = ServerMessage::FileOpenCapabilityIssued {
                    token: file_open_capabilities.issue(),
                };
                if !authorized {
                    codec.write_server_message(&mut stream, &replenish).await?;
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
                                "clay.client.selected_folder_open.unauthorized",
                                "AddSelectedWorkspaceRoot requires a valid server-issued selected-path capability token.",
                            )),
                        )
                        .await?;
                    continue;
                }
                for message in add_selected_workspace_root_messages(
                    &workspace,
                    &document,
                    &sdui,
                    client_id,
                    selected_path,
                )
                .await
                {
                    codec.write_server_message(&mut stream, &message).await?;
                }
                codec.write_server_message(&mut stream, &replenish).await?;
            }
            ClientMessage::SaveDocument {
                client_id: _,
                document_id,
                known_version,
            } => {
                let response =
                    save_document_response(&workspace, document_id, client_id, known_version).await;
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::ReloadDocument {
                client_id: _,
                document_id,
                known_version: _,
                force,
            } => {
                let response =
                    reload_document_response(&workspace, document_id, client_id, force).await;
                codec.write_server_message(&mut stream, &response).await?;
                if let ServerMessage::DocumentReloaded { metadata, text } = response {
                    completion.document_changed(document_id, metadata.version);
                    language_intelligence.document_changed(document_id, metadata.version);
                    document_analysis.reset_document(document_id, metadata.version, text);
                }
            }
            ClientMessage::CloseDocument {
                client_id,
                document_id,
                force,
            } => {
                let outcome = {
                    let mut workspace = workspace.lock().await;
                    workspace
                        .close_document(document_id, client_id, force)
                        .await
                };
                match outcome {
                    Ok(outcome) => {
                        // This connection's subscriptions end immediately; the
                        // document may stay alive for other connections.
                        parse_coordinator.unsubscribe_document(document_id, client_id);
                        document_analysis.unsubscribe_document(document_id, client_id);
                        if outcome.closed {
                            teardown_closed_document(
                                document_id,
                                outcome.version,
                                &parse_coordinator,
                                &completion,
                                &language_intelligence,
                                &document_analysis,
                            );
                        }
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::DocumentClosed {
                                    document_id,
                                    closed: outcome.closed,
                                },
                            )
                            .await?;
                    }
                    Err(error) => {
                        let response = file_operation_failed(error, None, Some(document_id));
                        codec.write_server_message(&mut stream, &response).await?;
                    }
                }
            }
            ClientMessage::GetDocumentStatus {
                client_id,
                document_id,
            } => {
                let response = document_status_response(&workspace, document_id, client_id).await;
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::ListDocuments { client_id } => {
                let response = document_list_response(&workspace, client_id).await;
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::SduiAction {
                client_id: _,
                ui_version: _,
                intent,
            } => {
                let validation_response = {
                    let state = sdui.lock().await;
                    sdui_action_response(&state, &intent)
                };
                if let Some(response) = validation_response {
                    codec.write_server_message(&mut stream, &response).await?;
                    continue;
                }
                let response = execute_command_intent(
                    sdui_command_request(&intent),
                    Arc::clone(&workspace),
                    &document,
                    &sdui,
                    client_id,
                    reload_server.as_ref(),
                )
                .await;
                if let Some(response) = response {
                    write_document_open_response(
                        &codec,
                        &mut stream,
                        response,
                        &behavior,
                        &runtime_generation,
                        &workspace,
                        &sdui,
                        &parse_coordinator,
                        &document_analysis,
                        client_id,
                    )
                    .await?;
                }
            }
            ClientMessage::CommandIntent {
                client_id: _,
                document_id,
                behavior_version,
                command_id,
            } => {
                // Commands never receive previous-generation grace.
                if behavior.lock().await.version() != behavior_version {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::Error {
                                code: ProtocolErrorCode::InvalidMessage,
                                message: "command intent behavior version is stale".to_string(),
                            },
                        )
                        .await?;
                    continue;
                }
                let response = execute_command_intent(
                    CommandExecutionRequest {
                        command_id,
                        arguments: serde_json::Value::Null,
                        target: CommandExecutionTarget::ActiveDocument { document_id },
                        provenance: None,
                        expected_permissions: Vec::new(),
                    },
                    Arc::clone(&workspace),
                    &document,
                    &sdui,
                    client_id,
                    reload_server.as_ref(),
                )
                .await;
                if let Some(response) = response {
                    codec.write_server_message(&mut stream, &response).await?;
                }
            }
            ClientMessage::CompletionRequest { mut request } => {
                request.client_id = client_id;
                if let Err(rejection) = request.validate() {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::Error {
                                code: ProtocolErrorCode::InvalidMessage,
                                message: format!("completion request rejected: {rejection:?}"),
                            },
                        )
                        .await?;
                    continue;
                }
                let manifest_id = behavior.lock().await.manifest().manifest_id.clone();
                let package_prefix = manifest_id.split('.').next().unwrap_or("");
                let target_document =
                    document_for_message(request.document_id, &document, &workspace).await;
                let document_text = target_document.lock().await.text();
                let providers = runtime_generation
                    .current()
                    .await
                    .service
                    .completion_providers();
                let fallback = static_package_completion_result(
                    &request,
                    &manifest_id,
                    &document_text,
                    &providers,
                )
                .unwrap_or_else(|| CompletionResultSet {
                    request_id: request.request_id,
                    client_id: request.client_id,
                    document_id: request.document_id,
                    document_version: request.document_version,
                    behavior_version: request.behavior_version,
                    provider_generation: request.provider_generation,
                    replacement_range: request.replacement_range,
                    status: CompletionStatus::Empty,
                    items: Vec::new(),
                    provenance: CompletionProvenance::builtin_core(),
                });
                let analysis_provider_ids =
                    document_analysis.active_completion_provider_ids(request.document_id);
                let dynamic_provider = completion.providers().into_iter().find(|provider| {
                    (provider.provenance.package_prefix == package_prefix
                        || analysis_provider_ids.contains(&provider.id))
                        && match &request.trigger {
                            CompletionTrigger::Manual => true,
                            CompletionTrigger::Character(character) => provider
                                .trigger_metadata
                                .trigger_characters
                                .iter()
                                .any(|trigger| trigger == character),
                        }
                });
                if let Some(provider) = dynamic_provider {
                    request.provider_generation = provider.generation;
                    let window = completion_document_window(
                        &request,
                        &document_text,
                        &provider.provenance.package_prefix,
                    );
                    if let Ok(reply_rx) =
                        completion.schedule_completion(&provider.id, request.clone(), window)
                    {
                        let tx = completion_tx.clone();
                        let dropped = std::sync::Arc::clone(&dropped_results);
                        tokio::spawn(async move {
                            let message = match tokio::time::timeout(
                                std::time::Duration::from_millis(
                                    provider.timeout_ms.saturating_add(50),
                                ),
                                reply_rx,
                            )
                            .await
                            {
                                Ok(Ok(result)) => ServerMessage::CompletionResult { result },
                                // Provider timeout/failure/supersede: fall back
                                // to the static result so the client never waits
                                // on a dropped request-scoped reply.
                                _ => ServerMessage::CompletionResult { result: fallback },
                            };
                            if tx.try_send(message).is_err() {
                                let count = dropped
                                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                                    .saturating_add(1);
                                eprintln!(
                                    "clay server: completion result lane full; dropped {count} result(s)"
                                );
                            }
                        });
                        continue;
                    }
                }

                codec
                    .write_server_message(
                        &mut stream,
                        &ServerMessage::CompletionResult { result: fallback },
                    )
                    .await?;
            }
            ClientMessage::LanguageIntelligenceRequest { mut request } => {
                // Stamp the connection's client identity; ignore any client-supplied
                // client_id so results cannot be forged across clients.
                request.client_id = client_id;
                if let Err(rejection) = request.validate() {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::Error {
                                code: ProtocolErrorCode::InvalidMessage,
                                message: format!(
                                    "language-intelligence request rejected: {rejection:?}"
                                ),
                            },
                        )
                        .await?;
                    continue;
                }

                let target_document =
                    document_for_message(request.document_id, &document, &workspace).await;
                let document_text = target_document.lock().await.text();
                let window = language_intelligence_document_window_for_behavior(
                    &request,
                    &document_text,
                    &*behavior.lock().await,
                );
                match language_intelligence.schedule(None, request.clone(), window) {
                    Ok(reply_rx) => {
                        let tx = language_intelligence_tx.clone();
                        let dropped = std::sync::Arc::clone(&dropped_results);
                        tokio::spawn(async move {
                            match reply_rx.await {
                                Ok(result) => {
                                    if tx
                                        .try_send(ServerMessage::LanguageIntelligenceResult {
                                            result,
                                        })
                                        .is_err()
                                    {
                                        let count = dropped
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                                            .saturating_add(1);
                                        eprintln!(
                                            "clay server: language-intelligence result lane full; dropped {count} result(s)"
                                        );
                                    }
                                }
                                Err(_canceled) => {
                                    // Stale/canceled work drops silently so a newer
                                    // cursor/edit request can replace it without a
                                    // late empty/error flash.
                                }
                            }
                        });
                    }
                    Err(LanguageIntelligenceCoordinatorError::NoProviderForFeature) => {
                        let empty_payload = empty_language_intelligence_payload(request.feature);
                        let result = LanguageIntelligenceResult {
                            request_id: request.request_id,
                            client_id: request.client_id,
                            document_id: request.document_id,
                            document_version: request.document_version,
                            behavior_version: request.behavior_version,
                            provider_generation: request.provider_generation,
                            feature: request.feature,
                            status: LanguageIntelligenceStatus::Empty,
                            payload: empty_payload,
                            provenance: CompletionProvenance::builtin_core(),
                        };
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::LanguageIntelligenceResult { result },
                            )
                            .await?;
                    }
                    Err(LanguageIntelligenceCoordinatorError::OutstandingRequestLimit {
                        ..
                    }) => {
                        let result = LanguageIntelligenceResult {
                            request_id: request.request_id,
                            client_id: request.client_id,
                            document_id: request.document_id,
                            document_version: request.document_version,
                            behavior_version: request.behavior_version,
                            provider_generation: request.provider_generation,
                            feature: request.feature,
                            status: LanguageIntelligenceStatus::ProviderError,
                            payload: empty_language_intelligence_payload(request.feature),
                            provenance: CompletionProvenance::builtin_core(),
                        };
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::LanguageIntelligenceResult { result },
                            )
                            .await?;
                    }
                    Err(error) => {
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::Error {
                                    code: ProtocolErrorCode::InvalidMessage,
                                    message: format!(
                                        "language-intelligence schedule rejected: {error}"
                                    ),
                                },
                            )
                            .await?;
                    }
                }
            }
            ClientMessage::RuntimeGenerationInstalled {
                client_id: ack_client_id,
                runtime_generation_id,
            } => {
                let _ = runtime_generation
                    .note_runtime_generation_installed(
                        ack_client_id,
                        client_id,
                        runtime_generation_id,
                    )
                    .await;
            }
            ClientMessage::SelectionQueryRequest { request } => {
                // Plan 071 task 10: read-only tree-sitter text-object/smart-
                // select ranges. Every miss (validation, no grammar, no parse
                // handler, timed-out parse) degrades to empty ranges so an
                // advisory selection query can never block editing.
                if let Err(rejection) = request.validate() {
                    codec
                        .write_server_message(
                            &mut stream,
                            &ServerMessage::Error {
                                code: ProtocolErrorCode::InvalidMessage,
                                message: format!("selection query request rejected: {rejection:?}"),
                            },
                        )
                        .await?;
                    continue;
                }
                let metadata = workspace
                    .lock()
                    .await
                    .document_metadata(request.document_id, client_id)
                    .await
                    .ok();
                let mut ranges: Vec<Option<SelectionQueryRange>> =
                    vec![None; request.selections.len()];
                if let Some(metadata) = metadata {
                    let document_text =
                        document_for_message(request.document_id, &document, &workspace)
                            .await
                            .lock()
                            .await
                            .text();
                    let runtime = runtime_generation.current().await;
                    if let Some((meta, _policy)) = runtime
                        .service
                        .registered_native_syntax_handler(runtime.id, &metadata.path)
                        && let Some(handler) =
                            parse_coordinator.handler_for(&meta.package_prefix, &meta.mode_id)
                        && let Some(query_ranges) = handler.selection_query_ranges(
                            request.document_id,
                            request.document_version,
                            &document_text,
                            request.query,
                            &request.selections,
                        )
                    {
                        ranges = query_ranges;
                    }
                }
                codec
                    .write_server_message(
                        &mut stream,
                        &ServerMessage::SelectionQueryResult {
                            result: SelectionQueryResult {
                                request_id: request.request_id,
                                client_id,
                                document_id: request.document_id,
                                document_version: request.document_version,
                                behavior_version: request.behavior_version,
                                ranges,
                            },
                        },
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
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared open orchestration keeps server-owned state explicit instead of hiding authority in a context bag"
)]
async fn write_document_open_response<S>(
    codec: &Codec,
    stream: &mut S,
    response: ServerMessage,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    parse_coordinator: &ParseCoordinator,
    document_analysis: &crate::server::document_analysis::DocumentAnalysisCoordinator,
    client_id: ClientId,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    codec.write_server_message(stream, &response).await?;
    let ServerMessage::DocumentOpened { metadata, text } = &response else {
        return Ok(());
    };
    parse_coordinator.subscribe_document(metadata.document_id, client_id);
    document_analysis.subscribe_document(metadata.document_id, client_id);
    let runtime = runtime_generation.current().await;
    for message in open_document_followup_messages(
        metadata,
        text,
        behavior,
        sdui,
        runtime.id,
        &runtime.service,
        parse_coordinator,
    )
    .await
    {
        codec.write_server_message(stream, &message).await?;
    }
    for message in start_document_analysis(
        document_analysis,
        workspace,
        behavior,
        runtime.id,
        metadata,
        text,
    )
    .await
    {
        codec.write_server_message(stream, &message).await?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared edit/intent dispatch keeps server-owned state explicit instead of hiding authority in a context bag"
)]
async fn dispatch_edit_operation<S>(
    codec: Codec,
    stream: &mut S,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    completion: &crate::server::completion::CompletionCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
    document_analysis: &crate::server::document_analysis::DocumentAnalysisCoordinator,
    parse_coordinator: &ParseCoordinator,
    client_id: ClientId,
    document_id: DocumentId,
    lease_id: Option<crate::protocol::LeaseId>,
    base_version: crate::protocol::DocumentVersion,
    behavior_version: crate::protocol::BehaviorVersion,
    transaction_id: crate::protocol::TransactionId,
    operation: crate::protocol::EditOperation,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let behavior_decision = match validate_edit_behavior_version(
        behavior,
        runtime_generation,
        client_id,
        document_id,
        transaction_id,
        behavior_version,
    )
    .await
    {
        Ok(decision) => decision,
        Err(response) => {
            reject_invalid_behavior_version(
                &codec,
                stream,
                runtime_generation,
                client_id,
                response,
            )
            .await?;
            return Ok(());
        }
    };

    let target_document = document_for_message(document_id, document, workspace).await;
    let analysis_delta = document_analysis_delta(&operation);
    let (response, parse_input) = {
        let mut document = target_document.lock().await;
        document.apply_edit_with_parse_input(
            document_id,
            client_id,
            lease_id,
            base_version,
            transaction_id,
            operation,
        )
    };
    codec.write_server_message(stream, &response).await?;
    if let (
        ServerMessage::EditAck {
            confirmed_version, ..
        },
        Some(parse_input),
    ) = (response, parse_input)
    {
        if matches!(
            behavior_decision,
            BehaviorVersionDecision::PreviousWithinGrace
        ) {
            let _ = runtime_generation
                .behavior_grace()
                .record_previous_accepted(std::time::Instant::now())
                .await;
        }
        completion.document_changed(document_id, confirmed_version);
        language_intelligence.document_changed(document_id, confirmed_version);
        let (byte_start, byte_end, inserted_text) = analysis_delta;
        if document_analysis.change_document(
            document_id,
            base_version,
            confirmed_version,
            byte_start,
            byte_end,
            inserted_text,
        ) {
            let text = target_document.lock().await.text();
            document_analysis.reset_document(document_id, confirmed_version, text);
        }
        if let Err(diagnostic) = refresh_native_syntax_after_edit(
            workspace,
            behavior,
            runtime_generation,
            parse_coordinator,
            client_id,
            document_id,
            parse_input,
        )
        .await
        {
            codec
                .write_server_message(stream, &ServerMessage::RuntimeDiagnostic(diagnostic))
                .await?;
        }
    }
    Ok(())
}

async fn execute_command_intent(
    request: CommandExecutionRequest,
    workspace: Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
    reload_server: Option<&super::IpcServer>,
) -> Option<ServerMessage> {
    let executor = CommandExecutor::new();
    let registry = CommandRegistry::new();

    if crate::server::command_execution::is_settings_command(&request.command_id) {
        // Phase 20.6: settings intents validate, then persist + reload so the
        // change applies live through the canonical apply path (persist →
        // reload → init.js re-eval + preferences apply → RuntimeStateSnapshot
        // fanout). `setTheme`/`setAppearance` carry their value as
        // `arguments.item_id`; `setTypography` has no value payload yet (free-
        // form textInput value carriage is a follow-up protocol task), so it
        // validates and acknowledges without persisting. `settings.reset`
        // clears the persisted preferences store.
        let validated = match executor.execute_settings(request.clone()) {
            Ok(result) => result,
            Err(error) => {
                return Some(ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: format!(
                        "command execution rejected: {:?}: {}",
                        error.rule, error.message
                    ),
                });
            }
        };
        if let Some(server) = reload_server {
            match persist_settings_change(server, &validated.command_id, &request.arguments).await {
                Ok(PersistOutcome::Reloaded(outcome)) => {
                    if !outcome.reloaded {
                        return outcome
                            .diagnostics
                            .into_iter()
                            .next()
                            .map(ServerMessage::RuntimeDiagnostic);
                    }
                }
                Ok(PersistOutcome::Acknowledged) => {}
                Err(message) => {
                    return Some(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    });
                }
            }
        }
        return None;
    }

    if crate::server::command_execution::is_reload_command(&request.command_id) {
        let Some(server) = reload_server else {
            return Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: "runtime reload service is unavailable".to_string(),
            });
        };
        return match server.execute_reload_command(request).await {
            Ok(outcome) if outcome.reloaded => Some(ServerMessage::RuntimeDiagnostic(
                crate::protocol::RuntimeDiagnostic {
                    severity: crate::protocol::DiagnosticSeverity::Info,
                    code: "clay.runtime.reload_succeeded".to_string(),
                    message: format!(
                        "Runtime configuration reloaded as generation {}.",
                        outcome.active_generation_id
                    ),
                },
            )),
            Ok(outcome) => outcome
                .diagnostics
                .into_iter()
                .next()
                .map(ServerMessage::RuntimeDiagnostic),
            Err(error) => Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!(
                    "command execution rejected: {:?}: {}",
                    error.rule, error.message
                ),
            }),
        };
    }

    if crate::server::command_execution::is_workspace_command(&request.command_id) {
        let result = {
            let mut workspace_guard = workspace.lock().await;
            executor
                .execute_workspace(&registry, &mut workspace_guard, client_id, request)
                .await
        };
        match result {
            Ok(result) => {
                workspace_command_result_message(result, &workspace, document, sdui, client_id)
                    .await
            }
            Err(error) => Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!(
                    "command execution rejected: {:?}: {}",
                    error.rule, error.message
                ),
            }),
        }
    } else {
        executor
            .execute(&registry, request)
            .err()
            .map(|error| ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!(
                    "command execution rejected: {:?}: {}",
                    error.rule, error.message
                ),
            })
    }
}

async fn workspace_command_result_message(
    result: crate::server::command_execution::CommandExecutionResult,
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
) -> Option<ServerMessage> {
    use crate::server::command_execution::{CommandExecutionStatus, WorkspaceActionResult};
    match result.status {
        CommandExecutionStatus::Workspace(WorkspaceActionResult::Opened(snapshot)) => {
            Some(ServerMessage::DocumentOpened {
                metadata: snapshot.metadata,
                text: snapshot.text,
            })
        }
        CommandExecutionStatus::Workspace(WorkspaceActionResult::Navigated {
            root_id,
            relative_path,
        }) => Some(
            file_browser_snapshot_message(
                workspace,
                document,
                sdui,
                client_id,
                root_id,
                relative_path,
            )
            .await,
        ),
        _ => None,
    }
}

/// Outcome of persisting a settings command and (optionally) reloading.
enum PersistOutcome {
    /// Preference persisted and the runtime reloaded; the reload outcome is
    /// forwarded so the caller can surface any reload diagnostic.
    Reloaded(super::RuntimeReloadOutcome),
    /// Command acknowledged without persistence (e.g. `settings.open`,
    /// `settings.close`, `settings.setTypography` which has no value payload).
    Acknowledged,
}

/// Persist a settings command to `preferences.json` and trigger a runtime
/// reload so the change applies live through the canonical apply path. Returns
/// `Acknowledged` for commands that do not carry a persistable value.
/// `settings.reset` clears the store and reloads.
async fn persist_settings_change(
    server: &super::IpcServer,
    command_id: &str,
    arguments: &serde_json::Value,
) -> Result<PersistOutcome, String> {
    use crate::server::configuration::ConfigurationRuntime;
    let Some(config_root) = server.config.configuration_root.as_ref() else {
        return Err("settings persistence requires a configured configuration root".to_string());
    };
    let runtime = ConfigurationRuntime::from_config_root(config_root)
        .map_err(|error| format!("settings persistence root error: {error}"))?;
    let should_reload = match command_id {
        "settings.setTheme" => {
            let value = settings_value(arguments).ok_or_else(|| {
                "settings.setTheme requires an item_id/specifier argument".to_string()
            })?;
            runtime
                .persist_preference("theme", serde_json::Value::String(value))
                .map(|_| true)
                .map_err(|error| format!("settings.setTheme persistence failed: {error}"))?
        }
        "settings.setAppearance" => {
            let value = settings_value(arguments).ok_or_else(|| {
                "settings.setAppearance requires an item_id/appearance argument".to_string()
            })?;
            runtime
                .persist_preference("appearance", serde_json::Value::String(value))
                .map(|_| true)
                .map_err(|error| format!("settings.setAppearance persistence failed: {error}"))?
        }
        "settings.reset" => runtime
            .clear_preferences()
            .map(|_| true)
            .map_err(|error| format!("settings.reset failed: {error}"))?,
        // settings.open / settings.close / settings.setTypography: no
        // persistable value yet (setTypography free-form value carriage is a
        // follow-up protocol task). Acknowledge without reloading.
        "settings.open" | "settings.close" | "settings.setTypography" => false,
        _ => false,
    };
    if should_reload {
        let outcome = server.reload_runtime_generation().await;
        Ok(PersistOutcome::Reloaded(outcome))
    } else {
        Ok(PersistOutcome::Acknowledged)
    }
}

fn settings_value(arguments: &serde_json::Value) -> Option<String> {
    arguments
        .as_object()
        .and_then(|object| {
            object
                .get("item_id")
                .or_else(|| object.get("specifier"))
                .or_else(|| object.get("appearance"))
        })
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

fn sdui_command_request(intent: &SduiActionIntent) -> CommandExecutionRequest {
    CommandExecutionRequest {
        command_id: intent.command_id.clone(),
        arguments: sdui_action_arguments_json(&intent.arguments, &intent.source),
        target: CommandExecutionTarget::Global,
        provenance: None,
        expected_permissions: Vec::new(),
    }
}

/// Phase 20.6: forward the originating `SduiActionSource` so command handlers
/// receive the selected list/dropdown item id (`arguments.item_id`) or the
/// originating node id (`arguments.node_id`). Package component declarations
/// carry no argument data, so without this the choice value never reaches the
/// handler. Additive: handlers that ignore `arguments` are unaffected.
fn sdui_action_arguments_json(
    arguments: &[SduiActionArgument],
    source: &SduiActionSource,
) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for argument in arguments {
        object.insert(
            argument.name.clone(),
            sdui_action_value_json(&argument.value),
        );
    }
    match source {
        SduiActionSource::ListItem { item_id, .. } => {
            object
                .entry("item_id".to_string())
                .or_insert_with(|| serde_json::Value::String(item_id.clone()));
        }
        SduiActionSource::Button { node_id } => {
            object
                .entry("node_id".to_string())
                .or_insert_with(|| serde_json::Value::String(node_id.0.to_string()));
        }
    }
    serde_json::Value::Object(object)
}

fn sdui_action_value_json(value: &SduiActionValue) -> serde_json::Value {
    match value {
        SduiActionValue::String(value) => serde_json::Value::String(value.clone()),
        SduiActionValue::Bool(value) => serde_json::Value::Bool(*value),
        SduiActionValue::I64(value) => serde_json::Value::Number((*value).into()),
        SduiActionValue::U64(value) => serde_json::Value::Number((*value).into()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn send_welcome_snapshot_and_manifest<S>(
    stream: &mut S,
    client_id: u64,
    document: &Arc<Mutex<DocumentState>>,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    active_theme: &Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: &Arc<Mutex<RuntimeDiagnosticStore>>,
    runtime_generation: &RuntimeGenerationStore,
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

    let initial_document = {
        let mut document = document.lock().await;
        let access = document.acquire_access(client_id);
        document.initial_document_message(access)
    };
    codec
        .write_server_message(stream, &initial_document)
        .await?;

    let manifest_message = behavior.lock().await.manifest_message();
    codec
        .write_server_message(stream, &manifest_message)
        .await?;

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

    let (document_id, document_version) = match &initial_document {
        ServerMessage::InitialDocument {
            document_id,
            version,
            ..
        } => (*document_id, *version),
        _ => (0, 0),
    };

    let file_browser_tree = {
        let workspace = workspace.lock().await;
        let roots = workspace.list_root_metadata();
        roots.first().and_then(|root| {
            let browser =
                FileBrowserState::from_workspace(&workspace, root.workspace_root_id).ok()?;
            Some(browser.to_sdui_tree(document_id, document_version))
        })
    };

    if let Some(tree) = file_browser_tree {
        let mut state = sdui.lock().await;
        let _ = state.replace_for_document_with_runtime_tree(document_id, tree.clone());
        codec
            .write_server_message(stream, &ServerMessage::SduiSnapshot { client_id, tree })
            .await?;
    } else {
        let sdui_snapshot = sdui.lock().await.snapshot_message(client_id);
        if let Some(sdui_snapshot) = sdui_snapshot {
            codec.write_server_message(stream, &sdui_snapshot).await?;
        }
    }

    let diagnostics = runtime_diagnostics.lock().await.snapshot();
    for diagnostic in diagnostics {
        codec
            .write_server_message(stream, &ServerMessage::RuntimeDiagnostic(diagnostic))
            .await?;
    }

    Ok(())
}

async fn validate_edit_behavior_version(
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    client_id: ClientId,
    document_id: DocumentId,
    transaction_id: crate::protocol::TransactionId,
    behavior_version: crate::protocol::BehaviorVersion,
) -> Result<BehaviorVersionDecision, ServerMessage> {
    let current = behavior.lock().await.clone();
    let current_runtime_generation = runtime_generation.generation_id().await;
    let acknowledged_generation = runtime_generation
        .acknowledged_runtime_generation(client_id)
        .await;
    runtime_generation
        .behavior_grace()
        .validate_edit_version(
            &current,
            client_id,
            document_id,
            transaction_id,
            behavior_version,
            current_runtime_generation,
            acknowledged_generation,
            std::time::Instant::now(),
        )
        .await
}

async fn reject_invalid_behavior_version<S>(
    codec: &Codec,
    stream: &mut S,
    runtime_generation: &RuntimeGenerationStore,
    client_id: ClientId,
    rejection: ServerMessage,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    codec.write_server_message(stream, &rejection).await?;
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

async fn document_for_message(
    document_id: DocumentId,
    default_document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
) -> Arc<Mutex<DocumentState>> {
    workspace
        .lock()
        .await
        .document_handle(document_id)
        .unwrap_or_else(|| Arc::clone(default_document))
}

fn document_analysis_delta(operation: &crate::protocol::EditOperation) -> (u64, u64, String) {
    match operation {
        crate::protocol::EditOperation::Insert { byte_offset, text } => {
            (*byte_offset, *byte_offset, text.clone())
        }
        crate::protocol::EditOperation::Delete { start, end } => (*start, *end, String::new()),
        crate::protocol::EditOperation::Replace { start, end, text } => {
            (*start, *end, text.clone())
        }
    }
}

async fn start_document_analysis(
    coordinator: &crate::server::document_analysis::DocumentAnalysisCoordinator,
    workspace: &Arc<Mutex<WorkspaceState>>,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    generation: u64,
    metadata: &DocumentMetadata,
    text: &str,
) -> Vec<ServerMessage> {
    let canonical_root = workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .find(|root| root.workspace_root_id == metadata.workspace_root_id)
        .map(|root| root.canonical_path);
    let Some(canonical_root) = canonical_root else {
        return Vec::new();
    };
    let manifest_id = behavior.lock().await.manifest().manifest_id.clone();
    let active_mode = manifest_id.rsplit('.').next().unwrap_or(&manifest_id);
    coordinator
        .open_document(
            generation,
            metadata,
            active_mode,
            canonical_root,
            text.to_string(),
        )
        .into_iter()
        .map(ServerMessage::RuntimeDiagnostic)
        .collect()
}

/// Release every access grant the connection holds (disconnect) and tear down
/// document-scoped coordinator state for documents whose final holder left
/// (Plan 060 T6, P1-4). Documents still held by other connections keep their
/// analysis routes, versions, and provider state.
#[allow(
    clippy::too_many_arguments,
    reason = "disconnect teardown needs every document-scoped coordinator explicitly"
)]
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

async fn open_document_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    workspace_root_id: WorkspaceRootId,
    path: String,
    client_id: ClientId,
) -> ServerMessage {
    let opened =
        match open_existing_file_unlocked(workspace, workspace_root_id, &path, client_id).await {
            Ok(opened) => opened,
            Err(error) => return file_operation_failed(error, Some(workspace_root_id), None),
        };

    let document = opened.document.lock().await;
    let metadata = DocumentMetadata {
        document_id: opened.document_id,
        version: document.version(),
        lease_id: opened.access.lease_id(),
        access: opened.access,
        dirty: document.is_dirty(),
        workspace_root_id,
        path: opened.file_state.display_path(),
    };
    ServerMessage::DocumentOpened {
        metadata,
        text: document.text(),
    }
}

async fn add_selected_workspace_root_messages(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
    selected_path: String,
) -> Vec<ServerMessage> {
    let root_id = {
        let mut workspace = workspace.lock().await;
        match workspace.add_root(PathBuf::from(&selected_path)) {
            Ok(root_id) => root_id,
            Err(error) => return vec![file_operation_failed(error, None, None)],
        }
    };
    vec![
        file_browser_snapshot_message(
            workspace,
            document,
            sdui,
            client_id,
            root_id,
            PathBuf::new(),
        )
        .await,
    ]
}

async fn file_browser_snapshot_message(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
    root_id: WorkspaceRootId,
    relative_path: PathBuf,
) -> ServerMessage {
    let (document_id, document_version) = {
        let document = document.lock().await;
        (document.document_id(), document.version())
    };
    let tree = {
        let workspace = workspace.lock().await;
        match FileBrowserState::from_workspace_at(&workspace, root_id, relative_path) {
            Ok(browser) => browser.to_sdui_tree(document_id, document_version),
            Err(crate::shell::file_browser::FileBrowserError::Workspace(error)) => {
                return file_operation_failed(error, Some(root_id), None);
            }
            Err(crate::shell::file_browser::FileBrowserError::UnknownRoot(root_id)) => {
                return file_operation_failed(
                    WorkspaceError::UnknownRoot { root_id },
                    Some(root_id),
                    None,
                );
            }
        }
    };
    let _ = sdui
        .lock()
        .await
        .replace_for_document_with_runtime_tree(document_id, tree.clone());
    ServerMessage::SduiSnapshot { client_id, tree }
}

async fn open_selected_file_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    selected_path: String,
    client_id: ClientId,
) -> ServerMessage {
    let opened = match open_selected_file_unlocked(
        workspace,
        std::path::PathBuf::from(&selected_path),
        client_id,
    )
    .await
    {
        Ok(opened) => opened,
        Err(error) => return file_operation_failed(error, None, None),
    };

    let document = opened.document.lock().await;
    let metadata = DocumentMetadata {
        document_id: opened.document_id,
        version: document.version(),
        lease_id: opened.access.lease_id(),
        access: opened.access,
        dirty: document.is_dirty(),
        workspace_root_id: opened.file_state.workspace_root_id(),
        path: opened.file_state.display_path(),
    };
    ServerMessage::DocumentOpened {
        metadata,
        text: document.text(),
    }
}

async fn save_document_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
    known_version: crate::protocol::DocumentVersion,
) -> ServerMessage {
    match save_document_unlocked(workspace, document_id, client_id, known_version).await {
        Ok(outcome) => ServerMessage::DocumentSaved {
            document_id: outcome.document_id,
            version: outcome.version,
            dirty: outcome.dirty,
        },
        Err(error) => file_operation_failed(error, None, Some(document_id)),
    }
}

async fn reload_document_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
    force: bool,
) -> ServerMessage {
    let outcome = match reload_document_unlocked(workspace, document_id, client_id, force).await {
        Ok(outcome) => outcome,
        Err(error) => return file_operation_failed(error, None, Some(document_id)),
    };
    match workspace
        .lock()
        .await
        .document_metadata(document_id, client_id)
        .await
    {
        Ok(metadata) => ServerMessage::DocumentReloaded {
            metadata,
            text: outcome.text,
        },
        Err(error) => file_operation_failed(error, None, Some(document_id)),
    }
}

fn empty_language_intelligence_payload(
    feature: LanguageIntelligenceFeature,
) -> LanguageIntelligencePayload {
    match feature {
        LanguageIntelligenceFeature::Hover => {
            LanguageIntelligencePayload::Hover(crate::protocol::HoverResult {
                range: None,
                markdown: String::new(),
            })
        }
        LanguageIntelligenceFeature::GoToDefinition => {
            LanguageIntelligencePayload::GoToDefinition(crate::protocol::GoToDefinitionResult {
                locations: Vec::new(),
            })
        }
        LanguageIntelligenceFeature::CodeAction => {
            LanguageIntelligencePayload::CodeAction(crate::protocol::CodeActionResult {
                actions: Vec::new(),
            })
        }
        LanguageIntelligenceFeature::SignatureHelp => {
            LanguageIntelligencePayload::SignatureHelp(crate::protocol::SignatureHelpResult {
                signatures: Vec::new(),
                active_signature: None,
                active_parameter: None,
            })
        }
    }
}

fn completion_document_window(
    request: &CompletionRequest,
    text: &str,
    package_prefix: &str,
) -> crate::server::completion::CompletionDocumentWindow {
    const WINDOW_BYTES: usize = 64 * 1024;
    let cursor = (request.cursor_byte_offset as usize).min(text.len());
    let mut start = cursor.saturating_sub(WINDOW_BYTES / 2);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + WINDOW_BYTES).min(text.len());
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    crate::server::completion::CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: package_prefix.to_string(),
        byte_start: start as u64,
        byte_end: end as u64,
        text: text[start..end].to_string(),
    }
}

fn language_intelligence_document_window_for_behavior(
    request: &crate::protocol::LanguageIntelligenceRequest,
    text: &str,
    behavior: &ActiveBehaviorManifest,
) -> LanguageIntelligenceDocumentWindow {
    let manifest_id = &behavior.manifest().manifest_id;
    language_intelligence_document_window(
        request,
        text,
        manifest_id.rsplit('.').next().unwrap_or(manifest_id),
    )
}

fn language_intelligence_document_window(
    request: &crate::protocol::LanguageIntelligenceRequest,
    text: &str,
    active_mode: &str,
) -> LanguageIntelligenceDocumentWindow {
    use crate::perf::budgets::LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES;

    let cursor = (request.cursor_byte_offset as usize).min(text.len());
    let half = LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES / 2;
    let mut start = cursor.saturating_sub(half);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end < start {
        end = start;
    }

    LanguageIntelligenceDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        byte_start: start as u64,
        byte_end: end as u64,
        text: text[start..end].to_string(),
        active_mode: active_mode.to_string(),
    }
}

fn static_package_completion_result(
    request: &CompletionRequest,
    manifest_id: &str,
    document_text: &str,
    providers: &[CompletionProviderMeta],
) -> Option<CompletionResultSet> {
    let package_prefix = manifest_id.split('.').next()?;
    let mut matched: Vec<_> = providers
        .iter()
        .filter(|provider| {
            provider.provenance.package_prefix == package_prefix
                && match &request.trigger {
                    CompletionTrigger::Manual => true,
                    CompletionTrigger::Character(character) => provider
                        .trigger_metadata
                        .trigger_characters
                        .iter()
                        .any(|trigger| trigger == character),
                }
        })
        .collect();
    matched.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
    apply_exclusive_suppression(&mut matched);
    let provenance = matched.first()?.provenance.clone();
    let start = usize::try_from(request.replacement_range.byte_start).ok()?;
    let end = usize::try_from(request.replacement_range.byte_end).ok()?;
    let prefix = document_text.get(start..end)?;
    let mut result = CompletionResultSet {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        replacement_range: request.replacement_range,
        status: CompletionStatus::Empty,
        items: Vec::new(),
        provenance,
    };

    'providers: for provider in matched {
        for item in provider
            .items
            .iter()
            .filter(|item| item.insert_text.starts_with(prefix))
            .take(provider.max_items)
        {
            let mut candidate = result.clone();
            candidate.items.push(item.clone());
            if candidate.items.len() > COMPLETION_RESULT_MAX_ITEMS
                || estimated_result_payload_bytes(&candidate)
                    > COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES
            {
                break 'providers;
            }
            result.items.push(item.clone());
        }
    }
    if !result.items.is_empty() {
        result.status = CompletionStatus::Ok;
    }
    Some(result)
}

async fn document_status_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
) -> ServerMessage {
    match workspace
        .lock()
        .await
        .document_metadata(document_id, client_id)
        .await
    {
        Ok(metadata) => ServerMessage::DocumentStatus { metadata },
        Err(error) => file_operation_failed(error, None, Some(document_id)),
    }
}

async fn document_list_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    client_id: ClientId,
) -> ServerMessage {
    match workspace.lock().await.list_documents(client_id).await {
        Ok(documents) => ServerMessage::DocumentList { documents },
        Err(error) => file_operation_failed(error, None, None),
    }
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

#[allow(
    clippy::too_many_arguments,
    reason = "shared open-document/reload follow-up primitive keeps server-owned state explicit"
)]
pub(crate) async fn open_document_followup_messages(
    metadata: &DocumentMetadata,
    text: &str,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    generation_id: u64,
    js_runtime: &ClayJsRuntimeService,
    parse_coordinator: &ParseCoordinator,
) -> Vec<ServerMessage> {
    let Some(activation) = classify_open_document(
        generation_id,
        js_runtime,
        parse_coordinator,
        metadata,
        text,
        behavior,
        sdui,
    )
    .await
    else {
        return vec![behavior.lock().await.manifest_message()];
    };

    let mut messages = vec![behavior.lock().await.manifest_message()];
    match schedule_open_parse(parse_coordinator, metadata, text, behavior, &activation).await {
        Ok(Some(set)) => messages.push(ServerMessage::DecorationSet(set)),
        Ok(None) => {}
        Err(diagnostic) => messages.push(ServerMessage::RuntimeDiagnostic(diagnostic)),
    }

    messages
}

#[derive(Debug)]
struct OpenModeActivation {
    package_prefix: String,
    mode_id: String,
    parse_handler_mode_id: String,
    native_parse_policy: Option<ParsePolicy>,
}

async fn classify_open_document(
    generation_id: u64,
    js_runtime: &ClayJsRuntimeService,
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    text: &str,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    _sdui: &Arc<Mutex<StaticSduiState>>,
) -> Option<OpenModeActivation> {
    // Supply the open path's bounded leading-content slice and shebang line so
    // server-owned classification probes can route scripts (shebang) and
    // magic-prefixed files (content probes). The slice is bounded to
    // MAX_LEADING_CONTENT_BYTES; `ModeRegistry::classify` rejects anything
    // larger, so probes never read unbounded content and no new filesystem
    // authority is introduced beyond the already-open document text.
    let shebang = text
        .lines()
        .next()
        .filter(|line| line.starts_with("#!"))
        .map(str::to_string);
    let leading_content =
        bounded_utf8_prefix(text, crate::packages::modes::MAX_LEADING_CONTENT_BYTES)
            .0
            .to_string();
    let shebang_json = serde_json::to_string(&shebang).unwrap_or_else(|_| "null".to_string());
    let leading_json =
        serde_json::to_string(&leading_content).unwrap_or_else(|_| "null".to_string());
    let source = format!(
        r#"
import {{ serverActivateClassifiedMode, serverClassifyDocument }} from "clay:modes";
import {{ loadPackage, serverListFirstPartyPackageSpecifiers }} from "clay:packages";
const input = {{ documentId: {}, path: {}, shebang: {}, leadingContent: {} }};
let classification = null;
try {{ classification = serverClassifyDocument(input); }} catch {{}}
// Built-in fallback modes (apiPrefix "core", e.g. core.text/core.code) are a
// last resort. Discard a built-in-only match so first-party packages still
// load and win precedence over the fallback, then only activate a real
// (non-built-in) classification below.
if (classification && classification.apiPrefix === "core") {{
  classification = null;
}}
if (!classification) {{
  for (const specifier of serverListFirstPartyPackageSpecifiers()) {{
    try {{
      await loadPackage(specifier);
      classification = serverClassifyDocument(input);
      if (classification && classification.apiPrefix !== "core") break;
    }} catch {{}}
  }}
}}
if (classification && classification.apiPrefix === "core") {{
  classification = null;
}}
if (classification) {{
  serverActivateClassifiedMode(classification, input);
}}
Deno.core.ops.op_clay_runtime_record(JSON.stringify(classification));
"#,
        metadata.document_id,
        serde_json::to_string(&metadata.path).ok()?,
        shebang_json,
        leading_json,
    );
    let evaluation = js_runtime.evaluate_controlled_module(source).await.ok()?;
    super::apply_runtime_outputs_without_sdui(&evaluation, behavior).await;
    let record = evaluation.op_records.last()?;
    let value: serde_json::Value = serde_json::from_str(record).ok()?;
    let mut activation = OpenModeActivation {
        package_prefix: value.get("apiPrefix")?.as_str()?.to_string(),
        mode_id: value.get("modeId")?.as_str()?.to_string(),
        parse_handler_mode_id: value.get("modeId")?.as_str()?.to_string(),
        native_parse_policy: None,
    };
    if let Some((meta, policy)) = js_runtime
        .register_native_syntax_handler(
            parse_coordinator,
            generation_id,
            &evaluation,
            &metadata.path,
            &activation.package_prefix,
            &activation.mode_id,
        )
        .ok()
        .flatten()
    {
        activation.parse_handler_mode_id = meta.mode_id;
        activation.native_parse_policy = Some(policy);
    }
    // Tier 1 registers first. A same-generation JS handler remains available
    // only when no selected native handler owns this package/mode key.
    let _ = js_runtime.register_parse_handlers(parse_coordinator, generation_id, &evaluation);
    Some(activation)
}

async fn refresh_native_syntax_after_edit(
    workspace: &Arc<Mutex<WorkspaceState>>,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    parse_coordinator: &ParseCoordinator,
    client_id: ClientId,
    document_id: DocumentId,
    accepted_edit: ParseInputEdit,
) -> Result<(), RuntimeDiagnostic> {
    let (metadata, document) = {
        let workspace = workspace.lock().await;
        let Ok(metadata) = workspace.document_metadata(document_id, client_id).await else {
            return Ok(());
        };
        let Some(document) = workspace.document_handle(document_id) else {
            return Ok(());
        };
        (metadata, document)
    };
    let runtime = runtime_generation.current().await;
    let Some((meta, policy)) = runtime
        .service
        .registered_native_syntax_handler(runtime.id, &metadata.path)
    else {
        return Ok(());
    };
    let window = document
        .lock()
        .await
        .parse_window_after_edit(&meta.package_prefix, &meta.mode_id, policy, accepted_edit)
        .map_err(|message| {
            RuntimeDiagnostic::error(
                "clay.parse.window_failed",
                format!("Parse window failed: {message}"),
            )
        })?;
    let Some(window) = window else {
        return Ok(());
    };
    parse_coordinator.record_native_edit_accepted(metadata.document_id, metadata.version);
    let viewport = window.byte_range();
    schedule_parse_snapshot(
        parse_coordinator,
        &metadata,
        behavior.lock().await.version(),
        policy,
        window,
        viewport,
        Some(accepted_edit),
    )?;
    Ok(())
}

async fn schedule_open_parse(
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    text: &str,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    activation: &OpenModeActivation,
) -> Result<Option<crate::protocol::DecorationSet>, RuntimeDiagnostic> {
    let policy = activation.native_parse_policy.unwrap_or(ParsePolicy::new(
        64 * 1024,
        4 * 1024,
        30 * 1024 * 1024,
        5_000,
    ));
    schedule_parse_window(
        parse_coordinator,
        metadata,
        text,
        behavior.lock().await.version(),
        &activation.package_prefix,
        &activation.parse_handler_mode_id,
        policy,
        ParseByteRange::new(0, text.len() as u64),
    )
}

#[allow(clippy::too_many_arguments)]
fn schedule_parse_window(
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    text: &str,
    behavior_version: u64,
    package_prefix: &str,
    mode_id: &str,
    policy: ParsePolicy,
    requested: ParseByteRange,
) -> Result<Option<crate::protocol::DecorationSet>, RuntimeDiagnostic> {
    let text_len = text.len();
    let viewport_start = floor_char_boundary(text, requested.start.min(text_len as u64) as usize);
    let output_budget = policy
        .max_window_bytes
        .min(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES as u64);
    let viewport_end = floor_char_boundary(
        text,
        requested
            .end
            .max(viewport_start as u64)
            .min(text_len as u64)
            .min((viewport_start as u64).saturating_add(output_budget)) as usize,
    );
    if viewport_start >= viewport_end {
        return Ok(None);
    }
    let viewport = ParseByteRange::new(viewport_start as u64, viewport_end as u64);

    let (window_start, window_end) = if text_len as u64 <= policy.max_window_bytes {
        (0, text_len)
    } else {
        let guard_budget = policy.max_window_bytes.saturating_sub(viewport.len());
        let before = policy.guard_bytes.min(guard_budget / 2);
        let after = policy.guard_bytes.min(guard_budget.saturating_sub(before));
        let start = floor_char_boundary(text, viewport_start.saturating_sub(before as usize));
        let mut end = ceil_char_boundary(
            text,
            viewport_end.saturating_add(after as usize).min(text_len),
        );
        if end.saturating_sub(start) > policy.max_window_bytes as usize {
            end = floor_char_boundary(text, start.saturating_add(policy.max_window_bytes as usize));
        }
        (start, end)
    };

    let prefix = &text[..window_start];
    let base_line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u64;
    let base_column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, trailing)| trailing.len()) as u64;
    let window = ParseWindowSnapshot {
        document_id: metadata.document_id,
        document_version: metadata.version,
        package_prefix: package_prefix.to_string(),
        mode_id: mode_id.to_string(),
        window_id: window_start as u64,
        byte_start: window_start as u64,
        byte_end: window_end as u64,
        base_line,
        base_column,
        incremental_edit: false,
        text: text[window_start..window_end].to_string(),
    };

    schedule_parse_snapshot(
        parse_coordinator,
        metadata,
        behavior_version,
        policy,
        window,
        viewport,
        None,
    )
}

fn schedule_parse_snapshot(
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    behavior_version: u64,
    policy: ParsePolicy,
    window: ParseWindowSnapshot,
    viewport: ParseByteRange,
    accepted_edit: Option<ParseInputEdit>,
) -> Result<Option<crate::protocol::DecorationSet>, RuntimeDiagnostic> {
    let invalidated_ranges =
        accepted_edit.map_or_else(|| vec![viewport], |edit| vec![edited_range(edit, viewport)]);
    let request = ParseScheduleRequest {
        document_id: metadata.document_id,
        document_version: metadata.version,
        behavior_version,
        package_prefix: window.package_prefix.clone(),
        mode_id: window.mode_id.clone(),
        viewport,
        invalidated_ranges,
        accepted_edit,
    };
    match parse_coordinator.schedule_parse_with_windows(request, vec![window], Some(policy)) {
        Ok(_) | Err(ParseCoordinatorError::HandlerNotRegistered { .. }) => Ok(None),
        Err(error) => Err(RuntimeDiagnostic::error(
            "clay.parse.viewport_activation_failed",
            format!("Viewport parse scheduling failed: {error:?}"),
        )),
    }
}

fn floor_char_boundary(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset = offset.saturating_sub(1);
    }
    offset
}

fn ceil_char_boundary(text: &str, mut offset: usize) -> usize {
    while offset < text.len() && !text.is_char_boundary(offset) {
        offset += 1;
    }
    offset
}

fn edited_range(edit: ParseInputEdit, window: ParseByteRange) -> ParseByteRange {
    let start = edit.start_byte.clamp(window.start, window.end);
    let mut end = edit.new_end_byte.clamp(start, window.end);
    if start == end {
        if end < window.end {
            end += 1;
        } else if start > window.start {
            return ParseByteRange::new(start - 1, start);
        }
    }
    ParseByteRange::new(start, end)
}

fn bounded_utf8_prefix(text: &str, max_bytes: usize) -> (&str, u64) {
    if text.len() <= max_bytes {
        return (text, text.len() as u64);
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], end as u64)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc, time::SystemTime};

    use tokio::{
        io::duplex,
        sync::Mutex,
        time::{Duration, timeout},
    };

    use super::{
        RuntimeDiagnosticStore, execute_command_intent, handle_connection,
        language_intelligence_document_window_for_behavior, sdui_command_request,
        static_package_completion_result,
    };
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

        let window =
            language_intelligence_document_window_for_behavior(&request, "fn main() {}", &behavior);

        assert_eq!(window.active_mode, "rust");
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
    fn static_package_completion_merges_equal_priority_plain_and_snippet_providers() {
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
        };

        let result =
            static_package_completion_result(&request, "rust.rust", "fn", &providers).unwrap();

        assert_eq!(result.items.len(), 2);
        assert_eq!(
            result.items[1].text_format,
            crate::protocol::CompletionItemTextFormat::Snippet
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
await loadPackage("@clay/markdown");"#,
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
            "clay.controlCenter.open",
            SduiActionSource::Button {
                node_id: SduiNodeId(5),
            },
        ));
        let keybinding_request = CommandExecutionRequest {
            command_id: "clay.controlCenter.open".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
            provenance: None,
            expected_permissions: Vec::new(),
        };

        let document = document_state();
        let sdui = sdui_state();
        assert_eq!(
            execute_command_intent(sdui_request, workspace_state(), &document, &sdui, 1, None)
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
                command_id: "clay.runtime.reloadConfiguration".to_string(),
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
        )
        .await
        .expect("reload command returns status");

        assert!(matches!(
            response,
            ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. })
                if code == "clay.runtime.reload_succeeded"
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
            "clay.workspace.openDirectory",
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
        let (client, server) = duplex(4096);
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
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    /// Plan 060 T4 test helpers: drain the bootstrap sequence through the
    /// always-terminal capability issue so tests start from a clean cursor.
    async fn drain_bootstrap(client: &mut tokio::io::DuplexStream, codec: Codec) {
        loop {
            if matches!(
                codec.read_server_message(client).await.unwrap(),
                ServerMessage::FileOpenCapabilityIssued { .. }
            ) {
                break;
            }
        }
    }

    struct TestConnection {
        client: tokio::io::DuplexStream,
        server_task: tokio::task::JoinHandle<Result<(), crate::protocol::codec::CodecError>>,
        codec: Codec,
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
            let (client, server) = duplex(4096);
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
            drain_bootstrap(&mut client, codec).await;
            Self {
                client,
                server_task,
                codec,
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
                        "clay.controlCenter.open",
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
                    command_id: "clay.controlCenter.open".to_string(),
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
        let (client, server) = duplex(4096);
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
        let (client, server) = duplex(4096);
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
        let (client, server) = duplex(4096);
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
        let (client, server) = duplex(4096);
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
        let (client, server) = duplex(4096);
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
    async fn server_sends_runtime_diagnostics_after_bootstrap() {
        let (client, server) = duplex(4096);
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let diagnostics = Arc::new(Mutex::new(RuntimeDiagnosticStore::default()));
        diagnostics.lock().await.push(RuntimeDiagnostic::error(
            "clay.runtime.invalid_import",
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
            diagnostics,
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
                "clay.runtime.invalid_import",
                "Only clay:* facades and relative local configuration modules are allowed.",
            ))
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_acknowledges_insert_edit() {
        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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
        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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
        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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

        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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
                | ServerMessage::RuntimeDiagnostic(_) => {}
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
                ServerMessage::DiagnosticSet(_) | ServerMessage::RuntimeDiagnostic(_) => {}
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
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::DocumentStatus {
                metadata: DocumentMetadata {
                    document_id: 1,
                    version: 2,
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    dirty: true,
                    workspace_root_id: root_id,
                    path: "main.rs".to_string(),
                },
            }
        );

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
        let tree = match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => tree,
            message => panic!("expected file browser SduiSnapshot, got {message:?}"),
        };
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
                | ServerMessage::RuntimeDiagnostic(_) => {}
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
        for _ in 0..6 {
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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
        assert!(matches!(
            &messages[0],
            ServerMessage::BehaviorManifest(manifest)
                if manifest.manifest_id == "markdown.markdown"
                    && matches!(manifest.scope, BehaviorScope::Document { document_id: 2 })
        ));
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
                super::ParseByteRange::new(0, text.len() as u64),
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
                super::ParseByteRange::new(start, text.len() as u64),
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
        let source = include_str!("connection.rs");
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

        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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

        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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

        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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
                if diagnostic.code == "clay.client.selected_folder_open.unauthorized"
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

        let (client, server) = duplex(4096);
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
        let _sdui = codec.read_server_message(&mut client).await.unwrap();
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
        for _ in 0..7 {
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

        let (client, server) = duplex(4096);
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
                assert_eq!(
                    diagnostic.code,
                    "clay.client.selected_file_open.unauthorized"
                );
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

    /// Runtime-diagnostic retention: consecutive duplicates collapse, the
    /// deque never exceeds its capacity, and drops are counted (Plan 060 T6,
    /// P1-8).
    #[test]
    fn runtime_diagnostic_store_deduplicates_and_bounds() {
        let mut store = RuntimeDiagnosticStore::default();
        let duplicate = RuntimeDiagnostic::warning("clay.test.dup", "same");
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
                "clay.test.flood",
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
            command_id: "clay.workspace.openFile".to_string(),
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
