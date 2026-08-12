use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::theme::default_property_set;
use masonry::widgets::TextAction;
use masonry_winit::app::{AppDriver, DriverCtx, EventLoop, MasonryUserEvent, NewWindow, WindowId};
use masonry_winit::winit::dpi::LogicalSize;
use masonry_winit::winit::window::Window;
use tokio::sync::mpsc;

use clay::client::{self, ClientConnectionEvent};
use clay::ipc::{IpcEndpoint, default_endpoint, smoke_endpoint};
use clay::masonry_editor::{
    EditorAction, EditorStatus, EditorWidget, PackageButtonPress, PackageDropdownSelect,
    PackageListRowPress, SduiButtonPress, SduiListRowPress,
};
use clay::masonry_pane_document::{CrossPaneDocumentEntry, PaneDocumentView};
use clay::masonry_pane_host::PaneContentHost;
use clay::masonry_shell::ClayShellWidget;
use clay::perf::fixtures::{FixtureKind, FixtureSpec, default_fixture_path, generate_fixture_file};
use clay::perf::metrics::{PERF_PROFILE_FLAG, PerfConfig, install_global_recorder};
use clay::protocol::{ClientId, TabRegistrySnapshot};

mod driver;

#[cfg(any(unix, windows))]
use clay::server::{IpcServer, ServerConfig};
use clay::shell::{PaneId, PersistedWindowState, TransientMenuSession};
use driver::{
    Driver, PendingOpenRequest, RESTORE_CONFIRM_TIMEOUT, TabState,
    advance_pending_close_after_saves, spawn_client_connection_event_bridge, take_pending_open_for,
};

const WINDOW_TITLE: &str = "Clay";
const WINDOW_WIDTH: f64 = 900.0;
const WINDOW_HEIGHT: f64 = 600.0;

impl Driver {
    fn next_dialog_generation(&mut self) -> Option<u64> {
        self.dialog_generation = self.dialog_generation.checked_add(1)?;
        Some(self.dialog_generation)
    }

    fn reserve_file_dialog(&mut self) -> Option<u64> {
        if self.file_dialog_in_flight.is_some() {
            return None;
        }
        let generation = self.next_dialog_generation()?;
        self.file_dialog_in_flight = Some(generation);
        Some(generation)
    }

    fn reserve_folder_dialog(&mut self) -> Option<u64> {
        if self.folder_dialog_in_flight.is_some() {
            return None;
        }
        let generation = self.next_dialog_generation()?;
        self.folder_dialog_in_flight = Some(generation);
        Some(generation)
    }

    fn finish_file_dialog(&mut self, generation: u64) -> bool {
        if self.file_dialog_in_flight != Some(generation) {
            return false;
        }
        self.file_dialog_in_flight = None;
        true
    }

    fn finish_folder_dialog(&mut self, generation: u64) -> bool {
        if self.folder_dialog_in_flight != Some(generation) {
            return false;
        }
        self.folder_dialog_in_flight = None;
        true
    }

    fn clear_native_dialogs(&mut self) {
        self.file_dialog_in_flight = None;
        self.folder_dialog_in_flight = None;
    }

    fn spawn_native_dialog_command(&mut self, command: clay::client::ClientUiCommandRoute) {
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
    fn apply_connection_to_chrome(
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
        }
    }

