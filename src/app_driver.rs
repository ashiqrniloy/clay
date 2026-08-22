//! App event dispatch + native dialog/action routing: `impl Driver` window
//! methods (tab/pane/document event application, shell command dispatch,
//! theme/typography/runtime fan-out, native dialog generation bookkeeping),
//! `impl AppDriver for Driver` (the input/action routing hot path), and the
//! native dialog helpers (`ClientUiCommandResult`, `handle_client_ui_command`,
//! `SelectedPathKind`, `client_dialog_result_to_command_result`,
//! `apply_native_dialog_completion`). Direct exhaustive matches; no command
//! bus/factory/service locator.

use std::{path::PathBuf, time::Instant};

use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::widgets::TextAction;
use masonry_winit::app::{AppDriver, DriverCtx, MasonryUserEvent, WindowId};

use clay::client::{self, ClientConnectionEvent};
use clay::masonry_editor::{
    EditorAction, EditorWidget, PackageButtonPress, PackageDropdownSelect, PackageListRowPress,
    PackageModalDismiss, SduiButtonPress, SduiListRowPress,
};
use clay::masonry_pane_document::{CrossPaneDocumentEntry, PaneDocumentView};
use clay::masonry_pane_host::PaneContentHost;
use clay::masonry_shell::ClayShellWidget;
use clay::protocol::ClientId;

use crate::driver::{
    Driver, PendingOpenRequest, advance_pending_close_after_saves, take_pending_open_for,
};
use clay::shell::{PaneId, TransientMenuSession};

impl Driver {
    fn next_dialog_generation(&mut self) -> Option<u64> {
        self.dialog_generation = self.dialog_generation.checked_add(1)?;
        Some(self.dialog_generation)
    }

    pub(crate) fn reserve_file_dialog(&mut self) -> Option<u64> {
        if self.file_dialog_in_flight.is_some() {
            return None;
        }
        let generation = self.next_dialog_generation()?;
        self.file_dialog_in_flight = Some(generation);
        Some(generation)
    }

    pub(crate) fn reserve_folder_dialog(&mut self) -> Option<u64> {
        if self.folder_dialog_in_flight.is_some() {
            return None;
        }
        let generation = self.next_dialog_generation()?;
        self.folder_dialog_in_flight = Some(generation);
        Some(generation)
    }

    pub(crate) fn finish_file_dialog(&mut self, generation: u64) -> bool {
        if self.file_dialog_in_flight != Some(generation) {
            return false;
        }
        self.file_dialog_in_flight = None;
        true
    }

    pub(crate) fn finish_folder_dialog(&mut self, generation: u64) -> bool {
        if self.folder_dialog_in_flight != Some(generation) {
            return false;
        }
        self.folder_dialog_in_flight = None;
        true
    }

    pub(crate) fn clear_native_dialogs(&mut self) {
        self.file_dialog_in_flight = None;
        self.folder_dialog_in_flight = None;
    }

    pub(crate) fn spawn_native_dialog_command(
        &mut self,
        command: clay::client::ClientUiCommandRoute,
    ) {
        let Some(proxy) = self.proxy.clone() else {
            return;
        };
        let (generation, is_file) = match command.command_id.as_str() {
            "documents.clientOpenFileDialog" => {
                let Some(generation) = self.reserve_file_dialog() else {
                    return;
                };
                (generation, true)
            }
            "workspace.clientOpenFolderDialog" => {
                let Some(generation) = self.reserve_folder_dialog() else {
                    return;
                };
                (generation, false)
            }
            _ => return,
        };
        let window_id = self.window_id;
        let editor_widget_id = self.editor_widget_id;
        let spawn = std::thread::Builder::new()
            .name("clay-native-dialog".into())
            .spawn(move || {
                let action = if is_file {
                    EditorAction::FileDialogCompleted {
                        generation,
                        result: clay::client::open_markdown_file_dialog(),
                    }
                } else {
                    EditorAction::FolderDialogCompleted {
                        generation,
                        result: clay::client::open_folder_dialog(),
                    }
                };
                // Failure means the event loop is already shutting down; its Driver
                // (and therefore all in-flight state) is being dropped.
                let _ = proxy.send_event(MasonryUserEvent::Action(
                    window_id,
                    Box::new(action),
                    editor_widget_id,
                ));
            });
        if spawn.is_err() {
            if is_file {
                self.finish_file_dialog(generation);
            } else {
                self.finish_folder_dialog(generation);
            }
        }
    }

    // -- Phase 22.2: pane document-view routing --

    /// Apply one connection event to the chrome (pane-1 view + connection
    /// chrome) with the usual post-apply sync, and drain its pending menu push.
    pub(crate) fn apply_connection_to_chrome(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        chrome_id: WidgetId,
        event: ClientConnectionEvent,
    ) {
        let menu = ctx
            .render_root(window_id)
            .edit_widget(chrome_id, |mut widget| {
                let mut editor = widget.try_downcast::<EditorWidget>()?;
                let caret_override = matches!(&event, ClientConnectionEvent::CaretStyleOverride(_));
                let changed = editor.widget.apply_connection_event(event);
                editor.widget.sync_region(&mut editor.ctx);
                editor.widget.sync_panels(&mut editor.ctx);
                editor.widget.sync_overlays(&mut editor.ctx);
                editor.widget.sync_empty_tab(&mut editor.ctx);
                if editor.widget.take_layout_invalidation() {
                    editor.ctx.request_layout();
                }
                if changed {
                    editor.ctx.request_render();
                    editor.ctx.request_accessibility_update();
                }
                // Plan 071 caret-transport fix: a newly installed animating caret
                // style must start its blink loop immediately.
                if caret_override && editor.widget.caret_animates() {
                    editor.ctx.request_anim_frame();
                }
                editor.widget.take_pending_menu()
            });
        if let Some(menu) = menu {
            self.apply_menu_sync(ctx, window_id, chrome_id, menu);
        } else {
            // Theme/SDUI events can change the centered host's cached render
            // context without pushing a new menu snapshot.
            self.sync_centered_layer(ctx, window_id, chrome_id);
        }
    }

