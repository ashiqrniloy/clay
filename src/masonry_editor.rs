use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use masonry::accesskit::{Node, NodeId, Role};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, MutateCtx, NewWidget,
    PaintCtx, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update,
    UpdateCtx, Widget, WidgetMut, WidgetPod,
};
use masonry::kurbo::{Point, Rect, Size};
use masonry::vello::Scene;

use crate::client::{
    ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientRuntimeStateCandidate,
    ClientRuntimeStateInstallError, ClientUiCommandRoute,
};
use crate::editor::typography::UiTextVariant;
use crate::masonry_package_region::{PackageOverlayHost, PackagePanelHost};
use crate::masonry_pane_document::PaneDocumentView;
use crate::masonry_sdui::{SduiNativeState, editor_region_for_document};
use crate::masonry_sdui_region::SduiRegionWidget;
// Re-exported so the native app driver (`main.rs`) can downcast the reconciled
// SDUI button's action without exposing the whole `pub(crate)` region module.
pub use crate::masonry_sdui_region::{SduiButtonPress, SduiListRowPress};
// Same for the reconciled package fixed-panel button/list-row actions (13b).
pub use crate::masonry_package_region::{
    PackageButtonPress, PackageDropdownSelect, PackageListRowPress,
};
use crate::protocol::{
    ClientId, DocumentAccess, DocumentId, DocumentVersion, FontRole, RuntimeDiagnostic,
    RuntimeGenerationId,
};
use crate::shell::{PaneId, TransientMenuSession};

#[derive(Debug, Default, PartialEq)]
pub struct ClipboardCommandOutcome {
    pub changed: bool,
    pub diagnostic: Option<ClientConnectionEvent>,
}

impl ClipboardCommandOutcome {
    pub(crate) fn unchanged() -> Self {
        Self {
            changed: false,
            diagnostic: None,
        }
    }

    pub(crate) fn diagnostic(diagnostic: ClientConnectionEvent) -> Self {
        Self {
            changed: false,
            diagnostic: Some(diagnostic),
        }
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "editor event channel is low-volume; boxing would add churn without measured benefit"
)]
/// Phase 22.3: a fresh client session carried to the driver (reconnect or
/// new tab). `ClientSession` is not `PartialEq` (its queue sender and event
/// receiver are unique handles), so equality compares only the connection
/// identity — enough for action assertions in tests.
#[derive(Debug)]
pub struct DriverSession {
    pub session: crate::client::ClientSession,
}

impl PartialEq for DriverSession {
    fn eq(&self, other: &Self) -> bool {
        self.session.initial_state.client_id == other.session.initial_state.client_id
    }
}

#[derive(Debug, PartialEq)]
pub enum EditorAction {
    ClientConnection(ClientConnectionEvent),
    ClientUiCommand(ClientUiCommandRoute),
    /// Native file dialog finished on a background thread (Linux portal must not
    /// block the Wayland/UI event loop or the chooser never appears).
    FileDialogCompleted {
        generation: u64,
        result: crate::client::FileDialogResult,
    },
    /// Native folder dialog finished on a background thread.
    FolderDialogCompleted {
        generation: u64,
        result: crate::client::FileDialogResult,
    },
    /// Phase 22.3: the new-tab folder picker finished; the driver spawns the
    /// tab's connection with the picked folder as its workspace root.
    NewTabFolderDialogCompleted {
        generation: u64,
        result: crate::client::FileDialogResult,
    },
    /// Phase 22.3: a tab's reconnect or new-tab connection succeeded. The
    /// driver re-keys the tab (reconnect) or mounts it (new tab).
    ReconnectTabConnected {
        client_id: ClientId,
        session: DriverSession,
    },
    /// Phase 22.3: a new tab's connection was established; the picked folder
    /// becomes the tab's workspace root (`TabCommand::New`).
    OpenTabConnected {
        session: DriverSession,
        workspace_root: std::path::PathBuf,
    },
    /// Phase 22.3: a new tab's connection failed (refused at the connection
    /// cap, server down, …): the tab is not opened and a diagnostic surfaces.
    OpenTabFailed {
        message: String,
    },
    /// A transient menu's local state (selection/query/cancel) changed via the
    /// keyboard outside any server connection event. Re-syncs the hosted
    /// overlay — the reconcile needs a `MutateCtx`, which only the action loop
    /// provides (`EventCtx` can't reach one), plan 070 step 13e.
    MenuStateChanged,
    /// Phase 22.2: a pane's content gained Masonry focus. The driver syncs the
    /// shell's active pane and moves keyboard routing to the pane's view.
    PaneFocused(PaneId),
    /// Phase 22.2: the open-documents menu selected a document owned by
    /// another pane. The driver switches that pane to the document and
    /// focuses it (duplicate opens stay blocked).
    ActivateDocumentInPane {
        document_id: DocumentId,
        pane_id: PaneId,
    },
    /// Phase 22.4: the tab-close confirm menu selected an item. The session
    /// is driver-owned (it orchestrates saving every dirty pane of the tab
    /// before `TabCommand::Close`, or the discard/cancel choice), so the
    /// pane view hands the selection here instead of routing it locally or
    /// to the server. `command_id` is one of the driver-local
    /// `clay.shell.clientTabClose*` family; `client_id` identifies the tab.
    TabCloseMenuAction {
        client_id: u64,
        command_id: String,
    },
    /// Phase 22.2: a pane view is about to dispatch a workspace open intent
    /// (definition navigation) that bypasses `route_sdui_intent`; the driver
    /// records the active pane as the open target so the answering
    /// `DocumentOpened` lands in the requesting pane.
    RecordPendingOpenIntent {
        root_id: u64,
        relative_path: String,
    },
    /// Phase 22.3: a tab bar card was clicked. `Activate` switches
    /// optimistically (the server registry is the reconciling authority);
    /// `Close` closes the tab's connection (the registry snapshot drives the
    /// removal).
    TabBar(TabBarAction),
}

/// Phase 22.3: tab bar card actions. The client id identifies the mounted
/// tab; the driver resolves the server `TabId` from its registry state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBarAction {
    Activate {
        client_id: ClientId,
    },
    Close {
        client_id: ClientId,
    },
    /// Phase 22.3: the tab bar's "new tab" affordance (folder picker → new
    /// connection → `TabCommand::New`). Keybindings are the 22.4 task.
    NewTab,
}

/// Argless, direction-specific editor client commands reachable from the
/// keybinding `ClientUiCommand` route (Plan 071 task 5). Each maps 1:1 to an
/// `EditorCommand` motion/selection. The generic `clientMoveCursor`/
/// `clientSetSelection` *ops* (`src/server/ops/editor.rs`) are the programmatic
/// typed-args validation surface; these IDs are the keybinding/rebinding
/// execution surface (argless chords, client-local, no round trip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorClientCommand {
    MoveWordStartForward,
    MoveWordStartBackward,
    MoveParagraphForward,
    MoveParagraphBackward,
    SelectWord,
    SelectLine,
    /// Plan 071 task 9 multi-cursor commands.
    AddCursorBelow,
    AddCursorAbove,
    ColumnSelectDown,
    ColumnSelectUp,
    ColumnSelectLeft,
    ColumnSelectRight,
    SelectNextMatch,
    SelectPrevMatch,
    SelectAllMatches,
    CancelMultipleSelections,
    KeepSelection,
    RemoveSelection,
    UndoCursorMove,
}