    /// Show/clear the shared transient menu in the chrome overlay of the tab
    /// that produced it (Phase 22.3: menus are per-tab chrome state).
    fn apply_menu_sync(
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

fn is_linux_portal_dialog_command(command_id: &str) -> bool {
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
                            self.apply_connection_to_chrome(ctx, window_id, chrome_id, event);
                            self.fan_out_runtime_snapshot(
                                ctx, window_id, client_id, chrome_id, snapshot,
                            );
                        }
                        ClientConnectionEvent::Disconnected
                        | ClientConnectionEvent::ConnectionError(_) => {
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
                            // with no state mutation.
                            if let Some(command) =
                                clay::masonry_shell::ShellClientCommand::from_command_id(command_id)
                            {
                                self.apply_shell_client_command(ctx, window_id, command);
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
enum ClientUiCommandResult {
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

fn handle_client_ui_command(command: &clay::client::ClientUiCommandRoute) -> ClientUiCommandResult {
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
enum SelectedPathKind {
    File,
    Folder,
}

fn client_dialog_result_to_command_result(
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
                if changed {
                    editor.ctx.request_render();
                    editor.ctx.request_accessibility_update();
                }
            });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClayCommand {
    Auto {
        endpoint: IpcEndpoint,
    },
    Client {
        endpoint: IpcEndpoint,
    },
    Restart {
        endpoint: IpcEndpoint,
    },
    Server {
        endpoint: IpcEndpoint,
        configuration_root: Option<PathBuf>,
    },
    SmokeGui {
        endpoint: IpcEndpoint,
        configuration_root: Option<PathBuf>,
    },
    PerfFixture {
        kind: FixtureKind,
        size_mib: usize,
        seed: u64,
        output: Option<PathBuf>,
    },
    Help,
    Package {
        subcommand: PackageCliSubcommand,
    },
}

/// Subcommand for `clay package <op> [args...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PackageCliSubcommand {
    /// Install a package by spec (delegates to the configured npm-compatible manager).
    Add {
        package_spec: String,
        /// Allow third-party lifecycle scripts to run during install.
        allow_scripts: bool,
    },
    /// Remove an installed package.
    Remove { package_name: String },
    /// List all installed packages and their enabled status.
    List,
    /// Enable a previously installed package (runs Clay-owned validation).
    Enable { package_name: String },
    /// Disable a currently enabled package.
    Disable { package_name: String },
    /// Inspect metadata for a specific package.
    Inspect { package_name: String },
    /// Approve an installed package for execution (writes a durable exact
    /// approval record after host-side fact assembly).
    Adopt { package_name: String },
    /// Revoke a package's durable approval and disable it if enabled.
    Revoke { package_name: String },
    /// Roll back an active replacement: disable the replacement and restore
    /// the named target package.
    Rollback { target_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(formatter, "{}", self.message)?;
        formatter.write_str(CLI_USAGE)
    }
}

impl Error for CliError {}

const CLI_USAGE: &str = "Usage:\n  clay\n  clay server [endpoint] [--config-fixture <name>]\n  clay client [endpoint]\n  clay restart\n  clay smoke-gui [--config-fixture <name>]\n  clay perf-fixture --kind <kind> --size-mib <n> [--output <path>] [--seed <n>]\n  clay package add <spec> [--allow-scripts]\n  clay package remove <name>\n  clay package list\n  clay package enable <name>\n  clay package disable <name>\n  clay package inspect <name>\n  clay package adopt <name>\n  clay package revoke <name>\n  clay package rollback <name>\n  clay <endpoint>\n\nModes:\n  clay                  Connect to the default local endpoint, start a background server if missing, then open the GUI.\n  clay server           Run a foreground server on the default local endpoint.\n  clay client           Connect to the default local endpoint, or open a local fallback GUI if missing.\n  clay restart          Stop and replace the default background server on Linux, then exit.\n  clay smoke-gui        App-managed GUI smoke mode; starts an isolated child server, opens a client, then cleans up.\n  clay perf-fixture     Generate deterministic large UTF-8 plain-text performance fixtures.\n  clay package         Manage Clay packages (install/enable/disable/list/inspect/adopt/revoke/rollback).\n  clay <endpoint>       Advanced debugging shorthand for 'clay client <endpoint>'.\n\nOptions:\n  --config-fixture <name>  Development smoke fixture under tests/fixtures/configuration/<name>.\n  --allow-scripts          Allow package lifecycle scripts during `clay package add` (dangerous).\n  --profile-perf          Enable internal developer performance metric snapshots for this process.\n\nEnvironment:\n  CLAY_ALLOW_LIFECYCLE_SCRIPTS=1  Same as --allow-scripts (dangerous).\n\nPerf fixture kinds:\n  long-lines, many-short-lines, mixed-unicode, newline-heavy\n";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchDiagnostic {
    message: String,
}

impl LaunchDiagnostic {
    fn server_starting(endpoint: &IpcEndpoint) -> Self {
        Self::new(format!(
            "clay server starting on local IPC endpoint {endpoint}"
        ))
    }

    fn smoke_server_starting(endpoint: &IpcEndpoint) -> Self {
        Self::new(format!(
            "clay smoke-gui starting managed local server at {endpoint}"
        ))
    }

    fn connected(endpoint: &IpcEndpoint) -> Self {
        Self::new(format!("clay client connected to {endpoint}"))
    }

    fn auto_starting_server(endpoint: &IpcEndpoint, error: &client::ClientBootstrapError) -> Self {
        Self::new(format!(
            "no Clay server was ready at {endpoint} ({:?}: {error}); starting a background local server",
            error.kind()
        ))
    }

    fn local_fallback(endpoint: &IpcEndpoint, error: &client::ClientBootstrapError) -> Self {
        Self::new(format!(
            "Clay server unavailable at {endpoint} ({:?}: {error}); opening a local fallback editor",
            error.kind()
        ))
    }

    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for LaunchDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Debug)]
struct LaunchError {
    endpoint: IpcEndpoint,
    failure: LaunchReadinessFailure,
    attempts: usize,
}

impl LaunchError {
    fn readiness(endpoint: IpcEndpoint, attempts: usize, failure: LaunchReadinessFailure) -> Self {
        Self {
            endpoint,
            attempts,
            failure,
        }
    }

    fn server_start_failed(endpoint: IpcEndpoint, error: impl Into<String>) -> Self {
        Self::readiness(
            endpoint,
            0,
            LaunchReadinessFailure::ServerStart(error.into()),
        )
    }
}

#[derive(Debug)]
enum LaunchReadinessFailure {
    ConnectFailed(client::ClientBootstrapError),
    ChildExited(ExitStatus),
    ChildStatus(std::io::Error),
    ServerStart(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.failure {
            LaunchReadinessFailure::ConnectFailed(error) => write!(
                formatter,
                "Clay server at {} did not become ready after {} attempts ({:?}: {error})",
                self.endpoint,
                self.attempts,
                error.kind()
            ),
            LaunchReadinessFailure::ChildExited(status) => write!(
                formatter,
                "managed Clay server for {} exited before readiness after {} attempts with status {status}",
                self.endpoint, self.attempts
            ),
            LaunchReadinessFailure::ChildStatus(error) => write!(
                formatter,
                "failed to inspect managed Clay server for {} after {} attempts: {error}",
                self.endpoint, self.attempts
            ),
            LaunchReadinessFailure::ServerStart(error) => write!(
                formatter,
                "Clay server failed to start on {}: {error}",
                self.endpoint
            ),
        }
    }
}

impl Error for LaunchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.failure {
            LaunchReadinessFailure::ConnectFailed(error) => Some(error),
            LaunchReadinessFailure::ChildStatus(error) => Some(error),
            LaunchReadinessFailure::ChildExited(_) | LaunchReadinessFailure::ServerStart(_) => None,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let (args, profile_perf) = extract_profile_perf_flag(std::env::args_os().skip(1));
    install_global_recorder(PerfConfig::from_env().with_flag(profile_perf));

    match parse_command(args)? {
        ClayCommand::Server {
            endpoint,
            configuration_root,
        } => run_server(endpoint, configuration_root),
        ClayCommand::Client { endpoint } => run_client(endpoint, false),
        ClayCommand::Restart { endpoint } => run_restart(endpoint),
        ClayCommand::Auto { endpoint } => run_client(endpoint, true),
        ClayCommand::SmokeGui {
            endpoint,
            configuration_root,
        } => run_smoke_gui(endpoint, configuration_root),
        ClayCommand::PerfFixture {
            kind,
            size_mib,
            seed,
            output,
        } => run_perf_fixture(kind, size_mib, seed, output),
        ClayCommand::Help => {
            println!("{CLI_USAGE}");
            Ok(())
        }
        ClayCommand::Package { subcommand } => run_package_subcommand(subcommand),
    }
}

fn extract_profile_perf_flag(args: impl Iterator<Item = OsString>) -> (Vec<OsString>, bool) {
    let mut profile_perf = false;
    let mut retained = Vec::new();
    for argument in args {
        if argument == PERF_PROFILE_FLAG {
            profile_perf = true;
        } else {
            retained.push(argument);
        }
    }
    (retained, profile_perf)
}

fn parse_command(args: Vec<OsString>) -> Result<ClayCommand, CliError> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(ClayCommand::Auto {
            endpoint: default_endpoint(),
        });
    };

    match first.to_string_lossy().as_ref() {
        "help" | "--help" | "-h" => Ok(ClayCommand::Help),
        "server" | "--server" => parse_server_subcommand(args),
        "client" | "--client" => parse_endpoint_subcommand("client", args)
            .map(|endpoint| ClayCommand::Client { endpoint }),
        "restart" => {
            if let Some(extra) = args.next() {
                return Err(CliError::new(format!(
                    "unexpected argument for 'restart': {}",
                    extra.to_string_lossy()
                )));
            }
            Ok(ClayCommand::Restart {
                endpoint: default_endpoint(),
            })
        }
        "smoke-gui" | "smoke" | "--smoke-gui" => parse_smoke_gui_subcommand(args),
        "perf-fixture" => parse_perf_fixture_subcommand(args),
        "package" => parse_package_subcommand(args),
        _ => {
            if let Some(extra) = args.next() {
                return Err(CliError::new(format!(
                    "unexpected extra argument after endpoint shorthand: {}",
                    extra.to_string_lossy()
                )));
            }
            Ok(ClayCommand::Client {
                endpoint: IpcEndpoint::from_argument(first),
            })
        }
    }
}

fn parse_perf_fixture_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let mut kind = None;
    let mut size_mib = None;
    let mut seed = 0xC1A4_F14E;
    let mut output = None;
    let mut args = args.peekable();

    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--kind" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --kind for 'perf-fixture'",
                    ));
                };
                let value = value.to_string_lossy();
                kind = Some(FixtureKind::parse(&value).ok_or_else(|| {
                    CliError::new(format!("unknown performance fixture kind '{value}'"))
                })?);
            }
            "--size-mib" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --size-mib for 'perf-fixture'",
                    ));
                };
                size_mib = Some(parse_positive_usize("--size-mib", &value)?);
            }
            "--seed" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --seed for 'perf-fixture'",
                    ));
                };
                seed = parse_u64("--seed", &value)?;
            }
            "--output" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --output for 'perf-fixture'",
                    ));
                };
                output = Some(PathBuf::from(value));
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for 'perf-fixture': {other}"
                )));
            }
        }
    }

    Ok(ClayCommand::PerfFixture {
        kind: kind.ok_or_else(|| CliError::new("missing --kind for 'perf-fixture'"))?,
        size_mib: size_mib.ok_or_else(|| CliError::new("missing --size-mib for 'perf-fixture'"))?,
        seed,
        output,
    })
}

