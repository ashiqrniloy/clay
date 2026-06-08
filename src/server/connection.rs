use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};

use crate::protocol::{
    ClientId, ClientMessage, DocumentId, DocumentMetadata, PROTOCOL_VERSION, ProtocolErrorCode,
    RuntimeDiagnostic, ServerMessage, WorkspaceRootId,
    codec::{Codec, CodecError},
};

use super::{
    behavior::ActiveBehaviorManifest,
    document::DocumentState,
    js_runtime::ClayJsRuntimeService,
    sdui::{StaticSduiState, sdui_action_response},
    workspace::{WorkspaceError, WorkspaceState},
};

pub(crate) async fn handle_connection<S>(
    mut stream: S,
    client_id: u64,
    document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    sdui: Arc<Mutex<StaticSduiState>>,
    runtime_diagnostics: Arc<Mutex<Vec<RuntimeDiagnostic>>>,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let first_message = codec.read_client_message(&mut stream).await?;
    match first_message {
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
    }

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
                selected_path,
            } => {
                let response =
                    open_selected_file_response(&workspace, selected_path, client_id).await;
                codec.write_server_message(&mut stream, &response).await?;
                if let ServerMessage::DocumentOpened { metadata, text } = &response {
                    let messages = selected_file_open_followup_messages(
                        client_id, metadata, text, &behavior, &sdui,
                    )
                    .await;
                    for message in messages {
                        codec.write_server_message(&mut stream, &message).await?;
                    }
                }
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
                let response = {
                    let state = sdui.lock().await;
                    sdui_action_response(&state, &intent)
                };
                if let Some(response) = response {
                    codec.write_server_message(&mut stream, &response).await?;
                }
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
    codec.write_server_message(stream, &sdui_snapshot).await?;

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

async fn open_document_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    workspace_root_id: WorkspaceRootId,
    path: String,
    client_id: ClientId,
) -> ServerMessage {
    let opened = match workspace
        .lock()
        .await
        .open_existing_file(workspace_root_id, &path, client_id)
        .await
    {
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
    let opened = match workspace
        .lock()
        .await
        .open_selected_file(std::path::PathBuf::from(&selected_path), client_id)
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
    match workspace.lock().await.save_document(document_id).await {
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
    let outcome = match workspace
        .lock()
        .await
        .reload_document(document_id, force)
        .await
    {
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

async fn selected_file_open_followup_messages(
    client_id: ClientId,
    metadata: &DocumentMetadata,
    text: &str,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
) -> Vec<ServerMessage> {
    if !is_markdown_path(&metadata.path) || !markdown_package_is_loaded(behavior).await {
        return vec![behavior.lock().await.manifest_message()];
    }

    match evaluate_markdown_open(metadata, text).await {
        Ok(evaluation) => {
            let mut messages = Vec::new();
            if let Some(manifest) = evaluation.behavior_manifest {
                match behavior.lock().await.publish_replacement(manifest.clone()) {
                    Ok(installed) => messages.push(ServerMessage::BehaviorManifest(installed)),
                    Err(_) => {
                        messages.push(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
                            "clay.markdown.invalid_open_manifest",
                            "Markdown behavior manifest for the opened document failed validation.",
                        )))
                    }
                }
            } else {
                messages.push(behavior.lock().await.manifest_message());
            }

            if let Some(set) = evaluation.published_decoration_set {
                messages.push(ServerMessage::DecorationSet(set));
            }

            if let Some(tree) = evaluation.published_sdui_tree {
                match sdui
                    .lock()
                    .await
                    .replace_for_document_with_runtime_tree(metadata.document_id, tree.clone())
                {
                    Ok(()) => messages.push(ServerMessage::SduiSnapshot { client_id, tree }),
                    Err(_) => {
                        messages.push(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
                            "clay.markdown.invalid_open_sdui",
                            "Markdown status UI for the opened document failed validation.",
                        )))
                    }
                }
            }
            messages
        }
        Err(error) => vec![
            behavior.lock().await.manifest_message(),
            ServerMessage::RuntimeDiagnostic(error.diagnostic()),
        ],
    }
}

