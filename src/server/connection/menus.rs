//! Server-owned menu session intents: query/backspace/selection-move/activate/
//! cancel plus shared command-centre session opening. Plan 090 task 2.

use std::sync::Arc;

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    packages::commands::CommandRegistry,
    protocol::{
        ClientId, ProtocolErrorCode, ServerMessage, TabId, TabRegistrySnapshot,
        TransientMenuSnapshotData,
        codec::{Codec, CodecError},
    },
    server::{
        command_execution::{
            CONTROL_CENTER_COMMAND_ID, CommandExecutionTarget, OPEN_PATH_BROWSER_COMMAND_ID,
        },
        control_center::ServerMenuActivation,
        document::DocumentState,
        document_analysis::DocumentAnalysisCoordinator,
        menu_sessions::{ServerMenuActivateOutcome, ServerMenuSessions, snapshot_from_session},
        parse_coordinator::ParseCoordinator,
        sdui::StaticSduiState,
        tab_registry::TabRegistry,
        workspace::{
            UserBrowseListingPlan, WorkspaceState, execute_user_browse_listing,
            resolve_user_browse_seed,
        },
    },
    shell::path_browser::PathBrowserSession,
};

use crate::server::{IpcServer, RuntimeGenerationStore, behavior::ActiveBehaviorManifest};

use super::{
    documents::write_document_open_response,
    runtime::execute_command_intent,
    tabs::open_workspace_for_bound_tab,
    unknown_menu_session_diagnostic,
    workspace::{open_selected_file_response, path_browser_relist},
};

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn open_command_centre_session(
    command_id: &str,
    menu_sessions: &mut ServerMenuSessions,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    tab_registry: &Arc<Mutex<TabRegistry>>,
    bound_tab_id: Option<TabId>,
) -> Result<(Option<u64>, TransientMenuSnapshotData), String> {
    if command_id == CONTROL_CENTER_COMMAND_ID {
        let document_id = document.lock().await.document_id();
        let active_manifest = behavior.lock().await.manifest_for(document_id).clone();
        let (generation_id, catalogue) = match runtime_generation
            .command_catalogue_snapshot(&active_manifest)
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => return Err(format!("command catalogue could not be opened: {error}")),
        };
        let (snapshot, replaced_id) = menu_sessions.open_control_center(&catalogue, generation_id);
        Ok((replaced_id, snapshot))
    } else if command_id == OPEN_PATH_BROWSER_COMMAND_ID {
        let document_id = document.lock().await.document_id();
        let generation_id = runtime_generation.generation_id().await;
        let tab_root = {
            let tab_registry = tab_registry.lock().await;
            bound_tab_id
                .and_then(|tab_id| tab_registry.entry(tab_id))
                .map(|entry| entry.workspace_root)
        };
        let seed =
            resolve_user_browse_seed(workspace, Some(document_id), tab_root.as_deref()).await;
        let plan = UserBrowseListingPlan {
            target: seed.clone(),
            max_entries: crate::perf::budgets::COMMAND_CENTRE_LISTING_MAX_ENTRIES,
        };
        let mut session = PathBrowserSession::new(seed);
        match execute_user_browse_listing(plan).await {
            Ok(page) => session.install(page),
            Err(error) => session.set_error(error.to_string()),
        }
        let (snapshot, replaced_id) = menu_sessions.open_path_browser(session, generation_id);
        Ok((replaced_id, snapshot))
    } else {
        Err(format!(
            "unexpected command centre command id: {command_id}"
        ))
    }
}

// ---------- coordinator loop handlers (Plan 090 task 2 extraction) ----------

pub(super) async fn handle_menu_query_update<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    client_id: ClientId,
    session_id: u64,
    query: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let Some(session) = menu_sessions.get_mut(session_id) else {
        codec
            .write_server_message(
                stream,
                &unknown_menu_session_diagnostic(client_id, session_id),
            )
            .await?;
        return Ok(());
    };
    // Filter-only edits re-score installed entries locally (no filesystem
    // work); a changed directory prefix relists the target and installs the
    // bounded page back (plan 083 task 8).
    let (snapshot, relist) = {
        let edit = session.set_query(&query);
        (edit.snapshot, edit.relist)
    };
    let snapshot = match relist {
        Some(target) => match path_browser_relist(menu_sessions, session_id, target).await {
            Some(snapshot) => snapshot,
            None => {
                codec
                    .write_server_message(
                        stream,
                        &unknown_menu_session_diagnostic(client_id, session_id),
                    )
                    .await?;
                return Ok(());
            }
        },
        None => snapshot,
    };
    codec
        .write_server_message(
            stream,
            &ServerMessage::TransientMenuSnapshot(Box::new(snapshot_from_session(&snapshot))),
        )
        .await?;
    Ok(())
}