fn parse_positive_usize(option: &str, value: &OsString) -> Result<usize, CliError> {
    let text = value.to_string_lossy();
    let parsed = text
        .parse::<usize>()
        .map_err(|_| CliError::new(format!("invalid numeric value for {option}: {text}")))?;
    if parsed == 0 {
        return Err(CliError::new(format!("{option} must be greater than zero")));
    }
    Ok(parsed)
}

fn parse_u64(option: &str, value: &OsString) -> Result<u64, CliError> {
    let text = value.to_string_lossy();
    text.parse::<u64>()
        .map_err(|_| CliError::new(format!("invalid numeric value for {option}: {text}")))
}

fn parse_endpoint_subcommand(
    mode: &str,
    mut args: impl Iterator<Item = OsString>,
) -> Result<IpcEndpoint, CliError> {
    let endpoint = args
        .next()
        .map(IpcEndpoint::from_argument)
        .unwrap_or_else(default_endpoint);

    if let Some(extra) = args.next() {
        return Err(CliError::new(format!(
            "unexpected extra argument for '{mode}': {}",
            extra.to_string_lossy()
        )));
    }

    Ok(endpoint)
}

fn parse_server_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let (endpoint, configuration_root) = parse_endpoint_and_config_fixture("server", args, true)?;
    Ok(ClayCommand::Server {
        endpoint,
        configuration_root,
    })
}

fn parse_smoke_gui_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let (_endpoint, configuration_root) =
        parse_endpoint_and_config_fixture("smoke-gui", args, false)?;
    Ok(ClayCommand::SmokeGui {
        endpoint: smoke_endpoint("gui"),
        configuration_root,
    })
}

fn parse_endpoint_and_config_fixture(
    mode: &str,
    args: impl Iterator<Item = OsString>,
    allow_endpoint: bool,
) -> Result<(IpcEndpoint, Option<PathBuf>), CliError> {
    let mut endpoint = None;
    let mut configuration_root = None;

    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        if argument == "--config-fixture" {
            let Some(name) = args.next() else {
                return Err(CliError::new(format!(
                    "missing fixture name after --config-fixture for '{mode}'"
                )));
            };
            if configuration_root.is_some() {
                return Err(CliError::new(format!(
                    "duplicate --config-fixture option for '{mode}'"
                )));
            }
            configuration_root = Some(resolve_config_fixture(&name)?);
        } else if allow_endpoint && endpoint.is_none() {
            endpoint = Some(IpcEndpoint::from_argument(argument));
        } else {
            return Err(CliError::new(format!(
                "unexpected extra argument for '{mode}': {}",
                argument.to_string_lossy()
            )));
        }
    }

    Ok((
        endpoint.unwrap_or_else(default_endpoint),
        configuration_root,
    ))
}

fn resolve_config_fixture(name: &OsString) -> Result<PathBuf, CliError> {
    let name = name.to_string_lossy();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(CliError::new(format!(
            "invalid configuration fixture name '{name}'"
        )));
    }

    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join(name.as_ref());
    if !fixture_root.join("init.js").is_file() {
        return Err(CliError::new(format!(
            "configuration fixture '{}' does not contain init.js",
            name
        )));
    }
    Ok(fixture_root)
}

