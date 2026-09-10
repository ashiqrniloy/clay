//! Server-owned menu session intents: query/backspace/selection-move/activate/
//! cancel plus shared command-centre session opening. Plan 090 task 2.

use std::sync::Arc;

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    packages::commands::CommandRegistry,
    protocol::{
        AgentPickerKind, ClientId, ProtocolErrorCode, ServerMessage, TabId, TabRegistrySnapshot,
        TransientMenuSnapshotData,
        codec::{Codec, CodecError},
    },
    server::{
        agent::AgentHost,
        agent_picker::{
            AGENT_SESSION_SEARCH_LIMIT, AgentPickerActivate, AgentSearchHit,
            package_profile_commands, picker_kind_for_command,
        },
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

/// Plan 109 I9: bounded resume list page size (daemon clamps further).
const RESUME_LIST_LIMIT: u32 = 20;

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
    agent: Option<&AgentHost>,
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
    } else if let Some(kind) = picker_kind_for_command(command_id) {
        let mut inventory = match agent {
            Some(host) => host.picker_inventory().await,
            None => crate::server::agent::AgentPickerInventory::default(),
        };
        // Plan 109 I9: the resume list is workspace-scoped server-side.
        // The root comes from the tab registry (authoritative server
        // state), never from webview input; a tab without a workspace
        // (or a fresh workspace with no sessions yet) yields an empty
        // list rather than a cross-workspace dump.
        if kind == AgentPickerKind::Session {
            let root: Option<String> = {
                let registry = tab_registry.lock().await;
                bound_tab_id
                    .and_then(|tab_id| registry.entry(tab_id))
                    .map(|entry| entry.workspace_root)
                    .filter(|root| !root.is_empty())
            };
            inventory.sessions = match (agent, root) {
                (Some(host), Some(root)) => host
                    .resumable_sessions(&root, RESUME_LIST_LIMIT)
                    .await
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
        }
        let document_id = document.lock().await.document_id();
        let active_manifest = behavior.lock().await.manifest_for(document_id).clone();
        let (generation_id, profiles) = match runtime_generation
            .command_catalogue_snapshot(&active_manifest)
            .await
        {
            Ok((generation_id, catalogue)) => (generation_id, package_profile_commands(&catalogue)),
            Err(_) => (runtime_generation.generation_id().await, Vec::new()),
        };
        let (snapshot, replaced_id) =
            menu_sessions.open_agent_picker(kind, inventory, profiles, generation_id);
        Ok((replaced_id, snapshot))
    } else {
        Err(format!(
            "unexpected command centre command id: {command_id}"
        ))
    }
}

