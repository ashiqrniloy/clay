//! Phase 22.7 (plan 078): the window driver's tab subsystem.
//!
//! Extracted from `main.rs` (review findings C1/C2/D7/D8): the `Driver`
//! struct, per-tab state, tab lifecycle (mount/switch/close/reconnect),
//! registry reconcile, the restore state machine, and persistence
//! orchestration. `main.rs` keeps the window/run-loop/dialog/CLI concerns
//! and the action dispatch, calling into this module through `Driver`.
//!
//! Module map (one line each):
//! - `mod.rs`: driver state, tab lifecycle, tab commands, persistence,
//!   event-bridge plumbing, and the typed access helpers.
//! - `reconcile.rs`: server-authoritative registry snapshot → driver state.
//! - `restore.rs`: the whole-window restore state machine.

mod reconcile;
mod restore;

use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use masonry::app::RenderRoot;
use masonry::core::{MutateCtx, WidgetId};
use masonry_winit::app::{DriverCtx, EventLoopProxy, MasonryUserEvent, WindowId};
use tokio::sync::mpsc;

use clay::client::{self, ClientConnectionEvent, ClientEditQueue};
use clay::ipc::IpcEndpoint;
use clay::masonry_editor::{EditorAction, EditorWidget};
use clay::masonry_pane_document::PaneDocumentView;
use clay::masonry_shell::ClayShellWidget;
use clay::protocol::{
    ClientId, DocumentMetadata, SduiActionIntent, SduiActionValue, TabId, TabRegistrySnapshot,
    WorkspaceRootId,
};
use clay::shell::{PaneId, PersistedTabState, PersistedWindowState, TransientMenuSession};

/// Phase 22.5: a restore mount must be confirmed by a registry snapshot
/// within this window; a server-rejected mount never confirms (the server
/// answers `FileOperationFailed` instead of a snapshot), so the deadline
/// abandons the remaining restore rather than stall the gate. Confirmation
/// is normally sub-second (handshake replay + `TabCommand::New` broadcast).
pub(crate) const RESTORE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) struct Driver {
    /// Phase 22.3: the active tab's chrome widget id (mirror of the shell's
    /// active tab; updated on switch). Active-tab operations (SDUI intents,
    /// native dialog completions, keybinding commands) route through it.
    pub(crate) editor_widget_id: WidgetId,
    /// Phase 22.1: the ClayShellWidget's id (root widget) for shell command dispatch.
    pub(crate) shell_widget_id: WidgetId,
    pub(crate) window_id: WindowId,
    /// Phase 22.3: one tab state per connection, keyed by the connection's
    /// `ClientId` (the client-known identity at mount time; the server-assigned
    /// `TabId` arrives asynchronously via the registry snapshot).
    pub(crate) tabs: BTreeMap<ClientId, TabState>,
    /// Phase 22.3: the mounted (active) tab.
    pub(crate) active_tab: ClientId,
    /// Phase 22.3: the latest server-authoritative tab registry snapshot.
    pub(crate) registry: TabRegistrySnapshot,
    /// Monotonic registry revision of the last applied snapshot. Registry
    /// relays from different connections interleave (a connection's handshake
    /// replay races the broadcast of its own pending tab command); a snapshot
    /// that does not advance this revision is stale and must be discarded, or
    /// it would delete tabs a newer broadcast already confirmed.
    pub(crate) registry_revision: Option<u64>,
    /// Phase 22.3: runtime handle for per-tab event bridges and reconnect /
    /// new-tab connect tasks.
    pub(crate) runtime: tokio::runtime::Handle,
    /// Phase 22.3: the IPC endpoint used for per-tab reconnects and new-tab
    /// connections.
    pub(crate) endpoint: IpcEndpoint,
    /// Phase 22.3: cancellation flags for in-flight per-tab reconnect tasks
    /// (set when the tab is removed so a retrying task exits instead of
    /// connecting forever for a closed tab).
    pub(crate) reconnect_cancel: BTreeMap<ClientId, Arc<std::sync::atomic::AtomicBool>>,
    /// Present in the live app; unit tests that only check action targeting may omit it.
    pub(crate) proxy: Option<EventLoopProxy>,
    pub(crate) dialog_generation: u64,
    pub(crate) file_dialog_in_flight: Option<u64>,
    pub(crate) folder_dialog_in_flight: Option<u64>,
    /// Phase 22.4: a "Save all and close" tab-close flow awaiting save
    /// completions. `Some((client_id, pending_document_ids))` while the tab's
    /// dirty documents save; each `DocumentSaved` for the tab removes its
    /// document id, and at zero the driver enqueues `TabCommand::Close`. A
    /// failed save (or a disconnect) cancels the flow — the close never
    /// happens until every save acked.
    pub(crate) pending_close_after_saves: Option<(u64, std::collections::BTreeSet<u64>)>,
    /// Phase 22.4: session-id counter for driver-owned tab-close confirm
    /// menus (the views use their own counters for their own sessions).
    pub(crate) tab_menu_session_id: u64,
    /// Phase 22.5: persisted tabs still to mount, `(persisted index, tab)`.
    pub(crate) restore_queue: VecDeque<(usize, PersistedTabState)>,
    /// Phase 22.5: the restore tab whose connection is in flight.
    pub(crate) restore_pending: Option<(usize, PersistedTabState)>,
    /// Phase 22.5: mounted restore tabs, `(client id, persisted index, tab)`.
    pub(crate) restore_mounted: Vec<(ClientId, usize, PersistedTabState)>,
    /// Phase 22.5: the restore mount awaiting its server `TabId` confirmation
    /// and the deadline after which the restore is abandoned (a
    /// server-rejected mount never confirms; the deadline keeps the gate from
    /// stalling forever). `None` when no restore is in flight.
    pub(crate) restore_gate: Option<(ClientId, Instant)>,
    /// Phase 22.5: persisted active-tab index (0-based; `None` when the file
    /// had none — restore falls back to the first mounted tab).
    pub(crate) restore_active: Option<usize>,
    /// Phase 22.5: restore skip diagnostics, flushed to the chrome when the
    /// restore settles (finishes or is abandoned).
    pub(crate) restore_diagnostics: Vec<String>,
}

