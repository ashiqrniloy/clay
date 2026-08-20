//! Server-authoritative tab lifecycle: New/OpenWorkspace/Close/Activate/Reclaim/
//! MoveLeft/MoveRight/MoveTo and bound-tab initial-state bootstrap. Plan 090
//! task 2 extraction.

use std::{path::PathBuf, sync::Arc};

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    protocol::{
        ClientId, DocumentId, DocumentVersion, ProtocolErrorCode, ServerMessage, TabCommand, TabId,
        TabRegistrySnapshot,
        codec::{Codec, CodecError},
    },
    server::{
        document::DocumentState, menu_sessions::ServerMenuSessions, sdui::StaticSduiState,
        tab_registry::TabRegistry, workspace::WorkspaceState,
    },
};

use crate::server::IpcServer;
use crate::server::TabServerState;

use super::{
    file_operation_failed, new_tab_binding_conflict_error, tab_binding_conflict_error,
    workspace::{file_browser_snapshot_for_visibility, send_tab_file_browser_snapshot},
};

/// Coordinator result of one tab command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TabDispatch {
    /// The connection keeps serving.
    Continue,
    /// The tab's own connection closed it; end this connection so the permit
    /// and leases release through the existing disconnect cleanup path.
    CloseConnection,
}

pub(super) async fn send_tab_initial_document<S>(
    stream: &mut S,
    client_id: ClientId,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    codec: Codec,
) -> Result<(DocumentId, DocumentVersion), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let initial_document = {
        let mut document = document.lock().await;
        let access = document.acquire_access(client_id);
        let workspace_root = workspace
            .lock()
            .await
            .list_root_metadata()
            .first()
            .map(|root| root.display_path.clone())
            .unwrap_or_default();
        document.initial_document_message(access, workspace_root)
    };
    let (document_id, document_version) = match &initial_document {
        ServerMessage::InitialDocument {
            document_id,
            version,
            ..
        } => (*document_id, *version),
        _ => (0, 0),
    };
    codec
        .write_server_message(stream, &initial_document)
        .await?;
    Ok((document_id, document_version))
}

