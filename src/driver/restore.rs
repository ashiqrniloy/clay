//! Whole-window restore state machine: mount persisted tabs sequentially,
//! gated on server `TabId` confirmation, then reopen documents and activate
//! the persisted active tab. Phase 22.5 (plan 078).
use super::*;

impl Driver {
    /// Phase 22.5: advance the restore gate. Called on every registry
    /// snapshot while a restore is in flight: the last mounted tab must have
    /// received its server `TabId` (the snapshot confirms it) before the next
    /// tab connects — the server appends tabs in `New` order, so registry
    /// order matches persisted order without any `MoveTo`. Tabs whose
    /// workspace root is gone are skipped with a diagnostic; when the queue
    /// empties, the restore finishes (documents reopen, active tab
    /// activates).
    pub(crate) fn advance_restore(&mut self, ctx: &mut DriverCtx<'_, '_>, window_id: WindowId) {
        if let Some((index, tab)) = self.restore_next_mount() {
            self.restore_pending = Some((index, tab));
            self.spawn_restore_connect();
        } else if self.restore_gate.is_some() && self.restore_queue.is_empty() {
            // The last mount is confirmed and nothing is left to mount.
            self.finish_restore(ctx, window_id);
        }
    }

    /// Phase 22.5: the gate's next mount — `Some((persisted index, tab))`
    /// once the last mounted tab's server `TabId` is confirmed, or `None`
    /// while the gate waits or the queue is empty (the caller distinguishes:
    /// a live gate with an empty queue finishes the restore). Tabs whose
    /// workspace root is gone are skipped here, with a diagnostic.
    pub(crate) fn restore_next_mount(&mut self) -> Option<(usize, PersistedTabState)> {
        loop {
            let (last, _) = self.restore_gate?;
            let confirmed = self.tabs.get(&last).and_then(|tab| tab.tab_id).is_some();
            if !confirmed {
                // Wait for the confirmation snapshot (the deadline check in
                // `on_action` abandons the restore if it never comes).
                return None;
            }
            let (index, tab) = self.restore_queue.pop_front()?;
            if !Path::new(&tab.workspace_root).is_dir() {
                self.restore_diagnostics.push(format!(
                    "Restore skipped {}: workspace root is missing or not a directory",
                    tab.workspace_root
                ));
                continue;
            }
            return Some((index, tab));
        }
    }

    /// Phase 22.5: connect the pending restore tab on the runtime (the
    /// handshake may involve retries); the session returns via
    /// `OpenTabConnected` and mounts with the persisted layout. A refused
    /// connection returns via `OpenTabFailed`, which abandons the restore.
    pub(crate) fn spawn_restore_connect(&mut self) {
        let Some(proxy) = self.proxy.clone() else {
            return;
        };
        let Some((_, tab)) = self.restore_pending.as_ref() else {
            return;
        };
        let endpoint = self.endpoint.clone();
        let window_id = self.window_id;
        let shell_widget_id = self.shell_widget_id;
        let workspace_root = tab.workspace_root.clone();
        self.runtime.spawn(async move {
            match client::connect_with_workspace_root(&endpoint, workspace_root.clone()).await {
                Ok(session) => {
                    let _ = proxy.send_event(MasonryUserEvent::Action(
                        window_id,
                        Box::new(EditorAction::OpenTabConnected {
                            session: clay::masonry_editor::DriverSession { session },
                            workspace_root: PathBuf::from(workspace_root),
                        }),
                        shell_widget_id,
                    ));
                }
                Err(error) => {
                    let _ = proxy.send_event(MasonryUserEvent::Action(
                        window_id,
                        Box::new(EditorAction::OpenTabFailed {
                            message: format!(
                                "Could not restore a tab for {workspace_root}: {error}"
                            ),
                        }),
                        shell_widget_id,
                    ));
                }
            }
        });
    }

