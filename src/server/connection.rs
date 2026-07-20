use std::{
    collections::{HashMap, HashSet},
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
        SduiActionIntent, SduiActionValue, ServerMessage, WorkspaceRootId,
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
    runtime_diagnostics: Arc<Mutex<Vec<RuntimeDiagnostic>>>,
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
    mut stream: S,
    client_id: u64,
    document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    sdui: Arc<Mutex<StaticSduiState>>,
    active_theme: Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: Arc<Mutex<Vec<RuntimeDiagnostic>>>,
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
    let (completion_tx, mut completion_rx) =
        tokio::sync::mpsc::unbounded_channel::<ServerMessage>();
    let (language_intelligence_tx, mut language_intelligence_rx) =
        tokio::sync::mpsc::unbounded_channel::<ServerMessage>();
    let mut analysis_documents = HashMap::new();
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
            // ponytail: one connection drains shared parse channel. Desktop is
            // single-client; broadcast fan-out if multi-client parse delivery
            // becomes required.
            update = parse_coordinator.next_update() => {
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
            diagnostic = parse_coordinator.next_diagnostic() => {
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
            output = document_analysis.next_output() => {
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
            None => {
                close_analysis_documents(&document_analysis, &analysis_documents);
                release_client_access(client_id, &document, &workspace).await;
                return Ok(());
            }
            Some(Err(CodecError::Io(error)))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                close_analysis_documents(&document_analysis, &analysis_documents);
                release_client_access(client_id, &document, &workspace).await;
                return Ok(());
            }
            Some(Err(error)) => {
                close_analysis_documents(&document_analysis, &analysis_documents);
                release_client_access(client_id, &document, &workspace).await;
                return Err(error);
            }
        };

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
                let behavior_decision = match validate_edit_behavior_version(
                    &behavior,
                    &runtime_generation,
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
                            &mut stream,
                            &runtime_generation,
                            client_id,
                            response,
                        )
                        .await?;
                        continue;
                    }
                };

                let target_document =
                    document_for_message(document_id, &document, &workspace).await;
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
                codec.write_server_message(&mut stream, &response).await?;
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
                    analysis_documents.insert(document_id, confirmed_version);
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
                        &workspace,
                        &behavior,
                        &runtime_generation,
                        &parse_coordinator,
                        client_id,
                        document_id,
                        parse_input,
                    )
                    .await
                    {
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::RuntimeDiagnostic(diagnostic),
                            )
                            .await?;
                    }
                }
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
                let behavior_decision = match validate_edit_behavior_version(
                    &behavior,
                    &runtime_generation,
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
                            &mut stream,
                            &runtime_generation,
                            client_id,
                            response,
                        )
                        .await?;
                        continue;
                    }
                };

                let operation = match intent {
                    crate::protocol::EditorIntent::InsertText { byte_offset, text } => {
                        crate::protocol::EditOperation::Insert { byte_offset, text }
                    }
                    crate::protocol::EditorIntent::DeleteRange { start, end } => {
                        crate::protocol::EditOperation::Delete { start, end }
                    }
                };
                let target_document =
                    document_for_message(document_id, &document, &workspace).await;
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
                codec.write_server_message(&mut stream, &response).await?;
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
                    analysis_documents.insert(document_id, confirmed_version);
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
                        &workspace,
                        &behavior,
                        &runtime_generation,
                        &parse_coordinator,
                        client_id,
                        document_id,
                        parse_input,
                    )
                    .await
                    {
                        codec
                            .write_server_message(
                                &mut stream,
                                &ServerMessage::RuntimeDiagnostic(diagnostic),
                            )
                            .await?;
                    }
                }
            }
            ClientMessage::RequestResync {
                document_id,
                client_id,
                known_version: _,
            } => {
                let target_document =
                    document_for_message(document_id, &document, &workspace).await;
                let response = {
                    let document = target_document.lock().await;
                    document.resync_snapshot_message_for_client(document_id, client_id)
                };
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::DecorationViewportRequest {
                client_id: request_client_id,
                document_id,
                document_version,
                byte_start,
                byte_end,
            } => {
                if request_client_id != client_id || byte_start > byte_end {
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
                codec.write_server_message(&mut stream, &response).await?;
                if let ServerMessage::DocumentOpened { metadata, text } = &response {
                    let runtime = runtime_generation.current().await;
                    for message in open_document_followup_messages(
                        metadata,
                        text,
                        &behavior,
                        &sdui,
                        runtime.id,
                        &runtime.service,
                        &parse_coordinator,
                    )
                    .await
                    {
                        codec.write_server_message(&mut stream, &message).await?;
                    }
                    for message in start_document_analysis(
                        &document_analysis,
                        &workspace,
                        &behavior,
                        runtime.id,
                        metadata,
                        text,
                    )
                    .await
                    {
                        codec.write_server_message(&mut stream, &message).await?;
                    }
                    analysis_documents.insert(metadata.document_id, metadata.version);
                }
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
                codec.write_server_message(&mut stream, &response).await?;
                if let ServerMessage::DocumentOpened { metadata, text } = &response {
                    let runtime = runtime_generation.current().await;
                    let messages = open_document_followup_messages(
                        metadata,
                        text,
                        &behavior,
                        &sdui,
                        runtime.id,
                        &runtime.service,
                        &parse_coordinator,
                    )
                    .await;
                    for message in messages {
                        codec.write_server_message(&mut stream, &message).await?;
                    }
                    for message in start_document_analysis(
                        &document_analysis,
                        &workspace,
                        &behavior,
                        runtime.id,
                        metadata,
                        text,
                    )
                    .await
                    {
                        codec.write_server_message(&mut stream, &message).await?;
                    }
                    analysis_documents.insert(metadata.document_id, metadata.version);
                }
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
                known_version: _,
            } => {
                let response = save_document_response(&workspace, document_id).await;
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::ReloadDocument {
                client_id,
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
                    analysis_documents.insert(document_id, metadata.version);
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
                client_id: request_client_id,
                ui_version: _,
                intent,
            } => {
                if request_client_id != client_id {
                    continue;
                }
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
                    codec.write_server_message(&mut stream, &response).await?;
                    if let ServerMessage::DocumentOpened { metadata, text } = &response {
                        let runtime = runtime_generation.current().await;
                        for message in open_document_followup_messages(
                            metadata,
                            text,
                            &behavior,
                            &sdui,
                            runtime.id,
                            &runtime.service,
                            &parse_coordinator,
                        )
                        .await
                        {
                            codec.write_server_message(&mut stream, &message).await?;
                        }
                        for message in start_document_analysis(
                            &document_analysis,
                            &workspace,
                            &behavior,
                            runtime.id,
                            metadata,
                            text,
                        )
                        .await
                        {
                            codec.write_server_message(&mut stream, &message).await?;
                        }
                        analysis_documents.insert(metadata.document_id, metadata.version);
                    }
                }
            }
            ClientMessage::CommandIntent {
                client_id: request_client_id,
                document_id,
                behavior_version,
                command_id,
            } => {
                if request_client_id != client_id {
                    continue;
                }
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
                    if completion
                        .schedule_completion(&provider.id, request.clone(), window)
                        .is_ok()
                    {
                        let coordinator = completion.clone();
                        let tx = completion_tx.clone();
                        tokio::spawn(async move {
                            let message = match tokio::time::timeout(
                                std::time::Duration::from_millis(
                                    provider.timeout_ms.saturating_add(50),
                                ),
                                coordinator.next_result(),
                            )
                            .await
                            {
                                Ok(Some(result)) => ServerMessage::CompletionResult { result },
                                _ => ServerMessage::CompletionResult { result: fallback },
                            };
                            let _ = tx.send(message);
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
                        tokio::spawn(async move {
                            match reply_rx.await {
                                Ok(result) => {
                                    let _ = tx
                                        .send(ServerMessage::LanguageIntelligenceResult { result });
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

fn sdui_command_request(intent: &SduiActionIntent) -> CommandExecutionRequest {
    CommandExecutionRequest {
        command_id: intent.command_id.clone(),
        arguments: sdui_action_arguments_json(&intent.arguments),
        target: CommandExecutionTarget::Global,
        provenance: None,
        expected_permissions: Vec::new(),
    }
}

fn sdui_action_arguments_json(arguments: &[SduiActionArgument]) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for argument in arguments {
        object.insert(
            argument.name.clone(),
            sdui_action_value_json(&argument.value),
        );
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
    runtime_diagnostics: &Arc<Mutex<Vec<RuntimeDiagnostic>>>,
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

    let diagnostics = runtime_diagnostics.lock().await.clone();
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

fn close_analysis_documents(
    coordinator: &crate::server::document_analysis::DocumentAnalysisCoordinator,
    documents: &HashMap<DocumentId, crate::protocol::DocumentVersion>,
) {
    for (&document_id, &version) in documents {
        coordinator.close_document(document_id, version);
    }
}

async fn release_client_access(
    client_id: ClientId,
    default_document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
) {
    default_document.lock().await.release_access(client_id);
    workspace
        .lock()
        .await
        .release_client_access(client_id)
        .await;
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
) -> ServerMessage {
    match save_document_unlocked(workspace, document_id).await {
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
    let outcome = match reload_document_unlocked(workspace, document_id, force).await {
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
        execute_command_intent, handle_connection,
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

    fn runtime_diagnostics() -> Arc<Mutex<Vec<RuntimeDiagnostic>>> {
        Arc::new(Mutex::new(Vec::new()))
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
            ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1))
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
        let diagnostics = Arc::new(Mutex::new(vec![RuntimeDiagnostic::error(
            "clay.runtime.invalid_import",
            "Only clay:* facades and relative local configuration modules are allowed.",
        )]));
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
}
