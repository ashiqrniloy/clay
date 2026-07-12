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
    protocol::{
        ClientId, ClientMessage, CompletionProvenance, CompletionResultSet, CompletionStatus,
        DocumentId, DocumentMetadata, PROTOCOL_VERSION, ParseByteRange, ParsePolicy,
        ParseWindowSnapshot, ProtocolErrorCode, RuntimeDiagnostic, SduiActionArgument,
        SduiActionIntent, SduiActionValue, ServerMessage, WorkspaceRootId,
        codec::{Codec, CodecError},
    },
};

use super::{
    RuntimeGenerationStore,
    behavior::ActiveBehaviorManifest,
    command_execution::{CommandExecutionRequest, CommandExecutionTarget, CommandExecutor},
    document::DocumentState,
    js_runtime::ClayJsRuntimeService,
    parse_coordinator::{ParseCoordinator, ParseScheduleRequest},
    sdui::{StaticSduiState, sdui_action_response},
    workspace::{
        WorkspaceError, WorkspaceState, open_existing_file_unlocked, open_selected_file_unlocked,
        reload_document_unlocked, save_document_unlocked,
    },
};
use crate::shell::file_browser::FileBrowserState;

#[allow(
    clippy::too_many_arguments,
    reason = "connection handler receives server-owned state explicitly instead of hiding authority in a context bag"
)]
pub(crate) async fn handle_connection<S>(
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
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut typography_updates = runtime_generation.subscribe_typography();
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
            // ponytail: one connection drains shared parse channel. Desktop is
            // single-client; broadcast fan-out if multi-client parse delivery
            // becomes required.
            update = parse_coordinator.next_update() => {
                if let Some(update) = update {
                    if let Some(set) = update.decoration_update {
                        codec
                            .write_server_message(&mut stream, &ServerMessage::DecorationSet(set))
                            .await?;
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
            message = codec.read_client_message(&mut stream) => message,
        } {
            Ok(message) => message,
            Err(CodecError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                release_client_access(client_id, &document, &workspace).await;
                return Ok(());
            }
            Err(error) => {
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
                if let Err(response) = behavior.lock().await.validate_message_version(
                    document_id,
                    transaction_id,
                    behavior_version,
                ) {
                    codec.write_server_message(&mut stream, &response).await?;
                    continue;
                }

                let target_document =
                    document_for_message(document_id, &document, &workspace).await;
                let response = {
                    let mut document = target_document.lock().await;
                    document.apply_edit(
                        document_id,
                        client_id,
                        lease_id,
                        base_version,
                        transaction_id,
                        operation,
                    )
                };
                codec.write_server_message(&mut stream, &response).await?;
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
                if let Err(response) = behavior.lock().await.validate_message_version(
                    document_id,
                    transaction_id,
                    behavior_version,
                ) {
                    codec.write_server_message(&mut stream, &response).await?;
                    continue;
                }

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
                let response = {
                    let mut document = target_document.lock().await;
                    document.apply_edit(
                        document_id,
                        client_id,
                        lease_id,
                        base_version,
                        transaction_id,
                        operation,
                    )
                };
                codec.write_server_message(&mut stream, &response).await?;
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
                client_id,
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
                    Some(client_id),
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
                    }
                }
            }
            ClientMessage::CommandIntent {
                client_id: _,
                document_id,
                behavior_version,
                command_id,
            } => {
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
                    None,
                )
                .await;
                if let Some(response) = response {
                    codec.write_server_message(&mut stream, &response).await?;
                }
            }
            ClientMessage::CompletionRequest { request } => {
                // Phase 18.11 task 3 wires the protocol shapes only; the
                // server-side provider registry and cancellable UI-reactive
                // coordinator are task 4. Until then, acknowledge the request
                // with an empty, versioned result set so the protocol path is
                // type-correct and the codec round-trips without implementing
                // provider execution. No document mutation, no provider code.
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
                let empty = CompletionResultSet {
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
                };
                codec
                    .write_server_message(
                        &mut stream,
                        &ServerMessage::CompletionResult { result: empty },
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

async fn execute_command_intent(
    request: CommandExecutionRequest,
    workspace: Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: Option<ClientId>,
) -> Option<ServerMessage> {
    let executor = CommandExecutor::new();
    let registry = CommandRegistry::new();

    if crate::server::command_execution::is_workspace_command(&request.command_id) {
        let result = {
            let mut workspace_guard = workspace.lock().await;
            executor
                .execute_workspace(&registry, &mut workspace_guard, request)
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
    client_id: Option<ClientId>,
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
        }) => {
            let client_id = client_id?;
            Some(
                file_browser_snapshot_message(
                    workspace,
                    document,
                    sdui,
                    client_id,
                    root_id,
                    relative_path,
                )
                .await,
            )
        }
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
    let _ = js_runtime.register_parse_handlers(parse_coordinator, generation_id, &evaluation);
    super::apply_runtime_outputs_without_sdui(&evaluation, behavior).await;
    let record = evaluation.op_records.last()?;
    let value: serde_json::Value = serde_json::from_str(record).ok()?;
    Some(OpenModeActivation {
        package_prefix: value.get("apiPrefix")?.as_str()?.to_string(),
        mode_id: value.get("modeId")?.as_str()?.to_string(),
    })
}

async fn schedule_open_parse(
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    text: &str,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    activation: &OpenModeActivation,
) -> Result<Option<crate::protocol::DecorationSet>, RuntimeDiagnostic> {
    let (window_text, window_byte_end) = bounded_utf8_prefix(text, 64 * 1024);
    let behavior_version = behavior.lock().await.version();
    let request = ParseScheduleRequest {
        document_id: metadata.document_id,
        document_version: metadata.version,
        behavior_version,
        package_prefix: activation.package_prefix.clone(),
        mode_id: activation.mode_id.clone(),
        viewport: ParseByteRange::new(0, window_byte_end),
        invalidated_ranges: vec![ParseByteRange::new(0, window_byte_end)],
    };
    let windows = vec![ParseWindowSnapshot {
        document_id: metadata.document_id,
        document_version: metadata.version,
        package_prefix: activation.package_prefix.clone(),
        mode_id: activation.mode_id.clone(),
        byte_start: 0,
        byte_end: window_byte_end,
        base_line: 0,
        text: window_text.to_string(),
    }];
    parse_coordinator
        .schedule_parse_with_windows(
            request,
            windows,
            Some(ParsePolicy::new(
                64 * 1024,
                4 * 1024,
                30 * 1024 * 1024,
                5_000,
            )),
        )
        .map_err(|error| {
            RuntimeDiagnostic::error(
                "clay.parse.open_activation_failed",
                format!("Open-time parse scheduling failed: {error:?}"),
            )
        })?;

    Ok(None)
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

    use super::{execute_command_intent, handle_connection, sdui_command_request};
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

    fn runtime_generation() -> super::RuntimeGenerationStore {
        runtime_generation_from(js_runtime())
    }

    fn runtime_generation_from(runtime: ClayJsRuntimeService) -> super::RuntimeGenerationStore {
        super::RuntimeGenerationStore {
            current: Arc::new(Mutex::new(super::super::RuntimeGeneration {
                id: 1,
                service: runtime,
                diagnostics: Vec::new(),
            })),
            typography: super::super::ActiveTypographyState::default(),
        }
    }

    fn parse_coordinator() -> ParseCoordinator {
        ParseCoordinator::default()
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
            EditOperation, EditRejection, FileErrorCode, PROTOCOL_VERSION, RuntimeDiagnostic,
            SduiActionArgument, SduiActionIntent, SduiActionSource, SduiActionValue, SduiNodeId,
            SduiNodeKind, ServerMessage, TokenType, codec::Codec,
        },
        server::{
            behavior::ActiveBehaviorManifest, document::DocumentState,
            js_runtime::ClayJsRuntimeService, parse_coordinator::ParseCoordinator,
            sdui::StaticSduiState, workspace::WorkspaceState,
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
            execute_command_intent(sdui_request, workspace_state(), &document, &sdui, Some(1))
                .await,
            None
        );
        assert_eq!(
            execute_command_intent(
                keybinding_request,
                workspace_state(),
                &document,
                &sdui,
                None
            )
            .await,
            None
        );
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
            Some(1),
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
            Some(42),
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
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1))
        );

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 1,
                    client_id: 99,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 1,
                    transaction_id: 444,
                    operation: EditOperation::Insert {
                        byte_offset: 13,
                        text: "// ok\n".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::EditAck {
                document_id: 1,
                confirmed_version: 2,
                transaction_id: 444,
            }
        );

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

        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DocumentOpened { metadata, text } => {
                assert_eq!(metadata.document_id, 2);
                assert_eq!(metadata.workspace_root_id, root_id);
                assert_eq!(metadata.path, "note.md");
                assert_eq!(text, "# Browser note\n\n- item\n");
            }
            message => panic!("expected file-browser DocumentOpened, got {message:?}"),
        }
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => {
                assert_eq!(manifest.manifest_id, "markdown.markdown");
                assert!(matches!(
                    manifest.scope,
                    BehaviorScope::Document { document_id: 2 }
                ));
            }
            message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
        }
        // Open follow-up returns after scheduling parse. Background
        // DecorationSet/DiagnosticSet may arrive later; they must not have
        // blocked DocumentOpened or the behavior manifest.
        match timeout(
            Duration::from_millis(250),
            codec.read_server_message(&mut client),
        )
        .await
        {
            Err(_) => {}
            Ok(Ok(
                ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::RuntimeDiagnostic(_),
            )) => {}
            Ok(Ok(other)) => panic!("unexpected open follow-up message: {other:?}"),
            Ok(Err(error)) => panic!("unexpected codec error after open: {error}"),
        }

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(selected);
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
            .decoration_update
            .expect("background markdown decorations");
        assert_eq!(set.document_id, 2);
        assert!(
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::Heading1)
        );
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

        let update = timeout(Duration::from_secs(1), coordinator.next_update())
            .await
            .unwrap()
            .unwrap();
        let set = update.decoration_update.expect("background decorations");

        assert_eq!(set.document_id, 2);
        assert_eq!(set.viewport_byte_start, 0);
        assert_eq!(set.viewport_byte_end, 64 * 1024);
        assert!(set.spans.iter().all(|span| span.byte_end <= 64 * 1024));
        assert!(
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::Heading1 && span.byte_start == 0)
        );
        assert!(!set.spans.iter().any(|span| span.byte_start > 64 * 1024));
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
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::OutsideRoot,
                workspace_root_id: Some(id),
                document_id: None,
                ..
            } if id == selected_root_id
        ));

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