    /// Show/clear the shared transient menu in the chrome overlay of the tab
    /// that produced it (Phase 22.3: menus are per-tab chrome state).
    pub(crate) fn apply_menu_sync(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        chrome_id: WidgetId,
        menu: Option<TransientMenuSession>,
    ) {
        ctx.render_root(window_id)
            .edit_widget(chrome_id, |mut widget| {
                if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                    editor.widget.set_active_menu(menu);
                    editor.widget.sync_overlays(&mut editor.ctx);
                    editor.ctx.request_render();
                    editor.ctx.request_accessibility_update();
                }
            });
        self.sync_centered_layer(ctx, window_id, chrome_id);
    }

    /// Apply one connection event to a pane content target (chrome or a
    /// `PaneDocumentView`) with post-apply sync, and forward the target's
    /// pending menu push to the chrome overlay.
    fn apply_event_to_target(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        chrome_id: WidgetId,
        target: WidgetId,
        event: ClientConnectionEvent,
    ) -> bool {
        let menu = ctx
            .render_root(window_id)
            .edit_widget(target, |mut widget| {
                if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                    let changed = editor.widget.apply_connection_event(event);
                    editor.widget.sync_region(&mut editor.ctx);
                    editor.widget.sync_panels(&mut editor.ctx);
                    editor.widget.sync_overlays(&mut editor.ctx);
                    editor.widget.sync_empty_tab(&mut editor.ctx);
                    if editor.widget.take_layout_invalidation() {
                        editor.ctx.request_layout();
                    }
                    if changed {
                        editor.ctx.request_render();
                        editor.ctx.request_accessibility_update();
                    }
                    editor.widget.take_pending_menu()
                } else if let Some(mut view) = widget.try_downcast::<PaneDocumentView>() {
                    let changed = view.widget.apply_connection_event(event);
                    view.widget.sync_empty_tab(&mut view.ctx);
                    if view.widget.take_layout_invalidation() {
                        view.ctx.request_layout();
                    }
                    if changed {
                        view.ctx.request_render();
                        view.ctx.request_accessibility_update();
                    }
                    view.widget.take_pending_menu()
                } else {
                    None
                }
            });
        let changed = menu.is_some();
        if let Some(menu) = menu {
            self.apply_menu_sync(ctx, window_id, chrome_id, menu);
        }
        changed
    }

    /// Apply one connection event to the event's tab active pane content
    /// target only (server-pushed editor commands; never fan out).
    fn apply_event_to_active_pane(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        chrome_id: WidgetId,
        event: ClientConnectionEvent,
    ) {
        let target = ctx
            .render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                widget
                    .try_downcast::<ClayShellWidget>()
                    .and_then(|shell| shell.widget.active_pane_target_for(client_id))
            });
        if let Some(target) = target {
            let _ = self.apply_event_to_target(ctx, window_id, chrome_id, target, event);
        }
    }

    /// Phase 24.2 shared shell-command driver path: used by local keybindings
    /// (`ClientUiCommandResult::ShellCommand`) and server-approved menu
    /// activation (`ShellClientCommandRequest`). Tab commands are
    /// driver-routed (active-tab resolution + bounds/wraparound policies +
    /// server-confirmed ordering); pane commands dispatch through the shell
    /// widget, with the dirty-close gate for `ClosePane`.
    fn apply_shell_client_command(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        command: clay::masonry_shell::ShellClientCommand,
    ) {
        // Phase 22.4: tab commands are driver-routed (active-tab resolution
        // + bounds/wraparound policies + server-confirmed ordering); the
        // shell widget's tab arms stay inert.
        if self.apply_tab_command(ctx, window_id, command) {
            return;
        }
        // Phase 22.2: closing a pane with unsaved edits is blocked (the view
        // shows the save-conflict menu); clean panes release their documents
        // server-side before the tree op.
        if matches!(command, clay::masonry_shell::ShellClientCommand::ClosePane) {
            let (may_close, target) =
                ctx.render_root(window_id)
                    .edit_widget(self.shell_widget_id, |mut widget| {
                        let Some(shell) = widget.try_downcast::<ClayShellWidget>() else {
                            return (false, None);
                        };
                        // Single-pane close is a no-op; skip the gate.
                        if shell.widget.pane_targets().len() <= 1 {
                            return (true, None);
                        }
                        // Phase 22.2: a closed pane's pending open can never be
                        // answered into it. Phase 22.3: the active tab's
                        // attribution map.
                        if let Some(tab) = self.tabs.get_mut(&self.active_tab) {
                            tab.pending_opens.remove(&shell.widget.active_pane_id());
                        }
                        (true, shell.widget.active_pane_target())
                    });
            let menu = if may_close {
                target.map(|target| {
                    ctx.render_root(window_id)
                        .edit_widget(target, |mut widget| {
                            if let Some(editor) = widget.try_downcast::<EditorWidget>() {
                                if !editor.widget.guard_pane_close() {
                                    return (false, editor.widget.take_pending_menu());
                                }
                                editor.widget.close_pane_view();
                                (true, None)
                            } else if let Some(view) = widget.try_downcast::<PaneDocumentView>() {
                                if !view.widget.guard_pane_close() {
                                    return (false, view.widget.take_pending_menu());
                                }
                                view.widget.close_pane();
                                (true, None)
                            } else {
                                (true, None)
                            }
                        })
                })
            } else {
                None
            };
            if let Some((may_close, menu)) = menu {
                if let Some(menu) = menu {
                    self.apply_menu_sync(ctx, window_id, self.editor_widget_id, menu);
                }
                if !may_close {
                    // Dirty pane: keep it open; the conflict menu offers
                    // Save/Discard/Keep.
                    return;
                }
            }
        }
        // Phase 22.1: dispatch pane-management commands to the shell.
        let shell_widget_id = self.shell_action_target();
        ctx.render_root(window_id)
            .edit_widget(shell_widget_id, |mut widget| {
                if let Some(mut shell) = widget.try_downcast::<ClayShellWidget>() {
                    shell
                        .widget
                        .apply_shell_client_command(&mut shell.ctx, command);
                    shell.ctx.request_accessibility_update();
                }
            });
        // Phase 22.2: keyboard routing follows pane focus — move Masonry
        // focus to the (possibly new) active pane's content.
        let target = ctx
            .render_root(window_id)
            .edit_widget(shell_widget_id, |mut widget| {
                widget
                    .try_downcast::<ClayShellWidget>()
                    .and_then(|shell| shell.widget.active_pane_target())
            });
        if let Some(target) = target {
            let _ = ctx.render_root(window_id).focus_on(Some(target));
        }
    }

    /// Route a document-scoped event to the pane view owning the document:
    /// try the focused pane's target first (the hot path), then the rest.
    /// Non-owning views no-op on foreign documents.
    fn route_document_event(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        chrome_id: WidgetId,
        event: ClientConnectionEvent,
    ) {
        let (active, targets) =
            ctx.render_root(window_id)
                .edit_widget(self.shell_widget_id, |mut widget| {
                    let Some(shell) = widget.try_downcast::<ClayShellWidget>() else {
                        return (PaneId(1), Vec::new());
                    };
                    (
                        shell.widget.active_pane_id_for(client_id),
                        shell.widget.pane_targets_for(client_id),
                    )
                });
        let mut ordered: Vec<(PaneId, WidgetId)> = Vec::with_capacity(targets.len());
        if let Some((pane, target)) = targets.iter().find(|(pane, _)| *pane == active) {
            ordered.push((*pane, *target));
        }
        for (pane, target) in &targets {
            if !ordered.iter().any(|(_, existing)| *existing == *target) {
                ordered.push((*pane, *target));
            }
        }
        for (pane, target) in ordered {
            if self.apply_event_to_target(ctx, window_id, chrome_id, target, event.clone()) {
                // Phase 22.6: keep the consuming pane's accessibility label
                // in sync with the document it now shows.
                if let Some(path) = event.metadata_path() {
                    self.set_pane_document_name(ctx, window_id, client_id, pane, path);
                }
                return;
            }
        }
        // Unmapped documents (pane closed mid-flight): dropped.
    }

    /// Phase 22.6: set a pane host's accessibility document name from a raw
    /// document path (sanitized at the shell boundary).
    fn set_pane_document_name(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        pane: PaneId,
        path: &str,
    ) {
        let path = path.to_string();
        ctx.render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                if let Some(mut shell) = widget.try_downcast::<ClayShellWidget>() {
                    shell.widget.set_pane_document_name(
                        &mut shell.ctx,
                        client_id,
                        pane,
                        Some(&path),
                    );
                }
            });
    }

    /// Route a request-scoped event (no document id) to the focused pane's
    /// view first, then the rest (request-id guards make non-owners no-op).
    fn route_request_scoped_event(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        chrome_id: WidgetId,
        event: ClientConnectionEvent,
    ) {
        self.route_document_event(ctx, window_id, client_id, chrome_id, event);
    }

    /// Route `DocumentOpened`: existing owners get the document applied in
    /// place (duplicate-open: no second view, the existing pane is focused);
    /// new documents open into the active pane (mounting a fresh view when the
    /// pane is a placeholder).
    fn route_document_opened(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        chrome_id: WidgetId,
        event: ClientConnectionEvent,
    ) {
        let Some(document_id) = event.document_id() else {
            return;
        };
        let metadata = match &event {
            ClientConnectionEvent::DocumentOpened { metadata, .. } => Some(metadata),
            _ => None,
        };
        let owner = self.find_pane_for_document(ctx, window_id, client_id, document_id);
        let active = ctx
            .render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                widget
                    .try_downcast::<ClayShellWidget>()
                    .map(|shell| shell.widget.active_pane_id_for(client_id))
                    .unwrap_or(PaneId(1))
            });
        // Pending-open attribution: consume the requesting pane's entry now,
        // whichever branch resolves (the request is answered either way).
        let requesting = metadata.and_then(|metadata| {
            self.tabs
                .get_mut(&client_id)
                .and_then(|tab| take_pending_open_for(&mut tab.pending_opens, metadata))
        });
        // Phase 22.6: document path for the consuming pane's accessibility
        // label, captured before `event` moves into the apply calls.
        let opened_path: Option<String> = metadata.map(|metadata| metadata.path.clone());
        let target_pane = match owner {
            Some(pane) => {
                // Duplicate open (or re-open of a retained document): apply to
                // the owning view (same-document opens no-op; retained copies
                // reload) and focus that pane. The requesting pane keeps its
                // previous content; no second view is ever created. Focus only
                // follows for the active tab (an inactive tab's open must not
                // steal focus from the mounted tab).
                let target =
                    ctx.render_root(window_id)
                        .edit_widget(self.shell_widget_id, |mut widget| {
                            widget
                                .try_downcast::<ClayShellWidget>()
                                .and_then(|shell| shell.widget.pane_target_for(client_id, pane))
                        });
                if let Some(target) = target {
                    let _ = self.apply_event_to_target(ctx, window_id, chrome_id, target, event);
                }
                if client_id == self.active_tab && pane != active {
                    self.focus_pane_target(ctx, window_id, pane);
                }
                if let Some(path) = &opened_path {
                    self.set_pane_document_name(ctx, window_id, client_id, pane, path);
                }
                return;
            }
            None => requesting.unwrap_or(active),
        };
        // New document: open into the requesting pane (falling back to the
        // active pane for server-initiated opens); placeholder panes get a
        // freshly mounted view.
        let target = ctx
            .render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                widget
                    .try_downcast::<ClayShellWidget>()
                    .and_then(|shell| shell.widget.pane_target_for(client_id, target_pane))
            });
        match target {
            Some(target) => {
                let _ = self.apply_event_to_target(ctx, window_id, chrome_id, target, event);
            }
            None => {
                if let Some(view_id) =
                    self.mount_document_view(ctx, window_id, client_id, target_pane)
                {
                    let _ = self.apply_event_to_target(ctx, window_id, chrome_id, view_id, event);
                }
            }
        }
        if let Some(path) = &opened_path {
            self.set_pane_document_name(ctx, window_id, client_id, target_pane, path);
        }
    }

    /// Find the pane whose view owns `document_id` (active or retained).
    fn find_pane_for_document(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        document_id: clay::protocol::DocumentId,
    ) -> Option<PaneId> {
        let targets = ctx
            .render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                widget
                    .try_downcast::<ClayShellWidget>()
                    .map(|shell| shell.widget.pane_targets_for(client_id))
                    .unwrap_or_default()
            });
        for (pane, target) in targets {
            let owns = ctx
                .render_root(window_id)
                .edit_widget(target, |mut widget| {
                    if let Some(editor) = widget.try_downcast::<EditorWidget>() {
                        editor.widget.contains_document(document_id)
                    } else if let Some(view) = widget.try_downcast::<PaneDocumentView>() {
                        view.widget.contains_document(document_id)
                    } else {
                        false
                    }
                });
            if owns {
                return Some(pane);
            }
        }
        None
    }

    /// Mount a fresh `PaneDocumentView` into a placeholder pane and register
    /// its routing target. Returns the new view's widget id.
    fn mount_document_view(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        pane: PaneId,
    ) -> Option<WidgetId> {
        let queue = self.tabs.get(&client_id)?.edit_queue.clone()?;
        let chrome_id =
            ctx.render_root(window_id)
                .edit_widget(self.shell_widget_id, |mut widget| {
                    widget
                        .try_downcast::<ClayShellWidget>()
                        .and_then(|shell| shell.widget.editor_widget_id_for(client_id))
                })?;
        let (baseline, menu_ids, ui_version) =
            ctx.render_root(window_id)
                .edit_widget(chrome_id, |mut widget| {
                    widget.try_downcast::<EditorWidget>().map(|editor| {
                        (
                            editor.widget.runtime_baseline(),
                            editor.widget.menu_session_ids_shared(),
                            editor.widget.sdui_ui_version_shared(),
                        )
                    })
                })?;
        let host_id =
            ctx.render_root(window_id)
                .edit_widget(self.shell_widget_id, |mut widget| {
                    widget
                        .try_downcast::<ClayShellWidget>()
                        .and_then(|shell| shell.widget.pane_host_id_for(client_id, pane))
                })?;
        let view = PaneDocumentView::new(pane, menu_ids, ui_version)
            .with_edit_queue(queue)
            .with_runtime_baseline(&baseline);
        let view_new = NewWidget::new(view);
        let view_id = view_new.id();
        ctx.render_root(window_id)
            .edit_widget(host_id, |mut widget| {
                if let Some(mut host) = widget.try_downcast::<PaneContentHost>() {
                    host.widget.set_document_view(&mut host.ctx, view_new);
                }
            });
        ctx.render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                if let Some(shell) = widget.try_downcast::<ClayShellWidget>() {
                    shell.widget.set_pane_target_for(client_id, pane, view_id);
                }
            });
        Some(view_id)
    }

    /// Activate `pane` and move Masonry focus to its content widget.
    fn focus_pane_target(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        pane: PaneId,
    ) {
        let target = ctx
            .render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                let mut shell = widget.try_downcast::<ClayShellWidget>()?;
                shell.widget.set_active_pane(pane);
                shell.ctx.request_render();
                shell.widget.pane_target(pane)
            });
        if let Some(target) = target {
            let _ = ctx.render_root(window_id).focus_on(Some(target));
        }
    }

    /// Fan one connection-wide event out to every pane view (panes 2+; the
    /// chrome already applied it to pane 1).
    fn fan_out_event(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        chrome_id: WidgetId,
        event: ClientConnectionEvent,
    ) {
        let targets = ctx
            .render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                widget
                    .try_downcast::<ClayShellWidget>()
                    .map(|shell| shell.widget.pane_targets_for(client_id))
                    .unwrap_or_default()
            });
        for (_, target) in targets {
            if target == chrome_id {
                continue;
            }
            let _ = self.apply_event_to_target(ctx, window_id, chrome_id, target, event.clone());
        }
    }

    /// Resolve one `ActiveTheme` snapshot into the shell's UI theme so split
    /// placeholder panes, dividers, and the tab bar follow the active theme.
    fn sync_shell_ui_theme(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        theme: &clay::protocol::ActiveTheme,
    ) {
        ctx.render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                if let Some(mut shell) = widget.try_downcast::<ClayShellWidget>() {
                    shell.widget.set_active_theme(&mut shell.ctx, theme);
                    shell.ctx.request_render();
                }
            });
    }

    /// Resolve one `ActiveTypography` snapshot for the shell's window-level
    /// chrome. The shell keeps one cached registry per mounted tab and uses
    /// the active tab's logical metrics for tab-bar geometry and paint.
    fn sync_shell_ui_typography(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        typography: &clay::protocol::ActiveTypography,
    ) {
        ctx.render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                if let Some(mut shell) = widget.try_downcast::<ClayShellWidget>()
                    && shell
                        .widget
                        .set_active_typography(client_id, typography.clone())
                {
                    shell.ctx.request_layout();
                    shell.ctx.request_render();
                    shell.ctx.request_accessibility_update();
                }
            });
    }

    /// Fan the per-document render parts of a runtime snapshot out to every
    /// pane view (panes 2+; the chrome installed pane 1).
    fn fan_out_runtime_snapshot(
        &mut self,
        ctx: &mut DriverCtx<'_, '_>,
        window_id: WindowId,
        client_id: ClientId,
        chrome_id: WidgetId,
        snapshot: clay::protocol::RuntimeStateSnapshot,
    ) {
        let targets = ctx
            .render_root(window_id)
            .edit_widget(self.shell_widget_id, |mut widget| {
                widget
                    .try_downcast::<ClayShellWidget>()
                    .map(|shell| shell.widget.pane_targets_for(client_id))
                    .unwrap_or_default()
            });
        for (_, target) in targets {
            if target == chrome_id {
                continue;
            }
            ctx.render_root(window_id)
                .edit_widget(target, |mut widget| {
                    if let Some(mut view) = widget.try_downcast::<PaneDocumentView>() {
                        let changed = view.widget.apply_runtime_snapshot(&snapshot);
                        if view.widget.take_layout_invalidation() {
                            view.ctx.request_layout();
                        }
                        if changed {
                            view.ctx.request_render();
                            view.ctx.request_accessibility_update();
                        }
                    }
                });
        }
    }
}

