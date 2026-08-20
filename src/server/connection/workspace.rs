//! Workspace family: selected-file/root grants, file-browser snapshots, browse
//! listing, path-browser relist, workspace command results. Plan 090 task 2.

use std::{path::PathBuf, sync::Arc};

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    protocol::{
        ClientId, DocumentId, DocumentMetadata, DocumentVersion, ServerMessage, WorkspaceRootId,
        codec::{Codec, CodecError},
    },
    server::{
        document::DocumentState,
        document_analysis::DocumentAnalysisCoordinator,
        menu_sessions::ServerMenuSessions,
        sdui::StaticSduiState,
        workspace::{
            UserBrowseListingPlan, WorkspaceError, WorkspaceState, execute_user_browse_listing,
            open_selected_file_unlocked,
        },
    },
    shell::{file_browser::FileBrowserState, transient_menu::TransientMenuSession},
};

use crate::server::IpcServer;

use super::{
    FileOpenCapabilityPool, documents::write_document_open_response, file_operation_failed,
};

pub(super) async fn path_browser_relist(
    menu_sessions: &mut ServerMenuSessions,
    session_id: u64,
    target: PathBuf,
) -> Option<TransientMenuSession> {
    let page = match execute_user_browse_listing(UserBrowseListingPlan {
        target,
        max_entries: crate::perf::budgets::TRANSIENT_MENU_MAX_ITEMS,
    })
    .await
    {
        Ok(page) => page,
        Err(error) => {
            let session = menu_sessions.get_mut(session_id)?;
            return Some(session.set_path_browser_error(error.to_string()));
        }
    };
    let session = menu_sessions.get_mut(session_id)?;
    Some(session.install_path_browser(page))
}

/// The Path Browser performs exactly one seed resolution (active document's
/// canonical parent > bound tab's workspace root > server cwd) and one
/// bounded listing on open. A failed listing opens the session in its sticky
/// error state instead of failing the command, so the editable path input
/// stays recoverable.
#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn workspace_command_result_message(
    result: crate::server::command_execution::CommandExecutionResult,
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
    reload_server: Option<&crate::server::IpcServer>,
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
            let workspace_pane_visible = match reload_server {
                Some(server) => server
                    .state_for_client(client_id)
                    .await
                    .is_some_and(|state| state.workspace_pane_visible()),
                None => true,
            };
            Some(
                file_browser_snapshot_for_visibility(
                    workspace,
                    document,
                    sdui,
                    client_id,
                    workspace_pane_visible,
                    root_id,
                    relative_path,
                )
                .await,
            )
        }
        CommandExecutionStatus::Workspace(WorkspaceActionResult::Toggled) => {
            let server = reload_server?;
            let state = server.state_for_client(client_id).await?;
            let workspace_pane_visible = state.toggle_workspace_pane();
            let root_id = workspace
                .lock()
                .await
                .list_root_metadata()
                .first()
                .map(|root| root.workspace_root_id)?;
            Some(
                file_browser_snapshot_for_visibility(
                    workspace,
                    document,
                    sdui,
                    client_id,
                    workspace_pane_visible,
                    root_id,
                    PathBuf::new(),
                )
                .await,
            )
        }
        _ => None,
    }
}