    /// Phase 22.5: mount a restored tab's already-connected session with its
    /// persisted split tree (the handshake already registers the tab and
    /// installs its initial state; the persisted index maps the tab back to
    /// its documents). Does not switch
    /// the active tab — restore activates the persisted active tab after
    /// every mount confirms. Returns `None` on a duplicate connection.
    pub(crate) fn mount_restored_tab(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        session: client::ClientSession,
        index: usize,
        persisted: PersistedTabState,
    ) -> Option<ClientId> {
        let client_id = session.initial_state.client_id;
        if self.tabs.contains_key(&client_id) {
            return None;
        }
        let chrome = EditorWidget::with_initial_state(session.initial_state)
            .with_edit_queue(session.edit_queue.clone());
        let edit_queue = session.edit_queue.clone();
        let events = session.events;
        let chrome_id = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, shell_ctx| shell.install_restored_tab(shell_ctx, client_id, chrome, &persisted),
        );
        let chrome_id = chrome_id?;
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
                workspace_root: persisted.workspace_root.clone(),
            },
        );
        self.restore_mounted.push((client_id, index, persisted));
        self.restore_gate = Some((client_id, Instant::now() + RESTORE_CONFIRM_TIMEOUT));
        Some(client_id)
    }

    /// Phase 22.5: reopen every persisted document into its pane through the
    /// plain `OpenDocument` path with pending-open attribution (the 22.2
    /// mechanism: each `DocumentOpened` lands in exactly the pane that asked).
    /// Documents that no longer exist are skipped — the pane stays empty.
    /// Path safety rides the server's `OpenDocument` validation (out-of-root
    /// and traversal paths are rejected there, never here).
    pub(crate) fn reopen_restored_documents(&mut self) {
        for (client_id, _, persisted) in &self.restore_mounted {
            let Some(root_id) = self
                .registry
                .tabs
                .iter()
                .find(|entry| entry.client_id == *client_id)
                .map(|entry| entry.workspace_root_id)
            else {
                continue;
            };
            let Some(tab) = self.tabs.get_mut(client_id) else {
                continue;
            };
            for (pane_id, document) in &persisted.panes {
                let Some(path) = document else {
                    continue;
                };
                if !Path::new(&persisted.workspace_root).join(path).is_file() {
                    continue;
                }
                tab.pending_opens.insert(
                    *pane_id,
                    PendingOpenRequest {
                        path: None,
                        root_id: Some(root_id),
                        relative_path: Some(path.clone()),
                    },
                );
                if let Some(queue) = tab.edit_queue.as_ref() {
                    let _ = queue.enqueue_open_document(root_id, path.clone());
                }
            }
        }
    }

    /// Phase 22.5: finish the restore: documents reopen, the persisted active
    /// tab activates (fallback: the first mounted tab), skip diagnostics
    /// surface on the chrome, and the gate turns off.
    pub(crate) fn finish_restore(&mut self, ctx: &mut DriverCtx<'_, '_>, window_id: WindowId) {
        self.reopen_restored_documents();
        let active_client = self
            .restore_active
            .and_then(|index| {
                self.restore_mounted
                    .iter()
                    .find(|(_, mounted_index, _)| *mounted_index == index)
                    .map(|(client_id, _, _)| *client_id)
            })
            .or_else(|| {
                self.restore_mounted
                    .first()
                    .map(|(client_id, _, _)| *client_id)
            })
            .or_else(|| self.tabs.keys().next().copied());
        if let Some(active_client) = active_client {
            self.activate_tab(ctx, window_id, active_client);
        }
        self.restore_gate = None;
        self.flush_restore_diagnostics(ctx, window_id);
    }

    /// Phase 22.5: abandon the remaining restore (a connect failed or the
    /// confirmation deadline passed). Mounted tabs stay; queued tabs drop.
    pub(crate) fn abandon_restore(&mut self, ctx: &mut DriverCtx<'_, '_>, window_id: WindowId) {
        self.cancel_restore();
        self.flush_restore_diagnostics(ctx, window_id);
    }

    /// Phase 22.5: drop the remaining restore state (mounted tabs stay;
    /// diagnostics stay queued for `flush_restore_diagnostics`).
    pub(crate) fn cancel_restore(&mut self) {
        self.restore_queue.clear();
        self.restore_pending = None;
        self.restore_gate = None;
    }

    /// Phase 22.5: surface collected skip diagnostics on the active chrome
    /// (`clay.tabs.open_failed` family, as the new-tab failure path).
    pub(crate) fn flush_restore_diagnostics(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
    ) {
        let messages: Vec<String> = self.restore_diagnostics.drain(..).collect();
        for message in messages {
            self.apply_connection_to_chrome(
                ctx,
                window_id,
                self.editor_widget_id,
                ClientConnectionEvent::RuntimeDiagnostic(clay::protocol::RuntimeDiagnostic::error(
                    "clay.tabs.open_failed",
                    message,
                )),
            );
        }
    }

    /// Phase 22.3: structural close gate — the window never goes to zero
    /// tabs, and only mounted tabs can close.
    pub(crate) fn tab_close_allowed(&self, client_id: ClientId) -> bool {
        self.tabs.len() >= 2 && self.tabs.contains_key(&client_id)
    }

    /// Phase 22.4: the tab-close confirm menu session for one tab. Names the
    /// tab's workspace and every dirty document, with the three choices. The
    /// session is driver-owned (its action IDs are driver-local orchestration,
    /// never declared, never server-routed) so tab-confirm and per-view
    /// save-conflict sessions cannot cross-route.
    pub(crate) fn tab_close_confirm_menu(
        &mut self,
        client_id: u64,
        dirty: &[String],
    ) -> TransientMenuSession {
        self.tab_menu_session_id = self.tab_menu_session_id.saturating_add(1).max(1);
        let tab_name = self
            .registry
            .tabs
            .iter()
            .find(|entry| entry.client_id == client_id)
            .map(|entry| {
                std::path::Path::new(&entry.workspace_root)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| entry.workspace_root.clone())
            })
            .unwrap_or_else(|| "this tab".to_string());
        let documents = dirty.join(", ");
        let prompt = if dirty.len() == 1 {
            format!("Close tab '{tab_name}' with 1 unsaved document ({documents})?")
        } else {
            format!(
                "Close tab '{tab_name}' with {} unsaved documents ({documents})?",
                dirty.len()
            )
        };
        clay::shell::tab_close_confirm_session(self.tab_menu_session_id, prompt, client_id)
    }

    /// Phase 22.4: inventory the tab's dirty pane targets — the pane-1
    /// chrome view first, then every document pane — with each document's
    /// display name. Replaces the 22.3 `guard_tab_close` block: the walk
    /// became the confirm menu's inventory (the block itself is the 22.4
    /// replacement target).
    pub(crate) fn dirty_documents_in_tab(
        &self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: u64,
    ) -> Vec<(String, WidgetId)> {
        let targets = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, _| shell.pane_targets_for(client_id),
        )
        .unwrap_or_default();
        let mut dirty = Vec::new();
        for (_, target) in targets {
            let is_dirty = with_editor(ctx.render_root(window_id), target, |editor, _| {
                editor.is_dirty()
            })
            .or_else(|| {
                with_view(ctx.render_root(window_id), target, |view, _| {
                    view.is_dirty()
                })
            })
            .unwrap_or(false);
            if is_dirty {
                let name = with_editor(ctx.render_root(window_id), target, |editor, _| {
                    editor.document_display_name()
                })
                .or_else(|| {
                    with_view(ctx.render_root(window_id), target, |view, _| {
                        view.document_display_name()
                    })
                })
                .flatten()
                .unwrap_or_else(|| "unsaved document".to_string());
                dirty.push((name, target));
            }
        }
        dirty
    }

    /// Phase 22.5: assemble the whole-window persisted state: every tab's
    /// layout (topology/ratios/slots/active pane) from the shell, ordered by
    /// the server registry (mount order at restore), plus per-pane active
    /// document identity from the pane views. Retained-but-inactive documents
    /// are not persisted; a pane with an open still in flight (nothing
    /// installed yet) serializes as document-less. Returns `None` when no
    /// tabs can be collected.
    pub(crate) fn collect_window_state(
        &self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
    ) -> Option<PersistedWindowState> {
        let layouts = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, _| shell.tab_layout_data(),
        )
        .unwrap_or_default();
        if layouts.is_empty() {
            return None;
        }
        let ordered = ordered_tab_clients(&self.registry, &self.tabs);
        let mut tabs = Vec::new();
        for client_id in &ordered {
            let Some((_, layout)) = layouts.iter().find(|(id, _)| id == client_id) else {
                continue;
            };
            let Some(tab_state) = self.tabs.get(client_id) else {
                continue;
            };
            let targets = with_shell(
                ctx.render_root(window_id),
                self.shell_widget_id,
                |shell, _| shell.pane_targets_for(*client_id),
            )
            .unwrap_or_default();
            let mut panes = BTreeMap::new();
            for (pane_id, target) in targets {
                let document = with_editor(ctx.render_root(window_id), target, |editor, _| {
                    editor.active_document_identity()
                })
                .or_else(|| {
                    with_view(ctx.render_root(window_id), target, |view, _| {
                        view.active_document_identity()
                    })
                })
                .flatten()
                .map(|(_, path)| path);
                panes.insert(pane_id, document);
            }
            tabs.push(PersistedTabState {
                workspace_root: tab_state.workspace_root.clone(),
                active_pane: layout.active_pane,
                tree: Some(layout.tree.clone()),
                slots: layout.slots.clone(),
                panes,
            });
        }
        if tabs.is_empty() {
            return None;
        }
        let active_tab = ordered
            .iter()
            .position(|client_id| *client_id == self.active_tab);
        Some(PersistedWindowState { tabs, active_tab })
    }

    /// Phase 22.5: collect the current window state and write it. Called by
    /// the shell's `PersistenceDue` signal (debounced), tab/document
    /// lifecycle events, and the quit-time flush.
    pub(crate) fn persist_window_state(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
    ) {
        if let Some(state) = self.collect_window_state(ctx, window_id) {
            clay::shell::save_window_state(&state);
        }
    }

    /// Phase 22.4: show (or clear) the tab-close confirm menu on a tab's
    /// active pane view — the interactive menu host that receives the
    /// keyboard — and sync the chrome overlay through the usual pending-menu
    /// path.
    pub(crate) fn show_tab_close_confirm_menu(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: u64,
        menu: Option<TransientMenuSession>,
    ) {
        let chrome_id = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, _| shell.editor_widget_id_for(client_id),
        )
        .flatten();
        let active_target = with_shell(
            ctx.render_root(window_id),
            self.shell_widget_id,
            |shell, _| shell.active_pane_target_for(client_id),
        )
        .flatten();
        let target = active_target.or(chrome_id).unwrap_or(self.editor_widget_id);
        let pending = with_editor(ctx.render_root(window_id), target, |editor, _| {
            editor.push_menu(menu.clone());
            editor.take_pending_menu()
        })
        .or_else(|| {
            with_view(ctx.render_root(window_id), target, |view, _| {
                view.push_menu(menu);
                view.take_pending_menu()
            })
        })
        .flatten();
        if let (Some(chrome_id), Some(pending)) = (chrome_id, pending) {
            self.apply_menu_sync(ctx, window_id, chrome_id, pending);
        }
    }

    /// Phase 22.4: save every dirty pane of a tab, then close it once all
    /// saves ack (a failed save cancels the close and surfaces the pane's
    /// existing save diagnostic).
    pub(crate) fn save_all_then_close_tab(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: u64,
    ) {
        let mut expected = std::collections::BTreeSet::new();
        for (_, target) in self.dirty_documents_in_tab(ctx, window_id, client_id) {
            let outcome = with_editor(ctx.render_root(window_id), target, |editor, _| {
                editor.request_save_active_document()
            })
            .or_else(|| {
                with_view(ctx.render_root(window_id), target, |view, _| {
                    view.request_save_active_document()
                })
            })
            .unwrap_or(Ok(0));
            match outcome {
                Ok(document_id) => {
                    if document_id != 0 {
                        expected.insert(document_id);
                    }
                }
                Err(diagnostic) => {
                    // A save could not even be enqueued: cancel the close and
                    // surface the diagnostic on the pane.
                    self.pending_close_after_saves = None;
                    let event = ClientConnectionEvent::RuntimeDiagnostic(diagnostic);
                    with_editor(ctx.render_root(window_id), target, |editor, _| {
                        let _ = editor.apply_connection_event(event.clone());
                    })
                    .or_else(|| {
                        with_view(ctx.render_root(window_id), target, |view, _| {
                            let _ = view.apply_connection_event(event);
                        })
                    });
                    return;
                }
            }
        }
        if expected.is_empty() {
            // Every document was saved while the menu was open: close now.
            self.enqueue_close(client_id);
        } else {
            self.pending_close_after_saves = Some((client_id, expected));
        }
    }

    // -- Phase 22.4: keyboard tab management (driver-routed commands) --

    /// Phase 22.4: the tab bar's card order — the server registry's order
    /// (mounted tabs only) with entry-less mounted tabs appended. Every
    /// numbered tab command resolves against this order, so the number the
    /// user sees on the bar is the number the command means.
    pub(crate) fn tab_order(&self) -> Vec<ClientId> {
        let mut order: Vec<ClientId> = self
            .registry
            .tabs
            .iter()
            .filter(|entry| self.tabs.contains_key(&entry.client_id))
            .map(|entry| entry.client_id)
            .collect();
        for client_id in self.tabs.keys() {
            if !order.contains(client_id) {
                order.push(*client_id);
            }
        }
        order
    }

    /// Phase 22.4: 1-based card-order position of a mounted tab.
    pub(crate) fn tab_position_of(&self, client_id: ClientId) -> Option<u32> {
        self.tab_order()
            .iter()
            .position(|id| *id == client_id)
            .map(|index| index as u32 + 1)
    }

    /// Phase 22.4: the tab at a 1-based card-order position. Positions 0 and
    /// beyond the tab count resolve to `None`: the numbered-activate policy
    /// is a silent no-op there — never switch to a non-existent position.
    pub(crate) fn tab_at_position(&self, position: u32) -> Option<ClientId> {
        if position == 0 {
            return None;
        }
        self.tab_order().get(position as usize - 1).copied()
    }

    /// Phase 22.4: the tab `offset` steps from the active tab in card order,
    /// with wraparound — next from the last tab goes to the first, prev from
    /// the first goes to the last. `None` when cycling is impossible (fewer
    /// than two tabs, or the active tab is not in the order): the next/prev
    /// policy is a silent no-op there (a single tab has nothing to cycle).
    pub(crate) fn tab_at_offset(&self, offset: i64) -> Option<ClientId> {
        let order = self.tab_order();
        if order.len() < 2 {
            return None;
        }
        let current = order.iter().position(|id| *id == self.active_tab)?;
        let wrapped = (current as i64 + offset).rem_euclid(order.len() as i64) as usize;
        Some(order[wrapped])
    }

    /// Phase 22.4: enqueue `TabCommand::Activate` on a tab's connection when
    /// the server knows its `TabId` (entry-less tabs have no server entry to
    /// activate). The server registry is the reconciling authority: a
    /// rejected activate pushes a snapshot that reverts the switch.
    pub(crate) fn enqueue_activate(&mut self, client_id: ClientId) {
        let Some(tab_id) = self.tabs.get(&client_id).and_then(|tab| tab.tab_id) else {
            return;
        };
        if let Some(queue) = self.tabs[&client_id].edit_queue.as_ref() {
            let _ = queue.enqueue_tab_command(clay::protocol::TabCommand::Activate { tab_id });
        }
    }

    /// Phase 22.4: switch to a mounted tab — enqueue the server activate and
    /// switch optimistically (the server's pushed snapshot reconciles).
    pub(crate) fn activate_tab(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
    ) {
        if !self.tabs.contains_key(&client_id) {
            return;
        }
        self.enqueue_activate(client_id);
        if self.switch_tab(ctx, window_id, client_id) {
            // Phase 22.6: one polite announcement per user-initiated switch
            // (restore and registry-reconcile switches don't route here).
            let shell_widget_id = self.shell_widget_id;
            with_shell(
                ctx.render_root(window_id),
                shell_widget_id,
                |shell, shell_ctx| {
                    shell.announce_tab_activated(shell_ctx, client_id);
                },
            );
        }
    }

    /// Phase 22.4: enqueue `TabCommand::Close` once a tab passed the close
    /// gates (last-tab protection + dirty guard). Server-confirmed removal
    /// arrives via the pushed registry snapshot.
    pub(crate) fn enqueue_close(&mut self, client_id: ClientId) {
        let Some(tab_id) = self.tabs.get(&client_id).and_then(|tab| tab.tab_id) else {
            return;
        };
        if let Some(queue) = self.tabs[&client_id].edit_queue.as_ref() {
            let _ = queue.enqueue_tab_command(clay::protocol::TabCommand::Close { tab_id });
        }
    }

    /// Phase 22.4: close a mounted tab through the shared close path. Last-tab
    /// protection first (`tab_close_allowed`); a clean tab closes immediately
    /// (server-confirmed `TabCommand::Close`), a tab holding any dirty
    /// document shows the confirm menu — Save all and close / Discard and
    /// close / Cancel — instead of the 22.3 plain block. Every affordance
    /// (bar close, `clientTabClose`, any future surface) routes here.
    pub(crate) fn close_tab(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
    ) {
        if !self.tab_close_allowed(client_id) {
            return;
        }
        if self
            .pending_close_after_saves
            .as_ref()
            .is_some_and(|(pending, _)| *pending == client_id)
        {
            // A save-all-and-close flow is already awaiting acks for this
            // tab: ignore a second close request (the flow completes on its
            // own, or cancels on a failed save).
            return;
        }
        let dirty = self.dirty_documents_in_tab(ctx, window_id, client_id);
        if dirty.is_empty() {
            self.enqueue_close(client_id);
            return;
        }
        let names = dirty.into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        let menu = self.tab_close_confirm_menu(client_id, &names);
        self.show_tab_close_confirm_menu(ctx, window_id, client_id, Some(menu));
    }

    /// Phase 22.4: move the active tab one step in card order. No wraparound:
    /// a move at a boundary is a no-op (the server agrees and no-ops too).
    /// Server-confirmed: the pushed snapshot reorders the cards; there is no
    /// optimistic reorder.
    pub(crate) fn move_active_tab(&mut self, left: bool) {
        let Some(position) = self.tab_position_of(self.active_tab) else {
            return;
        };
        let count = self.tab_order().len() as u32;
        if (left && position <= 1) || (!left && position >= count) {
            return;
        }
        let Some(tab_id) = self.tabs.get(&self.active_tab).and_then(|tab| tab.tab_id) else {
            return;
        };
        if let Some(queue) = self.tabs[&self.active_tab].edit_queue.as_ref() {
            let command = if left {
                clay::protocol::TabCommand::MoveLeft { tab_id }
            } else {
                clay::protocol::TabCommand::MoveRight { tab_id }
            };
            let _ = queue.enqueue_tab_command(command);
        }
    }

    /// Phase 22.4: move the active tab to a 1-based card-order position.
    /// Positions 0 or beyond the tab count are silent no-ops — the client
    /// never enqueues a move the server would reject (the server rejects
    /// out-of-range positions with a reconciling snapshot regardless).
    pub(crate) fn move_active_tab_to(&mut self, position: u32) {
        if position == 0 || position as usize > self.tab_order().len() {
            return;
        }
        let Some(tab_id) = self.tabs.get(&self.active_tab).and_then(|tab| tab.tab_id) else {
            return;
        };
        if let Some(queue) = self.tabs[&self.active_tab].edit_queue.as_ref() {
            let _ =
                queue.enqueue_tab_command(clay::protocol::TabCommand::MoveTo { tab_id, position });
        }
    }

    /// Phase 22.4: open-tab affordance — the folder picker's selection
    /// becomes the new tab's workspace root. Shared by the tab bar `+` and
    /// the `clientTabNew` chord; the dialog runs on a background thread so
    /// it never blocks the UI event loop (portal).
    pub(crate) fn open_new_tab_dialog(&mut self) {
        let Some(generation) = self.reserve_folder_dialog() else {
            return;
        };
        let Some(proxy) = self.proxy.clone() else {
            return;
        };
        let window_id = self.window_id;
        let shell_widget_id = self.shell_widget_id;
        let spawn = std::thread::Builder::new()
            .name("clay-new-tab-dialog".into())
            .spawn(move || {
                let _ = proxy.send_event(MasonryUserEvent::Action(
                    window_id,
                    Box::new(EditorAction::NewTabFolderDialogCompleted {
                        generation,
                        result: clay::client::open_folder_dialog(),
                    }),
                    shell_widget_id,
                ));
            });
        if spawn.is_err() {
            self.finish_folder_dialog(generation);
        }
    }

    /// Phase 22.4: driver-routed tab commands. Returns `true` when the
    /// command is a tab operation (handled here, or a policy no-op); `false`
    /// for pane commands, which the shell widget applies. Tab commands act on
    /// the driver's tab state (active tab, connections, registry order), never
    /// on the shell widget's pane tree (its tab arms stay inert).
    pub(crate) fn apply_tab_command(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        command: clay::masonry_shell::ShellClientCommand,
    ) -> bool {
        match command {
            clay::masonry_shell::ShellClientCommand::TabNext => {
                if let Some(target) = self.tab_at_offset(1) {
                    self.activate_tab(ctx, window_id, target);
                }
            }
            clay::masonry_shell::ShellClientCommand::TabPrev => {
                if let Some(target) = self.tab_at_offset(-1) {
                    self.activate_tab(ctx, window_id, target);
                }
            }
            clay::masonry_shell::ShellClientCommand::TabNew => self.open_new_tab_dialog(),
            clay::masonry_shell::ShellClientCommand::TabClose => {
                self.close_tab(ctx, window_id, self.active_tab);
            }
            clay::masonry_shell::ShellClientCommand::TabMoveLeft => self.move_active_tab(true),
            clay::masonry_shell::ShellClientCommand::TabMoveRight => self.move_active_tab(false),
            clay::masonry_shell::ShellClientCommand::TabActivate(position) => {
                if let Some(target) = self.tab_at_position(position) {
                    self.activate_tab(ctx, window_id, target);
                }
            }
            clay::masonry_shell::ShellClientCommand::TabMoveTo(position) => {
                self.move_active_tab_to(position);
            }
            _ => return false,
        }
        true
    }

    /// Phase 22.3/22.8: spawn a per-tab reconnect task after a connection
    /// drop. The task retries a `Reclaim` with the existing backoff until it
    /// succeeds or the tab is removed (cancellation flag). If the in-memory
    /// server registry was reset or evicted the tab, the client rebuilds it
    /// with `New(workspace_root)` instead of reviving a stale `TabId`; on
    /// success the fresh session returns as
    /// [`EditorAction::ReconnectTabConnected`] and the driver re-keys the
    /// tab and re-opens its documents.
    pub(crate) fn start_tab_reconnect(&mut self, client_id: ClientId) {
        if !self.tabs.contains_key(&client_id) || self.reconnect_cancel.contains_key(&client_id) {
            return;
        }
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.reconnect_cancel.insert(client_id, cancel.clone());
        let endpoint = self.endpoint.clone();
        let window_id = self.window_id;
        let shell_widget_id = self.shell_widget_id;
        let Some(proxy) = self.proxy.clone() else {
            return;
        };
        let Some(tab) = self.tabs.get(&client_id) else {
            return;
        };
        let tab_id = tab.tab_id;
        let workspace_root = tab.workspace_root.clone();
        self.runtime.spawn(async move {
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let result = match tab_id {
                    Some(tab_id) => {
                        client::connect_for_reclaim_or_new(
                            &endpoint,
                            tab_id,
                            workspace_root.clone(),
                        )
                        .await
                    }
                    None => {
                        client::connect_with_workspace_root(&endpoint, workspace_root.clone()).await
                    }
                };
                match result {
                    Ok(session) => {
                        let _ = proxy.send_event(MasonryUserEvent::Action(
                            window_id,
                            Box::new(EditorAction::ReconnectTabConnected {
                                client_id,
                                session: clay::masonry_editor::DriverSession { session },
                            }),
                            shell_widget_id,
                        ));
                        return;
                    }
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{tab_state_with_queue, test_driver_with_tabs};
    use super::super::*;
    use std::collections::{BTreeMap, VecDeque};
    use std::time::{Duration, Instant};

    use clay::shell::PersistedTabState;

    fn persisted_tab_state(workspace_root: &str) -> PersistedTabState {
        PersistedTabState {
            workspace_root: workspace_root.to_string(),
            active_pane: PaneId(1),
            tree: None,
            slots: Vec::new(),
            panes: BTreeMap::new(),
        }
    }

    #[test]
    fn restore_gate_waits_for_tab_id_confirmation_before_next_mount() {
        let (queue, _receiver) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([(11, tab_state_with_queue(queue))]));
        driver.restore_gate = Some((11, Instant::now() + RESTORE_CONFIRM_TIMEOUT));
        driver.restore_queue = VecDeque::from([(1, persisted_tab_state("/tmp"))]);
        // The bootstrap tab has no server `TabId` yet: the gate waits.
        assert!(driver.restore_next_mount().is_none());
        assert_eq!(driver.restore_queue.len(), 1);
        // The registry snapshot fills the `TabId`: the next mount pops.
        driver.tabs.get_mut(&11).expect("tab").tab_id = Some(101);
        let (index, tab) = driver.restore_next_mount().expect("confirmed");
        assert_eq!(index, 1);
        assert_eq!(tab.workspace_root, "/tmp");
        // Queue drained and the gate still live: the caller finishes.
        assert!(driver.restore_queue.is_empty());
        assert!(driver.restore_gate.is_some());
        assert!(driver.restore_next_mount().is_none());
    }

    #[test]
    fn restore_skips_missing_workspace_root_and_continues_in_order() {
        let (queue, _receiver) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([(11, tab_state_with_queue(queue))]));
        driver.tabs.get_mut(&11).expect("tab").tab_id = Some(101);
        driver.restore_gate = Some((11, Instant::now() + RESTORE_CONFIRM_TIMEOUT));
        driver.restore_queue = VecDeque::from([
            (1, persisted_tab_state("/nonexistent/restore-root-1")),
            (2, persisted_tab_state("/tmp")),
            (3, persisted_tab_state("/nonexistent/restore-root-3")),
        ]);
        // The first entry's root is gone: skipped with a diagnostic, the
        // loop continues to the next valid root (order preserved).
        let (index, tab) = driver.restore_next_mount().expect("next valid tab");
        assert_eq!(index, 2);
        assert_eq!(tab.workspace_root, "/tmp");
        assert_eq!(driver.restore_diagnostics.len(), 1);
        assert!(driver.restore_diagnostics[0].contains("/nonexistent/restore-root-1"));
        assert_eq!(driver.restore_queue.len(), 1);
    }

    #[test]
    fn restore_deadline_cancel_drops_remaining_queue_and_pending() {
        let (queue, _receiver) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([(11, tab_state_with_queue(queue))]));
        driver.restore_gate = Some((11, Instant::now() - Duration::from_secs(1)));
        driver.restore_queue = VecDeque::from([(1, persisted_tab_state("/tmp"))]);
        driver.restore_pending = Some((2, persisted_tab_state("/tmp")));
        driver
            .restore_diagnostics
            .push("Restore skipped /gone: root missing".to_string());
        // The deadline expired: the remaining restore drops; mounted tabs
        // and queued diagnostics stay (flushed by the caller).
        driver.cancel_restore();
        assert!(driver.restore_queue.is_empty());
        assert!(driver.restore_pending.is_none());
        assert!(driver.restore_gate.is_none());
        assert_eq!(driver.restore_diagnostics.len(), 1);
        assert_eq!(driver.tabs.len(), 1);
    }

    #[test]
    fn reopen_restored_documents_attributes_panes_and_skips_missing_files() {
        let (queue, _receiver) = ClientEditQueue::bounded(4);
        let mut driver = test_driver_with_tabs(BTreeMap::from([(11, tab_state_with_queue(queue))]));
        driver.registry = clay::protocol::TabRegistrySnapshot {
            tabs: vec![clay::protocol::TabEntry {
                tab_id: 101,
                workspace_root_id: 7,
                client_id: 11,
                workspace_root: ".".to_string(),
            }],
            active: Some(101),
            revision: 0,
        };
        // Persisted panes: pane 1 has an existing file, pane 2 is empty,
        // pane 3's file is gone (stale state) — only pane 1 reopens.
        driver.restore_mounted = vec![(
            11,
            0,
            PersistedTabState {
                workspace_root: ".".to_string(),
                active_pane: PaneId(1),
                tree: None,
                slots: Vec::new(),
                panes: BTreeMap::from([
                    (PaneId(1), Some("Cargo.toml".to_string())),
                    (PaneId(2), None),
                    (PaneId(3), Some("no-such-file-22-5.md".to_string())),
                ]),
            },
        )];
        driver.reopen_restored_documents();
        let tab = driver.tabs.get(&11).expect("tab");
        assert_eq!(tab.pending_opens.len(), 1);
        let request = tab.pending_opens.get(&PaneId(1)).expect("pane 1 request");
        assert_eq!(request.root_id, Some(7));
        assert_eq!(request.relative_path.as_deref(), Some("Cargo.toml"));
    }
}