// ---------- coordinator loop handlers (Plan 090 task 2 extraction) ----------

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_menu_query_update<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    client_id: ClientId,
    session_id: u64,
    query: String,
    agent: Option<&AgentHost>,
    bound_tab_id: Option<TabId>,
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
    // Session-search picker: the query runs against the shared FTS index
    // (plan 108 task 11) — the local filter is skipped for this kind.
    let snapshot = match session_search_snapshot(
        menu_sessions,
        session_id,
        agent,
        bound_tab_id.unwrap_or(client_id),
        &query,
    )
    .await
    {
        Some(snapshot) => snapshot,
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
    agent: Option<&AgentHost>,
    bound_tab_id: Option<TabId>,
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
    // Session-search picker: backspace is a query edit too — re-run FTS so
    // hits match the shortened query.
    let query = menu_sessions
        .get(session_id)
        .and_then(|session| {
            session
                .agent_picker_ref()
                .map(|picker| picker.query().to_string())
        })
        .unwrap_or_default();
    let snapshot = match session_search_snapshot(
        menu_sessions,
        session_id,
        agent,
        bound_tab_id.unwrap_or(client_id),
        &query,
    )
    .await
    {
        Some(snapshot) => snapshot,
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
        Ok(ServerMenuActivateOutcome::Agent(outcome)) => {
            handle_agent_picker_outcome(
                codec,
                stream,
                menu_sessions,
                reload_server,
                client_id,
                bound_tab_id,
                session_id,
                outcome,
            )
            .await
        }
        Ok(ServerMenuActivateOutcome::Dispatch(activation)) => {
            match activation {
                ServerMenuActivation::Command(request) => {
                    // Selecting "Browse Filesystem" from the Control Center
                    // opens the Path Browser through the same shared helper
                    // as its keybinding. The swap is ATOMIC: the replacement
                    // session is built first (its inventory can take
                    // seconds), and only then is the old session reported
                    // closed ahead of the new snapshot — the modal must never
                    // vanish-then-reappear, because that gap reads as "the
                    // button did nothing" and invites racing retries.
                    if request.command_id == OPEN_PATH_BROWSER_COMMAND_ID
                        || picker_kind_for_command(&request.command_id).is_some()
                    {
                        match open_command_centre_session(
                            &request.command_id,
                            menu_sessions,
                            behavior,
                            runtime_generation,
                            document,
                            workspace,
                            tab_registry,
                            bound_tab_id,
                            reload_server.map(|server| &server.agent),
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
                                menu_sessions.cancel(session_id);
                                codec
                                    .write_server_message(
                                        stream,
                                        &ServerMessage::TransientMenuClosed { session_id },
                                    )
                                    .await?;
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
                    menu_sessions.cancel(session_id);
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::TransientMenuClosed { session_id },
                        )
                        .await?;
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
                    menu_sessions.cancel(session_id);
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::TransientMenuClosed { session_id },
                        )
                        .await?;
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

#[allow(clippy::too_many_arguments)]
async fn handle_agent_picker_outcome<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    reload_server: Option<&IpcServer>,
    client_id: ClientId,
    bound_tab_id: Option<TabId>,
    session_id: u64,
    outcome: AgentPickerActivate,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let host = reload_server.map(|server| &server.agent);
    match outcome {
        AgentPickerActivate::StayOpen => {
            push_active_picker(codec, stream, menu_sessions, client_id, session_id).await
        }
        AgentPickerActivate::PutSecret {
            provider,
            name,
            secret,
        } => {
            if let Some(host) = host {
                if let Err(error) = host.put_credential(&provider, &name, &secret).await {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::RuntimeDiagnostic(
                                crate::protocol::RuntimeDiagnostic::error(
                                    "agent.credential_put_failed",
                                    error.to_string(),
                                ),
                            ),
                        )
                        .await?;
                    return push_active_picker(codec, stream, menu_sessions, client_id, session_id)
                        .await;
                }
                host.select_picker(
                    crate::protocol::AgentPickerKind::Provider,
                    &provider,
                    bound_tab_id,
                )
                .await;
                let models = host
                    .picker_inventory()
                    .await
                    .models
                    .iter()
                    .filter(|model| model.provider == provider)
                    .count();
                let message = if models > 0 {
                    format!("{provider} configured — {models} model(s) available")
                } else {
                    format!(
                        "{provider} API key stored; no models discovered yet — pick a model after it appears"
                    )
                };
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::RuntimeDiagnostic(
                            crate::protocol::RuntimeDiagnostic::info(
                                "agent.credential_stored",
                                message,
                            ),
                        ),
                    )
                    .await?;
            }
            menu_sessions.cancel(session_id);
            codec
                .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
                .await?;
            Ok(())
        }
        AgentPickerActivate::StartOauth { provider } => {
            if let Some(host) = host {
                match host.start_oauth(&provider).await {
                    Ok(start) => {
                        if let Some(session) = menu_sessions.get_mut(session_id)
                            && let Some(picker) = session.agent_picker_mut()
                        {
                            let uri = if start.user_code.is_empty() {
                                start.authorization_url
                            } else {
                                start.verification_uri
                            };
                            picker.enter_oauth(start.login_id, start.user_code, uri.clone());
                            // Plan 116: automatically try the default OS
                            // browser once the OAuth stage appears; if that
                            // fails the stage itself offers "Open in
                            // browser" / "Copy URL" fallbacks.
                            if let Err(error) = crate::server::open::open_url(&uri) {
                                codec
                                    .write_server_message(
                                        stream,
                                        &ServerMessage::RuntimeDiagnostic(
                                            crate::protocol::RuntimeDiagnostic::warning(
                                                "agent.oauth_auto_open_failed",
                                                format!(
                                                    "Could not open the authorization URL automatically: {error}. Use 'Open in browser' or 'Copy URL' below."
                                                ),
                                            ),
                                        ),
                                    )
                                    .await?;
                            }
                        }
                    }
                    Err(error) => {
                        codec
                            .write_server_message(
                                stream,
                                &ServerMessage::RuntimeDiagnostic(
                                    crate::protocol::RuntimeDiagnostic::error(
                                        "agent.oauth_start_failed",
                                        error.to_string(),
                                    ),
                                ),
                            )
                            .await?;
                    }
                }
            }
            push_active_picker(codec, stream, menu_sessions, client_id, session_id).await
        }
        AgentPickerActivate::PollOauth { login_id } => {
            let done = match host {
                Some(host) => matches!(
                    host.poll_oauth(&login_id).await,
                    Ok(crate::server::agent::AgentOauthPoll::Complete)
                ),
                None => false,
            };
            if done {
                menu_sessions.cancel(session_id);
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::TransientMenuClosed { session_id },
                    )
                    .await?;
                Ok(())
            } else {
                push_active_picker(codec, stream, menu_sessions, client_id, session_id).await
            }
        }
        // Plan 116: manual browser-open and clipboard-copy fallbacks for the
        // OAuth authorization URL. Both keep the stage open so the user can
        // switch browsers, re-open, or check authorization afterwards.
        AgentPickerActivate::OpenOauthUrl { uri } => {
            match crate::server::open::open_url(&uri) {
                Ok(()) => {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::RuntimeDiagnostic(
                                crate::protocol::RuntimeDiagnostic::info(
                                    "agent.oauth_open_url",
                                    "Opened the authorization URL in your default browser.",
                                ),
                            ),
                        )
                        .await?;
                }
                Err(error) => {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::RuntimeDiagnostic(
                                crate::protocol::RuntimeDiagnostic::error(
                                    "agent.oauth_open_url_failed",
                                    format!(
                                        "Could not open the authorization URL: {error}. Use 'Copy URL' to open it in another browser."
                                    ),
                                ),
                            ),
                        )
                        .await?;
                }
            }
            push_active_picker(codec, stream, menu_sessions, client_id, session_id).await
        }
        AgentPickerActivate::CopyOauthUrl { uri } => {
            match crate::server::open::copy_to_clipboard(&uri) {
                Ok(()) => {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::RuntimeDiagnostic(
                                crate::protocol::RuntimeDiagnostic::info(
                                    "agent.oauth_copy_url",
                                    "Authorization URL copied to the clipboard.",
                                ),
                            ),
                        )
                        .await?;
                }
                Err(error) => {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::RuntimeDiagnostic(
                                crate::protocol::RuntimeDiagnostic::error(
                                    "agent.oauth_copy_url_failed",
                                    format!("Could not copy the authorization URL: {error}."),
                                ),
                            ),
                        )
                        .await?;
                }
            }
            push_active_picker(codec, stream, menu_sessions, client_id, session_id).await
        }
        AgentPickerActivate::Select { kind, id } => {
            if let Some(host) = host {
                // Index 871 and the menu-session helpers all resolve the tab
                // as `bound_tab_id.unwrap_or(client_id)`; a bare `None` here
                // published a session-less snapshot over a live panel.
                host.select_picker(kind, &id, Some(bound_tab_id.unwrap_or(client_id)))
                    .await;
                // Visible confirmation: without this, choosing a model or
                // provider just closes the modal with no shell-visible
                // change (silent-success reads as a dead button).
                let bare = id
                    .strip_prefix("provider:")
                    .or_else(|| id.strip_prefix("model:"))
                    .or_else(|| id.strip_prefix("agent:"))
                    .unwrap_or(&id);
                let subject = match kind {
                    crate::protocol::AgentPickerKind::Provider => {
                        format!("Provider {bare} selected")
                    }
                    crate::protocol::AgentPickerKind::Model => {
                        format!("Model {bare} selected")
                    }
                    _ => format!("{bare} selected"),
                };
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::RuntimeDiagnostic(
                            crate::protocol::RuntimeDiagnostic::info(
                                "agent.picker_selected",
                                subject,
                            ),
                        ),
                    )
                    .await?;
            }
            menu_sessions.cancel(session_id);
            codec
                .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
                .await?;
            Ok(())
        }
        AgentPickerActivate::Resume {
            session_id: agent_session,
            entry_id,
        } => {
            if let Some(host) = host {
                let tab = bound_tab_id.unwrap_or(client_id);
                let snapshot = host
                    .resume_tab(tab, &agent_session, entry_id.as_deref())
                    .await;
                codec
                    .write_server_message(stream, &ServerMessage::Agent(Box::new(snapshot)))
                    .await?;
            }
            menu_sessions.cancel(session_id);
            codec
                .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
                .await?;
            Ok(())
        }
        AgentPickerActivate::Delete {
            session_id: agent_session,
        } => {
            if let Some(host) = host {
                host.dispatch(crate::protocol::AgentClientCommand::DeleteSession {
                    session_id: agent_session,
                });
                let inventory = host.picker_inventory().await;
                if let Some(session) = menu_sessions.get_mut(session_id)
                    && let Some(picker) = session.agent_picker_mut()
                {
                    picker.replace_inventory(inventory);
                }
            }
            push_active_picker(codec, stream, menu_sessions, client_id, session_id).await
        }
    }
}