impl EditorClientCommand {
    /// Maps an allowlisted `clay.editor.clientMoveCursor.*` /
    /// `clay.editor.clientSetSelection.*` / multi-cursor command ID to its
    /// editor command. `None` for IDs outside the allowlisted surface.
    pub fn from_command_id(command_id: &str) -> Option<Self> {
        match command_id {
            "clay.editor.clientMoveCursor.nextWordStart" => Some(Self::MoveWordStartForward),
            "clay.editor.clientMoveCursor.prevWordStart" => Some(Self::MoveWordStartBackward),
            "clay.editor.clientMoveCursor.nextParagraph" => Some(Self::MoveParagraphForward),
            "clay.editor.clientMoveCursor.prevParagraph" => Some(Self::MoveParagraphBackward),
            "clay.editor.clientSetSelection.selectWord" => Some(Self::SelectWord),
            "clay.editor.clientSetSelection.selectLine" => Some(Self::SelectLine),
            "clay.editor.clientAddCursor.below" => Some(Self::AddCursorBelow),
            "clay.editor.clientAddCursor.above" => Some(Self::AddCursorAbove),
            "clay.editor.clientColumnSelect.down" => Some(Self::ColumnSelectDown),
            "clay.editor.clientColumnSelect.up" => Some(Self::ColumnSelectUp),
            "clay.editor.clientColumnSelect.left" => Some(Self::ColumnSelectLeft),
            "clay.editor.clientColumnSelect.right" => Some(Self::ColumnSelectRight),
            "clay.editor.clientSelectNextMatch" => Some(Self::SelectNextMatch),
            "clay.editor.clientSelectPrevMatch" => Some(Self::SelectPrevMatch),
            "clay.editor.clientSelectAllMatches" => Some(Self::SelectAllMatches),
            "clay.editor.clientCancelMultipleSelections" => Some(Self::CancelMultipleSelections),
            "clay.editor.clientKeepSelection" => Some(Self::KeepSelection),
            "clay.editor.clientRemoveSelection" => Some(Self::RemoveSelection),
            "clay.editor.clientUndoCursorMove" => Some(Self::UndoCursorMove),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorConnectionStatus {
    Connecting,
    Connected,
    LocalFallback,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorStatus {
    pub(crate) connection: EditorConnectionStatus,
    pub(crate) document_id: Option<DocumentId>,
    pub(crate) version: Option<DocumentVersion>,
    pub(crate) access: Option<DocumentAccess>,
    pub(crate) runtime_diagnostic: Option<RuntimeDiagnostic>,
    /// Server/client dirty bit for the active document (optimistic after local edits).
    pub(crate) dirty: bool,
    /// Sanitized basename-only title for status/accessibility (never an absolute path).
    pub(crate) document_display_name: Option<String>,
}

// Internal GUI observability surface for headless tests and future agent inspection.
// It intentionally remains pub(crate) instead of a Clay JS API because it only
// exposes status chrome already rendered by the native widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SduiStatusObservation {
    pub status_text: String,
    pub connection_label: String,
    pub access_label: String,
    pub sync_version: Option<DocumentVersion>,
    pub diagnostic_text: Option<String>,
    /// Compact active-theme label (`default`, `theme-gruvbox-material-dark`, …).
    pub theme_label: String,
    pub dirty: bool,
    pub document_display_name: Option<String>,
    pub composing: bool,
    pub pending_edit_count: usize,
    /// Sanitized recovery/prompt summary when a menu or conflict diagnostic is active.
    pub recovery_summary: Option<String>,
}

impl EditorStatus {
    pub fn connecting() -> Self {
        Self {
            connection: EditorConnectionStatus::Connecting,
            document_id: None,
            version: None,
            access: None,
            runtime_diagnostic: None,
            dirty: false,
            document_display_name: None,
        }
    }

    pub fn connected(
        document_id: DocumentId,
        version: DocumentVersion,
        access: DocumentAccess,
    ) -> Self {
        Self {
            connection: EditorConnectionStatus::Connected,
            document_id: Some(document_id),
            version: Some(version),
            access: Some(access),
            runtime_diagnostic: None,
            dirty: false,
            document_display_name: None,
        }
    }

    pub fn connected_with_metadata(
        document_id: DocumentId,
        version: DocumentVersion,
        access: DocumentAccess,
        dirty: bool,
        document_display_name: Option<String>,
    ) -> Self {
        Self {
            connection: EditorConnectionStatus::Connected,
            document_id: Some(document_id),
            version: Some(version),
            access: Some(access),
            runtime_diagnostic: None,
            dirty,
            document_display_name,
        }
    }

    pub fn local_fallback() -> Self {
        Self {
            connection: EditorConnectionStatus::LocalFallback,
            document_id: None,
            version: None,
            access: None,
            runtime_diagnostic: None,
            dirty: false,
            document_display_name: None,
        }
    }

    pub(crate) fn with_document_values(
        mut self,
        document_id: DocumentId,
        version: DocumentVersion,
        access: DocumentAccess,
    ) -> Self {
        self.document_id = Some(document_id);
        self.version = Some(version);
        self.access = Some(access);
        self
    }

    fn connection_label(&self) -> &'static str {
        match self.connection {
            EditorConnectionStatus::Connecting => "Connecting",
            EditorConnectionStatus::Connected => "Connected",
            EditorConnectionStatus::LocalFallback => "Local Fallback",
            EditorConnectionStatus::Disconnected => "Disconnected",
        }
    }

    fn access_label(&self) -> &'static str {
        match &self.access {
            Some(DocumentAccess::Editable { .. }) => "Editable",
            Some(DocumentAccess::ReadOnly) => "Read-only Observer",
            None => "No Server",
        }
    }

    fn version_label(&self) -> String {
        self.version
            .map(|version| format!("v{version}"))
            .unwrap_or_else(|| "version unknown".to_string())
    }

    fn document_label(&self) -> String {
        if let Some(name) = self.document_display_name.as_deref() {
            if let Some(document_id) = self.document_id {
                format!("{name} — doc {document_id}")
            } else {
                name.to_string()
            }
        } else {
            self.document_id
                .map(|document_id| format!("doc {document_id}"))
                .unwrap_or_else(|| "local document".to_string())
        }
    }

    fn diagnostic_text(&self) -> Option<String> {
        self.runtime_diagnostic
            .as_ref()
            .map(|diagnostic| format!("Runtime {}: {}", diagnostic.code, diagnostic.message))
    }

    fn text(&self) -> String {
        let mut text = format!(
            "Clay — {} — {} — {} — {}",
            self.connection_label(),
            self.access_label(),
            self.document_label(),
            self.version_label()
        );
        if self.dirty {
            let marker = crate::editor::accessibility::dirty_marker(true).trim();
            let marker = marker.trim_end_matches('.');
            text.push_str(&format!(" — {marker}"));
        }
        if let Some(diagnostic) = self.diagnostic_text() {
            text.push_str(&format!(" — {diagnostic}"));
        }
        text
    }

    pub(crate) fn observation(&self) -> SduiStatusObservation {
        SduiStatusObservation {
            status_text: self.text(),
            connection_label: self.connection_label().to_string(),
            access_label: self.access_label().to_string(),
            sync_version: self.version,
            diagnostic_text: self.diagnostic_text(),
            // Filled by the pane view's status_observation from the active theme / chrome.
            theme_label: String::new(),
            dirty: self.dirty,
            document_display_name: self.document_display_name.clone(),
            composing: false,
            pending_edit_count: 0,
            recovery_summary: None,
        }
    }
}

impl Default for EditorStatus {
    fn default() -> Self {
        Self::local_fallback()
    }
}

// Re-exported key-stroke helpers for the native driver/tests (implemented in
// the pane-document module alongside the view that uses them).

/// Connection owner widget (Phase 22.2 "chrome").
///
/// Owns everything connection-wide: the master `ClientEditQueue` handle, the
/// SDUI sidebar region, package fixed-panel and transient-overlay hosts, the
/// runtime-generation install, and the shared menu-session-id / ui-version
/// cells. Its per-document editing surface lives in the embedded
/// [`PaneDocumentView`] (pane 1), which the widget handlers delegate to;
/// other panes host `PaneDocumentView` widgets directly and share the same
/// queue (per-document sync states).
pub struct EditorWidget {
    /// Phase 22.2: the pane-1 document view. Not a pod — the chrome itself is
    /// pane 1's focusable content, so widget handlers delegate to the view.
    view: PaneDocumentView,
    /// The pane hosting this chrome (set by the shell at construction).
    pane_id: PaneId,
    edit_queue: Option<ClientEditQueue>,
    sdui: SduiNativeState,
    /// Plan 070 step 8: the reconciled SDUI sidebar, hosted as a real child so
    /// window events route through Masonry. Created once and reconciled in place
    /// by `sync_region` (stable-identity, step 11c); inert until the first
    /// sidebar tree.
    region: WidgetPod<dyn Widget>,
    /// Hosts the package_ui fixed panels as retained Masonry children (plan 070
    /// step 13b). Ordered BELOW `region` in `children_ids` so the SDUI sidebar
    /// keeps hit-test/paint priority; reconciled in place by `sync_panels`.
    panel_host: WidgetPod<PackagePanelHost>,
    /// Hosts the transient overlays (package overlays + the active menu
    /// projected as one) as retained Masonry children layered ABOVE `region`
    /// (plan 070 step 13e). Reconciled in place by `sync_overlays`.
    overlay_host: WidgetPod<PackageOverlayHost>,
    /// The editor main rect shared with `overlay_host` (set each layout from
    /// the SDUI sidebar geometry) so main-pane-anchored overlays resolve.
    overlay_main_rect: Rc<Cell<Rect>>,
    layout_invalidated: bool,
    /// Last successfully installed runtime generation (0 before any live snapshot).
    runtime_generation_id: RuntimeGenerationId,
    /// Connection-shared transient-menu session-id allocator (views clone it).
    menu_session_ids: Rc<Cell<u64>>,
    /// Connection-shared SDUI ui_version mirror (views clone it).
    sdui_ui_version: Rc<Cell<u64>>,
}

impl Default for EditorWidget {
    fn default() -> Self {
        let menu_session_ids = Rc::new(Cell::new(1));
        let sdui_ui_version = Rc::new(Cell::new(0));
        let view =
            PaneDocumentView::new(PaneId(1), menu_session_ids.clone(), sdui_ui_version.clone());
        let overlay_main_rect = Rc::new(Cell::new(Rect::ZERO));
        Self {
            view,
            pane_id: PaneId(1),
            edit_queue: None,
            sdui: SduiNativeState::empty(),
            region: Self::new_region_pod(),
            panel_host: Self::new_panel_host_pod(),
            overlay_host: Self::new_overlay_host_pod(&overlay_main_rect),
            overlay_main_rect,
            layout_invalidated: false,
            runtime_generation_id: 0,
            menu_session_ids,
            sdui_ui_version,
        }
    }
}

impl EditorWidget {
    pub fn with_initial_state(initial_state: ClientInitialState) -> Self {
        let menu_session_ids = Rc::new(Cell::new(1));
        let sdui_ui_version = Rc::new(Cell::new(0));
        let view =
            PaneDocumentView::new(PaneId(1), menu_session_ids.clone(), sdui_ui_version.clone())
                .with_initial_state(initial_state);
        let mut sdui = SduiNativeState::empty();
        // Mirror the editor's resolved theme (base palette + design tokens) so
        // the SDUI chrome can never diverge from the editor text theme.
        sdui.set_ui_theme(view.ui_theme().clone());
        sdui.set_typography(view.typography().clone());
        let overlay_main_rect = Rc::new(Cell::new(Rect::ZERO));
        Self {
            view,
            pane_id: PaneId(1),
            edit_queue: None,
            sdui,
            region: Self::new_region_pod(),
            panel_host: Self::new_panel_host_pod(),
            overlay_host: Self::new_overlay_host_pod(&overlay_main_rect),
            overlay_main_rect,
            layout_invalidated: false,
            runtime_generation_id: 0,
            menu_session_ids,
            sdui_ui_version,
        }
    }

