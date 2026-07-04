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
    runtime_diagnostics: Arc<Mutex<Vec<RuntimeDiagnostic>>>,
    runtime_generation: RuntimeGenerationStore,
    parse_coordinator: ParseCoordinator,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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
                &sdui,
                &runtime_diagnostics,
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
        let message = match codec.read_client_message(&mut stream).await {
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
                if matches!(response, ServerMessage::DocumentOpened { .. }) {
                    let manifest_message = behavior.lock().await.manifest_message();
                    codec
                        .write_server_message(&mut stream, &manifest_message)
                        .await?;
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
                    let messages = selected_file_open_followup_messages(
                        client_id,
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
                client_id: _,
                ui_version: _,
                intent,
            } => {
                let validation_response = {
                    let state = sdui.lock().await;
                    sdui_action_response(&state, &intent)
                };
                let response =
                    command_intent_response(sdui_command_request(&intent)).or(validation_response);
                if let Some(response) = response {
                    codec.write_server_message(&mut stream, &response).await?;
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
                if let Some(response) = command_intent_response(CommandExecutionRequest {
                    command_id,
                    arguments: serde_json::Value::Null,
                    target: CommandExecutionTarget::ActiveDocument { document_id },
                    provenance: None,
                    expected_permissions: Vec::new(),
                }) {
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

fn command_intent_response(request: CommandExecutionRequest) -> Option<ServerMessage> {
    CommandExecutor::new()
        .execute(&CommandRegistry::new(), request)
        .err()
        .map(|error| ServerMessage::Error {
            code: ProtocolErrorCode::InvalidMessage,
            message: format!(
                "command execution rejected: {:?}: {}",
                error.rule, error.message
            ),
        })
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

async fn send_welcome_snapshot_and_manifest<S>(
    stream: &mut S,
    client_id: u64,
    document: &Arc<Mutex<DocumentState>>,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    runtime_diagnostics: &Arc<Mutex<Vec<RuntimeDiagnostic>>>,
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

    let sdui_snapshot = sdui.lock().await.snapshot_message(client_id);
    if let Some(sdui_snapshot) = sdui_snapshot {
        codec.write_server_message(stream, &sdui_snapshot).await?;
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
    reason = "shared selected-file/reload follow-up primitive keeps server-owned state explicit"
)]
pub(crate) async fn selected_file_open_followup_messages(
    _client_id: ClientId,
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
    sdui: &Arc<Mutex<StaticSduiState>>,
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
    super::apply_runtime_outputs(&evaluation, metadata.document_id, behavior, sdui).await;
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
            Some(ParsePolicy::new(64 * 1024, 4 * 1024, 30 * 1024 * 1024, 50)),
        )
        .map_err(|error| {
            RuntimeDiagnostic::error(
                "clay.parse.open_activation_failed",
                format!("Open-time parse scheduling failed: {error:?}"),
            )
        })?;

    let deadline = tokio::time::Duration::from_millis(1000);
    loop {
        let update = tokio::time::timeout(deadline, parse_coordinator.next_update())
            .await
            .map_err(|_| {
                RuntimeDiagnostic::error(
                    "clay.parse.open_activation_timeout",
                    "Open-time parse did not finish before the decoration freshness deadline.",
                )
            })?
            .ok_or_else(|| {
                RuntimeDiagnostic::error(
                    "clay.parse.open_activation_closed",
                    "Open-time parse coordinator closed before publishing decorations.",
                )
            })?;
        if update.document_id == metadata.document_id
            && update.package_prefix == activation.package_prefix
            && update.mode_id == activation.mode_id
        {
            return Ok(update.decoration_update);
        }
    }
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

    use super::{command_intent_response, handle_connection, sdui_command_request};
    use crate::server::command_execution::{CommandExecutionRequest, CommandExecutionTarget};

    fn workspace_state() -> Arc<Mutex<WorkspaceState>> {
        Arc::new(Mutex::new(WorkspaceState::new()))
    }

    fn sdui_state() -> Arc<Mutex<StaticSduiState>> {
        Arc::new(Mutex::new(StaticSduiState::for_document(1, 1)))
    }

    fn empty_sdui_state() -> Arc<Mutex<StaticSduiState>> {
        Arc::new(Mutex::new(StaticSduiState::empty_for_document(1)))
    }

    fn runtime_diagnostics() -> Arc<Mutex<Vec<RuntimeDiagnostic>>> {
        Arc::new(Mutex::new(Vec::new()))
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
            BehaviorManifest, BehaviorScope, ClientMessage, DecorationKind, DocumentAccess,
            DocumentMetadata, EditOperation, EditRejection, FileErrorCode, PROTOCOL_VERSION,
            RuntimeDiagnostic, SduiActionIntent, SduiActionSource, SduiNodeId, SduiNodeKind,
            ServerMessage, codec::Codec,
        },
        server::{
            behavior::ActiveBehaviorManifest, document::DocumentState,
            js_runtime::ClayJsRuntimeService, parse_coordinator::ParseCoordinator,
            sdui::StaticSduiState, workspace::WorkspaceState,
        },
    };

    #[test]
    fn sdui_actions_and_keybinding_intents_share_command_execution_path() {
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

        assert_eq!(command_intent_response(sdui_request), None);
        assert_eq!(command_intent_response(keybinding_request), None);
    }

    #[test]
    fn package_ui_unregistered_action_is_rejected_by_command_execution() {
        let response = command_intent_response(sdui_command_request(&SduiActionIntent::command(
            "markdown.missingCommand",
            SduiActionSource::Button {
                node_id: SduiNodeId(5),
            },
        )))
        .expect("unknown package UI action returns protocol error");

        assert!(matches!(response, ServerMessage::Error { .. }));
        if let ServerMessage::Error { message, .. } = response {
            assert!(message.contains("UnknownCommand"));
        }
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
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DecorationSet(set) => {
                assert_eq!(set.document_id, 2);
                assert!(set.spans.iter().any(|span| {
                    span.kind == DecorationKind::Syntax && span.style_token == "markup.heading.1"
                }));
                assert!(set.spans.iter().any(|span| {
                    span.kind == DecorationKind::Syntax && span.style_token == "markup.list-marker"
                }));
                assert!(set.spans.iter().any(|span| {
                    span.kind == DecorationKind::Syntax && span.style_token == "markup.inline-code"
                }));
            }
            message => panic!("expected Markdown DecorationSet, got {message:?}"),
        }
        // Server re-issues one pending capability after the open attempt.
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));

        // Selected-file activation publishes behavior and decorations only;
        // optional package UI panels stay opt-in, so no extra SduiSnapshot
        // follows before the replenished capability.

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
        let messages = super::selected_file_open_followup_messages(
            99,
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
        assert!(messages.iter().any(|message| matches!(
            message,
            ServerMessage::DecorationSet(set)
                if set.document_id == 2
                    && set.spans.iter().any(|span| span.style_token == "markup.heading.1")
        )));
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
    async fn generic_open_parse_uses_bounded_window_for_large_file() {
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

        let set =
            super::schedule_open_parse(&coordinator, &metadata, &text, &behavior, &activation)
                .await
                .expect("open parse should schedule")
                .expect("open parse should publish decorations");

        assert_eq!(set.document_id, 2);
        assert_eq!(set.viewport_byte_start, 0);
        assert_eq!(set.viewport_byte_end, 64 * 1024);
        assert!(set.spans.iter().all(|span| span.byte_end <= 64 * 1024));
        assert!(
            set.spans
                .iter()
                .any(|span| span.style_token == "markup.heading.1" && span.byte_start == 0)
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
        for _ in 0..4 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }
        // Consume the post-handshake capability so it is no longer pending.
        assert!(matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));

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