/// Phase 22.3: the driver-side state of one tab: its connection's command
/// channel, pending-open attribution, and the server-assigned `TabId` once the
/// registry snapshot confirms it. The tab's chrome, split tree, pane targets,
/// and focus policy live in the shell's `TabChrome` (keyed by the same
/// `ClientId`); the session itself is consumed at mount (initial state → chrome,
/// events → bridge, edit queue → chrome + this state).
pub(crate) struct TabState {
    pub(crate) edit_queue: Option<ClientEditQueue>,
    /// Phase 22.2: pending file-open attribution. One entry per requesting
    /// pane (bounded by pane count; replaced on a new request from the same
    /// pane), consumed when the server's `DocumentOpened` for that request
    /// arrives so an unmapped open loads into exactly the pane that asked.
    pub(crate) pending_opens: BTreeMap<PaneId, PendingOpenRequest>,
    pub(crate) tab_id: Option<TabId>,
    /// Phase 22.3: the workspace root this tab was mounted with (tab bar card
    /// name fallback while the tab awaits its server registry entry).
    pub(crate) workspace_root: String,
}

/// Phase 22.3: the server registry diff applied to the shell: tabs to
/// uninstall, the tab to activate (switch + focus), and the tab bar cards
/// (registry order/names; mounted tabs awaiting their entry are appended with
/// close disabled).
pub(crate) struct TabRegistryReconcile {
    pub(crate) removed: Vec<ClientId>,
    pub(crate) new_active: Option<ClientId>,
    pub(crate) cards: Vec<clay::masonry_shell::TabCard>,
}

/// Phase 22.2: the client-known identity of one in-flight open request.
/// Exactly one of [`Self::path`] (native dialog / selected-file flows, where
/// the client knows the absolute path) or [`Self::root_id`] +
/// [`Self::relative_path`] (file-browser / fuzzy / definition-navigation
/// intents, where the server answers with a workspace-relative path) is set.
pub(crate) struct PendingOpenRequest {
    pub(crate) path: Option<PathBuf>,
    pub(crate) root_id: Option<WorkspaceRootId>,
    pub(crate) relative_path: Option<String>,
}

/// Resolve a pending-open request to its requesting pane (and consume it).
pub(crate) fn take_pending_open_for(
    pending: &mut BTreeMap<PaneId, PendingOpenRequest>,
    metadata: &DocumentMetadata,
) -> Option<PaneId> {
    let pane = pending.iter().find_map(|(pane, request)| {
        let matched = match (
            request.path.as_ref(),
            request.root_id,
            request.relative_path.as_deref(),
        ) {
            (Some(path), _, _) => path.as_path() == Path::new(metadata.path.as_str()),
            (None, Some(root_id), Some(relative_path)) => {
                root_id == metadata.workspace_root_id && relative_path == metadata.path.as_str()
            }
            _ => false,
        };
        matched.then_some(*pane)
    });
    if let Some(pane) = pane {
        pending.remove(&pane);
    }
    pane
}

/// Phase 22.5: the persistence tab order — server registry order first
/// (mount order at restore), then mounted tabs still awaiting their registry
/// entry appended in client-id order.
pub(crate) fn ordered_tab_clients(
    registry: &TabRegistrySnapshot,
    mounted: &BTreeMap<ClientId, TabState>,
) -> Vec<ClientId> {
    let mut ordered: Vec<ClientId> = registry.tabs.iter().map(|entry| entry.client_id).collect();
    for client_id in mounted.keys() {
        if !ordered.contains(client_id) {
            ordered.push(*client_id);
        }
    }
    ordered
}

/// Extract the pending-open identity from a file-browser / fuzzy-open /
/// definition-navigation intent, when it is a workspace open command with the
/// standard `(workspaceRootId, relativePath)` arguments.
pub(crate) fn open_intent_pending_request(intent: &SduiActionIntent) -> Option<PendingOpenRequest> {
    if !matches!(
        intent.command_id.as_str(),
        "clay.workspace.openFile" | "clay.workspace.openFuzzyFile"
    ) {
        return None;
    }
    let root_id = intent.arguments.iter().find_map(|argument| {
        if argument.name == "workspaceRootId"
            && let SduiActionValue::U64(root_id) = argument.value
        {
            return Some(root_id);
        }
        None
    })?;
    let relative_path = intent.arguments.iter().find_map(|argument| {
        if argument.name == "relativePath"
            && let SduiActionValue::String(path) = &argument.value
        {
            return Some(path.clone());
        }
        None
    })?;
    Some(PendingOpenRequest {
        path: None,
        root_id: Some(root_id),
        relative_path: Some(relative_path),
    })
}