    /// Phase 22.2: record the pane hosting this chrome (the shell calls this
    /// at construction; pane activation actions carry the pane id).
    pub fn set_pane_id(&mut self, pane_id: PaneId) {
        self.pane_id = pane_id;
        self.view.set_pane_id(pane_id);
    }

    pub fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    /// Create the inert region child pod (reconciled later by `sync_region`).
    fn new_region_pod() -> WidgetPod<dyn Widget> {
        NewWidget::new(SduiRegionWidget::new()).erased().to_pod()
    }

    fn new_panel_host_pod() -> WidgetPod<PackagePanelHost> {
        WidgetPod::new(PackagePanelHost::new())
    }

    fn new_overlay_host_pod(main_rect: &Rc<Cell<Rect>>) -> WidgetPod<PackageOverlayHost> {
        WidgetPod::new(PackageOverlayHost::new(main_rect.clone()))
    }

    /// Reconcile the retained fixed-panel children from the current package_ui
    /// state when it changed (plan 070 step 13b). Mirrors `sync_region`.
    pub fn sync_panels(&mut self, ctx: &mut MutateCtx<'_>) {
        if !self.sdui.take_panels_dirty() {
            return;
        }
        let (package_ui, typography, ui_theme) = self.sdui.panels_render_input();
        let mut host = ctx.get_mut(&mut self.panel_host);
        host.widget
            .sync_panels(&mut host.ctx, &package_ui, typography, ui_theme);
    }

    /// Reconcile the retained overlay children (package transient overlays +
    /// the active menu projected as one) from the current state when it changed
    /// (plan 070 step 13e). Mirrors `sync_panels`.
    pub fn sync_overlays(&mut self, ctx: &mut MutateCtx<'_>) {
        if !self.sdui.take_overlays_dirty() {
            return;
        }
        let (overlays, typography, ui_theme) = self.sdui.overlays_render_input();
        let mut host = ctx.get_mut(&mut self.overlay_host);
        host.widget
            .sync_overlays(&mut host.ctx, overlays, typography, ui_theme);
    }

    /// Route a reconciled package `textInput` commit (Enter) to its server
    /// intent, appending the committed `value` (plan 070 step 13c).
    pub fn package_text_input_commit(
        this: &mut WidgetMut<'_, Self>,
        area_id: masonry::core::WidgetId,
        value: &str,
    ) -> Option<crate::protocol::SduiActionIntent> {
        let mut host = this.ctx.get_mut(&mut this.widget.panel_host);
        crate::masonry_package_region::PackagePanelHost::text_input_commit(
            &mut host, area_id, value,
        )
    }

    /// Plan 071 caret-transport fix: whether the effective caret style
    /// animates (pane-1 view).
    pub fn caret_animates(&self) -> bool {
        self.view.caret_animates()
    }

    /// Reconcile the persistent `SduiRegionWidget` child in place from the
    /// current SDUI state when it changed (plan 070 step 11c).
    pub fn sync_region(&mut self, ctx: &mut MutateCtx<'_>) {
        if !self.sdui.take_region_dirty() {
            return;
        }
        let input = self.sdui.region_render_input();
        let mut region_widget = ctx.get_mut(&mut self.region);
        let mut region = region_widget
            .try_downcast::<SduiRegionWidget>()
            .expect("region child is an SduiRegionWidget");
        match input {
            Some(input) => {
                region
                    .widget
                    .set_render_context(input.typography, input.ui_theme);
                region
                    .widget
                    .reconcile_snapshot_live(&mut region.ctx, input.tree);
            }
            None => region.widget.clear_live(&mut region.ctx),
        }
    }

    pub fn with_status(mut self, status: EditorStatus) -> Self {
        self.view = std::mem::take(&mut self.view).with_status(status);
        self
    }