/// Session-search picker refresh (plan 108 task 11): re-runs the
/// workspace-scoped FTS query and installs the bounded hit page. Returns
/// `None` for every other session kind (or without a host) so callers keep
/// their locally projected snapshot.
async fn session_search_snapshot(
    menu_sessions: &mut ServerMenuSessions,
    session_id: u64,
    agent: Option<&AgentHost>,
    tab: TabId,
    query: &str,
) -> Option<crate::shell::transient_menu::TransientMenuSession> {
    let kind = menu_sessions
        .get(session_id)
        .and_then(|session| session.agent_picker_ref().map(|picker| picker.kind()))?;
    if kind != AgentPickerKind::SessionSearch {
        return None;
    }
    let hits: Vec<AgentSearchHit> = match (agent, query.is_empty()) {
        (Some(host), false) => host
            .search_sessions(tab, query, AGENT_SESSION_SEARCH_LIMIT)
            .await
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    menu_sessions.get_mut(session_id).and_then(|session| {
        session
            .agent_picker_mut()
            .map(|picker| picker.set_search_hits(hits))
    })
}

async fn push_active_picker<S>(
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
    codec
        .write_server_message(
            stream,
            &ServerMessage::TransientMenuSnapshot(Box::new(snapshot_from_session(
                &session.session(),
            ))),
        )
        .await?;
    Ok(())
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

/// A command catalogue is generation-bound: close the active menu session
/// before replaying the replacement generation's state (the loop's
/// runtime-state lane); activation also checks the stamp if both events race.
pub(super) async fn write_active_menu_session_closed<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    if let Some(session_id) = menu_sessions.cancel_active() {
        codec
            .write_server_message(stream, &ServerMessage::TransientMenuClosed { session_id })
            .await?;
    }
    Ok(())
}
