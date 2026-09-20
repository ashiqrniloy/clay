//! Workspace family: selected-file/root grants, file-browser snapshots, browse
//! listing, path-browser relist, workspace command results. Plan 090 task 2.

use std::{path::PathBuf, sync::Arc};

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    protocol::{
        ClientId, DocumentId, DocumentMetadata, DocumentVersion, RuntimeDiagnostic, ServerMessage,
        WorkspaceRootId,
        codec::{Codec, CodecError},
    },
    server::{
        agent_settings,
        document::DocumentState,
        launcher,
        menu_sessions::ServerMenuSessions,
        sdui::StaticSduiState,
        workspace::{
            UserBrowseListingPlan, WorkspaceError, WorkspaceState, execute_user_browse_listing,
            open_selected_file_unlocked,
        },
    },
    shell::{file_browser::FileBrowserState, transient_menu::TransientMenuSession},
};

use super::{ConnectionCtx, documents::write_document_open_response, file_operation_failed};

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
                head: snapshot.head,
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
    let head = document.document_text_head();
    ServerMessage::DocumentOpened { metadata, head }
}

// ---------- coordinator loop handlers (Plan 090 task 2 extraction) ----------

pub(super) async fn handle_open_selected_file<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    capability: String,
    selected_path: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let authorized = ctx.file_open_capabilities.consume(&capability);
    // Replenish one pending token regardless of outcome so a legitimate
    // client can retry or open another file.
    let replenish = ServerMessage::FileOpenCapabilityIssued {
        token: ctx.file_open_capabilities.issue(),
    };
    if !authorized {
        ctx.codec
            .write_server_message(ctx.stream, &replenish)
            .await?;
        ctx.codec
            .write_server_message(
                ctx.stream,
                &ServerMessage::RuntimeDiagnostic(crate::protocol::RuntimeDiagnostic::error(
                    "client.selected_file_open.unauthorized",
                    "OpenSelectedFile requires a valid server-issued file-open capability token.",
                )),
            )
            .await?;
        return Ok(());
    }
    let response = open_selected_file_response(ctx.workspace, selected_path, ctx.client_id).await;
    write_document_open_response(
        &ctx.codec,
        ctx.stream,
        response,
        ctx.behavior,
        ctx.runtime_generation,
        ctx.workspace,
        ctx.sdui,
        ctx.parse_coordinator,
        ctx.document_analysis,
        ctx.client_id,
    )
    .await?;
    ctx.codec
        .write_server_message(ctx.stream, &replenish)
        .await?;
    Ok(())
}

pub(super) async fn handle_add_selected_workspace_root<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    capability: String,
    selected_path: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let authorized = ctx.file_open_capabilities.consume(&capability);
    let replenish = ServerMessage::FileOpenCapabilityIssued {
        token: ctx.file_open_capabilities.issue(),
    };
    if !authorized {
        ctx.codec
            .write_server_message(ctx.stream, &replenish)
            .await?;
        ctx.codec
            .write_server_message(
                ctx.stream,
                &ServerMessage::RuntimeDiagnostic(crate::protocol::RuntimeDiagnostic::error(
                    "client.selected_folder_open.unauthorized",
                    "AddSelectedWorkspaceRoot requires a valid server-issued selected-path capability token.",
                )),
            )
            .await?;
        return Ok(());
    }
    let workspace_pane_visible = match ctx.reload_server {
        Some(server) => server
            .state_for_client(ctx.client_id)
            .await
            .is_some_and(|state| state.workspace_pane_visible()),
        None => true,
    };
    // Launcher (plan 118 Part D): the folder dialog is an explicit open, so
    // the picked folder leads the recents list. Best-effort.
    if let Some(server) = ctx.reload_server {
        launcher::record_recent_workspace(
            server.configuration_root().as_deref(),
            std::path::Path::new(&selected_path),
        );
    }
    for message in add_selected_workspace_root_messages(
        ctx.workspace,
        ctx.document,
        ctx.sdui,
        ctx.client_id,
        workspace_pane_visible,
        selected_path,
    )
    .await
    {
        ctx.codec.write_server_message(ctx.stream, &message).await?;
    }
    ctx.codec
        .write_server_message(ctx.stream, &replenish)
        .await?;
    Ok(())
}

/// Launcher (plan 118 Part D): answer with the server-resolved entries. The
/// rows are display data, so an absent data root yields the first-run state
/// rather than an error; pruned recents surface as one bounded diagnostic.
async fn write_launcher_entries<S>(ctx: &mut ConnectionCtx<'_, S>) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let entries = ctx
        .reload_server
        .map(|server| launcher::launcher_entries(server.configuration_root().as_deref()))
        .unwrap_or_default();
    if entries.pruned > 0 {
        ctx.codec
            .write_server_message(
                ctx.stream,
                &ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::info(
                    "launcher.recents_pruned",
                    format!(
                        "{} recent workspace(s) dropped: the folder no longer exists",
                        entries.pruned
                    ),
                )),
            )
            .await?;
    }
    ctx.codec
        .write_server_message(
            ctx.stream,
            &ServerMessage::LauncherEntries {
                client_id: ctx.client_id,
                entries: Box::new(entries),
            },
        )
        .await?;
    Ok(())
}

pub(super) async fn handle_list_launcher_entries<S>(
    ctx: &mut ConnectionCtx<'_, S>,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    write_launcher_entries(ctx).await
}

pub(super) async fn handle_remove_launcher_recent<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    index: u32,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    if let Some(server) = ctx.reload_server {
        launcher::remove_recent_workspace(server.configuration_root().as_deref(), index);
    }
    write_launcher_entries(ctx).await
}

/// Agent settings page (plan 117): server-resolved listing; no config root ⇒
/// empty page, never an error.
pub(super) async fn handle_list_agent_settings_files<S>(
    ctx: &mut ConnectionCtx<'_, S>,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let files = match ctx.reload_server {
        Some(server) => server
            .agent_settings_root(ctx.client_id)
            .await
            .map(|root| agent_settings::list_agent_settings_files(&root))
            .unwrap_or_default(),
        None => Vec::new(),
    };
    ctx.codec
        .write_server_message(
            ctx.stream,
            &ServerMessage::AgentSettingsFiles {
                client_id: ctx.client_id,
                files,
            },
        )
        .await
}

/// Agent settings page (plan 117): the name is validated against the fixed
/// delivered-file layout and resolved server-side; the open/save path is the
/// ordinary selected-file document pipeline (no extra capability — the name
/// carries no path authority).
pub(super) async fn handle_open_agent_settings_file<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    name: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let root = match ctx.reload_server {
        Some(server) => server.agent_settings_root(ctx.client_id).await,
        None => None,
    };
    let response = match root
        .map(|root| agent_settings::resolve_agent_settings_file(&root, &name))
        .unwrap_or_else(|| Err("agent config root unavailable".to_string()))
    {
        Ok(path) => {
            open_selected_file_response(ctx.workspace, path.display().to_string(), ctx.client_id)
                .await
        }
        Err(message) => file_operation_failed(
            WorkspaceError::FileUnavailable {
                path: PathBuf::from(&name),
                source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, message),
            },
            None,
            None,
        ),
    };
    write_document_open_response(
        &ctx.codec,
        ctx.stream,
        response,
        ctx.behavior,
        ctx.runtime_generation,
        ctx.workspace,
        ctx.sdui,
        ctx.parse_coordinator,
        ctx.document_analysis,
        ctx.client_id,
    )
    .await
}