    pub fn with_edit_queue(mut self, edit_queue: ClientEditQueue) -> Self {
        self.edit_queue = Some(edit_queue.clone());
        self.view = std::mem::take(&mut self.view).with_edit_queue(edit_queue);
        self
    }

    /// A clone of the master queue handle for mounting new pane views (all
    /// clones share the per-document sync state and the IPC channel).
    pub fn edit_queue_shared(&self) -> Option<ClientEditQueue> {
        self.edit_queue.clone()
    }

    /// Route a reconciled SDUI widget activation (button or list row) through
    /// the existing server-first command path. The intent is inert (registered
    /// command id + bounded args); the server validates and applies it.
    pub fn enqueue_sdui_intent(&mut self, intent: crate::protocol::SduiActionIntent) {
        if let Some(edit_queue) = &self.edit_queue {
            let _ = edit_queue.enqueue_sdui_action(self.sdui.ui_version(), intent);
        }
    }

    /// Phase 22.2: show/clear the shared transient menu in the chrome overlay
    /// (pushed by pane views through the app driver).
    pub fn set_active_menu(&mut self, menu: Option<TransientMenuSession>) {
        match menu {
            Some(menu) => self.sdui.set_active_menu(menu),
            None => self.sdui.clear_active_menu(),
        }
    }

    /// Phase 22.2: drain the pane-1 view's pending menu push.
    pub fn take_pending_menu(&mut self) -> Option<Option<TransientMenuSession>> {
        self.view.take_pending_menu()
    }

    /// Phase 22.2: whether the pane-1 view (active or retained) owns a document.
    pub fn contains_document(&self, document_id: DocumentId) -> bool {
        self.view.contains_document(document_id)
    }

    /// Phase 22.2: the pane-1 view's active document id.
    pub fn document_id(&self) -> DocumentId {
        self.view.document_id()
    }

    /// Phase 22.2: dirty bit of the pane-1 view's active document (pane-close gate).
    pub fn is_dirty(&self) -> bool {
        self.view.is_dirty()
    }

    /// Phase 22.2: pane-close gate for the pane-1 view (dirty → conflict menu).
    pub fn guard_pane_close(&mut self) -> bool {
        self.view.guard_pane_close()
    }

    /// Phase 22.4: enqueue a save of the pane-1 view's active document,
    /// returning its `DocumentId` (or the diagnostic to surface on failure).
    /// The driver uses this to save a tab's dirty panes before closing it.
    pub fn request_save_active_document(
        &mut self,
    ) -> Result<u64, crate::protocol::RuntimeDiagnostic> {
        self.view.request_save_active_document()
    }

    /// Phase 22.4: the pane-1 view's active document display name (tab-close
    /// confirm prompt naming), or `None` for a blank view.
    pub fn document_display_name(&self) -> Option<String> {
        self.view.document_display_name()
    }

    /// Phase 22.4: push a driver-owned menu session into the pane-1 view's
    /// interactive menu slot (the tab-close confirm flow; the chrome overlay
    /// render follows via the usual `take_pending_menu` → `apply_menu_sync`).
    pub fn push_menu(&mut self, menu: Option<TransientMenuSession>) {
        self.view.push_menu(menu);
    }

    /// Phase 22.3: reconnect the pane-1 view to a fresh connection for the
    /// same tab (queue swap; the next `DocumentOpened` reinstalls).
    pub fn reconnect(&mut self, edit_queue: ClientEditQueue) {
        self.view.reconnect(edit_queue);
    }

    /// Phase 22.3: the pane-1 view's documents to re-open after a reconnect.
    pub fn documents_for_reopen(&self) -> Vec<(crate::protocol::WorkspaceRootId, String)> {
        self.view.documents_for_reopen()
    }

    /// Phase 22.2: pane close cleanup for the pane-1 view.
    pub fn close_pane_view(&mut self) {
        self.view.close_pane();
    }

    /// Phase 22.2: the pane-1 view's opened-document flag.
    pub fn has_opened_document_view(&self) -> bool {
        self.view.has_opened_document()
    }

    /// Phase 22.2: runtime baseline freshly mounted pane views are seeded with.
    pub fn runtime_baseline(&self) -> crate::masonry_pane_document::RuntimeBaseline {
        self.view.runtime_baseline()
    }

    /// Phase 22.2: shared menu session-id allocator (for mounting new views).
    pub fn menu_session_ids_shared(&self) -> Rc<Cell<u64>> {
        self.menu_session_ids.clone()
    }

    /// Phase 22.2: shared SDUI ui_version mirror (for mounting new views).
    pub fn sdui_ui_version_shared(&self) -> Rc<Cell<u64>> {
        self.sdui_ui_version.clone()
    }

    /// Return and clear a layout request caused by a typography profile change.
    pub fn take_layout_invalidation(&mut self) -> bool {
        // Drain BOTH flags: the chrome's and the pane-1 view's (short-circuit
        // must not skip the view's flag or the next take would re-report).
        let chrome = std::mem::take(&mut self.layout_invalidated);
        let view = self.view.take_layout_invalidation();
        chrome || view
    }

    #[cfg(test)]
    #[cfg(test)]
    pub(crate) fn editor_state_for_test(&self) -> &crate::editor::surface::EditorDocumentState {
        self.view.editor_state_for_test()
    }

    #[cfg(test)]
    pub(crate) fn apply_behavior_manifest(&mut self, manifest: &crate::protocol::BehaviorManifest) {
        self.view.apply_behavior_manifest(manifest);
    }

    #[cfg(test)]
    pub(crate) fn view_mut(&mut self) -> &mut PaneDocumentView {
        &mut self.view
    }

    pub fn apply_connection_event(&mut self, event: ClientConnectionEvent) -> bool {
        // Phase 22.2: document-scoped events route to the pane view that owns
        // the document (the app driver routes other panes directly; this chrome
        // covers pane 1's active/retained documents). `DocumentOpened` for a
        // brand-new document must reach the view unconditionally — the view's
        // own same-document no-op protects live views from redundant snapshots.
        if let Some(document_id) = event.document_id() {
            let is_open = matches!(event, ClientConnectionEvent::DocumentOpened { .. });
            return if is_open || self.view.contains_document(document_id) {
                self.view.apply_connection_event(event)
            } else {
                false
            };
        }
        match event {
            ClientConnectionEvent::ActiveTheme(theme) => {
                let changed = self
                    .view
                    .apply_connection_event(ClientConnectionEvent::ActiveTheme(theme));
                self.sdui.set_ui_theme(self.view.ui_theme().clone());
                changed
            }
            ClientConnectionEvent::ActiveTypography(typography) => {
                let changed = self
                    .view
                    .apply_connection_event(ClientConnectionEvent::ActiveTypography(typography));
                if changed {
                    self.sdui.set_typography(self.view.typography().clone());
                    self.layout_invalidated = true;
                }
                changed
            }
            ClientConnectionEvent::SduiSnapshot { tree, .. } => {
                self.sdui.apply_snapshot(tree);
                self.sdui_ui_version.set(self.sdui.ui_version());
                true
            }
            ClientConnectionEvent::SduiUpdate(update) => {
                let changed = self.sdui.apply_update(update);
                self.sdui_ui_version.set(self.sdui.ui_version());
                changed
            }
            ClientConnectionEvent::RuntimeStateSnapshot(snapshot) => {
                self.install_runtime_state_snapshot(*snapshot)
            }
            ClientConnectionEvent::ShellPreferences(_)
            | ClientConnectionEvent::EditTransaction(_)
            | ClientConnectionEvent::BehaviorManifestRejected { .. } => false,
            // Behavior manifests, caret style, diagnostics, disconnect and
            // server-error handling are per-view concerns in Phase 22.2.
            _ => self.view.apply_connection_event(event),
        }
    }