/// Typed access helpers replacing the repeated
/// `render_root().edit_widget(...) + try_downcast` boilerplate across the
/// moved tab-subsystem call sites (plan 078; findings C1/C2/D7/D8). Each
/// helper downcasts once and hands the widget and its `MutateCtx` to the
/// closure; `None` when the widget is not of the expected type.
/// Typed widget access: edit `id` and downcast in one step. Returns `None`
/// when the downcast fails; a MISSING id panics (masonry `edit_widget`
/// semantics — the pre-extraction call sites had the same behavior).
pub(crate) fn with_shell<R>(
    root: &mut RenderRoot,
    shell_id: WidgetId,
    f: impl FnOnce(&mut ClayShellWidget, &mut MutateCtx<'_>) -> R,
) -> Option<R> {
    root.edit_widget(shell_id, |mut widget| {
        widget
            .try_downcast::<ClayShellWidget>()
            .map(|mut shell| f(shell.widget, &mut shell.ctx))
    })
}

/// Same semantics as [`with_shell`] for the tab's chrome `EditorWidget`.
pub(crate) fn with_editor<R>(
    root: &mut RenderRoot,
    editor_id: WidgetId,
    f: impl FnOnce(&mut EditorWidget, &mut MutateCtx<'_>) -> R,
) -> Option<R> {
    root.edit_widget(editor_id, |mut widget| {
        widget
            .try_downcast::<EditorWidget>()
            .map(|mut editor| f(editor.widget, &mut editor.ctx))
    })
}

/// Same semantics as [`with_shell`] for a pane's `PaneDocumentView`.
pub(crate) fn with_view<R>(
    root: &mut RenderRoot,
    view_id: WidgetId,
    f: impl FnOnce(&mut PaneDocumentView, &mut MutateCtx<'_>) -> R,
) -> Option<R> {
    root.edit_widget(view_id, |mut widget| {
        widget
            .try_downcast::<PaneDocumentView>()
            .map(|mut view| f(view.widget, &mut view.ctx))
    })
}

impl Driver {
    pub(crate) fn editor_action_target(&self, _source_widget_id: WidgetId) -> WidgetId {
        // Phase 18.2 has one editor component under the shell root. Keep
        // editor-specific actions aimed at that child even if Masonry reports a
        // shell/root source while the container boundary is settling.
        self.editor_widget_id
    }

    /// Phase 22.1: the shell widget id for shell-pane command dispatch.
    pub(crate) fn shell_action_target(&self) -> WidgetId {
        self.shell_widget_id
    }

    /// Route a reconciled SDUI widget's inert intent (button step 9 / list row
    /// step 10, and future transient surfaces) through the editor's existing
    /// server-first command path.
    pub(crate) fn route_sdui_intent(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        widget_id: WidgetId,
        intent: SduiActionIntent,
    ) {
        // Phase 22.2: workspace open intents (file browser, fuzzy open,
        // definition navigation) record the active pane as the open target so
        // the answering DocumentOpened loads into exactly that pane even if
        // focus moves before the server responds.
        if let Some(request) = open_intent_pending_request(&intent) {
            self.record_pending_open(ctx, window_id, request);
        }
        let editor_widget_id = self.editor_action_target(widget_id);
        with_editor(ctx.render_root(window_id), editor_widget_id, |editor, _| {
            editor.enqueue_sdui_intent(intent);
        });
    }

    /// Phase 22.2: record which pane requested a file open. One entry per
    /// pane (replaced on re-request), so pending-open state is bounded by the
    /// pane count. Phase 22.3: attribution is per-tab (the active tab's panes).
    pub(crate) fn record_pending_open(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        request: PendingOpenRequest,
    ) {
        let active = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, _| shell.active_pane_id(),
        )
        .unwrap_or(PaneId(1));
        if let Some(tab) = self.tabs.get_mut(&self.active_tab) {
            tab.pending_opens.insert(active, request);
        }
    }

    /// Phase 22.3/22.8: re-key a reconnected tab: swap the fresh
    /// connection's queue into the chrome and every pane view, collect the
    /// documents to re-open, re-key the tab to the new connection's identity,
    /// rebind its registry entry (`Reclaim`, or root-scoped `New` after a
    /// server reset/eviction), and restore the active mirror when the tab was
    /// active. A tab closed while
    /// reconnecting drops the fresh session (ending the connection and
    /// releasing the server permit) — the `tabs.remove` guard handles it.
    pub(crate) fn reconnect_tab(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        session: clay::masonry_editor::DriverSession,
    ) {
        self.reconnect_cancel.remove(&client_id);
        // The tab was closed while reconnecting: dropping the fresh
        // session ends the connection and releases the server permit.
        let Some(mut tab) = self.tabs.remove(&client_id) else {
            return;
        };
        let session = session.session;
        let new_client_id = session.initial_state.client_id;
        let edit_queue = session.edit_queue.clone();
        let was_active = self.active_tab == client_id;
        let chrome_id = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, _| shell.editor_widget_id_for(client_id),
        )
        .flatten();
        let Some(chrome_id) = chrome_id else {
            return;
        };
        // Swap the fresh connection's queue into the chrome and every
        // pane view, and collect the documents to re-open (the
        // retained sessions are the split-tree/per-pane restore
        // source; re-opening replaces them with fresh server state).
        let mut reopen = Vec::new();
        with_editor(ctx.render_root(window_id), chrome_id, |editor, _| {
            editor.reconnect(edit_queue.clone());
            reopen.extend(editor.documents_for_reopen());
        });
        let pane_targets = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, _| shell.pane_targets_for(client_id),
        )
        .unwrap_or_default();
        for (_, target) in pane_targets {
            if target == chrome_id {
                continue;
            }
            with_view(ctx.render_root(window_id), target, |view, _| {
                view.reconnect(edit_queue.clone());
                reopen.extend(view.documents_for_reopen());
            });
        }
        // Re-key the tab to the new connection's identity; widget ids
        // are unchanged, so mirrors and bridges keep working.
        with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, shell_ctx| {
                shell.rekey_tab(shell_ctx, client_id, new_client_id);
                shell_ctx.request_render();
            },
        );
        if was_active {
            self.active_tab = new_client_id;
            self.editor_widget_id = chrome_id;
        }
        tab.edit_queue = Some(edit_queue.clone());
        // In-flight open requests died with the old connection; clear
        // them so a later open is never mis-attributed to a stale pane.
        tab.pending_opens.clear();
        // The fresh connection already reclaimed/bound its tab during the
        // handshake; only re-open documents after that per-tab state arrives.
        // Re-open the tab's documents through the plain `OpenDocument`
        // path (a fresh connection holds no selected-file capability
        // for documents it opened before the drop).
        for (root_id, path) in reopen {
            let _ = edit_queue.enqueue_open_document(root_id, path);
        }
        if let Some(proxy) = self.proxy.clone() {
            spawn_client_connection_event_bridge(
                &self.runtime,
                session.events,
                proxy,
                window_id,
                chrome_id,
            );
        }
        self.tabs.insert(new_client_id, tab);
        if was_active {
            let target = with_shell(
                ctx.render_root(window_id),
                self.shell_widget_id,
                |shell, _| shell.active_pane_target_for(new_client_id),
            )
            .flatten();
            if let Some(target) = target {
                let _ = ctx.render_root(window_id).focus_on(Some(target));
            }
        }
    }

    /// Mount an already-bound session as a new tab. The handshake already
    /// registered the server tab and installed its deferred initial document;
    /// this mounts the tab's chrome + default split tree, spawns its event bridge, and
    /// switches to it. Returns `None` when the connection is already mounted
    /// (duplicate) — the caller keeps the previous tab.
    pub(crate) fn mount_tab(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        session: client::ClientSession,
        workspace_root: PathBuf,
    ) -> Option<ClientId> {
        let client_id = session.initial_state.client_id;
        if self.tabs.contains_key(&client_id) {
            return None;
        }
        let chrome = EditorWidget::with_initial_state(session.initial_state)
            .with_edit_queue(session.edit_queue.clone());
        let edit_queue = session.edit_queue.clone();
        let events = session.events;
        let tab_chrome = clay::masonry_shell::TabChrome::single_editor(chrome, false);
        let chrome_id = tab_chrome.editor_widget_id();
        with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, shell_ctx| {
                shell.install_tab(shell_ctx, client_id, tab_chrome);
                shell.set_active_tab(shell_ctx, client_id);
                // Phase 22.6: one polite announcement per user-initiated
                // tab (restore mounts don't route here).
                shell.announce_tab_created(shell_ctx, &workspace_root.to_string_lossy());
                shell_ctx.request_render();
            },
        );
        if let Some(proxy) = self.proxy.clone() {
            spawn_client_connection_event_bridge(
                &self.runtime,
                events,
                proxy,
                window_id,
                chrome_id,
            );
        }
        self.tabs.insert(
            client_id,
            TabState {
                edit_queue: Some(edit_queue),
                pending_opens: BTreeMap::new(),
                tab_id: None,
                workspace_root: workspace_root.to_string_lossy().into_owned(),
            },
        );
        self.active_tab = client_id;
        self.editor_widget_id = chrome_id;
        Some(client_id)
    }

    /// Switch the mounted tab: the shell mounts the target tab's chrome/tree
    /// (retaining the previous tab's state), the driver's active-tab mirror
    /// follows, and keyboard focus moves to the new tab's active pane.
    pub(crate) fn switch_tab(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
    ) -> bool {
        if client_id == self.active_tab || !self.tabs.contains_key(&client_id) {
            return false;
        }
        let (chrome_id, target) = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, shell_ctx| {
                if !shell.set_active_tab(shell_ctx, client_id) {
                    return (None, None);
                }
                shell_ctx.request_render();
                (
                    shell.editor_widget_id_for(client_id),
                    shell.active_pane_target_for(client_id),
                )
            },
        )
        .unwrap_or((None, None));
        self.active_tab = client_id;
        if let Some(chrome_id) = chrome_id {
            self.editor_widget_id = chrome_id;
        }
        if let Some(target) = target {
            let _ = ctx.render_root(window_id).focus_on(Some(target));
        }
        true
    }
}