pub(super) async fn handle_menu_backspace<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    client_id: ClientId,
    session_id: u64,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let Some(session) = menu_sessions.get_mut(session_id) else {
        codec
            .write_server_message(
                stream,
                &unknown_menu_session_diagnostic(client_id, session_id),
            )
            .await?;
        return Ok(());
    };
    let (snapshot, relist) = {
        let edit = session.backspace();
        (edit.snapshot, edit.relist)
    };
    let snapshot = match relist {
        Some(target) => match path_browser_relist(menu_sessions, session_id, target).await {
            Some(snapshot) => snapshot,
            None => {
                codec
                    .write_server_message(
                        stream,
                        &unknown_menu_session_diagnostic(client_id, session_id),
                    )
                    .await?;
                return Ok(());
            }
        },
        None => snapshot,
    };
    codec
        .write_server_message(
            stream,
            &ServerMessage::TransientMenuSnapshot(Box::new(snapshot_from_session(&snapshot))),
        )
        .await?;
    Ok(())
}

pub(super) async fn handle_menu_selection_move<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    client_id: ClientId,
    session_id: u64,
    delta: i64,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let Some(session) = menu_sessions.get_mut(session_id) else {
        codec
            .write_server_message(
                stream,
                &unknown_menu_session_diagnostic(client_id, session_id),
            )
            .await?;
        return Ok(());
    };
    let snapshot = snapshot_from_session(&session.move_selection(delta));
    codec
        .write_server_message(
            stream,
            &ServerMessage::TransientMenuSnapshot(Box::new(snapshot)),
        )
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_menu_activate<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    behavior: &Arc<Mutex<crate::server::behavior::ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    parse_coordinator: &ParseCoordinator,
    document_analysis: &DocumentAnalysisCoordinator,
    tab_registry: &Arc<Mutex<TabRegistry>>,
    tab_registry_tx: &tokio::sync::broadcast::Sender<TabRegistrySnapshot>,
    reload_server: Option<&IpcServer>,
    client_id: ClientId,
    session_id: u64,
    kind: crate::protocol::TransientMenuActivationData,
    bound_tab_id: Option<TabId>,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    // Activation resolves server-side: the session kind maps the selected
    // item to a dispatch (palette closes first, then the command executes
    // against the connection's tab) or a path navigation (the session stays
    // open and relists the target directory). `kind` distinguishes primary
    // (Enter/Tab) from secondary (Alt+Enter) activation; the Control Center
    // activates the same selection for both.
    let document_id = document.lock().await.document_id();
    let current_generation_id = runtime_generation.generation_id().await;
    let activation = {
        let Some(session) = menu_sessions.get_mut(session_id) else {
            codec
                .write_server_message(
                    stream,
                    &unknown_menu_session_diagnostic(client_id, session_id),
                )
                .await?;
            return Ok(());
        };
        session.activate(
            CommandExecutionTarget::ActiveDocument { document_id },
            kind,
            current_generation_id,
        )
    };
    match activation {
        // Path-mode descend: keep the session open, install the bounded
        // listing of the canonical target, push exactly one fresh snapshot
        // (plan 083 task 8).
        Ok(ServerMenuActivateOutcome::Navigate(target)) => {
            let Some(snapshot) = path_browser_relist(menu_sessions, session_id, target).await
            else {
                codec
                    .write_server_message(
                        stream,
                        &unknown_menu_session_diagnostic(client_id, session_id),
                    )
                    .await?;
                return Ok(());
            };
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::TransientMenuSnapshot(Box::new(snapshot_from_session(
                        &snapshot,
                    ))),
                )
                .await?;
            Ok(())
        }
        // Path-mode file open (plan 083 task 9): the session closes first,
        // then the ordinary selected-file open runs against the
        // server-held canonical path. The activation itself is the user
        // authorization event that converts ephemeral browse authority into
        // a single `SingleFile` grant; no capability token is involved.
        // Failures (directory, oversized, invalid UTF-8, disappeared,
        // permission) become the bounded file-operation error with no grant
        // allocated and the session already closed.
        Ok(ServerMenuActivateOutcome::OpenFile(path)) => {
            menu_sessions.cancel(session_id);
            codec
                .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
                .await?;
            let response = open_selected_file_response(
                workspace,
                path.to_string_lossy().into_owned(),
                client_id,
            )
            .await;
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
            Ok(())
        }
        // Path-mode workspace open (plan 083 task 10): secondary activation
        // on a directory closes the session, then adds/gets the canonical
        // `Directory` root in the bound tab's workspace, rebinds the tab
        // through the shared helper (broadcasting the reconciled registry
        // snapshot), and refreshes the tab's file-browser snapshot when the
        // pane is visible. The activation is the user authorization event
        // that converts ephemeral browse authority into a `Directory` root
        // grant; other tabs' roots, documents, grants, and menus are
        // untouched. A missing/foreign bound tab, non-directory root, or
        // vanished path rejects with the bounded failure and no grant.
        Ok(ServerMenuActivateOutcome::OpenWorkspace(path)) => {
            menu_sessions.cancel(session_id);
            codec
                .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
                .await?;
            for message in open_workspace_for_bound_tab(
                workspace,
                document,
                sdui,
                tab_registry,
                tab_registry_tx,
                reload_server,
                client_id,
                bound_tab_id,
                path,
            )
            .await
            {
                codec.write_server_message(stream, &message).await?;
            }
            Ok(())
        }
        // Dispatch outcomes consume the session: the palette closes first,
        // then the selected command executes. Server/package commands route
        // through the shared intent dispatcher (workspace/settings/reload
        // side effects included); shell `ClientUiCommand` items produce the
        // narrow shell-command request the client re-parses deny-by-default.
        Ok(ServerMenuActivateOutcome::Dispatch(activation)) => {
            menu_sessions.cancel(session_id);
            codec
                .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
                .await?;
            match activation {
                ServerMenuActivation::Command(request) => {
                    // Selecting "Browse Filesystem" from the Control Center
                    // opens the Path Browser through the same shared helper
                    // as its keybinding (the closed Control Center frame was
                    // already pushed above).
                    if request.command_id == OPEN_PATH_BROWSER_COMMAND_ID {
                        match open_command_centre_session(
                            &request.command_id,
                            menu_sessions,
                            behavior,
                            runtime_generation,
                            document,
                            workspace,
                            tab_registry,
                            bound_tab_id,
                        )
                        .await
                        {
                            Ok((replaced_id, snapshot)) => {
                                if let Some(replaced_id) = replaced_id {
                                    codec
                                        .write_server_message(
                                            stream,
                                            &ServerMessage::TransientMenuClosed {
                                                session_id: replaced_id,
                                            },
                                        )
                                        .await?;
                                }
                                codec
                                    .write_server_message(
                                        stream,
                                        &ServerMessage::TransientMenuSnapshot(Box::new(snapshot)),
                                    )
                                    .await?;
                            }
                            Err(message) => {
                                codec
                                    .write_server_message(
                                        stream,
                                        &ServerMessage::Error {
                                            code: ProtocolErrorCode::InvalidMessage,
                                            message,
                                        },
                                    )
                                    .await?;
                            }
                        }
                        return Ok(());
                    }
                    // Execute against the live aggregated registries so
                    // package commands validate through the shared path;
                    // built-ins resolve via executor fallback. The menu's
                    // own generation stamp already rejected stale sessions,
                    // so the service snapshot is consistent.
                    let (trusted, third_party) = runtime_generation
                        .current()
                        .await
                        .service
                        .command_registry_snapshots();
                    let registry = CommandRegistry::from_snapshots([trusted, third_party]);
                    let response = execute_command_intent(
                        request,
                        Arc::clone(workspace),
                        document,
                        sdui,
                        client_id,
                        reload_server,
                        &registry,
                    )
                    .await;
                    if let Some(response) = response {
                        codec.write_server_message(stream, &response).await?;
                    }
                }
                ServerMenuActivation::ShellClientCommand(command_id) => {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::ShellClientCommandRequest { command_id },
                        )
                        .await?;
                }
            }
            Ok(())
        }
        Err(error) => {
            // Bounded diagnostic; the session is consumed so the menu closes
            // (path mode keeps activation authority server-side even when
            // the selected item has no activation yet).
            menu_sessions.cancel(session_id);
            codec
                .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
                .await?;
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message: format!(
                            "command execution rejected: {:?}: {}",
                            error.rule, error.message
                        ),
                    },
                )
                .await?;
            Ok(())
        }
    }
}

pub(super) async fn handle_menu_cancel<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    client_id: ClientId,
    session_id: u64,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    if menu_sessions.cancel(session_id).is_some() {
        codec
            .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
            .await?;
    } else {
        codec
            .write_server_message(
                stream,
                &unknown_menu_session_diagnostic(client_id, session_id),
            )
            .await?;
    }
    Ok(())
}