pub(crate) fn is_linux_portal_dialog_command(command_id: &str) -> bool {
    cfg!(target_os = "linux")
        && matches!(
            command_id,
            "documents.clientOpenFileDialog" | "workspace.clientOpenFolderDialog"
        )
}

impl AppDriver for Driver {
    fn on_start(&mut self, state: &mut masonry_winit::app::MasonryState<'_>) {
        // Phase 22.2: the focus fallback follows pane focus — the active pane's
        // content widget (or the chrome when no pane has a view yet).
        for root in state.roots() {
            let fallback = root
                .edit_widget(self.shell_widget_id, |mut widget| {
                    widget
                        .try_downcast::<ClayShellWidget>()
                        .and_then(|shell| shell.widget.active_pane_target())
                })
                .unwrap_or(self.editor_widget_id);
            root.set_focus_fallback(Some(fallback));
        }
    }

    fn on_close_requested(&mut self, _window_id: WindowId, ctx: &mut DriverCtx<'_, '_>) {
        self.clear_native_dialogs();
        ctx.exit();
    }

    fn on_action(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_, '_>,
        widget_id: WidgetId,
        action: ErasedAction,
    ) {
        // Phase 22.5: a restore mount awaiting confirmation that never comes
        // (the server rejected it — `FileOperationFailed` instead of a
        // snapshot) abandons the remaining restore after the deadline. The
        // check rides every action, so user activity keeps the deadline live.
        if self
            .restore_gate
            .is_some_and(|(_, deadline)| Instant::now() >= deadline)
        {
            self.abandon_restore(ctx, window_id);
        }
        // Reconciled SDUI widget activation (button step 9 / list row step 10):
        // the widget carries its inert intent in the action; route it through the
        // editor's existing server-first command path.
        let action = match action.downcast::<SduiButtonPress>() {
            Ok(action) => {
                self.route_sdui_intent(ctx, window_id, widget_id, action.intent);
                return;
            }
            Err(action) => action,
        };
        let action = match action.downcast::<SduiListRowPress>() {
            Ok(action) => {
                self.route_sdui_intent(ctx, window_id, widget_id, action.intent);
                return;
            }
            Err(action) => action,
        };
        // Reconciled package fixed-panel activations (button/list row, step 13b):
        // same inert-intent server-first routing as the SDUI tree actions.
        let action = match action.downcast::<PackageButtonPress>() {
            Ok(action) => {
                self.route_sdui_intent(ctx, window_id, widget_id, action.intent);
                return;
            }
            Err(action) => action,
        };
        let action = match action.downcast::<PackageListRowPress>() {
            Ok(action) => {
                self.route_sdui_intent(ctx, window_id, widget_id, action.intent);
                return;
            }
            Err(action) => action,
        };
        // Reconciled package `dropdown` confirmation (step 13d): carries the
        // confirmed item's command intent; same server-first routing.
        let action = match action.downcast::<PackageDropdownSelect>() {
            Ok(action) => {
                self.route_sdui_intent(ctx, window_id, widget_id, action.intent);
                return;
            }
            Err(action) => action,
        };
        // Package modal Escape carries its declared inert dismiss intent, when
        // present, through the same server-first route as other package actions.
        let action = match action.downcast::<PackageModalDismiss>() {
            Ok(action) => {
                if let Some(intent) = action.intent {
                    self.route_sdui_intent(ctx, window_id, widget_id, intent);
                }
                return;
            }
            Err(action) => action,
        };
        // Package `textInput` commit (step 13c): the inner Masonry `TextArea`
        // emits `TextAction` with its own widget id; `Entered` is the committed
        // value (route to the server), `Changed` is optimistic-local (the field
        // already reflects it — no per-keystroke server sync).
        let action = match action.downcast::<TextAction>() {
            Ok(text_action) => {
                if let TextAction::Entered(value) = *text_action {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>()
                                && let Some(intent) = EditorWidget::package_text_input_commit(
                                    &mut editor,
                                    widget_id,
                                    &value,
                                )
                            {
                                editor.widget.enqueue_sdui_intent(intent);
                            }
                        });
                }
                return;
            }
            Err(action) => action,
        };

        let Ok(action) = action.downcast::<EditorAction>() else {
            return;
        };

        match *action {
            EditorAction::PersistenceDue => {
                // Phase 22.5: a shell layout mutation committed (the shell
                // debounced the signal); persist the whole window.
                self.persist_window_state(ctx, window_id);
            }
            EditorAction::MenuStateChanged => {
                // A transient menu's local state changed via the keyboard in a
                // pane view; push the view's current session to the chrome
                // overlay (Phase 22.2: menus are view-owned, displayed by the
                // connection owner).
                let menu = ctx
                    .render_root(window_id)
                    .edit_widget(widget_id, |mut widget| {
                        if let Some(view) = widget.try_downcast::<PaneDocumentView>() {
                            view.widget.take_pending_menu()
                        } else if let Some(editor) = widget.try_downcast::<EditorWidget>() {
                            editor.widget.take_pending_menu()
                        } else {
                            None
                        }
                    });
                if let Some(menu) = menu {
                    self.apply_menu_sync(ctx, window_id, self.editor_widget_id, menu);
                }
            }
            EditorAction::ClientConnection(event) => {
                // Phase 22.3: resolve the event's tab from the action's target
                // chrome id (each tab's bridge tags its events with that tab's
                // chrome). Events for a closed tab are dropped.
                let Some(client_id) =
                    ctx.render_root(window_id)
                        .edit_widget(self.shell_widget_id, |mut widget| {
                            widget
                                .try_downcast::<ClayShellWidget>()
                                .and_then(|shell| shell.widget.tab_for_chrome(widget_id))
                        })
                else {
                    return;
                };
                let chrome_id = widget_id;
                // Phase 22.4: a save-all-and-close flow awaiting acks for
                // this tab — count down `DocumentSaved`, cancel on a failed
                // save or a disconnect (the close never happens until every
                // save acked; the event still routes normally below so the
                // pane view updates its own state).
                if advance_pending_close_after_saves(
                    &mut self.pending_close_after_saves,
                    client_id,
                    &event,
                ) {
                    self.enqueue_close(client_id);
                }
                // Phase 22.3: the server-authoritative tab registry is
                // driver-level state (tab ids, order, active); the chrome does
                // not render it (the tab bar is the lifecycle task).
                if let clay::client::ClientConnectionEvent::TabRegistry(snapshot) = &event {
                    // Ordering guard: relays from different connections
                    // interleave — a stale replay must never delete tabs a
                    // newer broadcast already confirmed.
                    if !self.accept_registry_snapshot(snapshot) {
                        return;
                    }
                    let reconcile = self.apply_tab_registry(snapshot.clone());
                    self.apply_registry_reconcile(ctx, window_id, reconcile);
                    // Phase 22.5: tab lifecycle (mount/close/reorder/active/
                    // workspace) is registry-driven — persist the window.
                    self.persist_window_state(ctx, window_id);
                    // Phase 22.5: a registry snapshot confirms the last
                    // restore mount; advance the gate (next connect, or
                    // finish: documents + active tab).
                    self.advance_restore(ctx, window_id);
                    return;
                }
                // Phase 22.1: shell-level preferences go to the shell widget,
                // not the editor. Phase 22.3: each tab's connection carries
                // its own pane-focus policy.
                if let clay::client::ClientConnectionEvent::ShellPreferences(prefs) = &event {
                    let shell_widget_id = self.shell_action_target();
                    ctx.render_root(window_id)
                        .edit_widget(shell_widget_id, |mut widget| {
                            if let Some(mut shell) = widget.try_downcast::<ClayShellWidget>() {
                                shell.widget.set_pane_focus_policy_for(
                                    client_id,
                                    clay::masonry_shell::PaneFocusPolicy::from_config_str(
                                        &prefs.pane_focus_policy,
                                    ),
                                );
                                shell.ctx.request_render();
                            }
                        });
                    return;
                }
                // Phase 22.2: route document-scoped events to the pane view
                // owning the document; fan connection-wide events out to every
                // pane view through the chrome. Phase 22.3: all routing is
                // scoped to the event's tab.
                if let Some(_document_id) = event.document_id() {
                    if matches!(event, ClientConnectionEvent::DocumentOpened { .. }) {
                        self.route_document_opened(ctx, window_id, client_id, chrome_id, event);
                        // Phase 22.5: a pane's active document changed.
                        self.persist_window_state(ctx, window_id);
                    } else {
                        self.route_document_event(ctx, window_id, client_id, chrome_id, event);
                    }
                } else {
                    match &event {
                        ClientConnectionEvent::SduiSnapshot { .. }
                        | ClientConnectionEvent::SduiUpdate(_)
                        | ClientConnectionEvent::EditTransaction(_)
                        | ClientConnectionEvent::BehaviorManifestRejected { .. } => {
                            // Chrome-only connection state.
                            self.apply_connection_to_chrome(ctx, window_id, chrome_id, event);
                        }
                        // Phase 24.1: server-owned menu snapshots are per-tab
                        // chrome state — the chrome overlay renders them and
                        // every pane view keeps the interactive copy so keys
                        // dispatch from whichever pane is focused (each view
                        // pushes the same session through the pending-push
                        // path; `set_active_menu` is idempotent).
                        ClientConnectionEvent::TransientMenuSnapshot(_)
                        | ClientConnectionEvent::TransientMenuClosed { .. } => {
                            self.apply_connection_to_chrome(
                                ctx,
                                window_id,
                                chrome_id,
                                event.clone(),
                            );
                            self.fan_out_event(ctx, window_id, client_id, chrome_id, event);
                        }
                        ClientConnectionEvent::RuntimeStateSnapshot(_) => {
                            // Chrome installs fully (exactly one ack); other
                            // panes get the per-document render parts.
                            let snapshot = match &event {
                                ClientConnectionEvent::RuntimeStateSnapshot(snapshot) => {
                                    (**snapshot).clone()
                                }
                                _ => unreachable!("matched RuntimeStateSnapshot"),
                            };
                            self.sync_shell_ui_theme(ctx, window_id, &snapshot.active_theme);
                            self.sync_shell_ui_typography(
                                ctx,
                                window_id,
                                client_id,
                                &snapshot.active_typography,
                            );
                            self.apply_connection_to_chrome(ctx, window_id, chrome_id, event);
                            // A runtime-generation replacement cancels any
                            // centered session; never let the old layer outlive
                            // the new catalogue/theme install.
                            if client_id == self.active_tab {
                                self.remove_centered_layer(ctx.render_root(window_id));
                            }
                            self.fan_out_runtime_snapshot(
                                ctx, window_id, client_id, chrome_id, snapshot,
                            );
                        }
                        ClientConnectionEvent::Disconnected
                        | ClientConnectionEvent::ConnectionError(_) => {
                            if client_id == self.active_tab {
                                self.remove_centered_layer(ctx.render_root(window_id));
                            }
                            self.apply_connection_to_chrome(
                                ctx,
                                window_id,
                                chrome_id,
                                event.clone(),
                            );
                            self.fan_out_event(ctx, window_id, client_id, chrome_id, event);
                            // Phase 22.3/22.8: reconnect this tab's connection;
                            // `Reclaim` preserves live server state, while a
                            // reset/evicted registry entry falls back to `New`
                            // from the tab's workspace root.
                            self.start_tab_reconnect(client_id);
                        }
                        ClientConnectionEvent::ActiveTheme(_)
                        | ClientConnectionEvent::ActiveTypography(_)
                        | ClientConnectionEvent::BehaviorManifestInstalled { .. }
                        | ClientConnectionEvent::CaretStyleOverride(_)
                        | ClientConnectionEvent::RuntimeDiagnostic(_)
                        | ClientConnectionEvent::ServerError { .. } => {
                            if let ClientConnectionEvent::ActiveTheme(theme) = &event {
                                self.sync_shell_ui_theme(ctx, window_id, theme);
                            }
                            if let ClientConnectionEvent::ActiveTypography(typography) = &event {
                                self.sync_shell_ui_typography(
                                    ctx, window_id, client_id, typography,
                                );
                            }
                            self.apply_connection_to_chrome(
                                ctx,
                                window_id,
                                chrome_id,
                                event.clone(),
                            );
                            self.fan_out_event(ctx, window_id, client_id, chrome_id, event);
                        }
                        ClientConnectionEvent::EditorCommandRequest(_) => {
                            // Server-pushed editor commands execute on the
                            // focused pane only (never fan out).
                            self.apply_event_to_active_pane(
                                ctx, window_id, client_id, chrome_id, event,
                            );
                        }
                        ClientConnectionEvent::ShellClientCommandRequest { command_id } => {
                            // Phase 24.2: server-approved shell command from
                            // menu activation. The client re-parses the id
                            // deny-by-default; unknown/forged ids are dropped
                            // with no state mutation. Phase 28.2: the same
                            // bridge also dispatches client-mapped editor
                            // transforms (toggle comment/list/heading).
                            if let Some(command) =
                                clay::masonry_shell::ShellClientCommand::from_command_id(command_id)
                            {
                                self.apply_shell_client_command(ctx, window_id, command);
                            } else if let Some(command) =
                                clay::masonry_editor::EditorClientCommand::from_command_id(
                                    command_id,
                                )
                            {
                                let editor_widget_id =
                                    self.editor_action_target(self.shell_widget_id);
                                ctx.render_root(window_id).edit_widget(
                                    editor_widget_id,
                                    |mut widget| {
                                        if let Some(mut editor) =
                                            widget.try_downcast::<EditorWidget>()
                                            && editor.widget.apply_editor_client_command(command)
                                        {
                                            editor.ctx.request_render();
                                            editor.ctx.request_accessibility_update();
                                        }
                                    },
                                );
                            }
                        }
                        _ => {
                            // Request-scoped events (completion / language-
                            // intelligence rejects): try the focused pane's
                            // view first, then the rest (request ids gate).
                            self.route_request_scoped_event(
                                ctx, window_id, client_id, chrome_id, event,
                            );
                        }
                    }
                }
            }
            EditorAction::ReconnectTabConnected { client_id, session } => {
                self.reconnect_tab(ctx, window_id, client_id, session);
            }

            EditorAction::TabBar(action) => {
                // Phase 22.3: tab bar clicks. Activate switches optimistically
                // (the server registry is the reconciling authority — a
                // rejected activate pushes a snapshot that reverts). Close
                // enqueues `TabCommand::Close` on the tab's own connection;
                // the server removes the registry entry and ends the
                // connection, and the pushed snapshot drives the removal. The
                // 22.3 dirty-guard (blocking close with dirty documents) is
                // the lifecycle task; closing the last mounted tab is refused
                // (the bar hides at one tab).
                match action {
                    clay::masonry_editor::TabBarAction::Activate { client_id } => {
                        // Phase 22.4: shared activation path (also used by
                        // the `clientTabActivate.N` / next / prev chords):
                        // optimistic switch + server-registry reconcile.
                        self.activate_tab(ctx, window_id, client_id);
                    }
                    clay::masonry_editor::TabBarAction::Close { client_id } => {
                        // Phase 22.4: shared close path (also used by the
                        // `clientTabClose` chord): last-tab protection +
                        // dirty guard + server-confirmed `TabCommand::Close`.
                        self.close_tab(ctx, window_id, client_id);
                    }
                    clay::masonry_editor::TabBarAction::NewTab => {
                        // Phase 22.4: shared open-tab affordance (also used
                        // by the `clientTabNew` chord): folder picker →
                        // bind `TabCommand::New` during connect → mount.
                        self.open_new_tab_dialog();
                    }
                }
            }
            EditorAction::NewTabFolderDialogCompleted { generation, result } => {
                if !self.finish_folder_dialog(generation) {
                    return;
                }
                let clay::client::FileDialogResult::Selected(workspace_root) = result else {
                    return;
                };
                // Connect the new tab on the runtime (the handshake involves
                // retries); on success the session returns here to mount.
                let Some(proxy) = self.proxy.clone() else {
                    return;
                };
                let endpoint = self.endpoint.clone();
                let window_id = self.window_id;
                let shell_widget_id = self.shell_widget_id;
                self.runtime.spawn(async move {
                    match client::connect_with_workspace_root(
                        &endpoint,
                        workspace_root.to_string_lossy().into_owned(),
                    )
                    .await
                    {
                        Ok(session) => {
                            let _ = proxy.send_event(MasonryUserEvent::Action(
                                window_id,
                                Box::new(EditorAction::OpenTabConnected {
                                    session: clay::masonry_editor::DriverSession { session },
                                    workspace_root,
                                }),
                                shell_widget_id,
                            ));
                        }
                        Err(error) => {
                            let _ = proxy.send_event(MasonryUserEvent::Action(
                                window_id,
                                Box::new(EditorAction::OpenTabFailed {
                                    message: format!("Could not open a new tab: {error}"),
                                }),
                                shell_widget_id,
                            ));
                        }
                    }
                });
            }
            EditorAction::OpenTabConnected {
                session,
                workspace_root,
            } => {
                if let Some((index, persisted)) = self.restore_pending.take() {
                    // Phase 22.5: restore mount — the persisted tab drives
                    // the chrome (split tree + documents).
                    self.mount_restored_tab(ctx, window_id, session.session, index, persisted);
                } else {
                    // Mount the new tab (chrome + default split tree) and switch
                    // to it; `TabCommand::New` registers it server-side. A
                    // duplicate connection (already mounted) is dropped by
                    // `mount_tab` — the previous tab stays.
                    self.mount_tab(ctx, window_id, session.session, workspace_root);
                }
            }
            EditorAction::OpenTabFailed { message } => {
                // Phase 22.5: a restore connect failed (refused at the
                // connection cap, server down): mounted tabs stay, the
                // remaining restore drops.
                if self.restore_pending.is_some() || !self.restore_queue.is_empty() {
                    self.abandon_restore(ctx, window_id);
                }
                // Refused gracefully (connection cap, server down): no tab is
                // opened; the active tab's chrome surfaces the diagnostic.
                self.apply_connection_to_chrome(
                    ctx,
                    window_id,
                    self.editor_widget_id,
                    ClientConnectionEvent::RuntimeDiagnostic(
                        clay::protocol::RuntimeDiagnostic::error("tabs.open_failed", message),
                    ),
                );
            }
            EditorAction::PaneFocused(pane_id) => {
                // Phase 22.2: pane activation follows Masonry focus. Sync the
                // shell's active pane and move focus to the pane's content
                // widget only when the active pane changed (sidebar/panel
                // focus changes inside the chrome keep their focus).
                let (active, target) =
                    ctx.render_root(window_id)
                        .edit_widget(self.shell_widget_id, |mut widget| {
                            let Some(mut shell) = widget.try_downcast::<ClayShellWidget>() else {
                                return (pane_id, None);
                            };
                            let active = shell.widget.active_pane_id();
                            let target = shell.widget.pane_target(pane_id);
                            if active != pane_id {
                                shell.widget.set_active_pane(pane_id);
                                shell.ctx.request_render();
                            }
                            (active, target)
                        });
                if active != pane_id
                    && let Some(target) = target
                {
                    let _ = ctx.render_root(window_id).focus_on(Some(target));
                }
            }
            EditorAction::RecordPendingOpenIntent {
                root_id,
                relative_path,
            } => {
                // Phase 22.2: a pane view dispatched a workspace open intent
                // (definition navigation) directly; record the active pane as
                // the open target.
                self.record_pending_open(
                    ctx,
                    window_id,
                    PendingOpenRequest {
                        path: None,
                        root_id: Some(root_id),
                        relative_path: Some(relative_path),
                    },
                );
            }
            EditorAction::ActivateDocumentInPane {
                document_id,
                pane_id,
            } => {
                // Phase 22.2: cross-pane open-documents activation. The
                // document lives in `pane_id`'s session store (duplicate
                // opens stay blocked); switch that pane to it and focus it.
                let target =
                    ctx.render_root(window_id)
                        .edit_widget(self.shell_widget_id, |mut widget| {
                            widget
                                .try_downcast::<ClayShellWidget>()
                                .and_then(|shell| shell.widget.pane_target(pane_id))
                        });
                let changed = target.map(|target| {
                    ctx.render_root(window_id)
                        .edit_widget(target, |mut target_widget| {
                            if let Some(editor) = target_widget.try_downcast::<EditorWidget>() {
                                editor.widget.activate_document(document_id)
                            } else if let Some(view) =
                                target_widget.try_downcast::<PaneDocumentView>()
                            {
                                view.widget.activate_document(document_id)
                            } else {
                                false
                            }
                        })
                });
                if let (Some(target), Some(true)) = (target, changed) {
                    let _: Result<(), Box<dyn std::error::Error>> =
                        ctx.render_root(window_id).edit_widget(target, |mut w| {
                            w.ctx.request_render();
                            w.ctx.request_accessibility_update();
                            Ok(())
                        });
                }
                self.focus_pane_target(ctx, window_id, pane_id);
            }
            EditorAction::TabCloseMenuAction {
                client_id,
                command_id,
            } => {
                // Phase 22.4: the tab-close confirm menu's choice. The
                // session is driver-owned; the pane view handed the selection
                // here (tab-confirm actions never reach the server).
                match command_id.as_str() {
                    "shell.clientTabCloseSaveAll" => {
                        self.save_all_then_close_tab(ctx, window_id, client_id);
                    }
                    "shell.clientTabCloseDiscard" => {
                        // Explicit destructive choice: drop the unsaved edits
                        // and close. The server's disconnect teardown releases
                        // the tab's documents.
                        self.enqueue_close(client_id);
                    }
                    "shell.clientTabCloseCancel" => {
                        self.show_tab_close_confirm_menu(ctx, window_id, client_id, None);
                    }
                    _ => {}
                }
            }
            EditorAction::ClientUiCommand(command)
                if is_linux_portal_dialog_command(&command.command_id) =>
            {
                // Blocking the Wayland event loop prevents the portal chooser from presenting.
                self.spawn_native_dialog_command(command);
            }
            EditorAction::FileDialogCompleted { generation, result } => {
                if self.finish_file_dialog(generation) {
                    self.apply_native_dialog_completion(
                        window_id,
                        ctx,
                        self.editor_action_target(widget_id),
                        result,
                        SelectedPathKind::File,
                    );
                }
            }
            EditorAction::FolderDialogCompleted { generation, result } => {
                if self.finish_folder_dialog(generation) {
                    self.apply_native_dialog_completion(
                        window_id,
                        ctx,
                        self.editor_action_target(widget_id),
                        result,
                        SelectedPathKind::Folder,
                    );
                }
            }
            EditorAction::ClientUiCommand(command) => match handle_client_ui_command(&command) {
                ClientUiCommandResult::None => {}
                ClientUiCommandResult::ConnectionEvent(event) => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed = editor.widget.apply_connection_event(event);
                                editor.widget.sync_region(&mut editor.ctx);
                                editor.widget.sync_panels(&mut editor.ctx);
                                editor.widget.sync_overlays(&mut editor.ctx);
                                editor.widget.sync_empty_tab(&mut editor.ctx);
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::SelectedFile(path) => {
                    // Dialog commands are handled asynchronously above; keep this
                    // arm so non-dialog callers/tests still map cleanly if reused.
                    // Phase 22.2: attribute the open to the active pane so the
                    // answering DocumentOpened lands in the focused pane.
                    self.record_pending_open(
                        ctx,
                        window_id,
                        PendingOpenRequest {
                            path: Some(path.clone()),
                            root_id: None,
                            relative_path: None,
                        },
                    );
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed =
                                    editor.widget.request_selected_file_open(path).is_some_and(
                                        |event| editor.widget.apply_connection_event(event),
                                    );
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::SelectedFolder(path) => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed = editor
                                    .widget
                                    .request_selected_workspace_root(path)
                                    .is_some_and(|event| {
                                        editor.widget.apply_connection_event(event)
                                    });
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::CopySelection => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let changed = editor
                                    .widget
                                    .copy_selection_to_system_clipboard()
                                    .is_some_and(|event| {
                                        editor.widget.apply_connection_event(event)
                                    });
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::CutSelection => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let outcome = editor.widget.cut_selection_to_system_clipboard();
                                let mut changed = outcome.changed;
                                if let Some(event) = outcome.diagnostic {
                                    changed |= editor.widget.apply_connection_event(event);
                                }
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::PasteClipboard => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let outcome = editor.widget.paste_from_system_clipboard();
                                let mut changed = outcome.changed;
                                if let Some(event) = outcome.diagnostic {
                                    changed |= editor.widget.apply_connection_event(event);
                                }
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::Undo => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>()
                                && editor.widget.undo()
                            {
                                editor.ctx.request_render();
                                editor.ctx.request_accessibility_update();
                            }
                        });
                }
                ClientUiCommandResult::Redo => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>()
                                && editor.widget.redo()
                            {
                                editor.ctx.request_render();
                                editor.ctx.request_accessibility_update();
                            }
                        });
                }
                ClientUiCommandResult::ShowOpenDocuments => {
                    // Phase 22.2: the menu belongs to the focused pane and
                    // aggregates every pane's documents (own sessions plus
                    // cross-pane entries labeled by pane).
                    let (active, targets) = ctx.render_root(window_id).edit_widget(
                        self.shell_widget_id,
                        |mut widget| {
                            let Some(shell) = widget.try_downcast::<ClayShellWidget>() else {
                                return (PaneId(1), Vec::new());
                            };
                            (shell.widget.active_pane_id(), shell.widget.pane_targets())
                        },
                    );
                    let mut other = Vec::new();
                    for (pane, pane_target) in targets {
                        if pane == active {
                            continue;
                        }
                        let (active_info, retained) = ctx.render_root(window_id).edit_widget(
                            pane_target,
                            |mut pane_widget| {
                                if let Some(editor) = pane_widget.try_downcast::<EditorWidget>() {
                                    (
                                        editor.widget.active_document_info(),
                                        editor.widget.retained_documents(),
                                    )
                                } else if let Some(view) =
                                    pane_widget.try_downcast::<PaneDocumentView>()
                                {
                                    (
                                        view.widget.active_document_info(),
                                        view.widget.retained_documents(),
                                    )
                                } else {
                                    (None, Vec::new())
                                }
                            },
                        );
                        if let Some((document_id, display_name, dirty)) = active_info {
                            other.push(CrossPaneDocumentEntry {
                                pane,
                                document_id,
                                display_name,
                                dirty,
                                retained,
                            });
                        }
                    }
                    let target = ctx.render_root(window_id).edit_widget(
                        self.shell_widget_id,
                        |mut widget| {
                            widget
                                .try_downcast::<ClayShellWidget>()
                                .and_then(|shell| shell.widget.pane_target(active))
                        },
                    );
                    let Some(target) = target else {
                        return;
                    };
                    let changed =
                        ctx.render_root(window_id)
                            .edit_widget(target, |mut target_widget| {
                                if let Some(editor) = target_widget.try_downcast::<EditorWidget>() {
                                    editor.widget.show_open_documents_menu(&other)
                                } else if let Some(view) =
                                    target_widget.try_downcast::<PaneDocumentView>()
                                {
                                    view.widget.show_open_documents_menu(&other)
                                } else {
                                    false
                                }
                            });
                    if changed {
                        let _: Result<(), Box<dyn std::error::Error>> =
                            ctx.render_root(window_id).edit_widget(target, |mut w| {
                                w.ctx.request_render();
                                w.ctx.request_accessibility_update();
                                Ok(())
                            });
                    }
                }
                ClientUiCommandResult::RequestResync => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>() {
                                let mut changed = false;
                                if let Some(diagnostic) =
                                    editor.widget.request_resync_active_document()
                                {
                                    changed |= editor.widget.apply_connection_event(diagnostic);
                                } else {
                                    changed = true;
                                }
                                if changed {
                                    editor.ctx.request_render();
                                    editor.ctx.request_accessibility_update();
                                }
                            }
                        });
                }
                ClientUiCommandResult::DismissRecovery => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>()
                                && editor.widget.dismiss_recovery()
                            {
                                editor.ctx.request_render();
                                editor.ctx.request_accessibility_update();
                            }
                        });
                }
                ClientUiCommandResult::EditorCommand(command) => {
                    let editor_widget_id = self.editor_action_target(widget_id);
                    ctx.render_root(window_id)
                        .edit_widget(editor_widget_id, |mut widget| {
                            if let Some(mut editor) = widget.try_downcast::<EditorWidget>()
                                && editor.widget.apply_editor_client_command(command)
                            {
                                editor.ctx.request_render();
                                editor.ctx.request_accessibility_update();
                            }
                        });
                }
                ClientUiCommandResult::ShellCommand(command) => {
                    self.apply_shell_client_command(ctx, window_id, command);
                }
            },
        }
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "GUI command results are low-volume and only one variant carries a selected path"
)]
pub(crate) enum ClientUiCommandResult {
    None,
    ConnectionEvent(ClientConnectionEvent),
    SelectedFile(PathBuf),
    SelectedFolder(PathBuf),
    CopySelection,
    CutSelection,
    PasteClipboard,
    Undo,
    Redo,
    ShowOpenDocuments,
    RequestResync,
    DismissRecovery,
    EditorCommand(clay::masonry_editor::EditorClientCommand),
    /// Phase 22.1: shell pane-management command (split/close/focus/resize/move).
    ShellCommand(clay::masonry_shell::ShellClientCommand),
}