/// Phase 22.4: advance a save-all-and-close tab flow on one connection
/// event. `DocumentSaved` for an awaited document counts down (returning
/// `true` when every save acked, so the caller enqueues `TabCommand::Close`);
/// a failed save or a disconnect cancels the flow (the close never happens
/// until every save acked). Events for other tabs leave the flow untouched.
pub(crate) fn advance_pending_close_after_saves(
    pending: &mut Option<(u64, std::collections::BTreeSet<u64>)>,
    client_id: u64,
    event: &ClientConnectionEvent,
) -> bool {
    let Some((pending_client, expected)) = pending.as_mut() else {
        return false;
    };
    if *pending_client != client_id {
        return false;
    }
    match event {
        ClientConnectionEvent::DocumentSaved { document_id, .. } => {
            expected.remove(document_id);
            if expected.is_empty() {
                *pending = None;
                true
            } else {
                false
            }
        }
        ClientConnectionEvent::FileOperationFailed { .. }
        | ClientConnectionEvent::Disconnected
        | ClientConnectionEvent::ConnectionError(_) => {
            *pending = None;
            false
        }
        _ => false,
    }
}

/// Phase 22.3: the tab card label for a workspace root path: the final path
/// segment, or the full path when it has none.
pub(crate) fn tab_card_display_name(workspace_root: &str) -> String {
    std::path::Path::new(workspace_root)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| workspace_root.to_string())
}

pub(crate) fn spawn_client_connection_event_bridge(
    runtime: &tokio::runtime::Handle,
    mut events: mpsc::Receiver<ClientConnectionEvent>,
    proxy: EventLoopProxy,
    window_id: WindowId,
    editor_widget_id: WidgetId,
) {
    runtime.spawn(async move {
        while let Some(event) = events.recv().await {
            eprintln!("clay client IPC event: {event:?}");
            if proxy
                .send_event(connection_event_user_event(
                    window_id,
                    editor_widget_id,
                    event,
                ))
                .is_err()
            {
                break;
            }
        }
    });
}