async fn markdown_package_is_loaded(behavior: &Arc<Mutex<ActiveBehaviorManifest>>) -> bool {
    behavior
        .lock()
        .await
        .manifest()
        .commands
        .iter()
        .any(|command| command.command_id.starts_with("markdown."))
}

fn is_markdown_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown"
            )
        })
        .unwrap_or(false)
}

async fn evaluate_markdown_open(
    metadata: &DocumentMetadata,
    text: &str,
) -> Result<super::js_runtime::ClayRuntimeEvaluation, super::js_runtime::ClayRuntimeError> {
    let config_root = create_markdown_open_runtime_root(metadata, text)?;
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_for_document(config_root.clone(), metadata.document_id)
        .await;
    let _ = std::fs::remove_dir_all(config_root);
    result
}

fn create_markdown_open_runtime_root(
    metadata: &DocumentMetadata,
    text: &str,
) -> Result<PathBuf, super::js_runtime::ClayRuntimeError> {
    let config_root = unique_markdown_open_runtime_root();
    std::fs::create_dir_all(&config_root)
        .map_err(|error| super::js_runtime::ClayRuntimeError::Runtime(error.to_string()))?;
    let dist_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages")
        .join("markdown")
        .join("dist");
    for file_name in ["index.js", "load.js", "parser.js", "sdui.js"] {
        std::fs::copy(dist_root.join(file_name), config_root.join(file_name))
            .map_err(|error| super::js_runtime::ClayRuntimeError::Runtime(error.to_string()))?;
    }

    let (window_text, window_byte_end) = bounded_utf8_prefix(text, 64 * 1024);
    let init_source =
        markdown_open_init_source(metadata, text.len() as u64, window_text, window_byte_end);
    std::fs::write(config_root.join("init.js"), init_source)
        .map_err(|error| super::js_runtime::ClayRuntimeError::Runtime(error.to_string()))?;
    Ok(config_root)
}

fn unique_markdown_open_runtime_root() -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "clay-markdown-open-runtime-{}-{unique}",
        std::process::id()
    ))
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