    /// Validate and atomically install one complete runtime-generation snapshot.
    ///
    /// On success the widget acknowledges the generation through the edit queue
    /// exactly once (other pane views receive the per-document parts via the
    /// app driver's fan-out). On validation failure no partial state remains,
    /// no acknowledgement is sent, and the connection status becomes
    /// disconnected so the shell can rebootstrap into the latest authoritative
    /// state.
    fn install_runtime_state_snapshot(
        &mut self,
        snapshot: crate::protocol::RuntimeStateSnapshot,
    ) -> bool {
        let expected_client_id = self
            .edit_queue
            .as_ref()
            .map(|queue| queue.client_id())
            .unwrap_or(snapshot.client_id);
        let candidate = match ClientRuntimeStateCandidate::validate(
            snapshot,
            expected_client_id,
            self.runtime_generation_id,
        ) {
            Ok(candidate) => candidate,
            Err(ClientRuntimeStateInstallError::StaleOrDuplicateGeneration { .. }) => {
                // Already on this or a newer generation; keep state and send no ack.
                return false;
            }
            Err(_) => {
                let _ = self
                    .view
                    .apply_connection_event(ClientConnectionEvent::Disconnected);
                return true;
            }
        };

        let typography_changed = self.view.install_runtime_baseline(
            &candidate.behavior,
            &candidate.active_theme,
            &candidate.active_typography,
        );
        if typography_changed {
            self.sdui.set_typography(self.view.typography().clone());
        }
        self.sdui.set_ui_theme(self.view.ui_theme().clone());
        self.sdui.apply_snapshot(candidate.sdui_tree.clone());
        self.sdui.install_package_ui_snapshot(&candidate.package_ui);
        self.sdui_ui_version.set(self.sdui.ui_version());

        // Per-document decorations/diagnostics apply to the pane-1 view only
        // here; the driver fans the other panes' documents out.
        for document in &candidate.documents {
            let _ = self.view.apply_runtime_document_state(document);
        }
        if let Some(diagnostic) = candidate.diagnostics.last() {
            let _ = self.view.apply_runtime_status_diagnostic(diagnostic);
        }

        self.runtime_generation_id = candidate.runtime_generation_id;
        self.layout_invalidated = true;

        if let Some(queue) = &self.edit_queue {
            let _ = queue.enqueue_runtime_generation_installed(candidate.runtime_generation_id);
        }
        true
    }

    pub fn status_text(&self) -> String {
        self.view.status_text()
    }

    pub(crate) fn status_observation(&self) -> SduiStatusObservation {
        self.view.status_observation()
    }

    pub fn sdui_visible_texts(&self) -> Vec<String> {
        self.sdui.visible_texts()
    }

    pub fn sdui_ui_version(&self) -> u64 {
        self.sdui.ui_version()
    }

    #[cfg(test)]
    pub(crate) fn visible_text_for_test(&self) -> String {
        self.view.visible_text_for_test()
    }

    pub fn decoration_span_count(&self) -> usize {
        self.view.decoration_span_count()
    }

    pub fn diagnostic_span_count(&self) -> usize {
        self.view.diagnostic_span_count()
    }

    pub fn request_selected_file_open(&self, path: PathBuf) -> Option<ClientConnectionEvent> {
        let Some(queue) = &self.edit_queue else {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.client.selected_file_open.unavailable",
                    "Cannot open the selected file because this editor is not connected to a Clay server.",
                ),
            ));
        };
        queue.enqueue_open_selected_file(path).err().map(|error| {
            ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                "clay.client.selected_file_open.queue_failed",
                format!("Failed to send selected-file open request to the Clay server: {error}"),
            ))
        })
    }

    pub fn request_selected_workspace_root(&self, path: PathBuf) -> Option<ClientConnectionEvent> {
        let Some(queue) = &self.edit_queue else {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.client.selected_folder_open.unavailable",
                    "Cannot add the selected folder because this editor is not connected to a Clay server.",
                ),
            ));
        };
        queue
            .enqueue_add_selected_workspace_root(path)
            .err()
            .map(|error| {
                ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                    "clay.client.selected_folder_open.queue_failed",
                    format!("Failed to send selected-folder request to the Clay server: {error}"),
                ))
            })
    }

    pub fn copy_selection_to_system_clipboard(&self) -> Option<ClientConnectionEvent> {
        self.view.copy_selection_to_system_clipboard()
    }

    pub fn cut_selection_to_system_clipboard(&mut self) -> ClipboardCommandOutcome {
        self.view.cut_selection_to_system_clipboard()
    }

    pub fn paste_from_system_clipboard(&mut self) -> ClipboardCommandOutcome {
        self.view.paste_from_system_clipboard()
    }

    pub fn undo(&mut self) -> bool {
        self.view.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.view.redo()
    }

    pub fn apply_editor_client_command(&mut self, command: EditorClientCommand) -> bool {
        self.view.apply_editor_client_command(command)
    }

    pub fn cancel_composition(&mut self) -> bool {
        self.view.cancel_composition()
    }

    pub fn show_open_documents_menu(
        &mut self,
        other_panes: &[crate::masonry_pane_document::CrossPaneDocumentEntry],
    ) -> bool {
        self.view.show_open_documents_menu(other_panes)
    }

    /// Phase 22.2: active document summary for cross-pane menu aggregation.
    pub fn active_document_info(&self) -> Option<(DocumentId, String, bool)> {
        self.view.active_document_info()
    }

    /// Phase 22.2: retained sessions for cross-pane menu aggregation.
    pub fn retained_documents(&self) -> Vec<(DocumentId, String, bool)> {
        self.view.retained_documents()
    }

    pub fn activate_document(&mut self, document_id: DocumentId) -> bool {
        self.view.activate_document(document_id)
    }

    pub fn request_resync_active_document(&mut self) -> Option<ClientConnectionEvent> {
        self.view.request_resync_active_document()
    }

    pub fn dismiss_recovery(&mut self) -> bool {
        self.view.dismiss_recovery()
    }

    pub fn retained_session_count(&self) -> usize {
        self.view.retained_session_count()
    }

    // -- test-facing forwards (private; the test module drives the pane-1 view) --

    fn editor_main_rect(&self, size: Size) -> Rect {
        let document_id = self.view.document_id();
        editor_region_for_document(size, &self.sdui, document_id)
    }

    #[cfg(test)]
    fn editor_local_point(&self, size: Size, point: Point) -> Option<Point> {
        let rect = self.editor_main_rect(size);
        rect.contains(point)
            .then(|| Point::new(point.x - rect.x0, point.y - rect.y0))
    }

    fn accessibility_label(&self) -> String {
        self.view.accessibility_label()
    }
}

impl Widget for EditorWidget {
    type Action = EditorAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        // Sidebar scroll is handled by the reconciled region's
        // `SduiScrollViewport` as the event bubbles through it. Everything else
        // (including main-rect pointer interaction) is the pane-1 view's.
        match event {
            PointerEvent::Scroll(scroll)
                if self
                    .sdui
                    .scrolls_point(ctx.size(), ctx.local_position(scroll.state.position)) => {}
            _ => self.view.handle_pointer_event(ctx, props, event),
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        self.view.handle_text_event(ctx, props, event);
    }