pub(crate) fn handle_client_ui_command(
    command: &clay::client::ClientUiCommandRoute,
) -> ClientUiCommandResult {
    match command.command_id.as_str() {
        "documents.clientOpenFileDialog" => client_dialog_result_to_command_result(
            clay::client::open_markdown_file_dialog(),
            SelectedPathKind::File,
        ),
        "workspace.clientOpenFolderDialog" => client_dialog_result_to_command_result(
            clay::client::open_folder_dialog(),
            SelectedPathKind::Folder,
        ),
        "editor.clientCopySelection" => ClientUiCommandResult::CopySelection,
        "editor.clientCutSelection" => ClientUiCommandResult::CutSelection,
        "editor.clientPasteClipboard" => ClientUiCommandResult::PasteClipboard,
        "editor.clientUndo" => ClientUiCommandResult::Undo,
        "editor.clientRedo" => ClientUiCommandResult::Redo,
        "editor.clientShowOpenDocuments" => ClientUiCommandResult::ShowOpenDocuments,
        "editor.clientRequestResync" => ClientUiCommandResult::RequestResync,
        "editor.clientDismissRecovery" => ClientUiCommandResult::DismissRecovery,
        command_id
            if let Some(command) =
                clay::masonry_editor::EditorClientCommand::from_command_id(command_id) =>
        {
            ClientUiCommandResult::EditorCommand(command)
        }
        command_id
            if let Some(command) =
                clay::masonry_shell::ShellClientCommand::from_command_id(command_id) =>
        {
            ClientUiCommandResult::ShellCommand(command)
        }
        _ => ClientUiCommandResult::None,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SelectedPathKind {
    File,
    Folder,
}

pub(crate) fn client_dialog_result_to_command_result(
    result: clay::client::FileDialogResult,
    kind: SelectedPathKind,
) -> ClientUiCommandResult {
    match result {
        clay::client::FileDialogResult::Selected(path) => match kind {
            SelectedPathKind::File => ClientUiCommandResult::SelectedFile(path),
            SelectedPathKind::Folder => ClientUiCommandResult::SelectedFolder(path),
        },
        clay::client::FileDialogResult::Cancelled => ClientUiCommandResult::None,
        clay::client::FileDialogResult::Unsupported { message } => {
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(
                clay::protocol::RuntimeDiagnostic::error("client.file_dialog.unsupported", message),
            ))
        }
        clay::client::FileDialogResult::Failed { message } => {
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(
                clay::protocol::RuntimeDiagnostic::error("client.file_dialog.failed", message),
            ))
        }
    }
}