/// Outcome of persisting a settings command and (optionally) reloading.
pub(super) fn hidden_file_browser_snapshot(
    client_id: ClientId,
    document_id: DocumentId,
    document_version: DocumentVersion,
) -> ServerMessage {
    ServerMessage::SduiSnapshot {
        client_id,
        tree: FileBrowserState::hidden_sdui_tree(document_id, document_version),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn send_tab_file_browser_snapshot<S>(
    stream: &mut S,
    client_id: ClientId,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    document_id: DocumentId,
    document_version: DocumentVersion,
    workspace_pane_visible: bool,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    if !workspace_pane_visible {
        codec
            .write_server_message(
                stream,
                &hidden_file_browser_snapshot(client_id, document_id, document_version),
            )
            .await?;
        return Ok(());
    }

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
    } else if let Some(sdui_snapshot) = sdui.lock().await.snapshot_message(client_id) {
        codec.write_server_message(stream, &sdui_snapshot).await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn add_selected_workspace_root_messages(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
    workspace_pane_visible: bool,
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
        file_browser_snapshot_for_visibility(
            workspace,
            document,
            sdui,
            client_id,
            workspace_pane_visible,
            root_id,
            PathBuf::new(),
        )
        .await,
    ]
}

pub(super) async fn file_browser_snapshot_message(
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

pub(super) async fn file_browser_snapshot_for_visibility(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
    workspace_pane_visible: bool,
    root_id: WorkspaceRootId,
    relative_path: PathBuf,
) -> ServerMessage {
    if workspace_pane_visible {
        file_browser_snapshot_message(workspace, document, sdui, client_id, root_id, relative_path)
            .await
    } else {
        let (document_id, document_version) = {
            let document = document.lock().await;
            (document.document_id(), document.version())
        };
        hidden_file_browser_snapshot(client_id, document_id, document_version)
    }
}

pub(super) async fn open_selected_file_response(
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

// ---------- coordinator loop handlers (Plan 090 task 2 extraction) ----------

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_open_selected_file<S>(
    codec: Codec,
    stream: &mut S,
    file_open_capabilities: &mut FileOpenCapabilityPool,
    behavior: &Arc<Mutex<crate::server::behavior::ActiveBehaviorManifest>>,
    runtime_generation: &super::RuntimeGenerationStore,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    parse_coordinator: &crate::server::parse_coordinator::ParseCoordinator,
    document_analysis: &DocumentAnalysisCoordinator,
    client_id: ClientId,
    capability: String,
    selected_path: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let authorized = file_open_capabilities.consume(&capability);
    // Replenish one pending token regardless of outcome so a legitimate
    // client can retry or open another file.
    let replenish = ServerMessage::FileOpenCapabilityIssued {
        token: file_open_capabilities.issue(),
    };
    if !authorized {
        codec.write_server_message(stream, &replenish).await?;
        codec
            .write_server_message(
                stream,
                &ServerMessage::RuntimeDiagnostic(crate::protocol::RuntimeDiagnostic::error(
                    "client.selected_file_open.unauthorized",
                    "OpenSelectedFile requires a valid server-issued file-open capability token.",
                )),
            )
            .await?;
        return Ok(());
    }
    let response = open_selected_file_response(workspace, selected_path, client_id).await;
    write_document_open_response(
        &codec,
        stream,
        response,
        behavior,
        runtime_generation,
        workspace,
        sdui,
        parse_coordinator,
        document_analysis,
        client_id,
    )
    .await?;
    codec.write_server_message(stream, &replenish).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_add_selected_workspace_root<S>(
    codec: Codec,
    stream: &mut S,
    file_open_capabilities: &mut FileOpenCapabilityPool,
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    reload_server: Option<&IpcServer>,
    client_id: ClientId,
    capability: String,
    selected_path: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let authorized = file_open_capabilities.consume(&capability);
    let replenish = ServerMessage::FileOpenCapabilityIssued {
        token: file_open_capabilities.issue(),
    };
    if !authorized {
        codec.write_server_message(stream, &replenish).await?;
        codec
            .write_server_message(
                stream,
                &ServerMessage::RuntimeDiagnostic(crate::protocol::RuntimeDiagnostic::error(
                    "client.selected_folder_open.unauthorized",
                    "AddSelectedWorkspaceRoot requires a valid server-issued selected-path capability token.",
                )),
            )
            .await?;
        return Ok(());
    }
    let workspace_pane_visible = match reload_server {
        Some(server) => server
            .state_for_client(client_id)
            .await
            .is_some_and(|state| state.workspace_pane_visible()),
        None => true,
    };
    for message in add_selected_workspace_root_messages(
        workspace,
        document,
        sdui,
        client_id,
        workspace_pane_visible,
        selected_path,
    )
    .await
    {
        codec.write_server_message(stream, &message).await?;
    }
    codec.write_server_message(stream, &replenish).await?;
    Ok(())
}