    fn on_access_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, props: &mut PropertiesMut<'_>, event: &Update) {
        // Phase 22.2: a sidebar/panel/overlay child inside this chrome gained
        // focus → pane 1 is active. (The view submits `PaneFocused` for the
        // chrome's own focus via `handle_update`.)
        if let Update::ChildFocusChanged(true) = event {
            ctx.submit_action::<EditorAction>(EditorAction::PaneFocused(self.pane_id));
            return;
        }
        self.view.handle_update(ctx, props, event);
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        self.view.handle_anim_frame(ctx, props, interval);
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.region);
        ctx.register_child(&mut self.panel_host);
        ctx.register_child(&mut self.overlay_host);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(900.0, 600.0))
        };
        let editor_rect = self.editor_main_rect(size);
        // Share the main rect with the overlay host so main-pane-anchored
        // overlays resolve before its children are laid out below (step 13e).
        self.overlay_main_rect.set(editor_rect);
        // The pane-1 view fills the main rect (its status line paints at the
        // rect's bottom; its IME area is window-anchored by `layout_in`).
        let view_constraints =
            BoxConstraints::tight(Size::new(editor_rect.width(), editor_rect.height()));
        self.view.layout_in(ctx, &view_constraints, editor_rect);
        // Place the reconciled SDUI region child (plan 070 step 8). The sidebar
        // geometry comes from the same legacy walk that produces the hit-test
        // action rects, so the painted region and the click rects stay
        // pixel-aligned. The region is placed as a fixed scroll viewport:
        // sidebar width × (sidebar height below the top padding), at the
        // sidebar origin below the padding.
        if let Some(geo) = self.sdui.sidebar_geometry(size) {
            let region_size = Size::new(geo.rect.width(), geo.rect.height());
            let _ = ctx.run_layout(
                &mut self.region,
                &BoxConstraints::new(region_size, region_size),
            );
            ctx.place_child(&mut self.region, Point::new(geo.rect.x0, geo.rect.y0));
        } else {
            let _ = ctx.run_layout(
                &mut self.region,
                &BoxConstraints::new(Size::ZERO, Size::ZERO),
            );
            ctx.place_child(&mut self.region, Point::ZERO);
        }
        // The fixed-panel host fills the working area and places its panel-region
        // children at their absolute slot rects (plan 070 step 13b).
        let _ = ctx.run_layout(&mut self.panel_host, &BoxConstraints::new(size, size));
        ctx.place_child(&mut self.panel_host, Point::ZERO);
        // The overlay host fills the working area and places each overlay region
        // at its anchor rect, layered above the region (plan 070 step 13e).
        let _ = ctx.run_layout(&mut self.overlay_host, &BoxConstraints::new(size, size));
        ctx.place_child(&mut self.overlay_host, Point::ZERO);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        // The editor canvas paints in `paint()` so it lands BELOW the
        // `overlay_host` child (transient overlays/menus): Masonry order is
        // parent.paint() -> children -> parent.post_paint(), and the status
        // line stays in `post_paint` so it remains above the overlays (plan 070
        // step 13e). The pane-1 view paints its background + surface + status.
        self.view.paint_in(ctx, scene);
    }

    fn post_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        scene: &mut Scene,
    ) {
        // Runs AFTER the children pass, so the status line paints above the
        // `overlay_host` child (transient overlays/menus).
        self.view.post_paint_in(ctx, scene);
    }

    fn accessibility_role(&self) -> Role {
        Role::MultilineTextInput
    }

    fn accessibility(
        &mut self,
        ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.accessibility_label());
        // The reconciled SDUI tree's accessibility flows through the region
        // child's scroll-viewport subtree. Include it only when a sidebar tree
        // is present so an empty region doesn't contribute an empty group;
        // package fixed panels flow through the `panel_host` child (step 13b),
        // and transient overlays/menus flow through the `overlay_host` child
        // (step 13f).
        let mut children = Vec::new();
        if self.sdui.sidebar_geometry(ctx.size()).is_some() {
            children.push(self.region.id().into());
        }
        children.push(self.panel_host.id().into());
        children.push(self.overlay_host.id().into());
        let metrics = self
            .view
            .typography()
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Status);
        let size = ctx.size();
        let status_id = NodeId::from(masonry::core::WidgetId::next());
        let observation = self.status_observation();
        let mut status = Node::new(Role::Status);
        status.set_label(
            crate::editor::accessibility::compose_status_accessibility_label(
                &observation.status_text,
                None,
            ),
        );
        status.set_bounds(masonry::accesskit::Rect {
            x0: 0.0,
            y0: (size.height - metrics.status_height()).max(0.0),
            x1: size.width.max(0.0),
            y1: size.height.max(0.0),
        });
        ctx.tree_update().nodes.push((status_id, status));
        children.push(status_id);
        node.set_children(children);
    }

    fn children_ids(&self) -> ChildrenIds {
        // `panel_host` first so `region` (the SDUI sidebar) is later/topmost and
        // wins hit-testing where their rects could meet (plan 070 step 13b);
        // `overlay_host` last so the transient overlays layer above everything
        // (plan 070 step 13e).
        ChildrenIds::from_slice(&[
            self.panel_host.id(),
            self.region.id(),
            self.overlay_host.id(),
        ])
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn accepts_text_input(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorClientCommand, EditorWidget};
    use crate::client::{ClientConnectionEvent, ClientEditQueue, ClientInitialState};
    use crate::editor::EditorCommand;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, DocumentAccess, DocumentMetadata, FontRole,
        SduiEditorBinding, SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
        SduiTreeOperation, SduiTreeUpdate,
    };
    use crate::shell::{
        FixedPackagePanel, FixedSlotId, FixedSlotState, PackagePanelVisibility,
        PackageUiComponentTree, PackageUiRuntimeUpdate, PaneSlotLayout,
    };

    fn sdui_tree(label_text: &str) -> SduiTree {
        SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![
                SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Flex {
                        direction: SduiFlexDirection::Row,
                        children: vec![SduiNodeId(2), SduiNodeId(3)],
                    },
                ),
                SduiNode::new(
                    SduiNodeId(2),
                    SduiNodeKind::Label {
                        text: label_text.to_string(),
                    },
                ),
                SduiNode::new(
                    SduiNodeId(3),
                    SduiNodeKind::EditorView {
                        binding: SduiEditorBinding {
                            document_id: 7,
                            expected_version: Some(12),
                        },
                    },
                ),
            ],
        }
    }

    fn initial_state(access: DocumentAccess, version: u64) -> ClientInitialState {
        ClientInitialState {
            client_id: 11,
            document_id: 7,
            document_version: version,
            text: "server text".to_string(),
            access,
            behavior_manifest: BehaviorManifest::minimal_text_editing(3),
            active_theme: crate::protocol::ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: crate::protocol::ActiveTypography::default(),
            workspace_root: "/tmp/root".to_string(),
        }
    }

    #[test]
    fn live_typography_update_requests_layout_render_and_accessibility() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        let mut typography = crate::protocol::ActiveTypography {
            revision: 1,
            ..crate::protocol::ActiveTypography::default()
        };
        typography.ui.size = 16.0;

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::ActiveTypography(
                typography.clone()
            ))
        );
        assert_eq!(widget.sdui.typography_revision(), 1);
        assert!(widget.take_layout_invalidation());
        assert!(
            !widget.apply_connection_event(ClientConnectionEvent::ActiveTypography(typography))
        );
        assert!(!widget.take_layout_invalidation());
    }

    #[test]
    fn editor_client_command_maps_ids_and_moves_caret() {
        // Plan 071 task 5: the six direction-specific command IDs map to editor
        // commands; unknown IDs (e.g. argless clipboard commands) map to None.
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientMoveCursor.nextWordStart"),
            Some(EditorClientCommand::MoveWordStartForward)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientMoveCursor.prevWordStart"),
            Some(EditorClientCommand::MoveWordStartBackward)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientMoveCursor.nextParagraph"),
            Some(EditorClientCommand::MoveParagraphForward)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientMoveCursor.prevParagraph"),
            Some(EditorClientCommand::MoveParagraphBackward)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientSetSelection.selectWord"),
            Some(EditorClientCommand::SelectWord)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientSetSelection.selectLine"),
            Some(EditorClientCommand::SelectLine)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientCopySelection"),
            None
        );
        // Plan 071 task 9: the multi-cursor command IDs map to editor commands.
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientAddCursor.below"),
            Some(EditorClientCommand::AddCursorBelow)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientAddCursor.above"),
            Some(EditorClientCommand::AddCursorAbove)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientColumnSelect.down"),
            Some(EditorClientCommand::ColumnSelectDown)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientColumnSelect.left"),
            Some(EditorClientCommand::ColumnSelectLeft)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientSelectNextMatch"),
            Some(EditorClientCommand::SelectNextMatch)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientSelectPrevMatch"),
            Some(EditorClientCommand::SelectPrevMatch)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientSelectAllMatches"),
            Some(EditorClientCommand::SelectAllMatches)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientCancelMultipleSelections"),
            Some(EditorClientCommand::CancelMultipleSelections)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientKeepSelection"),
            Some(EditorClientCommand::KeepSelection)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientRemoveSelection"),
            Some(EditorClientCommand::RemoveSelection)
        );
        assert_eq!(
            EditorClientCommand::from_command_id("clay.editor.clientUndoCursorMove"),
            Some(EditorClientCommand::UndoCursorMove)
        );

        // Dispatch moves the caret/selection on the underlying pane-1 view.
        // Text: "server text" ("server" 0..6, space 6, "text" 7..11).
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ));
        widget.view_mut().editor_mut().set_caret_for_test(0);
        assert!(widget.apply_editor_client_command(EditorClientCommand::SelectWord));
        assert_eq!(widget.view_mut().editor_mut().caret_for_test(), 6);
        assert_eq!(
            widget.view_mut().editor_mut().selection_for_test(),
            Some((0, 6))
        );

        assert!(widget.apply_editor_client_command(EditorClientCommand::MoveWordStartForward));
        assert_eq!(widget.view_mut().editor_mut().caret_for_test(), 7);
        assert!(widget.apply_editor_client_command(EditorClientCommand::SelectLine));
        assert_eq!(
            widget.view_mut().editor_mut().selection_for_test(),
            Some((0, 11))
        );
    }

    #[test]
    fn sdui_snapshot_replaces_native_tree_state() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::SduiSnapshot {
                client_id: 11,
                tree: sdui_tree("Ready"),
            })
        );

        assert_eq!(widget.sdui_ui_version(), 1);
        assert!(widget.sdui_visible_texts().contains(&"Ready".to_string()));
    }

    #[test]
    fn sdui_update_preserves_editor_document_state() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.apply_connection_event(ClientConnectionEvent::SduiSnapshot {
            client_id: 11,
            tree: sdui_tree("Ready"),
        });
        let before_text = widget.view_mut().editor_mut().visible_text();
        let before_version = widget
            .view_mut()
            .editor_mut()
            .document_state()
            .document_version;

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::SduiUpdate(SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: SduiNode::new(
                        SduiNodeId(2),
                        SduiNodeKind::Label {
                            text: "Updated".to_string(),
                        },
                    ),
                }],
            },))
        );

        assert_eq!(widget.view_mut().editor_mut().visible_text(), before_text);
        assert_eq!(
            widget
                .view_mut()
                .editor_mut()
                .document_state()
                .document_version,
            before_version
        );
        assert!(widget.sdui_visible_texts().contains(&"Updated".to_string()));
    }

    #[test]
    fn side_panel_update_does_not_replace_editor_widget() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget
            .view_mut()
            .editor_mut()
            .command(EditorCommand::Insert(" local"));
        widget.apply_connection_event(ClientConnectionEvent::SduiSnapshot {
            client_id: 11,
            tree: sdui_tree("Ready"),
        });
        let before_text = widget.view_mut().editor_mut().visible_text();
        let before_document = widget.view_mut().editor_mut().document_state().clone();

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::SduiUpdate(SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: SduiNode::new(
                        SduiNodeId(2),
                        SduiNodeKind::Label {
                            text: "Side panel updated".to_string(),
                        },
                    ),
                }],
            },))
        );

        assert_eq!(widget.view_mut().editor_mut().visible_text(), before_text);
        assert_eq!(
            widget.view_mut().editor_mut().document_state(),
            &before_document
        );
        assert!(
            widget
                .sdui_visible_texts()
                .contains(&"Side panel updated".to_string())
        );
    }

    #[test]
    fn fixed_package_panel_shrinks_editor_hit_region() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget
            .sdui
            .apply_package_ui_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Left,
                    PackagePanelVisibility::Visible,
                    PackageUiComponentTree {
                        id: "markdown.preview.root".to_string(),
                        disabled: false,
                        kind: "panel".to_string(),
                        font_role: FontRole::Ui,
                        text_variant: None,
                        title: Some("Preview".to_string()),
                        text: None,
                        label: None,
                        action_command_id: None,
                        items: Vec::new(),
                        children: Vec::new(),
                        validation_state: None,
                    },
                    Vec::new(),
                )],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .unwrap();

        let size = masonry::kurbo::Size::new(900.0, 600.0);
        let main = widget.editor_main_rect(size);

        assert_eq!(main.x0, 240.0);
        assert!(
            widget
                .editor_local_point(size, masonry::kurbo::Point::new(100.0, 80.0))
                .is_none()
        );
        assert_eq!(
            widget.editor_local_point(size, masonry::kurbo::Point::new(300.0, 80.0)),
            Some(masonry::kurbo::Point::new(60.0, 80.0))
        );
    }

    #[test]
    fn editor_pointer_hit_testing_uses_non_overlapping_editor_region_after_open() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.apply_connection_event(ClientConnectionEvent::SduiSnapshot {
            client_id: 11,
            tree: sdui_tree("Workspace"),
        });
        // Opening a workspace file swaps the active document ID away from the
        // bootstrap document the SDUI editor view still binds. The editor main
        // region must still exclude the Clay-owned left slot, so a click in the
        // left file browser does not place a caret under the panel.
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 1,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "src/main.rs".to_string(),
                },
                text: "fn main() {}\n".to_string(),
            })
        );

        let size = masonry::kurbo::Size::new(900.0, 600.0);
        let main = widget.editor_main_rect(size);
        assert_eq!(main.x0, 240.0);
        assert_eq!(main.x1, 900.0);
        assert!(
            widget
                .editor_local_point(size, masonry::kurbo::Point::new(100.0, 80.0))
                .is_none(),
            "clicks in the left file-browser pane must not place a caret"
        );
        assert_eq!(
            widget.editor_local_point(size, masonry::kurbo::Point::new(300.0, 80.0)),
            Some(masonry::kurbo::Point::new(60.0, 80.0))
        );
    }

    #[test]
    fn shell_preserves_editor_caret_viewport_and_status_after_slot_resize() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget
            .view_mut()
            .editor_mut()
            .command(EditorCommand::Insert("\nsecond line\nthird line"));
        widget.view_mut().editor_mut().set_caret_for_test(6);
        widget
            .view_mut()
            .editor_mut()
            .set_visual_scroll_bounds_for_test(120.0);
        assert!(widget.view_mut().editor_mut().scroll_vertical_pixels(40.0));
        let before_text = widget.view_mut().editor_mut().visible_text();
        let before_caret = widget.view_mut().editor_mut().caret_for_test();
        let before_scroll_y = widget.view_mut().editor_mut().visual_scroll_y();
        let before_status = widget.status_observation();
        let narrow_slot = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 320.0, 120.0, 360.0).unwrap());
        let wide_slot = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 180.0, 120.0, 360.0).unwrap());

        let narrow_main = narrow_slot
            .compute_geometry(masonry::kurbo::Rect::new(0.0, 0.0, 900.0, 600.0))
            .main_rect;
        let wide_main = wide_slot
            .compute_geometry(masonry::kurbo::Rect::new(0.0, 0.0, 900.0, 600.0))
            .main_rect;

        assert_ne!(narrow_main, wide_main);
        assert_eq!(widget.view_mut().editor_mut().visible_text(), before_text);
        assert_eq!(
            widget.view_mut().editor_mut().caret_for_test(),
            before_caret
        );
        assert_eq!(
            widget.view_mut().editor_mut().visual_scroll_y(),
            before_scroll_y
        );
        assert_eq!(widget.status_observation(), before_status);
    }

    fn runtime_snapshot(
        generation: u64,
        client_id: u64,
        document_id: u64,
    ) -> crate::protocol::RuntimeStateSnapshot {
        let snapshot = crate::protocol::RuntimeStateSnapshot {
            runtime_generation_id: generation,
            client_id,
            behavior: BehaviorManifest::minimal_text_editing(generation),
            active_theme: crate::protocol::ActiveTheme {
                specifier: format!("@clay/theme-gen-{generation}"),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: {
                crate::protocol::ActiveTypography {
                    revision: generation,
                    monospace: crate::protocol::FontProfile {
                        size: 12.0 + generation as f32,
                        ..crate::protocol::ActiveTypography::default().monospace
                    },
                    ..crate::protocol::ActiveTypography::default()
                }
            },
            sdui_tree: SduiTree {
                ui_version: generation,
                root_id: SduiNodeId(1),
                nodes: vec![SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Label {
                        text: format!("runtime-{generation}"),
                    },
                )],
            },
            package_ui: crate::protocol::PackageUiSnapshot {
                version: generation,
            },
            documents: vec![crate::protocol::DocumentRuntimeRenderState {
                document_id,
                document_version: 12,
                reset_decorations: true,
                reset_diagnostics: true,
                initial_decorations: None,
                initial_diagnostics: None,
                behavior_manifest: None,
            }],
            diagnostics: Vec::new(),
        };
        snapshot.validate().expect("fixture snapshot");
        snapshot
    }

    #[test]
    fn client_installs_behavior_theme_typography_ui_and_render_generation_atomically() {
        let (queue, mut outgoing) = ClientEditQueue::bounded(8);
        let queue = queue.with_authority(11, &DocumentAccess::Editable { lease_id: 1 });
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            12,
        ))
        .with_edit_queue(queue);

        // Seed previous-generation package UI so the install must clear it.
        widget
            .sdui
            .apply_package_ui_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "old.panel",
                    FixedSlotId::Left,
                    PackagePanelVisibility::Visible,
                    PackageUiComponentTree {
                        id: "old.panel.root".to_string(),
                        disabled: false,
                        kind: "panel".to_string(),
                        font_role: FontRole::Ui,
                        text_variant: None,
                        title: Some("old".to_string()),
                        text: None,
                        label: None,
                        action_command_id: None,
                        items: Vec::new(),
                        children: Vec::new(),
                        validation_state: None,
                    },
                    Vec::new(),
                )],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .expect("seed package ui");
        assert_eq!(widget.sdui.package_ui_version(), 1);

        let g1_behavior = widget
            .view_mut()
            .editor_mut()
            .document_state()
            .behavior_version;
        assert_eq!(widget.runtime_generation_id, 0);

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeStateSnapshot(Box::new(
                runtime_snapshot(2, 11, 7)
            )))
        );

        assert_eq!(widget.runtime_generation_id, 2);
        assert_eq!(
            widget
                .view_mut()
                .editor_mut()
                .document_state()
                .behavior_version,
            2
        );
        assert_ne!(
            widget
                .view_mut()
                .editor_mut()
                .document_state()
                .behavior_version,
            g1_behavior
        );
        assert_eq!(widget.view_mut().editor_mut().typography().revision(), 2);
        assert_eq!(widget.sdui.ui_version(), 2);
        assert_eq!(widget.sdui.package_ui_version(), 2);
        assert!(
            widget
                .sdui
                .visible_texts()
                .iter()
                .any(|text| text.contains("runtime-2"))
        );
        assert!(widget.take_layout_invalidation());

        match outgoing.try_recv() {
            Ok(ClientMessage::RuntimeGenerationInstalled {
                client_id,
                runtime_generation_id,
            }) => {
                assert_eq!(client_id, 11);
                assert_eq!(runtime_generation_id, 2);
            }
            other => panic!("expected runtime install ack, got {other:?}"),
        }
        assert!(outgoing.try_recv().is_err());
    }

    #[test]
    fn invalid_snapshot_installs_nothing_and_disconnects_without_ack() {
        let (queue, mut outgoing) = ClientEditQueue::bounded(8);
        let queue = queue.with_authority(11, &DocumentAccess::Editable { lease_id: 1 });
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            12,
        ))
        .with_edit_queue(queue);

        let before_behavior = widget
            .view_mut()
            .editor_mut()
            .document_state()
            .behavior_manifest
            .clone();
        let before_theme = widget.view_mut().editor_mut().theme();
        let before_typography = widget.view_mut().editor_mut().typography().revision();
        let before_ui = widget.sdui.ui_version();
        let before_package_ui = widget.sdui.package_ui_version();
        let before_generation = widget.runtime_generation_id;

        let mut invalid = runtime_snapshot(2, 11, 7);
        invalid.behavior.manifest_id.clear();

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeStateSnapshot(Box::new(
                invalid
            )))
        );

        assert_eq!(widget.status_observation().connection_label, "Disconnected");
        assert_eq!(widget.runtime_generation_id, before_generation);
        assert_eq!(
            widget
                .view_mut()
                .editor_mut()
                .document_state()
                .behavior_manifest,
            before_behavior
        );
        assert_eq!(widget.view_mut().editor_mut().theme(), before_theme);
        assert_eq!(
            widget.view_mut().editor_mut().typography().revision(),
            before_typography
        );
        assert_eq!(widget.sdui.ui_version(), before_ui);
        assert_eq!(widget.sdui.package_ui_version(), before_package_ui);
        assert!(outgoing.try_recv().is_err());
    }

    #[test]
    fn runtime_install_preserves_caret_selection_viewport_and_focus_status() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            12,
        ));
        widget
            .view_mut()
            .editor_mut()
            .command(EditorCommand::Insert("alpha beta gamma"));
        widget.view_mut().editor_mut().set_selection_for_test(6, 10);
        widget
            .view_mut()
            .editor_mut()
            .set_visual_scroll_bounds_for_test(80.0);
        assert!(widget.view_mut().editor_mut().scroll_vertical_pixels(24.0));

        let before_caret = widget.view_mut().editor_mut().caret_for_test();
        let before_selection = widget.view_mut().editor_mut().selection_for_test();
        let before_scroll = widget.view_mut().editor_mut().visual_scroll_y();
        let before_connection = widget.status_observation().connection_label.clone();

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeStateSnapshot(Box::new(
                runtime_snapshot(4, 11, 7)
            )))
        );

        assert_eq!(widget.runtime_generation_id, 4);
        assert_eq!(
            widget.view_mut().editor_mut().caret_for_test(),
            before_caret
        );
        assert_eq!(
            widget.view_mut().editor_mut().selection_for_test(),
            before_selection
        );
        assert_eq!(
            widget.view_mut().editor_mut().visual_scroll_y(),
            before_scroll
        );
        assert_eq!(
            widget.status_observation().connection_label,
            before_connection
        );
    }

    #[test]
    fn runtime_install_invalidates_layout_once() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            12,
        ));
        assert!(!widget.take_layout_invalidation());

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeStateSnapshot(Box::new(
                runtime_snapshot(5, 11, 7)
            )))
        );
        assert!(widget.take_layout_invalidation());
        assert!(!widget.take_layout_invalidation());

        // Duplicate generation is ignored without another invalidation or ack.
        assert!(
            !widget.apply_connection_event(ClientConnectionEvent::RuntimeStateSnapshot(Box::new(
                runtime_snapshot(5, 11, 7)
            )))
        );
        assert!(!widget.take_layout_invalidation());
    }
}