impl Driver {
    fn apply_native_dialog_completion(
        &mut self,
        window_id: WindowId,
        ctx: &mut DriverCtx<'_, '_>,
        editor_widget_id: WidgetId,
        result: clay::client::FileDialogResult,
        kind: SelectedPathKind,
    ) {
        let result = client_dialog_result_to_command_result(result, kind);
        if let ClientUiCommandResult::SelectedFile(path) = &result {
            // Phase 22.2: attribute the dialog open to the active pane so the
            // answering DocumentOpened lands in the focused pane.
            self.record_pending_open(
                ctx,
                window_id,
                PendingOpenRequest {
                    path: Some(path.clone()),
                    root_id: None,
                    relative_path: None,
                },
            );
        }
        ctx.render_root(window_id)
            .edit_widget(editor_widget_id, |mut widget| {
                let Some(mut editor) = widget.try_downcast::<EditorWidget>() else {
                    return;
                };
                let changed = match result {
                    ClientUiCommandResult::None => false,
                    ClientUiCommandResult::SelectedFile(path) => editor
                        .widget
                        .request_selected_file_open(path)
                        .is_some_and(|event| editor.widget.apply_connection_event(event)),
                    ClientUiCommandResult::SelectedFolder(path) => editor
                        .widget
                        .request_selected_workspace_root(path)
                        .is_some_and(|event| editor.widget.apply_connection_event(event)),
                    ClientUiCommandResult::ConnectionEvent(event) => {
                        editor.widget.apply_connection_event(event)
                    }
                    ClientUiCommandResult::CopySelection
                    | ClientUiCommandResult::CutSelection
                    | ClientUiCommandResult::PasteClipboard
                    | ClientUiCommandResult::Undo
                    | ClientUiCommandResult::Redo
                    | ClientUiCommandResult::ShowOpenDocuments
                    | ClientUiCommandResult::RequestResync
                    | ClientUiCommandResult::DismissRecovery
                    | ClientUiCommandResult::EditorCommand(_)
                    | ClientUiCommandResult::ShellCommand(_) => false,
                };
                editor.widget.sync_region(&mut editor.ctx);
                editor.widget.sync_panels(&mut editor.ctx);
                editor.widget.sync_overlays(&mut editor.ctx);
                editor.widget.sync_empty_tab(&mut editor.ctx);
                if changed {
                    editor.ctx.request_render();
                    editor.ctx.request_accessibility_update();
                }
            });
    }
}