fn parse_package_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let mut args = args.peekable();
    let Some(op) = args.next() else {
        return Err(CliError::new(
            "clay package requires a subcommand: add | remove | list | enable | disable | inspect | adopt | revoke | rollback",
        ));
    };
    match op.to_string_lossy().as_ref() {
        "add" => {
            let mut spec = None;
            let mut allow_scripts = false;
            for arg in args {
                let text = arg.to_string_lossy();
                if text == "--allow-scripts" {
                    allow_scripts = true;
                } else if spec.is_none() {
                    spec = Some(arg);
                } else {
                    return Err(CliError::new(
                        "clay package add takes one package spec and optional --allow-scripts",
                    ));
                }
            }
            let spec = spec.ok_or_else(|| {
                CliError::new("clay package add requires a package spec, e.g. @clay/markdown")
            })?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Add {
                    package_spec: spec.to_string_lossy().into_owned(),
                    allow_scripts,
                },
            })
        }
        "remove" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package remove requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Remove {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "list" => Ok(ClayCommand::Package {
            subcommand: PackageCliSubcommand::List,
        }),
        "enable" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package enable requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Enable {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "disable" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package disable requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Disable {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "inspect" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package inspect requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Inspect {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "adopt" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package adopt requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Adopt {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "revoke" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package revoke requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Revoke {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "rollback" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package rollback requires a target name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Rollback {
                    target_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        unknown => Err(CliError::new(format!(
            "unknown clay package subcommand `{unknown}`; expected: add | remove | list | enable | disable | inspect | adopt | revoke | rollback"
        ))),
    }
}

fn run_package_subcommand(subcommand: PackageCliSubcommand) -> Result<(), Box<dyn Error>> {
    use clay::packages::manager::PnpmBackend;
    use clay::packages::service::PackageService;

    // Default store: ~/.config/clay/packages. The durable approval store
    // under the same root fails closed on corruption/unsafe permissions.
    let store_root = clay::packages::service::default_store_root();
    let mut service = PackageService::open(store_root, Box::new(PnpmBackend::new()))?;

    // A fresh service starts with an empty installed map. Repopulate it from
    // the package-manager store so `list`/`enable`/`disable`/`inspect`/`remove`
    // reflect packages installed by previous `clay package add` invocations.
    // `add` skips this: it installs via the backend (which re-discovers
    // internally) and a missing pnpm binary should fail at `pnpm add`, not at
    // the pre-list step.
    if !matches!(&subcommand, PackageCliSubcommand::Add { .. }) {
        service.refresh_installed()?;
    }

    match subcommand {
        PackageCliSubcommand::Add {
            package_spec,
            allow_scripts,
        } => {
            let allow_scripts = allow_scripts
                || std::env::var_os("CLAY_ALLOW_LIFECYCLE_SCRIPTS")
                    .is_some_and(|value| value == "1" || value == "true");
            println!("Installing {package_spec}…");
            service.install(
                &package_spec,
                clay::packages::manager::PackageInstallOptions {
                    allow_lifecycle_scripts: allow_scripts,
                },
            )?;
            println!("Installed {package_spec}");
        }
        PackageCliSubcommand::Remove { package_name } => {
            println!("Removing {package_name}…");
            service.remove(&package_name)?;
            println!("Removed {package_name}");
        }
        PackageCliSubcommand::List => {
            let packages = service.list();
            if packages.is_empty() {
                println!("No packages installed.");
            } else {
                for pkg in &packages {
                    let status = if pkg.is_enabled {
                        "[enabled]"
                    } else {
                        "[installed]"
                    };
                    println!("  {} {} {} {status}", pkg.name, pkg.version, pkg.api_prefix);
                }
            }
        }
        PackageCliSubcommand::Enable { package_name } => {
            println!("Enabling {package_name}…");
            service.enable(&package_name)?;
            println!("Enabled {package_name}");
        }
        PackageCliSubcommand::Disable { package_name } => {
            println!("Disabling {package_name}…");
            service.disable(&package_name)?;
            println!("Disabled {package_name}");
        }
        PackageCliSubcommand::Inspect { package_name } => match service.inspect(&package_name) {
            Some(inspection) => {
                println!("Package:     {}", inspection.name);
                println!("Version:     {}", inspection.version);
                println!("API prefix:  {}", inspection.api_prefix);
                println!(
                    "Status:      {}",
                    if inspection.is_enabled {
                        "enabled"
                    } else {
                        "installed"
                    }
                );
                println!("Modes:       {:?}", inspection.modes);
                println!("Permissions: {:?}", inspection.permissions);
                println!("Commands:    {}", inspection.command_count);
                println!("Config keys: {}", inspection.configuration_count);
                if let Some(docs) = &inspection.docs_path {
                    println!("Docs:        {docs}");
                }
                let adoption = match service.adoption_state(&package_name) {
                    Some(clay::packages::service::AdoptionState::Pending) => {
                        "pending adoption (cannot execute)"
                    }
                    Some(clay::packages::service::AdoptionState::Approved) => "approved",
                    Some(clay::packages::service::AdoptionState::Stale) => {
                        "stale approval (re-adopt required)"
                    }
                    Some(clay::packages::service::AdoptionState::Revoked) => "approval revoked",
                    None => "unknown",
                };
                println!("Adoption:    {adoption}");
            }
            None => eprintln!("Package `{package_name}` is not installed."),
        },
        PackageCliSubcommand::Adopt { package_name } => {
            if service.inspect(&package_name).is_none() {
                eprintln!("Package `{package_name}` is not installed.");
                return Ok(());
            }
            let approval = service.approve_package(&package_name, "cli")?;
            println!("Adopted {} {}", approval.package, approval.resolved_version);
            println!("  capabilities: {}", approval.capabilities.join(", "));
            if !approval.processes.is_empty() {
                println!("  processes:    {}", approval.processes.join(", "));
            }
            for relation in &approval.relations {
                println!(
                    "  relation:     {} {} {}@{}",
                    relation.operation,
                    relation.package,
                    relation.extension_point,
                    relation.version
                );
            }
            for replacement in &approval.replacements {
                println!("  replaces:     {}", replacement.target);
            }
        }
        PackageCliSubcommand::Revoke { package_name } => {
            if service.inspect(&package_name).is_none() {
                eprintln!("Package `{package_name}` is not installed.");
                return Ok(());
            }
            let revoked = service.revoke_package_approval(&package_name)?;
            if service.disable(&package_name).is_ok() {
                println!("Disabled {package_name}");
            }
            if revoked {
                println!("Revoked approval for {package_name}");
            } else {
                println!("No approval recorded for {package_name}");
            }
        }
        PackageCliSubcommand::Rollback { target_name } => {
            let replacement = service.rollback_replacement(&target_name)?;
            println!("Disabled replacement {replacement}; restored {target_name}");
        }
    }
    Ok(())
}

#[cfg(any(unix, windows))]
fn run_server(
    endpoint: IpcEndpoint,
    configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    eprintln!("{}", LaunchDiagnostic::server_starting(&endpoint));
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = configuration_root;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async { IpcServer::try_new(config)?.run().await })
        .map_err(|error| LaunchError::server_start_failed(endpoint, error.to_string()))?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn run_server(
    endpoint: IpcEndpoint,
    _configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    Err(format!("Clay server IPC is unsupported on this platform: {endpoint}").into())
}

fn run_perf_fixture(
    kind: FixtureKind,
    size_mib: usize,
    seed: u64,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let size_bytes = size_mib
        .checked_mul(1024 * 1024)
        .ok_or("--size-mib is too large")?;
    let spec = FixtureSpec {
        kind,
        size_bytes,
        seed,
    };
    let output = output.unwrap_or_else(|| default_fixture_path(kind, size_mib));
    let output = generate_fixture_file(&spec, &output)?;
    println!(
        "generated {} MiB {} fixture at {}",
        size_mib,
        kind.as_str(),
        output.display()
    );
    Ok(())
}

fn run_client(endpoint: IpcEndpoint, start_server_if_missing: bool) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    // Select persisted tab 0 before binding the bootstrap connection so the
    // server can scope its deferred InitialDocument to that workspace.
    let restore_candidate = clay::shell::load_window_state();
    let bootstrap_workspace_root = restore_candidate
        .as_ref()
        .and_then(|state| state.tabs.first())
        .filter(|tab| Path::new(&tab.workspace_root).is_dir())
        .map(|tab| tab.workspace_root.clone())
        .unwrap_or_default();

    let client_session = match runtime.block_on(client::connect_with_workspace_root(
        &endpoint,
        bootstrap_workspace_root.clone(),
    )) {
        Ok(session) => {
            eprintln!("{}", LaunchDiagnostic::connected(&endpoint));
            Some(session)
        }
        Err(connect_error) if start_server_if_missing => {
            eprintln!(
                "{}",
                LaunchDiagnostic::auto_starting_server(&endpoint, &connect_error)
            );
            start_background_server(&endpoint)?;
            Some(runtime.block_on(connect_with_workspace_root_retry(
                &endpoint,
                &bootstrap_workspace_root,
            ))?)
        }
        Err(connect_error) => {
            eprintln!(
                "{}",
                LaunchDiagnostic::local_fallback(&endpoint, &connect_error)
            );
            None
        }
    };

    let connected = client_session.is_some();

    let (client_id, editor_widget, events, initial_workspace_root) =
        if let Some(session) = client_session {
            let initial_workspace_root = session.initial_state.workspace_root.clone();
            let (client_id, editor_widget, events) = editor_widget_from_session(session);
            (client_id, editor_widget, events, initial_workspace_root)
        } else {
            (
                // Phase 22.3: the local-fallback tab has no connection; key 0
                // is never assigned by the server (ClientIds start at 1).
                0,
                EditorWidget::default().with_status(EditorStatus::local_fallback()),
                None,
                String::new(),
            )
        };

    // Phase 22.5: whole-window restore — only with a live server connection
    // (the local fallback has no registry to rebuild); missing/corrupt/legacy
    // state keeps today's bootstrap exactly.
    let restore = if connected { restore_candidate } else { None };

    run_editor(
        endpoint,
        client_id,
        editor_widget,
        events,
        initial_workspace_root,
        &runtime,
        restore,
    )
}

#[cfg(target_os = "linux")]
fn run_restart(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let stopped = stop_default_linux_servers(&endpoint)?;
    if stopped > 0 {
        eprintln!("stopped {stopped} Clay server process(es)");
    }

    start_background_server(&endpoint)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    drop(runtime.block_on(connect_with_retry(&endpoint))?);
    eprintln!("Clay server restarted at {endpoint}");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn run_restart(_endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    Err(CliError::new("'restart' is currently supported only on Linux").into())
}

#[cfg(target_os = "linux")]
fn stop_default_linux_servers(endpoint: &IpcEndpoint) -> Result<usize, std::io::Error> {
    use std::os::unix::ffi::OsStrExt;
    use std::time::Instant;

    const STOP_TIMEOUT: Duration = Duration::from_secs(2);

    let executable = std::env::current_exe()?;
    let endpoint_arg = endpoint.as_child_arg();
    let mut pids = Vec::new();

    for entry in std::fs::read_dir("/proc")? {
        let Ok(entry) = entry else { continue };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if linux_process_is_default_server(pid, &executable, endpoint_arg.as_bytes()) {
            pids.push(pid);
        }
    }

    for &pid in &pids {
        signal_linux_process(pid, libc::SIGTERM)?;
    }

    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline
        && pids
            .iter()
            .any(|&pid| linux_process_uses_executable(pid, &executable))
    {
        std::thread::sleep(Duration::from_millis(25));
    }

    for &pid in &pids {
        if linux_process_uses_executable(pid, &executable) {
            signal_linux_process(pid, libc::SIGKILL)?;
        }
    }

    Ok(pids.len())
}

#[cfg(target_os = "linux")]
fn linux_process_is_default_server(pid: u32, executable: &Path, endpoint: &[u8]) -> bool {
    if !linux_process_uses_executable(pid, executable) {
        return false;
    }
    let Ok(command_line) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    linux_command_line_is_default_server(&command_line, endpoint)
}

#[cfg(target_os = "linux")]
fn linux_process_uses_executable(pid: u32, executable: &Path) -> bool {
    let Ok(process_executable) = std::fs::read_link(format!("/proc/{pid}/exe")) else {
        return false;
    };
    process_executable == executable
        || process_executable
            .to_string_lossy()
            .strip_suffix(" (deleted)")
            .is_some_and(|path| Path::new(path) == executable)
}

#[cfg(target_os = "linux")]
fn linux_command_line_is_default_server(command_line: &[u8], endpoint: &[u8]) -> bool {
    let mut args = command_line.split(|byte| *byte == 0);
    let _executable = args.next();
    if !matches!(args.next(), Some(b"server") | Some(b"--server")) {
        return false;
    }
    match args.next() {
        None | Some(b"") | Some(b"--config-fixture") => true,
        Some(argument) => argument == endpoint,
    }
}

#[cfg(target_os = "linux")]
fn signal_linux_process(pid: u32, signal: libc::c_int) -> Result<(), std::io::Error> {
    // SAFETY: kill receives a PID discovered under /proc and a fixed signal constant.
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

fn run_smoke_gui(
    endpoint: IpcEndpoint,
    configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let executable = std::env::current_exe()?.into_os_string();
    let mut server = ManagedServer::spawn(executable, &endpoint, configuration_root.as_deref())?;

    eprintln!("{}", LaunchDiagnostic::smoke_server_starting(&endpoint));
    let session = runtime.block_on(connect_with_retry_while(&endpoint, || server.try_wait()))?;
    eprintln!("{}", LaunchDiagnostic::connected(&endpoint));
    let (client_id, editor_widget, events) = editor_widget_from_session(session);
    let result = run_editor(
        endpoint,
        client_id,
        editor_widget,
        events,
        String::new(),
        &runtime,
        None,
    );
    server.shutdown();
    result
}

fn start_background_server(endpoint: &IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?.into_os_string();
    background_server_command(executable, endpoint).spawn()?;
    Ok(())
}

struct ManagedServer {
    child: Option<Child>,
    endpoint: IpcEndpoint,
}

impl ManagedServer {
    fn spawn(
        executable: OsString,
        endpoint: &IpcEndpoint,
        configuration_root: Option<&Path>,
    ) -> Result<Self, Box<dyn Error>> {
        let child = managed_server_command(executable, endpoint, configuration_root).spawn()?;
        Ok(Self {
            child: Some(child),
            endpoint: endpoint.clone(),
        })
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, std::io::Error> {
        match self.child.as_mut() {
            Some(child) => child.try_wait(),
            None => Ok(None),
        }
    }

    fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            match child.try_wait() {
                Ok(Some(_status)) => {}
                Ok(None) => {
                    if let Err(error) = child.kill() {
                        eprintln!("failed to stop managed Clay server: {error}");
                    }
                    if let Err(error) = child.wait() {
                        eprintln!("failed to wait for managed Clay server shutdown: {error}");
                    }
                }
                Err(error) => eprintln!("failed to inspect managed Clay server: {error}"),
            }
        }
        cleanup_managed_endpoint(&self.endpoint);
    }
}

fn cleanup_managed_endpoint(endpoint: &IpcEndpoint) {
    #[cfg(unix)]
    if let Err(error) = std::fs::remove_file(endpoint.as_unix_socket_path())
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("failed to remove managed Clay socket {endpoint}: {error}");
    }

    #[cfg(not(unix))]
    let _ = endpoint;
}

impl Drop for ManagedServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn background_server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = server_command(executable, endpoint);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn managed_server_command(
    executable: OsString,
    endpoint: &IpcEndpoint,
    configuration_root: Option<&Path>,
) -> Command {
    let mut command = server_command(executable, endpoint);
    if let Some(configuration_root) = configuration_root {
        command.arg("--config-fixture").arg(
            configuration_root
                .file_name()
                .expect("fixture root has a name"),
        );
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn server_command(executable: OsString, endpoint: &IpcEndpoint) -> Command {
    let mut command = Command::new(executable);
    command.arg("server").arg(endpoint.as_child_arg());
    command
}

fn editor_widget_from_session(
    session: client::ClientSession,
) -> (
    ClientId,
    EditorWidget,
    Option<mpsc::Receiver<ClientConnectionEvent>>,
) {
    let client::ClientSession {
        initial_state,
        edit_queue,
        events,
    } = session;
    let client_id = initial_state.client_id;
    (
        client_id,
        EditorWidget::with_initial_state(initial_state).with_edit_queue(edit_queue),
        Some(events),
    )
}

async fn connect_with_retry(endpoint: &IpcEndpoint) -> Result<client::ClientSession, LaunchError> {
    connect_with_retry_while(endpoint, || Ok(None)).await
}

async fn connect_with_workspace_root_retry(
    endpoint: &IpcEndpoint,
    workspace_root: &str,
) -> Result<client::ClientSession, LaunchError> {
    let mut last_error = None;
    for _ in 1..=50 {
        match client::connect_with_workspace_root(endpoint, workspace_root.to_string()).await {
            Ok(session) => return Ok(session),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    Err(LaunchError::readiness(
        endpoint.clone(),
        50,
        LaunchReadinessFailure::ConnectFailed(
            last_error.expect("connect retry loop always records the last error"),
        ),
    ))
}

async fn connect_with_retry_while(
    endpoint: &IpcEndpoint,
    mut check_child_exit: impl FnMut() -> Result<Option<ExitStatus>, std::io::Error>,
) -> Result<client::ClientSession, LaunchError> {
    let mut last_error = None;
    for attempt in 1..=50 {
        if let Some(status) = check_child_exit().map_err(|error| {
            LaunchError::readiness(
                endpoint.clone(),
                attempt,
                LaunchReadinessFailure::ChildStatus(error),
            )
        })? {
            return Err(LaunchError::readiness(
                endpoint.clone(),
                attempt,
                LaunchReadinessFailure::ChildExited(status),
            ));
        }

        match client::connect(endpoint).await {
            Ok(session) => return Ok(session),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }

    Err(LaunchError::readiness(
        endpoint.clone(),
        50,
        LaunchReadinessFailure::ConnectFailed(
            last_error.expect("connect retry loop always records an error"),
        ),
    ))
}

fn run_editor(
    endpoint: IpcEndpoint,
    client_id: ClientId,
    editor_widget: EditorWidget,
    events: Option<mpsc::Receiver<ClientConnectionEvent>>,
    initial_workspace_root: String,
    runtime: &tokio::runtime::Runtime,
    restore: Option<PersistedWindowState>,
) -> Result<(), Box<dyn Error>> {
    // Phase 22.2: a master queue clone for mounting pane document views.
    // Phase 22.3: the initial tab's queue lives in its `TabState`.
    let edit_queue = editor_widget.edit_queue_shared();
    // Phase 22.5: whole-window restore plan. Persisted tab 0 rides the
    // bootstrap connection (already connected in `run_client`); tabs 1..
    // mount sequentially inside the event loop, gated on registry
    // confirmation. A missing tab-0 workspace root falls back to today's
    // bootstrap (server root) and the rest of the window restores around it.
    let restore_active = restore.as_ref().and_then(|state| state.active_tab);
    let (first_valid, restoring) = {
        let restore_first = restore.as_ref().and_then(|state| state.tabs.first());
        (
            restore_first.is_some_and(|tab| Path::new(&tab.workspace_root).is_dir()),
            restore_first.is_some(),
        )
    };
    let mut restore_tabs = restore.map(|state| state.tabs).unwrap_or_default();
    let mut restore_queue = VecDeque::new();
    let mut restore_mounted = Vec::new();
    let mut restore_diagnostics = Vec::new();
    if !restore_tabs.is_empty() {
        let first = restore_tabs.remove(0);
        if first_valid {
            restore_mounted.push((client_id, 0, first));
        } else {
            restore_diagnostics.push(format!(
                "Restore skipped {}: workspace root is missing or not a directory",
                first.workspace_root
            ));
        }
        restore_queue = restore_tabs
            .into_iter()
            .enumerate()
            .map(|(index, tab)| (index + 1, tab))
            .collect();
    }
    let bootstrap_root = restore_mounted
        .first()
        .map(|(_, _, tab)| tab.workspace_root.clone())
        .unwrap_or_else(|| initial_workspace_root.clone());
    // The bootstrap connection binds its tab during the handshake; the
    // deferred initial document already belongs to `bootstrap_root`.
    let shell_widget = if first_valid {
        ClayShellWidget::restored_single_editor(
            client_id,
            editor_widget,
            &restore_mounted
                .first()
                .expect("first_valid mounts persisted tab 0")
                .2,
        )
    } else {
        ClayShellWidget::single_editor(client_id, editor_widget)
    };
    let editor_widget_id = shell_widget.editor_widget_id();
    let root_widget = NewWidget::new(shell_widget);
    let shell_widget_id = root_widget.id();
    let window_id = WindowId::next();
    let window_attributes = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    if let Some(events) = events {
        spawn_client_connection_event_bridge(
            runtime.handle(),
            events,
            proxy.clone(),
            window_id,
            editor_widget_id,
        );
    }

    masonry_winit::app::run_with(
        event_loop,
        vec![NewWindow::new_with_id(
            window_id,
            window_attributes,
            root_widget.erased(),
        )],
        Driver {
            editor_widget_id,
            shell_widget_id,
            window_id,
            tabs: BTreeMap::from([(
                client_id,
                TabState {
                    edit_queue,
                    pending_opens: BTreeMap::new(),
                    tab_id: None,
                    workspace_root: bootstrap_root.clone(),
                },
            )]),
            active_tab: client_id,
            registry: TabRegistrySnapshot {
                tabs: Vec::new(),
                active: None,
                revision: 0,
            },
            registry_revision: None,
            runtime: runtime.handle().clone(),
            endpoint,
            reconnect_cancel: BTreeMap::new(),
            proxy: Some(proxy),
            dialog_generation: 0,
            file_dialog_in_flight: None,
            folder_dialog_in_flight: None,
            pending_close_after_saves: None,
            tab_menu_session_id: 0,
            restore_queue,
            restore_pending: None,
            restore_mounted,
            restore_gate: restoring.then(|| (client_id, Instant::now() + RESTORE_CONFIRM_TIMEOUT)),
            restore_active,
            restore_diagnostics,
        },
        default_property_set(),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

    #[cfg(not(windows))]
    use super::handle_client_ui_command;
    #[cfg(target_os = "linux")]
    use super::linux_command_line_is_default_server;
    use super::{
        ClayCommand, ClientUiCommandResult, FixtureKind, LaunchDiagnostic, LaunchReadinessFailure,
        SelectedPathKind, background_server_command, client_dialog_result_to_command_result,
        connect_with_retry, connect_with_retry_while, extract_profile_perf_flag,
        is_linux_portal_dialog_command, managed_server_command, parse_command,
    };
    use crate::driver::tests::test_driver_with_tabs;
    use clay::client::{ClientBootstrapError, ClientConnectionEvent};
    use clay::editor::{EditorSurface, is_printable_text};
    use clay::ipc::default_endpoint;
    use clay::protocol::codec::CodecError;

    #[test]
    fn parses_server_subcommand() {
        assert!(matches!(
            parse_command(vec!["server".into()]).expect("server parses"),
            ClayCommand::Server { .. }
        ));
    }

    #[test]
    fn parses_client_subcommand() {
        assert!(matches!(
            parse_command(vec!["client".into()]).expect("client parses"),
            ClayCommand::Client { .. }
        ));
    }

    #[test]
    fn parses_restart_subcommand() {
        assert!(matches!(
            parse_command(vec!["restart".into()]).expect("restart parses"),
            ClayCommand::Restart { .. }
        ));
        assert!(parse_command(vec!["restart".into(), "extra".into()]).is_err());
    }

    #[test]
    fn parses_no_args_as_auto() {
        assert!(matches!(
            parse_command(vec![]).expect("bare clay parses"),
            ClayCommand::Auto { .. }
        ));
    }

    #[test]
    fn parses_smoke_gui_subcommand() {
        assert!(matches!(
            parse_command(vec!["smoke-gui".into()]).expect("smoke-gui parses"),
            ClayCommand::SmokeGui { .. }
        ));
    }

    #[test]
    fn parses_profile_perf_as_global_developer_flag() {
        let (args, enabled) = extract_profile_perf_flag(
            vec!["smoke-gui".into(), "--profile-perf".into()].into_iter(),
        );

        assert!(enabled);
        assert_eq!(args, vec![OsString::from("smoke-gui")]);
        assert!(matches!(
            parse_command(args).expect("global profiling flag is stripped before parsing"),
            ClayCommand::SmokeGui { .. }
        ));
    }

    #[test]
    fn parses_default_launch_modes() {
        assert!(matches!(
            parse_command(vec![]).expect("bare clay parses"),
            ClayCommand::Auto { .. }
        ));
        assert!(matches!(
            parse_command(vec!["server".into()]).expect("server parses"),
            ClayCommand::Server { .. }
        ));
        assert!(matches!(
            parse_command(vec!["client".into()]).expect("client parses"),
            ClayCommand::Client { .. }
        ));
    }

    #[test]
    fn launch_modes_do_not_require_manual_endpoint() {
        for args in [
            vec![],
            vec!["server".into()],
            vec!["client".into()],
            vec!["restart".into()],
            vec!["smoke-gui".into()],
        ] {
            let command = parse_command(args).expect("mode parses with default endpoint");
            match command {
                ClayCommand::Auto { endpoint }
                | ClayCommand::Client { endpoint }
                | ClayCommand::Restart { endpoint }
                | ClayCommand::Server { endpoint, .. }
                | ClayCommand::SmokeGui { endpoint, .. } => {
                    assert!(!endpoint.to_string().is_empty())
                }
                ClayCommand::PerfFixture { .. } => {
                    panic!("perf fixture should not be selected by launch modes")
                }
                ClayCommand::Help => panic!("help should not be selected by launch modes"),
                ClayCommand::Package { .. } => {
                    panic!("package subcommand should not be selected by launch modes")
                }
            }
        }
    }

    #[test]
    fn default_server_and_clients_use_same_platform_endpoint() {
        let expected = default_endpoint();

        for args in [
            vec![],
            vec!["server".into()],
            vec!["client".into()],
            vec!["restart".into()],
        ] {
            let command = parse_command(args).expect("default launch mode parses");
            let endpoint = match command {
                ClayCommand::Auto { endpoint }
                | ClayCommand::Client { endpoint }
                | ClayCommand::Restart { endpoint }
                | ClayCommand::Server { endpoint, .. } => endpoint,
                ClayCommand::SmokeGui { .. } => {
                    panic!("default smoke endpoint must remain isolated")
                }
                ClayCommand::PerfFixture { .. } => {
                    panic!("perf fixture should not be selected by default launch modes")
                }
                ClayCommand::Help => panic!("help should not be selected by default launch modes"),
                ClayCommand::Package { .. } => {
                    panic!("package subcommand should not be selected by default launch modes")
                }
            };
            assert_eq!(endpoint, expected);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn restart_matches_only_default_server_command_lines() {
        let endpoint = b"/run/user/1000/clay.sock";

        assert!(linux_command_line_is_default_server(
            b"/tmp/clay\0server\0/run/user/1000/clay.sock\0",
            endpoint
        ));
        assert!(linux_command_line_is_default_server(
            b"/tmp/clay\0server\0",
            endpoint
        ));
        assert!(!linux_command_line_is_default_server(
            b"/tmp/clay\0server\0/tmp/smoke.sock\0",
            endpoint
        ));
        assert!(!linux_command_line_is_default_server(
            b"/tmp/clay\0client\0/run/user/1000/clay.sock\0",
            endpoint
        ));
    }

    #[test]
    fn parses_perf_fixture_subcommand() {
        match parse_command(vec![
            "perf-fixture".into(),
            "--kind".into(),
            "mixed-unicode".into(),
            "--size-mib".into(),
            "16".into(),
            "--output".into(),
            "target/perf-fixtures/mixed-16m.txt".into(),
            "--seed".into(),
            "42".into(),
        ])
        .expect("perf fixture parses")
        {
            ClayCommand::PerfFixture {
                kind,
                size_mib,
                seed,
                output,
            } => {
                assert_eq!(kind, FixtureKind::MixedUnicode);
                assert_eq!(size_mib, 16);
                assert_eq!(seed, 42);
                assert_eq!(
                    output.unwrap(),
                    PathBuf::from("target/perf-fixtures/mixed-16m.txt")
                );
            }
            command => panic!("expected perf fixture command, got {command:?}"),
        }
    }

    #[test]
    fn cli_parses_platform_endpoint() {
        let endpoint = "clay-test-endpoint";

        match parse_command(vec!["server".into(), endpoint.into()]).expect("server endpoint parses")
        {
            ClayCommand::Server {
                endpoint: parsed, ..
            } => {
                assert_eq!(parsed.as_child_arg(), OsString::from(endpoint));
            }
            command => panic!("expected server command, got {command:?}"),
        }

        match parse_command(vec!["client".into(), endpoint.into()]).expect("client endpoint parses")
        {
            ClayCommand::Client { endpoint: parsed } => {
                assert_eq!(parsed.as_child_arg(), OsString::from(endpoint));
            }
            command => panic!("expected client command, got {command:?}"),
        }
    }

    #[test]
    fn rejects_extra_cli_arguments() {
        let error = parse_command(vec!["server".into(), "one".into(), "two".into()])
            .expect_err("extra arguments should fail");
        assert!(error.to_string().contains("unexpected extra argument"));

        let smoke_error = parse_command(vec!["smoke-gui".into(), "manual-endpoint".into()])
            .expect_err("smoke-gui owns endpoint selection");
        assert!(
            smoke_error
                .to_string()
                .contains("unexpected extra argument")
        );
    }

    #[test]
    fn auto_start_uses_current_exe_without_shell() {
        let executable = OsString::from("clay-test-executable");
        let endpoint = clay::ipc::IpcEndpoint::from_argument("clay-test-endpoint");
        let endpoint_arg = endpoint.as_child_arg();
        let command = background_server_command(executable.clone(), &endpoint);

        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_owned())
                .collect::<Vec<_>>(),
            vec![OsString::from("server"), endpoint_arg]
        );
    }

    #[test]
    fn managed_server_command_uses_current_exe_without_shell() {
        let executable = OsString::from("clay-test-executable");
        let endpoint = clay::ipc::smoke_endpoint("gui");
        let endpoint_arg = endpoint.as_child_arg();
        let command = managed_server_command(executable.clone(), &endpoint, None);

        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_owned())
                .collect::<Vec<_>>(),
            vec![OsString::from("server"), endpoint_arg]
        );
    }

    #[test]
    fn smoke_launch_evaluates_runtime_config_fixture() {
        let command = parse_command(vec![
            "smoke-gui".into(),
            "--config-fixture".into(),
            "runtime-sdui".into(),
        ])
        .expect("runtime SDUI smoke fixture parses");

        match command {
            ClayCommand::SmokeGui {
                configuration_root: Some(root),
                ..
            } => {
                assert!(root.ends_with("runtime-sdui"));
                assert!(root.join("init.js").is_file());
            }
            command => panic!("expected smoke GUI fixture command, got {command:?}"),
        }
    }

    #[test]
    fn managed_server_command_forwards_config_fixture_without_shell() {
        let executable = OsString::from("clay-test-executable");
        let endpoint = clay::ipc::smoke_endpoint("gui");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("runtime-sdui");
        let endpoint_arg = endpoint.as_child_arg();
        let command = managed_server_command(executable.clone(), &endpoint, Some(&fixture));

        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_owned())
                .collect::<Vec<_>>(),
            vec![
                OsString::from("server"),
                endpoint_arg,
                OsString::from("--config-fixture"),
                OsString::from("runtime-sdui"),
            ]
        );
    }

    #[tokio::test]
    async fn connect_retry_reports_last_error() {
        let endpoint = clay::ipc::smoke_endpoint("missing-server");
        let error = connect_with_retry(&endpoint)
            .await
            .expect_err("missing server should exhaust readiness retry");

        assert_eq!(error.attempts, 50);
        assert!(matches!(
            error.failure,
            LaunchReadinessFailure::ConnectFailed(_)
        ));
        assert!(error.to_string().contains("did not become ready"));
    }

    #[test]
    fn client_mode_falls_back_with_status_when_server_missing() {
        let endpoint = clay::ipc::smoke_endpoint("fallback-message");
        let error = ClientBootstrapError::Codec(CodecError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing endpoint",
        )));
        let diagnostic = LaunchDiagnostic::local_fallback(&endpoint, &error).to_string();

        assert!(diagnostic.contains("local fallback editor"));
        assert!(diagnostic.contains("TransportUnavailable"));
        assert!(diagnostic.contains(&endpoint.to_string()));
    }

    #[test]
    fn file_dialog_cancellation_is_a_no_op() {
        let result = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Cancelled,
            SelectedPathKind::File,
        );

        assert!(matches!(result, ClientUiCommandResult::None));
    }

    #[test]
    fn file_dialog_result_conversion_reports_selected_and_sanitized_failures() {
        let selected_path = PathBuf::from(r"C:\Users\tester\note.md");
        let selected = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Selected(selected_path.clone()),
            SelectedPathKind::File,
        );
        assert!(
            matches!(selected, ClientUiCommandResult::SelectedFile(path) if path == selected_path)
        );
        let selected_folder = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Selected(selected_path.clone()),
            SelectedPathKind::Folder,
        );
        assert!(
            matches!(selected_folder, ClientUiCommandResult::SelectedFolder(path) if path == selected_path)
        );

        let unsupported = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Unsupported {
                message: "Windows only".to_string(),
            },
            SelectedPathKind::File,
        );
        assert!(matches!(
            unsupported,
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
                if diagnostic.code == "client.file_dialog.unsupported"
                    && diagnostic.message == "Windows only"
        ));

        let failed = client_dialog_result_to_command_result(
            clay::client::FileDialogResult::Failed {
                message: "dialog failed".to_string(),
            },
            SelectedPathKind::File,
        );
        assert!(matches!(
            failed,
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
                if diagnostic.code == "client.file_dialog.failed"
                    && diagnostic.message == "dialog failed"
        ));
    }

    #[test]
    fn client_copy_selection_command_routes_to_editor_widget() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientCopySelection".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(result, ClientUiCommandResult::CopySelection));
    }

    #[test]
    fn client_cut_selection_command_routes_to_editor_widget() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientCutSelection".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(result, ClientUiCommandResult::CutSelection));
    }

    #[test]
    fn client_paste_clipboard_command_routes_to_editor_widget() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientPasteClipboard".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(result, ClientUiCommandResult::PasteClipboard));
    }

    #[test]
    fn client_undo_and_redo_commands_route_to_editor_widget() {
        let undo = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientUndo".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(undo, ClientUiCommandResult::Undo));

        let redo = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientRedo".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(redo, ClientUiCommandResult::Redo));
        let show = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientShowOpenDocuments".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(show, ClientUiCommandResult::ShowOpenDocuments));
        let resync = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientRequestResync".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(resync, ClientUiCommandResult::RequestResync));
        let dismiss = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "editor.clientDismissRecovery".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });
        assert!(matches!(dismiss, ClientUiCommandResult::DismissRecovery));
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    #[test]
    fn unsupported_platform_client_open_file_dialog_command_reports_status_diagnostic() {
        let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
            command_id: "documents.clientOpenFileDialog".to_string(),
            routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
        });

        assert!(matches!(
            result,
            ClientUiCommandResult::ConnectionEvent(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
                if diagnostic.code == "client.file_dialog.unsupported"
                    && diagnostic.message.contains("not supported on this platform")
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_native_dialog_commands_use_non_blocking_driver_path() {
        assert!(is_linux_portal_dialog_command(
            "documents.clientOpenFileDialog"
        ));
        assert!(is_linux_portal_dialog_command(
            "workspace.clientOpenFolderDialog"
        ));
        assert!(!is_linux_portal_dialog_command(
            "editor.clientCopySelection"
        ));
    }

    #[test]
    fn native_dialog_generations_limit_duplicates_and_reject_stale_results() {
        let mut driver = test_driver_with_tabs(BTreeMap::new());

        let file_generation = driver.reserve_file_dialog().expect("first file dialog");
        let folder_generation = driver.reserve_folder_dialog().expect("first folder dialog");
        assert_eq!(driver.reserve_file_dialog(), None);
        assert_eq!(driver.reserve_folder_dialog(), None);

        assert!(driver.finish_file_dialog(file_generation));
        let next_file_generation = driver.reserve_file_dialog().expect("next file dialog");
        assert_ne!(next_file_generation, file_generation);
        assert!(!driver.finish_file_dialog(file_generation));
        assert_eq!(driver.file_dialog_in_flight, Some(next_file_generation));
        assert!(driver.finish_file_dialog(next_file_generation));

        driver.clear_native_dialogs();
        assert_eq!(driver.file_dialog_in_flight, None);
        assert_eq!(driver.folder_dialog_in_flight, None);
        assert!(!driver.finish_folder_dialog(folder_generation));
    }

    #[tokio::test]
    async fn smoke_mode_fails_if_child_server_exits_before_ready() {
        let endpoint = clay::ipc::smoke_endpoint("early-exit");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn helper process");

        let error = connect_with_retry_while(&endpoint, || child.try_wait())
            .await
            .expect_err("exited child should fail smoke readiness");
        let _ = child.wait();

        assert!(matches!(
            error.failure,
            LaunchReadinessFailure::ChildExited(_)
        ));
        assert!(error.to_string().contains("exited before readiness"));
    }

    #[test]
    fn editor_appends_input() {
        let mut editor = EditorSurface::default();

        editor.insert_text("Hello");
        editor.insert_text(", Clay");

        assert_eq!(editor.visible_text(), "Hello, Clay");
    }

    #[test]
    fn editor_backspace_removes_last_scalar() {
        let mut editor = EditorSurface::default();
        editor.insert_text("aé🦀");

        editor.backspace();
        assert_eq!(editor.visible_text(), "aé");

        editor.backspace();
        assert_eq!(editor.visible_text(), "a");

        editor.backspace();
        assert_eq!(editor.visible_text(), "");

        editor.backspace();
        assert_eq!(editor.visible_text(), "");
    }

    #[test]
    fn printable_text_filter_accepts_plain_text_and_rejects_controls() {
        assert!(is_printable_text("abc é 🦀"));
        assert!(!is_printable_text(""));
        assert!(!is_printable_text("\r"));
        assert!(!is_printable_text("\n"));
        assert!(!is_printable_text("a\n"));
    }

    #[test]
    fn tab_command_ids_route_to_shell_tab_variants() {
        for (id, expected) in [
            (
                "shell.clientTabNext",
                clay::masonry_shell::ShellClientCommand::TabNext,
            ),
            (
                "shell.clientTabPrev",
                clay::masonry_shell::ShellClientCommand::TabPrev,
            ),
            (
                "shell.clientTabNew",
                clay::masonry_shell::ShellClientCommand::TabNew,
            ),
            (
                "shell.clientTabClose",
                clay::masonry_shell::ShellClientCommand::TabClose,
            ),
            (
                "shell.clientTabMoveLeft",
                clay::masonry_shell::ShellClientCommand::TabMoveLeft,
            ),
            (
                "shell.clientTabMoveRight",
                clay::masonry_shell::ShellClientCommand::TabMoveRight,
            ),
            (
                "shell.clientTabActivate.3",
                clay::masonry_shell::ShellClientCommand::TabActivate(3),
            ),
            (
                "shell.clientTabMoveTo.9",
                clay::masonry_shell::ShellClientCommand::TabMoveTo(9),
            ),
        ] {
            let result = handle_client_ui_command(&clay::client::ClientUiCommandRoute {
                command_id: id.to_string(),
                routing_policy: clay::protocol::RoutingPolicy::ClientUiCommand,
            });
            assert!(
                matches!(result, ClientUiCommandResult::ShellCommand(command) if command == expected),
                "{id} must route to {expected:?}"
            );
        }
    }
}