pub(super) async fn send_tab_initial_state<S>(
    stream: &mut S,
    client_id: ClientId,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    workspace_pane_visible: bool,
    codec: Codec,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let (document_id, document_version) =
        send_tab_initial_document(stream, client_id, document, workspace, codec).await?;
    send_tab_file_browser_snapshot(
        stream,
        client_id,
        workspace,
        sdui,
        document_id,
        document_version,
        workspace_pane_visible,
        codec,
    )
    .await
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) async fn open_workspace_for_bound_tab(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    tab_registry: &Arc<Mutex<TabRegistry>>,
    tab_registry_tx: &tokio::sync::broadcast::Sender<TabRegistrySnapshot>,
    reload_server: Option<&crate::server::IpcServer>,
    client_id: ClientId,
    bound_tab_id: Option<TabId>,
    root: PathBuf,
) -> Vec<ServerMessage> {
    let Some(tab_id) = bound_tab_id else {
        return vec![ServerMessage::Error {
            code: ProtocolErrorCode::InvalidMessage,
            message: "workspace open requires a bound tab".to_string(),
        }];
    };
    let root_id = {
        let mut workspace = workspace.lock().await;
        match workspace.add_root(root.clone()) {
            Ok(root_id) => root_id,
            Err(error) => return vec![file_operation_failed(error, None, None)],
        }
    };
    let snapshot = {
        let mut registry = tab_registry.lock().await;
        registry.open_workspace(
            tab_id,
            client_id,
            root_id,
            root.to_string_lossy().into_owned(),
        );
        registry.snapshot()
    };
    let _ = tab_registry_tx.send(snapshot);
    let workspace_pane_visible = match reload_server {
        Some(server) => server
            .state_for_client(client_id)
            .await
            .is_some_and(|state| state.workspace_pane_visible()),
        None => true,
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

// ---------- coordinator loop handler (Plan 090 task 2 extraction) ----------

#[allow(
    clippy::too_many_arguments,
    reason = "tab lifecycle carries every server-owned state handle explicitly"
)]
pub(super) async fn handle_tab_command<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    bound_state: &Arc<std::sync::Mutex<Option<TabServerState>>>,
    document: &mut Arc<Mutex<DocumentState>>,
    workspace: &mut Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    tab_registry: &Arc<Mutex<TabRegistry>>,
    tab_registry_tx: &tokio::sync::broadcast::Sender<TabRegistrySnapshot>,
    reload_server: Option<&IpcServer>,
    client_id: ClientId,
    command: TabCommand,
    bound_tab_id: &mut Option<TabId>,
) -> Result<TabDispatch, CodecError>
where
    S: AsyncWrite + Unpin,
{
    match command {
        TabCommand::New { workspace_root } => {
            if bound_tab_id.is_some()
                || tab_registry
                    .lock()
                    .await
                    .tab_for_client(client_id)
                    .is_some()
            {
                codec
                    .write_server_message(stream, &new_tab_binding_conflict_error())
                    .await?;
                return Ok(TabDispatch::Continue);
            }
            let (snapshot, workspace_pane_visible) = if let Some(server) = reload_server {
                match server.create_tab_state(client_id, workspace_root).await {
                    Ok((snapshot, state)) => {
                        let workspace_pane_visible = state.workspace_pane_visible();
                        *bound_state.lock().unwrap() = Some(state.clone());
                        *document = state.welcome;
                        *workspace = state.workspace;
                        (snapshot, workspace_pane_visible)
                    }
                    Err(error) => {
                        let response = file_operation_failed(error, None, None);
                        codec.write_server_message(stream, &response).await?;
                        return Ok(TabDispatch::Continue);
                    }
                }
            } else {
                let root_id = {
                    let mut workspace = workspace.lock().await;
                    match workspace.add_root(std::path::PathBuf::from(workspace_root.clone())) {
                        Ok(root_id) => root_id,
                        Err(error) => {
                            let response = file_operation_failed(error, None, None);
                            codec.write_server_message(stream, &response).await?;
                            return Ok(TabDispatch::Continue);
                        }
                    }
                };
                let mut registry = tab_registry.lock().await;
                registry.create_tab(client_id, root_id, workspace_root);
                (registry.snapshot(), true)
            };
            *bound_tab_id = tab_registry.lock().await.tab_for_client(client_id);
            send_tab_initial_state(
                stream,
                client_id,
                document,
                workspace,
                sdui,
                workspace_pane_visible,
                codec,
            )
            .await?;
            let _ = tab_registry_tx.send(snapshot);
        }
        TabCommand::OpenWorkspace { tab_id, root } => {
            if bound_tab_id.is_some_and(|bound| bound != tab_id) {
                let snapshot = tab_registry.lock().await.snapshot();
                let _ = tab_registry_tx.send(snapshot);
                return Ok(TabDispatch::Continue);
            }
            // Shared with path-mode secondary activation (plan 083 task 10);
            // `bound_tab_id == tab_id` is proven above, so the helper's
            // bound-tab check is a no-op here.
            for message in open_workspace_for_bound_tab(
                workspace,
                document,
                sdui,
                tab_registry,
                tab_registry_tx,
                reload_server,
                client_id,
                *bound_tab_id,
                PathBuf::from(root),
            )
            .await
            {
                codec.write_server_message(stream, &message).await?;
            }
        }
        TabCommand::Close { tab_id } => {
            let closed = {
                let mut registry = tab_registry.lock().await;
                let closed = registry.close_tab(tab_id, client_id);
                let snapshot = registry.snapshot();
                let _ = tab_registry_tx.send(snapshot);
                closed
            };
            if closed {
                if let Some(server) = reload_server {
                    server.remove_tab_state(tab_id).await;
                }
                // The tab's connection is this connection (only the bound
                // client may close it): end the connection so the permit +
                // leases release via the existing disconnect cleanup path.
                return Ok(TabDispatch::CloseConnection);
            }
            // Rejected close (unknown/foreign tab): the snapshot above
            // reconciles the optimistic client; this connection keeps
            // serving.
        }
        TabCommand::Activate { tab_id } => {
            // Phase 24.1: switching tabs dismisses the active server menu
            // session (Escape-free dismissal on focus loss).
            if let Some(session_id) = menu_sessions.cancel_active() {
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::TransientMenuClosed { session_id },
                    )
                    .await?;
            }
            // Always push a snapshot, accepted or not: the client switches
            // optimistically on click and the server registry is the
            // reconciling authority — a rejected activate must revert the
            // client's active tab.
            let snapshot = {
                let mut registry = tab_registry.lock().await;
                registry.activate(tab_id, client_id);
                registry.snapshot()
            };
            let _ = tab_registry_tx.send(snapshot);
        }
        TabCommand::Reclaim { tab_id } => {
            let current_tab = bound_tab_id.or(tab_registry.lock().await.tab_for_client(client_id));
            if current_tab.is_some_and(|bound| bound != tab_id) {
                codec
                    .write_server_message(stream, &tab_binding_conflict_error())
                    .await?;
                return Ok(TabDispatch::Continue);
            }
            let existing_entry = tab_registry.lock().await.entry(tab_id);
            if let Some(server) = reload_server
                && let Some(entry) = existing_entry.as_ref()
                && let Err(error) = server
                    .ensure_tab_state(tab_id, std::path::PathBuf::from(&entry.workspace_root))
                    .await
            {
                let response = file_operation_failed(error, None, None);
                codec.write_server_message(stream, &response).await?;
                return Ok(TabDispatch::Continue);
            }
            let snapshot = {
                let mut registry = tab_registry.lock().await;
                if registry.reclaim(tab_id, client_id) {
                    Some(registry.snapshot())
                } else {
                    None
                }
            };
            if let Some(snapshot) = snapshot {
                *bound_tab_id = Some(tab_id);
                let mut workspace_pane_visible = true;
                if let Some(server) = reload_server
                    && let Some(state) = server.tab_state(tab_id).await
                {
                    workspace_pane_visible = state.workspace_pane_visible();
                    *bound_state.lock().unwrap() = Some(state.clone());
                    *document = state.welcome;
                    *workspace = state.workspace;
                }
                send_tab_initial_state(
                    stream,
                    client_id,
                    document,
                    workspace,
                    sdui,
                    workspace_pane_visible,
                    codec,
                )
                .await?;
                let _ = tab_registry_tx.send(snapshot);
            } else {
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message: "could not reclaim tab".to_string(),
                        },
                    )
                    .await?;
            }
        }
        // Phase 22.4: server-authoritative tab reorder. The registry
        // validates the bound client, existing tab, and (for `MoveTo`) the
        // 1-based position bounds; boundary moves are no-ops with no
        // wraparound. Always push a snapshot, accepted or not, so every
        // connection reconciles its card order — moves are server-confirmed
        // (no optimistic client reorder), but the uniform broadcast keeps
        // the Activate/Close reconcile pattern.
        TabCommand::MoveLeft { tab_id } => {
            let snapshot = {
                let mut registry = tab_registry.lock().await;
                registry.move_left(tab_id, client_id);
                registry.snapshot()
            };
            let _ = tab_registry_tx.send(snapshot);
        }
        TabCommand::MoveRight { tab_id } => {
            let snapshot = {
                let mut registry = tab_registry.lock().await;
                registry.move_right(tab_id, client_id);
                registry.snapshot()
            };
            let _ = tab_registry_tx.send(snapshot);
        }
        TabCommand::MoveTo { tab_id, position } => {
            let snapshot = {
                let mut registry = tab_registry.lock().await;
                registry.move_to(tab_id, client_id, position);
                registry.snapshot()
            };
            let _ = tab_registry_tx.send(snapshot);
        }
    }
    Ok(TabDispatch::Continue)
}