fn markdown_open_init_source(
    metadata: &DocumentMetadata,
    document_byte_length: u64,
    window_text: &str,
    window_byte_end: u64,
) -> String {
    let document_id = metadata.document_id;
    let document_version = metadata.version;
    let document_path = serde_json::to_string(&metadata.path).expect("path serializes");
    let window_text = serde_json::to_string(window_text).expect("text serializes");
    format!(
        r#"
import * as commands from "clay:commands";
import * as decorations from "clay:decorations";
import * as modes from "clay:modes";
import * as packages from "clay:packages";
import * as parse from "clay:parse";
import * as sdui from "clay:sdui";
import {{ getActiveBehaviorManifest }} from "clay:behavior";
import {{ loadMarkdownPackage }} from "./load.js";
import {{ publishMarkdownDecorations }} from "./parser.js";
import {{ publishMarkdownPreviewStatus }} from "./sdui.js";

const clay = {{ commands, decorations, modes, packages, parse, sdui }};
const documentId = {document_id};
const documentVersion = {document_version};
const documentPath = {document_path};
const windowText = {window_text};
const documentByteLength = {document_byte_length};
const viewport = {{ byteStart: 0, byteEnd: {window_byte_end} }};

await loadMarkdownPackage(clay, {{ documentId, path: documentPath }});
const manifest = getActiveBehaviorManifest(documentId);
const behaviorVersion = Number(manifest.behaviorVersion ?? manifest.behavior_version ?? manifest.version ?? 2);
await publishMarkdownDecorations(clay, {{
  documentId,
  documentVersion,
  currentDocumentVersion: documentVersion,
  behaviorVersion,
  documentByteLength,
  fileSizeBytes: documentByteLength,
  viewport,
  parseWindows: [{{
    documentId,
    documentVersion,
    packagePrefix: "markdown",
    mode: "markdown",
    byteStart: 0,
    byteEnd: {window_byte_end},
    baseLine: 0,
    text: windowText
  }}]
}});
await publishMarkdownPreviewStatus(clay, {{
  documentId,
  documentVersion,
  documentPath,
  documentByteLength,
  fileSizeBytes: documentByteLength
}});
"#
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc, time::SystemTime};

    use tokio::{io::duplex, sync::Mutex};

    use super::handle_connection;

    fn workspace_state() -> Arc<Mutex<WorkspaceState>> {
        Arc::new(Mutex::new(WorkspaceState::new()))
    }

    fn sdui_state() -> Arc<Mutex<StaticSduiState>> {
        Arc::new(Mutex::new(StaticSduiState::for_document(1, 1)))
    }

    fn runtime_diagnostics() -> Arc<Mutex<Vec<RuntimeDiagnostic>>> {
        Arc::new(Mutex::new(Vec::new()))
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
            BehaviorManifest, ClientMessage, CommandDeclaration, DecorationKind, DocumentAccess,
            DocumentMetadata, EditOperation, EditRejection, FileErrorCode, PROTOCOL_VERSION,
            RuntimeDiagnostic, SduiNodeKind, ServerMessage, codec::Codec,
        },
        server::{
            behavior::ActiveBehaviorManifest, document::DocumentState, sdui::StaticSduiState,
            workspace::WorkspaceState,
        },
    };

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
    async fn server_sends_initial_sdui_snapshot_after_bootstrap() {
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
            ServerMessage::SduiSnapshot { client_id, tree } => {
                assert_eq!(client_id, 99);
                assert_eq!(tree.ui_version, 1);
                assert!(
                    tree.nodes
                        .iter()
                        .any(|node| matches!(node.kind, SduiNodeKind::EditorView { .. }))
                );
            }
            message => panic!("expected SduiSnapshot, got {message:?}"),
        }

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
    async fn selected_markdown_file_publishes_manifest_decorations_and_status() {
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

        let mut loaded_markdown_manifest = BehaviorManifest::minimal_text_editing(7);
        loaded_markdown_manifest
            .commands
            .push(CommandDeclaration::server_intent(
                "markdown.togglePreview",
                "Toggle Markdown Preview",
            ));
        let behavior = Arc::new(Mutex::new(
            ActiveBehaviorManifest::new(loaded_markdown_manifest).unwrap(),
        ));

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
            sdui_state(),
            runtime_diagnostics(),
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

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenSelectedFile {
                    client_id: 99,
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
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => {
                assert!(tree.nodes.iter().any(|node| matches!(
                    &node.kind,
                    SduiNodeKind::Panel { title, .. } if title == "Markdown Preview"
                )));
                assert!(tree.nodes.iter().any(|node| matches!(
                    &node.kind,
                    SduiNodeKind::Label { text } if text == "Mode: markdown"
                )));
            }
            message => panic!("expected Markdown SDUI snapshot, got {message:?}"),
        }

        drop(client);
        server_task.await.unwrap().unwrap();
        let _ = fs::remove_file(selected);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn markdown_open_runtime_uses_bounded_parse_window_for_large_file() {
        let mut text = "# Top\n\n".to_string();
        text.push_str(&"a".repeat(80 * 1024));
        text.push_str("\n# Outside initial window\n");
        let metadata = DocumentMetadata {
            document_id: 1,
            version: 1,
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            dirty: false,
            workspace_root_id: 1,
            path: "large.md".to_string(),
        };

        let evaluation = super::evaluate_markdown_open(&metadata, &text)
            .await
            .expect("Markdown open runtime should evaluate");
        let set = evaluation
            .published_decoration_set
            .expect("Markdown decorations should publish");

        assert_eq!(set.document_id, 1);
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

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::OpenSelectedFile {
                    client_id: 99,
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
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1))
        );

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
            codec,
        ));

        tokio::io::AsyncWriteExt::write_all(&mut client, &[0, 0, 0, 4, 0xde, 0xad, 0xbe, 0xef])
            .await
            .unwrap();
        drop(client);

        let result = server_task.await.unwrap();
        assert!(result.is_err());
    }
}