pub(crate) fn connection_event_user_event(
    window_id: WindowId,
    editor_widget_id: WidgetId,
    event: ClientConnectionEvent,
) -> MasonryUserEvent {
    MasonryUserEvent::Action(
        window_id,
        Box::new(EditorAction::ClientConnection(event)),
        editor_widget_id,
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use clay::ipc::default_endpoint;

    #[test]
    fn connection_event_action_is_dispatched_to_shell_editor_child() {
        let window_id = WindowId::next();
        let shell = clay::masonry_shell::ClayShellWidget::single_editor(
            0,
            clay::masonry_editor::EditorWidget::default(),
        );
        let widget_id = shell.editor_widget_id();
        let event = ClientConnectionEvent::Disconnected;

        let user_event = connection_event_user_event(window_id, widget_id, event.clone());

        match user_event {
            MasonryUserEvent::Action(action_window_id, action, action_widget_id) => {
                assert_eq!(action_window_id, window_id);
                assert_eq!(action_widget_id, widget_id);
                assert_eq!(
                    *action
                        .downcast::<clay::masonry_editor::EditorAction>()
                        .expect("connection action type"),
                    clay::masonry_editor::EditorAction::ClientConnection(event)
                );
            }
            MasonryUserEvent::AccessKit(..) => panic!("connection events must use actions"),
        }
    }

    fn metadata(document_id: u64, path: &str) -> clay::protocol::DocumentMetadata {
        clay::protocol::DocumentMetadata {
            document_id,
            version: 1,
            access: clay::protocol::DocumentAccess::Editable {
                lease_id: document_id,
            },
            lease_id: Some(document_id),
            dirty: false,
            workspace_root_id: 77,
            path: path.to_string(),
        }
    }

    #[test]
    fn pending_open_absolute_path_matches_and_is_consumed() {
        let mut pending = BTreeMap::new();
        pending.insert(
            PaneId(2),
            PendingOpenRequest {
                path: Some(PathBuf::from("/home/user/proj/src/main.rs")),
                root_id: None,
                relative_path: None,
            },
        );
        // Matching DocumentOpened (native dialog / selected-file flow answers
        // with the canonical absolute path): attributed to pane 2, consumed.
        assert_eq!(
            take_pending_open_for(&mut pending, &metadata(9, "/home/user/proj/src/main.rs")),
            Some(PaneId(2))
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn pending_open_workspace_relative_reference_matches_by_root_and_path() {
        let mut pending = BTreeMap::new();
        pending.insert(
            PaneId(3),
            PendingOpenRequest {
                path: None,
                root_id: Some(77),
                relative_path: Some("src/lib.rs".to_string()),
            },
        );
        // File-browser / fuzzy / definition-navigation answers carry the
        // workspace-relative path.
        assert_eq!(
            take_pending_open_for(&mut pending, &metadata(9, "src/lib.rs")),
            Some(PaneId(3))
        );
        assert!(pending.is_empty(), "matched entries are consumed");
    }

    #[test]
    fn pending_open_does_not_match_other_paths_or_roots() {
        let mut pending = BTreeMap::new();
        pending.insert(
            PaneId(2),
            PendingOpenRequest {
                path: Some(PathBuf::from("/home/user/proj/a.md")),
                root_id: None,
                relative_path: None,
            },
        );
        pending.insert(
            PaneId(3),
            PendingOpenRequest {
                path: None,
                root_id: Some(77),
                relative_path: Some("src/lib.rs".to_string()),
            },
        );
        // Different file, different root, and a server-initiated open with no
        // pending entry at all must not match.
        assert_eq!(
            take_pending_open_for(&mut pending, &metadata(10, "/home/user/proj/b.md")),
            None
        );
        assert_eq!(
            take_pending_open_for(&mut pending, &metadata(10, "src/main.rs")),
            None
        );
        assert_eq!(pending.len(), 2, "unmatched entries stay pending");
    }

    #[test]
    fn pending_open_entries_are_bounded_per_pane() {
        let mut pending = BTreeMap::new();
        pending.insert(
            PaneId(2),
            PendingOpenRequest {
                path: Some(PathBuf::from("/a")),
                root_id: None,
                relative_path: None,
            },
        );
        // A new request from the same pane replaces the old one.
        pending.insert(
            PaneId(2),
            PendingOpenRequest {
                path: Some(PathBuf::from("/b")),
                root_id: None,
                relative_path: None,
            },
        );
        assert_eq!(pending.len(), 1);
        assert_eq!(
            take_pending_open_for(&mut pending, &metadata(11, "/b")),
            Some(PaneId(2))
        );
    }

    #[test]
    fn open_intent_pending_request_extracts_workspace_open_arguments() {
        fn intent(
            command_id: &str,
            root_id: Option<u64>,
            relative: Option<&str>,
        ) -> SduiActionIntent {
            let mut arguments = Vec::new();
            if let Some(root_id) = root_id {
                arguments.push(clay::protocol::SduiActionArgument {
                    name: "workspaceRootId".to_string(),
                    value: clay::protocol::SduiActionValue::U64(root_id),
                });
            }
            if let Some(relative) = relative {
                arguments.push(clay::protocol::SduiActionArgument {
                    name: "relativePath".to_string(),
                    value: clay::protocol::SduiActionValue::String(relative.to_string()),
                });
            }
            SduiActionIntent {
                command_id: command_id.to_string(),
                source: clay::protocol::SduiActionSource::Button {
                    node_id: clay::protocol::SduiNodeId(1),
                },
                arguments,
            }
        }
        // File browser and fuzzy open both carry (workspaceRootId, relativePath).
        for command_id in ["clay.workspace.openFile", "clay.workspace.openFuzzyFile"] {
            let request = open_intent_pending_request(&intent(command_id, Some(3), Some("a.md")))
                .expect("workspace open intent records a pending request");
            assert_eq!(request.path, None);
            assert_eq!(request.root_id, Some(3));
            assert_eq!(request.relative_path.as_deref(), Some("a.md"));
        }
        // Non-open intents and malformed arguments record nothing.
        assert!(
            open_intent_pending_request(&intent(
                "clay.workspace.revealInTree",
                Some(3),
                Some("a.md")
            ))
            .is_none()
        );
        assert!(
            open_intent_pending_request(&intent("clay.workspace.openFile", None, Some("a.md")))
                .is_none()
        );
        assert!(
            open_intent_pending_request(&intent("clay.workspace.openFile", Some(3), None))
                .is_none()
        );
    }

    #[test]
    fn driver_routes_editor_actions_to_shell_editor_child() {
        let editor_widget_id = WidgetId::next();
        let shell_or_source_widget_id = WidgetId::next();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let driver = Driver {
            editor_widget_id,
            shell_widget_id: WidgetId::next(),
            window_id: WindowId::next(),
            tabs: BTreeMap::new(),
            active_tab: 0,
            registry: TabRegistrySnapshot {
                tabs: Vec::new(),
                active: None,
                revision: 0,
            },
            registry_revision: None,
            runtime: runtime.handle().clone(),
            endpoint: default_endpoint(),
            reconnect_cancel: BTreeMap::new(),
            proxy: None,
            dialog_generation: 0,
            file_dialog_in_flight: None,
            folder_dialog_in_flight: None,
            pending_close_after_saves: None,
            tab_menu_session_id: 0,
            restore_queue: VecDeque::new(),
            restore_pending: None,
            restore_mounted: Vec::new(),
            restore_gate: None,
            restore_active: None,
            restore_diagnostics: Vec::new(),
        };

        assert_eq!(
            driver.editor_action_target(shell_or_source_widget_id),
            editor_widget_id
        );
    }

    #[test]
    fn smoke_launch_routes_sdui_events_to_gui() {
        let window_id = WindowId::next();
        let shell = clay::masonry_shell::ClayShellWidget::single_editor(
            0,
            clay::masonry_editor::EditorWidget::default(),
        );
        let widget_id = shell.editor_widget_id();
        let event = ClientConnectionEvent::SduiSnapshot {
            client_id: 1,
            tree: clay::protocol::SduiTree {
                ui_version: 1,
                root_id: clay::protocol::SduiNodeId(1),
                nodes: vec![clay::protocol::SduiNode::new(
                    clay::protocol::SduiNodeId(1),
                    clay::protocol::SduiNodeKind::Label {
                        text: "Workspace".to_string(),
                    },
                )],
            },
        };

        let user_event = connection_event_user_event(window_id, widget_id, event.clone());

        match user_event {
            MasonryUserEvent::Action(action_window_id, action, action_widget_id) => {
                assert_eq!(action_window_id, window_id);
                assert_eq!(action_widget_id, widget_id);
                assert_eq!(
                    *action
                        .downcast::<clay::masonry_editor::EditorAction>()
                        .expect("SDUI connection action type"),
                    clay::masonry_editor::EditorAction::ClientConnection(event)
                );
            }
            MasonryUserEvent::AccessKit(..) => panic!("SDUI events must use GUI actions"),
        }
    }

    // -- Phase 22.3: multi-connection tab model --

    pub(crate) fn test_driver_with_tabs(tabs: BTreeMap<ClientId, TabState>) -> Driver {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        Driver {
            editor_widget_id: WidgetId::next(),
            shell_widget_id: WidgetId::next(),
            window_id: WindowId::next(),
            tabs,
            active_tab: 0,
            registry: TabRegistrySnapshot {
                tabs: Vec::new(),
                active: None,
                revision: 0,
            },
            registry_revision: None,
            runtime: runtime.handle().clone(),
            endpoint: default_endpoint(),
            reconnect_cancel: BTreeMap::new(),
            proxy: None,
            dialog_generation: 0,
            file_dialog_in_flight: None,
            folder_dialog_in_flight: None,
            pending_close_after_saves: None,
            tab_menu_session_id: 0,
            restore_queue: VecDeque::new(),
            restore_pending: None,
            restore_mounted: Vec::new(),
            restore_gate: None,
            restore_active: None,
            restore_diagnostics: Vec::new(),
        }
    }

    #[test]
    fn tab_close_allowed_refuses_last_tab_and_unknown_tabs() {
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (1, tab_state_with_queue(ClientEditQueue::bounded(4).0)),
            (2, tab_state_with_queue(ClientEditQueue::bounded(4).0)),
        ]));
        driver.active_tab = 1;
        // Two tabs: either may close (the dirty guard is separate).
        assert!(driver.tab_close_allowed(1));
        assert!(driver.tab_close_allowed(2));
        // Unknown tabs never close.
        assert!(!driver.tab_close_allowed(9));
        // Closing down to one tab is allowed; the LAST tab is refused (the
        // window never goes to zero tabs — the bar hides at one tab).
        driver.tabs.remove(&2);
        assert!(!driver.tab_close_allowed(1), "the last tab cannot close");
    }

    #[test]
    fn tab_close_confirm_menu_names_tab_and_dirty_documents() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let mut driver =
            test_driver_with_tabs(BTreeMap::from([(11, tab_state_with_queue(queue_a))]));
        driver.registry = TabRegistrySnapshot {
            tabs: vec![clay::protocol::TabEntry {
                tab_id: 101,
                workspace_root_id: 7,
                client_id: 11,
                workspace_root: "/home/arn/work".to_string(),
            }],
            active: None,
            revision: 0,
        };

        // Phase 22.4: the confirm menu names the tab (final path segment) and
        // every dirty document; one document reads singular, several plural.
        let menu = driver.tab_close_confirm_menu(11, &["note.md".to_string()]);
        assert_eq!(
            menu.prompt(),
            "Close tab 'work' with 1 unsaved document (note.md)?"
        );
        let menu = driver.tab_close_confirm_menu(11, &["a.md".to_string(), "b.md".to_string()]);
        assert_eq!(
            menu.prompt(),
            "Close tab 'work' with 2 unsaved documents (a.md, b.md)?"
        );
        // Session ids are distinct per menu (driver-owned counter).
        assert_ne!(
            menu.session_id().0,
            driver
                .tab_close_confirm_menu(11, &["a.md".to_string()])
                .session_id()
                .0
        );
    }

    #[test]
    fn pending_close_after_saves_advances_acks_and_cancels_on_failure() {
        let saved = |document_id| ClientConnectionEvent::DocumentSaved {
            document_id,
            version: 1,
            dirty: false,
        };
        let failed = ClientConnectionEvent::FileOperationFailed {
            code: clay::protocol::FileErrorCode::DirtyDocument,
            message: "save failed".to_string(),
            workspace_root_id: Some(7),
            document_id: Some(7),
        };

        // No flow in flight: events pass through untouched.
        let mut pending = None;
        assert!(!advance_pending_close_after_saves(
            &mut pending,
            11,
            &saved(7)
        ));
        assert!(pending.is_none());

        // Partial acks keep the flow alive; the last ack completes it.
        let mut pending = Some((11, BTreeSet::from([7, 8])));
        assert!(!advance_pending_close_after_saves(
            &mut pending,
            11,
            &saved(7)
        ));
        assert!(pending.is_some(), "one of two acks still pending");
        assert!(advance_pending_close_after_saves(
            &mut pending,
            11,
            &saved(8)
        ));
        assert!(
            pending.is_none(),
            "flow completed; caller enqueues the close"
        );

        // A failed save cancels the close (the diagnostic surfaces in the pane).
        let mut pending = Some((11, BTreeSet::from([7])));
        assert!(!advance_pending_close_after_saves(
            &mut pending,
            11,
            &failed
        ));
        assert!(pending.is_none(), "failed save cancels the flow");

        // Other tabs' events never touch this tab's flow.
        let mut pending = Some((11, BTreeSet::from([7])));
        assert!(!advance_pending_close_after_saves(
            &mut pending,
            22,
            &saved(7)
        ));
        assert!(pending.is_some(), "another tab's ack is ignored");
    }

    pub(crate) fn tab_state_with_queue(queue: ClientEditQueue) -> TabState {
        TabState {
            edit_queue: Some(queue),
            pending_opens: BTreeMap::new(),
            tab_id: None,
            workspace_root: "/tmp/root".to_string(),
        }
    }

    // -- Phase 22.4: keyboard tab management --

    pub(crate) fn tab_snapshot(
        entries: &[(ClientId, clay::protocol::TabId)],
    ) -> TabRegistrySnapshot {
        TabRegistrySnapshot {
            tabs: entries
                .iter()
                .map(|(client_id, tab_id)| clay::protocol::TabEntry {
                    tab_id: *tab_id,
                    workspace_root_id: 1,
                    client_id: *client_id,
                    workspace_root: format!("/tmp/root-{client_id}"),
                })
                .collect(),
            active: None,
            revision: 0,
        }
    }

    #[test]
    fn tab_order_is_registry_order_with_entry_less_mounted_appended() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let (queue_c, _receiver_c) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
            (33, tab_state_with_queue(queue_c)),
        ]));
        // Server registry order: 33, 11. Tab 22 is mounted but has no
        // registry entry yet (entry-less) and appends.
        driver.registry = tab_snapshot(&[(33, 301), (11, 302)]);

        assert_eq!(driver.tab_order(), vec![33, 11, 22]);
        assert_eq!(driver.tab_position_of(33), Some(1));
        assert_eq!(driver.tab_position_of(11), Some(2));
        assert_eq!(driver.tab_position_of(22), Some(3));
        assert_eq!(driver.tab_position_of(99), None);
        // Numbered activation is 1-based over the same order.
        assert_eq!(driver.tab_at_position(1), Some(33));
        assert_eq!(driver.tab_at_position(3), Some(22));
        // Bounds policy: position 0 and positions beyond the tab count are
        // silent no-ops — never switch to a non-existent position.
        assert_eq!(driver.tab_at_position(0), None);
        assert_eq!(driver.tab_at_position(4), None);
    }

    #[test]
    fn tab_offset_resolution_wraps_around_card_order() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let (queue_c, _receiver_c) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
            (33, tab_state_with_queue(queue_c)),
        ]));
        driver.registry = tab_snapshot(&[(33, 301), (11, 302), (22, 303)]);

        // Middle tab: one step either way stays in the order.
        driver.active_tab = 11;
        assert_eq!(driver.tab_at_offset(1), Some(22));
        assert_eq!(driver.tab_at_offset(-1), Some(33));
        // Wraparound: next from the last tab goes to the first.
        driver.active_tab = 22;
        assert_eq!(driver.tab_at_offset(1), Some(33));
        // Wraparound: prev from the first tab goes to the last.
        driver.active_tab = 33;
        assert_eq!(driver.tab_at_offset(-1), Some(22));
        // Full cycles stay in the order: 33 sits at index 0, so +3 wraps to
        // itself and -4 wraps to 22.
        assert_eq!(driver.tab_at_offset(3), Some(33));
        assert_eq!(driver.tab_at_offset(-4), Some(22));
    }

    #[test]
    fn tab_offset_resolution_with_one_tab_is_noop() {
        let (queue, _receiver) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([(1, tab_state_with_queue(queue))]));
        driver.registry = tab_snapshot(&[(1, 101)]);
        driver.active_tab = 1;
        // A single tab has nothing to cycle: next and prev are no-ops.
        assert_eq!(driver.tab_at_offset(1), None);
        assert_eq!(driver.tab_at_offset(-1), None);
    }

    #[test]
    fn tab_activate_enqueues_activate_and_entry_less_tabs_skip() {
        let (queue_a, mut receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, mut receiver_b) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
        ]));
        driver.apply_tab_registry(tab_snapshot(&[(11, 101)]));

        driver.enqueue_activate(11);
        match receiver_a.try_recv() {
            Ok(clay::protocol::ClientMessage::TabCommand {
                command: clay::protocol::TabCommand::Activate { tab_id },
                ..
            }) => assert_eq!(tab_id, 101),
            other => panic!("expected Activate on tab 11, got {other:?}"),
        }
        // Entry-less tab (no server TabId yet): nothing to activate — a
        // silent no-op, never a queued command.
        driver.enqueue_activate(22);
        assert!(
            receiver_b.try_recv().is_err(),
            "entry-less tab must not enqueue Activate"
        );
    }

    #[test]
    fn tab_move_left_right_respects_boundary_no_op_policy() {
        let (queue_a, mut receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, _receiver_b) = ClientEditQueue::bounded(4);
        let (queue_c, mut receiver_c) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
            (33, tab_state_with_queue(queue_c)),
        ]));
        driver.apply_tab_registry(tab_snapshot(&[(11, 101), (22, 102), (33, 103)]));

        // First position: left is a boundary no-op (no wraparound); right
        // enqueues MoveRight.
        driver.active_tab = 11;
        driver.move_active_tab(true);
        assert!(
            receiver_a.try_recv().is_err(),
            "boundary left must not enqueue"
        );
        driver.move_active_tab(false);
        match receiver_a.try_recv() {
            Ok(clay::protocol::ClientMessage::TabCommand {
                command: clay::protocol::TabCommand::MoveRight { tab_id },
                ..
            }) => assert_eq!(tab_id, 101),
            other => panic!("expected MoveRight on tab 11, got {other:?}"),
        }

        // Last position: right is a boundary no-op; left enqueues MoveLeft.
        driver.active_tab = 33;
        driver.move_active_tab(false);
        assert!(
            receiver_c.try_recv().is_err(),
            "boundary right must not enqueue"
        );
        driver.move_active_tab(true);
        match receiver_c.try_recv() {
            Ok(clay::protocol::ClientMessage::TabCommand {
                command: clay::protocol::TabCommand::MoveLeft { tab_id },
                ..
            }) => assert_eq!(tab_id, 103),
            other => panic!("expected MoveLeft on tab 33, got {other:?}"),
        }
    }

    #[test]
    fn tab_move_to_enqueues_valid_positions_and_noops_out_of_range() {
        let (queue_a, _receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, mut receiver_b) = ClientEditQueue::bounded(4);
        let (queue_c, _receiver_c) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a)),
            (22, tab_state_with_queue(queue_b)),
            (33, tab_state_with_queue(queue_c)),
        ]));
        driver.apply_tab_registry(tab_snapshot(&[(11, 101), (22, 102), (33, 103)]));
        driver.active_tab = 22;

        // Position 0 and positions beyond the tab count are silent no-ops
        // (the client never enqueues a move the server would reject).
        driver.move_active_tab_to(0);
        assert!(
            receiver_b.try_recv().is_err(),
            "position 0 must not enqueue"
        );
        driver.move_active_tab_to(4);
        assert!(
            receiver_b.try_recv().is_err(),
            "beyond-count position must not enqueue"
        );
        // Valid positions (1..=count) enqueue MoveTo with the position.
        driver.move_active_tab_to(3);
        match receiver_b.try_recv() {
            Ok(clay::protocol::ClientMessage::TabCommand {
                command: clay::protocol::TabCommand::MoveTo { tab_id, position },
                ..
            }) => {
                assert_eq!(tab_id, 102);
                assert_eq!(position, 3);
            }
            other => panic!("expected MoveTo on tab 22, got {other:?}"),
        }
    }

    #[test]
    fn tab_new_chord_shares_open_tab_dialog_flow() {
        let mut driver = test_driver_with_tabs(BTreeMap::new());
        // In-flight guard: a `clientTabNew` chord while the folder dialog is
        // showing is refused — the exact flow the tab-bar `+` uses, so the
        // two affordances can never diverge.
        driver.folder_dialog_in_flight = Some(7);
        driver.open_new_tab_dialog();
        assert_eq!(driver.folder_dialog_in_flight, Some(7));
    }

    #[test]
    fn per_tab_edit_queues_are_isolated() {
        // Two tabs = two connections = two independent edit queues. An edit
        // enqueued on tab A's queue never reaches tab B's channel.
        let (queue_a, mut receiver_a) = ClientEditQueue::bounded(4);
        let (queue_b, mut receiver_b) = ClientEditQueue::bounded(4);
        let queue_a = queue_a
            .with_authority(
                11,
                &clay::protocol::DocumentAccess::Editable { lease_id: 1 },
            )
            .with_confirmed_version(3);
        let queue_b = queue_b
            .with_authority(
                22,
                &clay::protocol::DocumentAccess::Editable { lease_id: 2 },
            )
            .with_confirmed_version(3);
        let driver = test_driver_with_tabs(BTreeMap::from([
            (11, tab_state_with_queue(queue_a.clone())),
            (22, tab_state_with_queue(queue_b.clone())),
        ]));

        let event = clay::editor::EditorEditEvent {
            document_id: 7,
            base_version: 3,
            behavior_version: 3,
            operation: clay::protocol::EditOperation::Insert {
                byte_offset: 0,
                text: "tab A edit".to_string(),
            },
        };
        driver.tabs[&11]
            .edit_queue
            .as_ref()
            .expect("tab A queue")
            .enqueue_edit_event(event, 1)
            .expect("tab A enqueues");

        let message_a = receiver_a.try_recv().expect("tab A channel receives");
        assert!(matches!(
            message_a,
            clay::protocol::ClientMessage::Edit { client_id: 11, .. }
        ));
        assert!(
            receiver_b.try_recv().is_err(),
            "tab B's channel must not see tab A's edit"
        );
        assert_eq!(driver.tabs[&22].pending_opens.len(), 0);
    }
}
