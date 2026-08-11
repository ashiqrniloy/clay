//! Phase 22.2: per-pane document view.
//!
//! [`PaneDocumentView`] is the lightweight per-document editor view widget:
//! one `EditorSurface` (buffer, caret, selection, viewport, undo history),
//! per-document status chrome, the pane's retained-session stash, and the
//! per-document request-id/queue plumbing that used to live in `EditorWidget`.
//!
//! The connection owner (`EditorWidget` in `masonry_editor.rs`) keeps the
//! connection-wide concerns — `ClientEditQueue` master handle, SDUI sidebar,
//! package panels/overlays, runtime-generation install, window chrome — and
//! embeds the pane-1 view as a plain field (delegating widget handlers), while
//! other panes host `PaneDocumentView` widgets directly. Every pane shares one
//! `ClientEditQueue` (clones share the per-document sync state), so edits from
//! any pane reserve against the right document's base version.
//!
//! Menus (completion, language intelligence, save-conflict, sync-recovery) are
//! view-owned: the view keeps the keyboard-interactive session copy and pushes
//! the current session to the connection owner's overlay via
//! [`Self::take_pending_menu`]; the app driver forwards it to the chrome.

use std::cell::Cell;
use std::rc::Rc;

use masonry::accesskit::{Node, NodeId, Role};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, BrushIndex, ChildrenIds, EventCtx, LayoutCtx, PaintCtx,
    PointerEvent, PointerScrollEvent, PropertiesMut, PropertiesRef, RegisterCtx, ScrollDelta,
    TextEvent, Update, UpdateCtx, Widget, WidgetId, render_text,
};
use masonry::kurbo::{Affine, Point, Rect, Size};
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::Fill;
use masonry::vello::Scene;

use crate::client::{
    ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientSyncSnapshot, ClipboardSink,
    SystemClipboard,
};
use crate::editor::{
    CursorSelectDirection, EditorCommand, EditorCommandOutcome, EditorSurface,
    document_session::{DocumentSessionStore, RetainedDocumentSession},
    typography::{UiTextMetrics, UiTextVariant},
};
use crate::masonry_editor::{
    ClipboardCommandOutcome, EditorAction, EditorClientCommand, EditorConnectionStatus,
    EditorStatus, SduiStatusObservation,
};
use crate::perf::metrics::global_recorder;
use crate::protocol::{
    ActiveTheme, ActiveTypography, BehaviorManifest, CompletionRequestId, CompletionResultSet,
    DocumentId, DocumentVersion, EditRejection, EditorCommandRequest, FileErrorCode, FontRole,
    KeyCode, KeyModifiers, KeyStroke, LanguageIntelligenceRequestId, LanguageIntelligenceResult,
    ProtocolErrorCode, RuntimeDiagnostic, SduiActionIntent, WorkspaceRootId,
};
use crate::shell::{
    PaneId, TransientMenuAction, TransientMenuSession, TransientMenuSessionId,
    completion_result_to_menu_session, language_intelligence_result_to_menu_session,
};
// Doc-hidden pass-through so the native `clay` binary can carry menu sessions
// from pane views to the connection owner's overlay.
#[doc(hidden)]
pub use crate::shell::TransientMenuSession as ShellTransientMenuSession;

/// Shared runtime baseline a freshly mounted pane view is seeded with
/// (behavior manifest + theme + typography from the connection owner), so a
/// Phase 22.2: one other pane's document set for the cross-pane
/// open-documents menu (active document + retained sessions).
pub struct CrossPaneDocumentEntry {
    pub pane: crate::shell::PaneId,
    pub document_id: DocumentId,
    pub display_name: String,
    pub dirty: bool,
    pub retained: Vec<(DocumentId, String, bool)>,
}

/// new pane routes keys and paints correctly before the next runtime snapshot.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct RuntimeBaseline {
    pub behavior_manifest: BehaviorManifest,
    pub active_theme: ActiveTheme,
    pub active_typography: ActiveTypography,
}

impl Default for RuntimeBaseline {
    fn default() -> Self {
        Self {
            behavior_manifest: BehaviorManifest::minimal_text_editing(0),
            active_theme: ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: ActiveTypography::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingDefinitionNavigation {
    relative_path: String,
    byte_start: u64,
}

/// Phase 22.7 (C4): per-pane request-id allocators and in-flight request
/// bookkeeping, grouped out of `PaneDocumentView`'s field list. All
/// allocators saturate at `u64::MAX` (`saturating_add(1).max(1)`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PaneRequestBookkeeping {
    next_transaction_id: u64,
    next_completion_request_id: u64,
    active_completion_request_id: Option<CompletionRequestId>,
    next_language_intelligence_request_id: u64,
    active_language_intelligence_request_id: Option<LanguageIntelligenceRequestId>,
    next_selection_query_request_id: u64,
    pending_selection_query: Option<(u64, Vec<crate::protocol::SelectionQueryCursor>)>,
}

impl Default for PaneRequestBookkeeping {
    fn default() -> Self {
        Self {
            next_transaction_id: 1,
            next_completion_request_id: 1,
            active_completion_request_id: None,
            next_language_intelligence_request_id: 1,
            active_language_intelligence_request_id: None,
            next_selection_query_request_id: 1,
            pending_selection_query: None,
        }
    }
}

impl PaneRequestBookkeeping {
    /// Allocate the next id in a family (monotonic, saturating at `u64::MAX`).
    fn bump(next: &mut u64) -> u64 {
        let id = *next;
        *next = next.saturating_add(1).max(1);
        id
    }

    fn next_transaction_id(&mut self) -> u64 {
        Self::bump(&mut self.next_transaction_id)
    }

    fn next_completion_request_id(&mut self) -> CompletionRequestId {
        Self::bump(&mut self.next_completion_request_id)
    }

    fn next_language_intelligence_request_id(&mut self) -> LanguageIntelligenceRequestId {
        Self::bump(&mut self.next_language_intelligence_request_id)
    }

    fn next_selection_query_request_id(&mut self) -> u64 {
        Self::bump(&mut self.next_selection_query_request_id)
    }

    /// Clear the in-flight completion/language-intelligence ids (an edit or
    /// menu change invalidates them). The pending selection query survives
    /// until [`Self::reset`].
    fn clear_active(&mut self) {
        self.active_completion_request_id = None;
        self.active_language_intelligence_request_id = None;
    }

    /// Full reset (view blanking): in-flight ids and pending selection query.
    fn reset(&mut self) {
        self.clear_active();
        self.pending_selection_query = None;
    }

    /// Clear the completion request when it is the one in flight.
    fn take_completion_if_current(&mut self, request_id: CompletionRequestId) -> bool {
        if self.active_completion_request_id == Some(request_id) {
            self.active_completion_request_id = None;
            true
        } else {
            false
        }
    }

    /// Clear the language-intelligence request when it is the one in flight.
    fn take_language_intelligence_if_current(
        &mut self,
        request_id: LanguageIntelligenceRequestId,
    ) -> bool {
        if self.active_language_intelligence_request_id == Some(request_id) {
            self.active_language_intelligence_request_id = None;
            true
        } else {
            false
        }
    }
}

/// Phase 22.7 (C4): the view's transient-menu state — the keyboard-
/// interactive session copy, the pending push to the connection owner's
/// overlay (`Some(Some(menu))` shows, `Some(None)` clears, `None` means no
/// pending change), and the connection-shared session-id allocator.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PaneMenuSync {
    menu: Option<TransientMenuSession>,
    pending: Option<Option<TransientMenuSession>>,
    session_ids: Rc<Cell<u64>>,
}

impl Default for PaneMenuSync {
    fn default() -> Self {
        Self {
            menu: None,
            pending: None,
            session_ids: Rc::new(Cell::new(1)),
        }
    }
}

impl PaneMenuSync {
    /// Take the pending push, leaving the tri-state `None` (no pending change).
    fn take_pending(&mut self) -> Option<Option<TransientMenuSession>> {
        self.pending.take()
    }

    /// Remember `menu` as the current session and mark a pending push.
    fn push(&mut self, menu: Option<TransientMenuSession>) {
        self.menu = menu.clone();
        self.pending = Some(menu);
    }

    /// Connection-shared session-id allocation (monotonic, saturating).
    fn next_session_id(&self) -> u64 {
        let id = self.session_ids.get();
        self.session_ids.set(id.saturating_add(1).max(1));
        id
    }
}

/// Lightweight per-document editor view (Phase 22.2). Doc-hidden: constructed
/// and routed by the native `clay` binary; not a Clay JS API.
#[doc(hidden)]
pub struct PaneDocumentView {
    pane_id: PaneId,
    editor: EditorSurface,
    edit_queue: Option<ClientEditQueue>,
    /// Window-space rect this view paints/edits within (its pane main rect;
    /// `(0,0,size)` for standalone panes). Set during layout.
    editor_rect: Rect,
    pending_definition_navigation: Option<PendingDefinitionNavigation>,
    last_decoration_viewport: Option<(DocumentId, DocumentVersion, u64, u64)>,
    /// Phase 22.7 (C4): request-id allocators and in-flight request
    /// bookkeeping (grouped; see [`PaneRequestBookkeeping`]).
    requests: PaneRequestBookkeeping,
    status: EditorStatus,
    /// Phase 22.7 (C4): transient-menu state — session copy, pending push,
    /// session-id allocator (grouped; see [`PaneMenuSync`]).
    menu_sync: PaneMenuSync,
    layout_invalidated: bool,
    /// Inactive document sessions retained for within-pane switching (Phase 20).
    sessions: DocumentSessionStore,
    has_opened_document: bool,
    /// Shared SDUI ui_version mirror (one per connection; updated by the chrome).
    sdui_ui_version: Rc<Cell<u64>>,
    /// Last `ActiveTheme` installed (re-seeding baseline for new pane views).
    active_theme: Option<ActiveTheme>,
    /// Last `ActiveTypography` installed (re-seeding baseline for new pane views).
    active_typography: Option<ActiveTypography>,
    /// Phase 22.3: the open identity of the active document (workspace root id
    /// + relative path), retained so a reconnected tab can re-open it.
    active_document_path: Option<(WorkspaceRootId, String)>,
    /// Phase 22.3: set by [`Self::reconnect`]; the next `DocumentOpened` for
    /// the active document must reinstall the server snapshot instead of the
    /// Phase 22.2 duplicate-open no-op (the old connection's snapshot is
    /// stale).
    pending_reconnect_resync: bool,
}

impl Default for PaneDocumentView {
    fn default() -> Self {
        let mut editor = EditorSurface::default();
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(0));
        let status = EditorStatus::local_fallback().with_document_values(
            editor.document_state().document_id,
            editor.document_state().document_version,
            editor.document_state().access.clone(),
        );
        Self {
            pane_id: PaneId(1),
            editor,
            edit_queue: None,
            editor_rect: Rect::ZERO,
            pending_definition_navigation: None,
            last_decoration_viewport: None,
            requests: PaneRequestBookkeeping::default(),
            status,
            menu_sync: PaneMenuSync::default(),
            layout_invalidated: false,
            sessions: DocumentSessionStore::default(),
            has_opened_document: false,
            sdui_ui_version: Rc::new(Cell::new(0)),
            active_theme: None,
            active_typography: None,
            active_document_path: None,
            pending_reconnect_resync: false,
        }
    }
}

impl PaneDocumentView {
    /// Create a blank view for one pane. `menu_session_ids` and
    /// `sdui_ui_version` are connection-shared cells owned by the chrome.
    pub fn new(
        pane_id: PaneId,
        menu_session_ids: Rc<Cell<u64>>,
        sdui_ui_version: Rc<Cell<u64>>,
    ) -> Self {
        Self {
            pane_id,
            menu_sync: PaneMenuSync {
                session_ids: menu_session_ids,
                ..PaneMenuSync::default()
            },
            sdui_ui_version,
            ..Self::default()
        }
    }

    pub(crate) fn set_pane_id(&mut self, pane_id: PaneId) {
        self.pane_id = pane_id;
    }

    pub(crate) fn caret_animates(&self) -> bool {
        self.editor.caret_animates()
    }

    pub fn with_edit_queue(mut self, edit_queue: ClientEditQueue) -> Self {
        self.edit_queue = Some(edit_queue);
        self
    }

    pub(crate) fn with_status(mut self, status: EditorStatus) -> Self {
        self.status = status;
        self
    }

    pub(crate) fn with_initial_state(mut self, initial_state: ClientInitialState) -> Self {
        self.editor.load_snapshot(
            initial_state.document_id,
            initial_state.document_version,
            initial_state.text,
            initial_state.access.clone(),
        );
        self.editor
            .install_behavior_manifest(initial_state.behavior_manifest);
        self.editor.set_active_theme(&initial_state.active_theme);
        self.active_theme = Some(initial_state.active_theme);
        let _ = self
            .editor
            .set_typography(initial_state.active_typography.clone());
        self.active_typography = Some(initial_state.active_typography);
        self.status = EditorStatus::connected(
            self.editor.document_state().document_id,
            self.editor.document_state().document_version,
            self.editor.document_state().access.clone(),
        );
        self.has_opened_document = true;
        self
    }

    pub fn with_runtime_baseline(mut self, baseline: &RuntimeBaseline) -> Self {
        self.editor
            .install_behavior_manifest(baseline.behavior_manifest.clone());
        self.editor.set_active_theme(&baseline.active_theme);
        self.active_theme = Some(baseline.active_theme.clone());
        let _ = self
            .editor
            .set_typography(baseline.active_typography.clone());
        self.active_typography = Some(baseline.active_typography.clone());
        self
    }

    /// Baseline the connection owner re-seeds freshly mounted pane views with.
    pub(crate) fn runtime_baseline(&self) -> RuntimeBaseline {
        RuntimeBaseline {
            behavior_manifest: self
                .editor
                .document_state()
                .behavior_manifest
                .clone()
                .unwrap_or_else(|| BehaviorManifest::minimal_text_editing(0)),
            active_theme: self
                .active_theme
                .clone()
                .unwrap_or_else(|| RuntimeBaseline::default().active_theme),
            active_typography: self.active_typography.clone().unwrap_or_default(),
        }
    }

    pub(crate) fn typography(&self) -> &crate::editor::typography::TypographyRegistry {
        self.editor.typography()
    }

    pub(crate) fn ui_theme(&self) -> &crate::shell::ResolvedUiTheme {
        self.editor.ui_theme()
    }

    /// Return and clear a layout request caused by a typography profile change.
    pub fn take_layout_invalidation(&mut self) -> bool {
        std::mem::take(&mut self.layout_invalidated)
    }

    pub fn document_id(&self) -> DocumentId {
        self.editor.document_state().document_id
    }

    /// Whether this view owns `document_id` (active or retained).
    pub fn contains_document(&self, document_id: DocumentId) -> bool {
        document_id == self.document_id() || self.sessions.contains(document_id)
    }

    pub fn is_dirty(&self) -> bool {
        self.status.dirty
    }

    pub fn has_opened_document(&self) -> bool {
        self.has_opened_document
    }

    /// Phase 22.2: pane-close gate. Returns `true` when the pane may close
    /// (no unsaved edits); otherwise shows the save-conflict menu on this view
    /// so unsaved edits are never dropped silently.
    pub fn guard_pane_close(&mut self) -> bool {
        if !self.is_dirty() {
            return true;
        }
        self.show_save_conflict_menu(
            FileErrorCode::DirtyDocument,
            "Document has unsaved edits — save or discard before closing this pane.",
        );
        false
    }

    /// Phase 22.3: reconnect this view to a fresh connection for the same
    /// tab. Swaps in the new connection's edit queue, clears the disconnect
    /// recovery menu, and marks the view so the next `DocumentOpened` for the
    /// active document reinstalls server state (the retained sessions are the
    /// split-tree restore source and stay untouched — the re-open replaces
    /// them). The caller re-issues [`Self::documents_for_reopen`] opens.
    pub fn reconnect(&mut self, edit_queue: ClientEditQueue) {
        self.edit_queue = Some(edit_queue);
        self.pending_reconnect_resync = true;
        self.menu_sync.pending = Some(None);
    }

    /// Phase 22.5: the active document's open identity (workspace root id +
    /// relative path) — the persisted per-pane document identity. Retained
    /// (inactive) sessions are deliberately excluded; blank views (or a pane
    /// with an open still in flight) return `None`.
    pub fn active_document_identity(&self) -> Option<(WorkspaceRootId, String)> {
        self.active_document_path.clone()
    }

    /// Phase 22.3: every document this view holds (active + retained) as its
    /// open identity `(workspace_root_id, relative path)`, for a reconnected
    /// tab to re-open through the plain `OpenDocument` path. Entries without a
    /// recorded identity (blank views) are skipped.
    pub fn documents_for_reopen(&self) -> Vec<(WorkspaceRootId, String)> {
        let mut documents = Vec::new();
        if let Some((root_id, path)) = &self.active_document_path
            && !path.is_empty()
        {
            documents.push((*root_id, path.clone()));
        }
        for (root_id, path) in self.sessions.reopen_documents() {
            if !path.is_empty() && !documents.iter().any(|(_, p)| *p == path) {
                documents.push((root_id, path));
            }
        }
        documents
    }

    /// Pane closed: release the active document (clean close) and every
    /// retained session (force close discards unsaved edits), then reset this
    /// view to a blank placeholder-like state. The app driver blocks pane
    /// close while [`Self::is_dirty`] so unsaved edits are never dropped
    /// silently here.
    pub fn close_pane(&mut self) {
        let _ = self.editor.cancel_composition();
        let active = self
            .has_opened_document
            .then(|| self.editor.document_state().document_id);
        let retained = self.sessions.document_ids();
        if let Some(queue) = &self.edit_queue {
            if let Some(active) = active {
                let _ = queue.enqueue_close_document(active, false);
            }
            for retained_id in retained {
                let _ = queue.enqueue_close_document(retained_id, true);
            }
        }
        self.sessions.clear();
        self.active_document_path = None;
        self.pending_reconnect_resync = false;
        let theme = self.editor.theme();
        let theme_specifier = self.editor.theme_specifier().to_string();
        let typography = self.editor.typography().clone();
        let behavior = self.editor.document_state().behavior_manifest.clone();
        let ui_theme = self.editor.ui_theme().clone();
        let caret_override = self.editor.caret_style_override();
        let _outgoing = std::mem::take(&mut self.editor);
        self.blank_surface(
            theme,
            theme_specifier,
            typography,
            behavior,
            ui_theme,
            caret_override,
        );
        self.status = EditorStatus::local_fallback().with_document_values(
            self.editor.document_state().document_id,
            self.editor.document_state().document_version,
            self.editor.document_state().access.clone(),
        );
        self.has_opened_document = false;
        self.menu_sync.menu = None;
        self.menu_sync.pending = Some(None);
        self.last_decoration_viewport = None;
        self.requests.reset();
    }

    /// Blank the surface while preserving the shared theme/typography/behavior
    /// (same pattern as the Phase 20 session stash).
    fn blank_surface(
        &mut self,
        theme: crate::editor::theme::StyleRegistry,
        theme_specifier: String,
        typography: crate::editor::typography::TypographyRegistry,
        behavior: Option<BehaviorManifest>,
        ui_theme: crate::shell::ResolvedUiTheme,
        caret_override: Option<crate::protocol::CaretStyle>,
    ) {
        self.editor = EditorSurface::default();
        self.editor.set_theme(theme);
        self.editor.set_theme_specifier(theme_specifier);
        self.editor.set_typography_registry(typography);
        self.editor.set_ui_theme(ui_theme);
        self.editor.set_caret_style_override(caret_override);
        if let Some(manifest) = behavior {
            self.editor.install_behavior_manifest(manifest);
        }
    }

    // -- pending menu push to the connection owner's overlay --

    /// The pending menu session push for the app driver to forward to the
    /// connection owner's overlay (`Some(Some(menu))` show, `Some(None)` clear).
    pub fn take_pending_menu(&mut self) -> Option<Option<TransientMenuSession>> {
        self.menu_sync.take_pending()
    }

    pub fn push_menu(&mut self, menu: Option<TransientMenuSession>) {
        self.menu_sync.push(menu);
    }

    /// Phase 22.4: the pane's active document display name (tab-close
    /// confirm prompt naming), or `None` for a blank view.
    pub fn document_display_name(&self) -> Option<String> {
        self.status.document_display_name.clone()
    }

    fn next_menu_session_id(&self) -> u64 {
        self.menu_sync.next_session_id()
    }

    // -- connection events --

    pub fn apply_connection_event(&mut self, event: ClientConnectionEvent) -> bool {
        match event {
            ClientConnectionEvent::EditAck {
                document_id,
                version,
                ..
            } => {
                if document_id != self.editor.document_state().document_id {
                    if let Some(session) = self.sessions.get_mut(document_id) {
                        let _ = session.surface.note_confirmed_version(document_id, version);
                        session.confirmed_version = version;
                        session
                            .pending
                            .retain(|pending| pending.document_id == document_id);
                    }
                    return false;
                }
                let version_changed = self.editor.note_confirmed_version(document_id, version);
                let mut next_status = EditorStatus::connected(
                    document_id,
                    version,
                    self.editor.document_state().access.clone(),
                );
                next_status.dirty = self.status.dirty;
                next_status.document_display_name = self.status.document_display_name.clone();
                let status_changed = self.set_status(next_status);
                version_changed || status_changed
            }
            ClientConnectionEvent::EditRejected {
                document_id,
                reason,
                ..
            } => self.apply_edit_rejected(document_id, reason),
            ClientConnectionEvent::ResyncSnapshot(snapshot) => {
                if self.has_opened_document
                    && snapshot.document_id != self.editor.document_state().document_id
                {
                    if let Some(session) = self.sessions.get_mut(snapshot.document_id) {
                        session.surface.load_resync_snapshot(
                            snapshot.document_id,
                            snapshot.version,
                            snapshot.text,
                            snapshot.access.clone(),
                        );
                        session.confirmed_version = snapshot.version;
                        session.pending.clear();
                        session.dirty = false;
                    }
                    return false;
                }
                self.editor.load_resync_snapshot(
                    snapshot.document_id,
                    snapshot.version,
                    snapshot.text,
                    snapshot.access.clone(),
                );
                if let Some(queue) = self.edit_queue.as_mut() {
                    queue.update_opened_document_authority(
                        snapshot.document_id,
                        &snapshot.access,
                        snapshot.version,
                    );
                }
                self.clear_sync_recovery_menu();
                let mut next_status = EditorStatus::connected(
                    snapshot.document_id,
                    snapshot.version,
                    snapshot.access,
                );
                next_status.document_display_name = self.status.document_display_name.clone();
                next_status.dirty = false;
                next_status.runtime_diagnostic = None;
                self.set_status(next_status);
                true
            }
            ClientConnectionEvent::DocumentOpened { metadata, text } => {
                self.open_document_session(metadata, text)
            }
            ClientConnectionEvent::DocumentSaved {
                document_id,
                version,
                dirty,
            } => self.apply_document_saved(document_id, version, dirty),
            ClientConnectionEvent::DocumentReloaded { metadata, text } => {
                self.apply_document_reloaded(metadata, text)
            }
            ClientConnectionEvent::FileOperationFailed {
                code,
                message,
                document_id,
                ..
            } => self.apply_file_operation_failed(code, message, document_id),
            ClientConnectionEvent::BehaviorManifestInstalled { manifest, .. } => {
                self.apply_behavior_manifest(&manifest);
                false
            }
            ClientConnectionEvent::ActiveTheme(theme) => {
                self.active_theme = Some(theme.clone());
                self.editor.set_active_theme(&theme);
                true
            }
            ClientConnectionEvent::ActiveTypography(typography) => {
                self.active_typography = Some(typography.clone());
                let changed = self.editor.set_typography(typography);
                self.layout_invalidated |= changed;
                changed
            }
            // Runtime snapshots install the per-document render state directly
            // (also used by the driver's fan-out to non-owner panes).
            ClientConnectionEvent::RuntimeStateSnapshot(snapshot) => {
                self.apply_runtime_snapshot(&snapshot)
            }
            ClientConnectionEvent::DecorationSet(set) => self.editor.apply_decoration_set(set),
            ClientConnectionEvent::DecorationBatch(sets) => {
                let mut changed = false;
                for set in sets {
                    changed |= self.editor.apply_decoration_set(set);
                }
                changed
            }
            ClientConnectionEvent::DiagnosticSet(set) => self.editor.apply_diagnostic_set(set),
            ClientConnectionEvent::CompletionResult(result) => self.apply_completion_result(result),
            ClientConnectionEvent::LanguageIntelligenceResult(result) => {
                self.apply_language_intelligence_result(result)
            }
            ClientConnectionEvent::LanguageIntelligenceRejected { request_id, .. } => {
                if self
                    .requests
                    .take_language_intelligence_if_current(request_id)
                {
                    self.push_menu(None);
                    true
                } else {
                    false
                }
            }
            ClientConnectionEvent::SelectionQueryResult(result) => {
                self.apply_selection_query_result(result)
            }
            ClientConnectionEvent::EditorCommandRequest(request) => {
                self.apply_editor_command_request(request)
            }
            ClientConnectionEvent::CaretStyleOverride(style) => {
                self.editor.set_caret_style_override(style)
            }
            ClientConnectionEvent::CompletionRejected { request_id, .. } => {
                if self.requests.take_completion_if_current(request_id) {
                    self.push_menu(None);
                    true
                } else {
                    false
                }
            }
            ClientConnectionEvent::RuntimeDiagnostic(diagnostic) => {
                let mut next_status = self.status.clone();
                next_status.runtime_diagnostic = Some(diagnostic);
                self.set_status(next_status)
            }
            ClientConnectionEvent::ServerError { code, message } => {
                self.apply_server_error(code, message)
            }
            ClientConnectionEvent::Disconnected => self.apply_disconnect(None),
            ClientConnectionEvent::ConnectionError(message) => {
                self.apply_disconnect(Some(message.as_str()))
            }
            _ => false,
        }
    }

    /// Phase 22.2 per-view manifest layer resolution: install a behavior
    /// manifest only when it governs this view's document.
    ///
    /// - `BehaviorScope::Document` for this view's document: install content
    ///   (keymaps, editor rules, font role) plus the version.
    /// - `BehaviorScope::Document` for another document: version-only bump, so
    ///   outbound edit/completion stamps stay current without importing the
    ///   other mode's content.
    /// - Global scopes: install content plus the version.
    /// - Any manifest whose version is not newer than the installed one is
    ///   ignored (versions are connection-monotonic).
    pub(crate) fn apply_behavior_manifest(&mut self, manifest: &BehaviorManifest) {
        let current = self.editor.document_state().behavior_version;
        if manifest.behavior_version <= current {
            return;
        }
        let installs_content = match manifest.scope {
            crate::protocol::BehaviorScope::Document { document_id } => {
                document_id == self.editor.document_state().document_id
            }
            _ => true,
        };
        if installs_content {
            self.editor.install_behavior_manifest(manifest.clone());
        } else {
            self.editor
                .update_behavior_version(manifest.behavior_version);
        }
    }

    /// Install the runtime parts of one generation snapshot into this view:
    /// behavior manifest, theme, typography, this view's own document's
    /// decorations/diagnostics, and the connection diagnostic. No
    /// acknowledgement (the connection owner acks exactly once).
    pub fn apply_runtime_snapshot(
        &mut self,
        snapshot: &crate::protocol::RuntimeStateSnapshot,
    ) -> bool {
        let mut changed = self.install_runtime_baseline(
            &snapshot.behavior,
            &snapshot.active_theme,
            &snapshot.active_typography,
        );
        for document in &snapshot.documents {
            changed |= self.apply_runtime_document_state(document);
        }
        if let Some(diagnostic) = snapshot.diagnostics.last() {
            changed |= self.apply_runtime_status_diagnostic(diagnostic);
        }
        changed
    }

    pub(crate) fn install_runtime_baseline(
        &mut self,
        behavior: &BehaviorManifest,
        theme: &ActiveTheme,
        typography: &ActiveTypography,
    ) -> bool {
        self.editor.install_behavior_manifest(behavior.clone());
        self.active_theme = Some(theme.clone());
        self.editor.set_active_theme(theme);
        self.active_typography = Some(typography.clone());
        let changed = self.editor.install_runtime_typography(typography.clone());
        self.layout_invalidated |= changed;
        changed
    }

    pub(crate) fn apply_runtime_document_state(
        &mut self,
        document: &crate::protocol::DocumentRuntimeRenderState,
    ) -> bool {
        let open_document_version = self.editor.document_state().document_version;
        if document.document_id != self.editor.document_state().document_id {
            return false;
        }
        let mut changed = false;
        // Phase 22.2: restore this document's own mode layer (its per-document
        // behavior manifest) over the connection-wide baseline installed by
        // `install_runtime_baseline`. The snapshot is authoritative, so the
        // install bypasses the monotonic version gate; the connection-wide
        // version is then restored so outbound stamps stay current.
        if let Some(manifest) = &document.behavior_manifest {
            let connection_version = self.editor.document_state().behavior_version;
            self.editor.install_behavior_manifest(manifest.clone());
            if self.editor.document_state().behavior_version < connection_version {
                self.editor.update_behavior_version(connection_version);
            }
            changed = true;
        }
        if document.reset_decorations {
            self.editor.clear_decorations();
            changed = true;
        }
        if document.reset_diagnostics {
            self.editor.clear_diagnostics();
            changed = true;
        }
        if let Some(set) = document.initial_decorations.clone()
            && set.document_version == open_document_version
        {
            changed |= self.editor.apply_decoration_set(set);
        }
        if let Some(set) = document.initial_diagnostics.clone()
            && set.document_version == open_document_version
        {
            changed |= self.editor.apply_diagnostic_set(set);
        }
        changed
    }

    pub(crate) fn apply_runtime_status_diagnostic(
        &mut self,
        diagnostic: &RuntimeDiagnostic,
    ) -> bool {
        let mut next_status = self.status.clone();
        next_status.runtime_diagnostic = Some(diagnostic.clone());
        self.set_status(next_status)
    }

    fn open_document_session(
        &mut self,
        metadata: crate::protocol::DocumentMetadata,
        text: String,
    ) -> bool {
        let incoming_id = metadata.document_id;
        let active_id = self.editor.document_state().document_id;
        let mut eviction_notice = None;

        if self.has_opened_document && incoming_id == active_id && !self.pending_reconnect_resync {
            // Phase 22.2: duplicate open of this pane's active document is a
            // no-op — the live view keeps caret/selection/pending edits instead
            // of reinstalling the server snapshot over it. (Duplicate opens
            // across panes are detected and focused by the app driver.) The
            // Phase 22.3 reconnect path sets `pending_reconnect_resync` so the
            // re-opened document reinstalls fresh server state.
            return false;
        }
        if self.has_opened_document {
            eviction_notice = self.stash_active_session();
            // Server-authored open replaces any stale retained copy for this id.
            let _ = self.sessions.remove(incoming_id);
        }

        // Preserve shared theme/typography/behavior across document switches.
        let theme = self.editor.theme();
        let theme_specifier = self.editor.theme_specifier().to_string();
        let typography = self.editor.typography().clone();
        let behavior = self.editor.document_state().behavior_manifest.clone();

        self.editor.load_snapshot(
            metadata.document_id,
            metadata.version,
            text,
            metadata.access.clone(),
        );
        self.editor.set_theme(theme);
        self.editor.set_theme_specifier(theme_specifier);
        self.editor.set_typography_registry(typography);
        if let Some(manifest) = behavior {
            self.editor.install_behavior_manifest(manifest);
        }
        self.has_opened_document = true;
        self.active_document_path = Some((metadata.workspace_root_id, metadata.path.clone()));
        self.pending_reconnect_resync = false;

        if let Some(queue) = self.edit_queue.as_mut() {
            queue.update_opened_document_authority(
                metadata.document_id,
                &metadata.access,
                metadata.version,
            );
        }
        let jumped = self
            .take_pending_definition_navigation_for_path(&metadata.path)
            .map(|pending| self.editor.navigate_to_byte_offset(pending.byte_start))
            .unwrap_or(false);
        let display_name =
            crate::editor::accessibility::sanitize_document_display_name(&metadata.path);
        let mut status = EditorStatus::connected_with_metadata(
            metadata.document_id,
            metadata.version,
            metadata.access,
            metadata.dirty,
            Some(display_name),
        );
        if let Some(message) = eviction_notice {
            status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(
                "editor.document_session.evicted",
                message,
            ));
        }
        let status_changed = self.set_status(status);
        let _ = (jumped, status_changed);
        true
    }

    fn stash_active_session(&mut self) -> Option<String> {
        self.editor.cancel_composition();
        let document_id = self.editor.document_state().document_id;
        let sync = self
            .edit_queue
            .as_ref()
            .map(|queue| queue.sync_snapshot_for(document_id))
            .unwrap_or_else(|| ClientSyncSnapshot {
                confirmed_version: self.editor.document_state().document_version,
                optimistic_version: self.editor.document_state().document_version,
                pending: Vec::new(),
                last_resync: None,
            });

        let theme = self.editor.theme();
        let theme_specifier = self.editor.theme_specifier().to_string();
        let typography = self.editor.typography().clone();
        let behavior = self.editor.document_state().behavior_manifest.clone();
        let ui_theme = self.editor.ui_theme().clone();
        let caret_override = self.editor.caret_style_override();
        let outgoing = std::mem::take(&mut self.editor);
        // Leave a blank surface with shared theme/typography/behavior until caller loads.
        self.blank_surface(
            theme,
            theme_specifier,
            typography,
            behavior,
            ui_theme,
            caret_override,
        );

        let (workspace_root_id, path) = self
            .active_document_path
            .clone()
            .unwrap_or((0, String::new()));
        let session = RetainedDocumentSession {
            surface: outgoing,
            dirty: self.status.dirty,
            document_display_name: self.status.document_display_name.clone(),
            confirmed_version: sync.confirmed_version,
            pending: sync.pending,
            last_activated_order: 0,
            workspace_root_id,
            path,
        };
        let eviction = self.sessions.insert(document_id, session);
        // LRU eviction closes the server-side document too (force: the
        // evicted session's unsaved edits are discarded with the session), so
        // document state does not accumulate past the retention budget
        // (Plan 060 T6, P1-4).
        if let Some(queue) = self.edit_queue.as_mut() {
            for evicted_id in &eviction.evicted {
                let _ = queue.enqueue_close_document(*evicted_id, true);
            }
        }
        eviction.notice
    }

    /// Activate a retained session by document id without re-downloading text.
    pub fn activate_document(&mut self, document_id: DocumentId) -> bool {
        if !self.has_opened_document {
            return false;
        }
        if document_id == self.editor.document_state().document_id {
            return false;
        }
        let Some(retained) = self.sessions.remove(document_id) else {
            let mut next_status = self.status.clone();
            next_status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(
                "editor.document_session.missing",
                format!("No retained client session for document {document_id}."),
            ));
            return self.set_status(next_status);
        };

        let _eviction_notice = self.stash_active_session();
        let theme = self.editor.theme();
        let theme_specifier = self.editor.theme_specifier().to_string();
        let typography = self.editor.typography().clone();
        let ui_theme = self.editor.ui_theme().clone();
        let caret_override = self.editor.caret_style_override();
        self.editor = retained.surface;
        self.editor.set_theme(theme);
        self.editor.set_theme_specifier(theme_specifier);
        self.editor.set_typography_registry(typography);
        self.editor.set_ui_theme(ui_theme);
        self.editor.set_caret_style_override(caret_override);
        self.editor.cancel_composition();

        if let Some(queue) = self.edit_queue.as_mut() {
            queue.install_document_sync_state(
                document_id,
                &self.editor.document_state().access,
                retained.confirmed_version,
                retained.pending,
            );
        }

        let status = EditorStatus::connected_with_metadata(
            self.editor.document_state().document_id,
            self.editor.document_state().document_version,
            self.editor.document_state().access.clone(),
            retained.dirty,
            retained.document_display_name,
        );
        let _ = self.set_status(status);
        true
    }

    /// Phase 22.2: the open-documents menu of the focused pane. Lists the
    /// focused pane's own sessions (active + retained, as before) plus every
    /// other pane's open documents and retained sessions (labeled by pane) so
    /// the menu reflects the whole workspace. Activating an entry owned by
    /// another pane routes through the app driver, which switches the owning
    /// pane to that document and focuses it (duplicate opens stay blocked).
    pub fn show_open_documents_menu(&mut self, other_panes: &[CrossPaneDocumentEntry]) -> bool {
        if !self.has_opened_document {
            return false;
        }
        // Own entries first (pane `None`), then one entry per other pane's
        // active document and retained sessions, labeled by pane. The owning
        // pane is encoded in each cross-pane action so the driver can route
        // activation back to the pane that holds the session.
        let mut entries: Vec<(
            Option<crate::shell::PaneId>,
            crate::editor::document_session::SessionListEntry,
        )> = self
            .sessions
            .list_with_active(
                self.editor.document_state().document_id,
                self.status.document_display_name.as_deref(),
                self.status.dirty,
            )
            .into_iter()
            .map(|entry| (None, entry))
            .collect();
        for other in other_panes {
            let push = |entries: &mut Vec<(
                Option<crate::shell::PaneId>,
                crate::editor::document_session::SessionListEntry,
            )>,
                        pane: crate::shell::PaneId,
                        document_id: DocumentId,
                        display_name: String,
                        dirty: bool| {
                entries.push((
                    Some(pane),
                    crate::editor::document_session::SessionListEntry {
                        document_id,
                        display_name: format!("pane {}: {display_name}", pane.0),
                        dirty,
                        active: false,
                    },
                ));
            };
            push(
                &mut entries,
                other.pane,
                other.document_id,
                other.display_name.clone(),
                other.dirty,
            );
            for (document_id, display_name, dirty) in &other.retained {
                push(
                    &mut entries,
                    other.pane,
                    *document_id,
                    display_name.clone(),
                    *dirty,
                );
            }
        }
        let session_id = self.next_menu_session_id();
        let items = entries
            .into_iter()
            .map(|(pane, entry)| {
                let mut label = entry.display_name.clone();
                if entry.dirty {
                    label.push_str(" •");
                }
                if entry.active {
                    label.push_str(" (active)");
                }
                let mut arguments = serde_json::json!({ "documentId": entry.document_id });
                if let Some(pane) = pane {
                    arguments["paneId"] = serde_json::json!(pane.0);
                }
                let action = TransientMenuAction::new("editor.clientActivateDocument")
                    .with_arguments(arguments);
                crate::shell::TransientMenuItem::new(
                    format!("doc.{}", entry.document_id),
                    label,
                    action,
                )
                .with_accessibility_label(format!(
                    "{}{}{}",
                    entry.display_name,
                    if entry.dirty { ", dirty" } else { "" },
                    if entry.active { ", active" } else { "" }
                ))
            })
            .collect::<Vec<_>>();
        let menu = TransientMenuSession::new(TransientMenuSessionId(session_id), "Open documents")
            .with_items(items);
        self.push_menu(Some(menu));
        true
    }

    /// Phase 22.2: active document summary for cross-pane menu aggregation.
    pub fn active_document_info(&self) -> Option<(DocumentId, String, bool)> {
        self.has_opened_document.then(|| {
            (
                self.editor.document_state().document_id,
                self.status
                    .document_display_name
                    .clone()
                    .unwrap_or_else(|| "untitled".to_string()),
                self.status.dirty,
            )
        })
    }

    /// Phase 22.2: retained sessions for cross-pane menu aggregation.
    pub fn retained_documents(&self) -> Vec<(DocumentId, String, bool)> {
        self.sessions
            .list_retained()
            .into_iter()
            .map(|entry| (entry.document_id, entry.display_name, entry.dirty))
            .collect()
    }

    fn apply_document_saved(
        &mut self,
        document_id: DocumentId,
        version: DocumentVersion,
        dirty: bool,
    ) -> bool {
        if document_id != self.editor.document_state().document_id {
            if let Some(session) = self.sessions.get_mut(document_id) {
                let _ = session.surface.note_confirmed_version(document_id, version);
                session.confirmed_version = version;
                session.dirty = dirty;
            }
            return false;
        }
        let version_changed = self.editor.note_confirmed_version(document_id, version);
        let mut next_status = self.status.clone();
        next_status.version = Some(version);
        next_status.dirty = dirty;
        if !dirty {
            // Successful clean save clears stale conflict diagnostics.
            if next_status
                .runtime_diagnostic
                .as_ref()
                .is_some_and(|diagnostic| {
                    diagnostic.code.contains("StaleFileMetadata")
                        || diagnostic.code.contains("DirtyDocument")
                        || diagnostic.code.contains("file.")
                })
            {
                next_status.runtime_diagnostic = None;
            }
            if self.menu_sync.menu.as_ref().is_some_and(|menu| {
                let prompt = menu.prompt();
                prompt.contains("conflict")
                    || prompt.contains("unsaved edits")
                    || prompt.contains("Reload")
                    || prompt.contains("reload")
            }) {
                self.push_menu(None);
            }
        }
        version_changed || self.set_status(next_status)
    }

    fn apply_document_reloaded(
        &mut self,
        metadata: crate::protocol::DocumentMetadata,
        text: String,
    ) -> bool {
        if metadata.document_id != self.editor.document_state().document_id {
            if let Some(session) = self.sessions.get_mut(metadata.document_id) {
                session.surface.load_resync_snapshot(
                    metadata.document_id,
                    metadata.version,
                    text,
                    metadata.access.clone(),
                );
                session.confirmed_version = metadata.version;
                session.pending.clear();
                session.dirty = metadata.dirty;
                session.document_display_name = Some(
                    crate::editor::accessibility::sanitize_document_display_name(&metadata.path),
                );
            }
            return false;
        }
        self.editor.load_resync_snapshot(
            metadata.document_id,
            metadata.version,
            text,
            metadata.access.clone(),
        );
        if let Some(queue) = self.edit_queue.as_mut() {
            queue.update_opened_document_authority(
                metadata.document_id,
                &metadata.access,
                metadata.version,
            );
        }
        self.push_menu(None);
        let display_name =
            crate::editor::accessibility::sanitize_document_display_name(&metadata.path);
        let mut next_status = EditorStatus::connected_with_metadata(
            metadata.document_id,
            metadata.version,
            metadata.access,
            metadata.dirty,
            Some(display_name),
        );
        next_status.runtime_diagnostic = None;
        self.set_status(next_status);
        true
    }

    fn apply_file_operation_failed(
        &mut self,
        code: FileErrorCode,
        message: String,
        document_id: Option<DocumentId>,
    ) -> bool {
        let mut next_status = self.status.clone();
        next_status.runtime_diagnostic = Some(RuntimeDiagnostic::error(
            format!("file.{code:?}"),
            message.clone(),
        ));
        // Save/reload failures must never clear dirty; keep local edits.
        let status_changed = self.set_status(next_status);
        let targets_active =
            document_id.is_none_or(|id| id == self.editor.document_state().document_id);
        let opened_conflict_menu = targets_active
            && matches!(
                code,
                FileErrorCode::StaleFileMetadata | FileErrorCode::DirtyDocument
            )
            && self.show_save_conflict_menu(code, &message);
        status_changed || opened_conflict_menu
    }

    fn show_save_conflict_menu(&mut self, code: FileErrorCode, message: &str) -> bool {
        if !self.has_opened_document {
            return false;
        }
        let session_id = self.next_menu_session_id();
        let (prompt, items) = match code {
            FileErrorCode::StaleFileMetadata => {
                let prompt = "File changed on disk — resolve save conflict".to_string();
                let items = vec![
                    crate::shell::TransientMenuItem::new(
                        "conflict.reload",
                        "Reload from disk (discard local edits)",
                        TransientMenuAction::new("documents.serverReloadDocument")
                            .with_arguments(serde_json::json!({ "force": true })),
                    )
                    .with_accessibility_label("Reload from disk and discard unsaved local edits"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.keep",
                        "Keep unsaved edits",
                        TransientMenuAction::new("editor.clientKeepUnsavedEdits"),
                    )
                    .with_accessibility_label("Keep unsaved edits and dismiss conflict menu"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.defer",
                        "Compare later",
                        TransientMenuAction::new("editor.clientDeferConflictCompare"),
                    )
                    .with_accessibility_label("Defer conflict comparison and keep unsaved edits"),
                ];
                (prompt, items)
            }
            FileErrorCode::DirtyDocument => {
                let prompt = "Document has unsaved edits — resolve reload".to_string();
                let items = vec![
                    crate::shell::TransientMenuItem::new(
                        "conflict.save",
                        "Save first",
                        TransientMenuAction::new("documents.serverSaveDocument"),
                    )
                    .with_accessibility_label("Save the document before reloading"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.reload",
                        "Discard edits and reload",
                        TransientMenuAction::new("documents.serverReloadDocument")
                            .with_arguments(serde_json::json!({ "force": true })),
                    )
                    .with_accessibility_label("Discard unsaved edits and reload from disk"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.keep",
                        "Keep unsaved edits",
                        TransientMenuAction::new("editor.clientKeepUnsavedEdits"),
                    )
                    .with_accessibility_label("Keep unsaved edits and dismiss reload prompt"),
                ];
                (prompt, items)
            }
            _ => return false,
        };
        let _ = message;
        let menu =
            TransientMenuSession::new(TransientMenuSessionId(session_id), prompt).with_items(items);
        self.push_menu(Some(menu));
        true
    }

    pub fn request_save_active_document(&self) -> Result<DocumentId, RuntimeDiagnostic> {
        let Some(queue) = &self.edit_queue else {
            return Err(RuntimeDiagnostic::error(
                "client.save.unavailable",
                "Cannot save because this editor is not connected to a Clay server.",
            ));
        };
        if !self.has_opened_document {
            return Err(RuntimeDiagnostic::error(
                "client.save.no_document",
                "Cannot save because no document is open.",
            ));
        }
        let document = self.editor.document_state();
        queue
            .enqueue_save_document(document.document_id, document.document_version)
            .map(|()| document.document_id)
            .map_err(|error| {
                RuntimeDiagnostic::error(
                    "client.save.queue_failed",
                    format!("Failed to send save request to the Clay server: {error}"),
                )
            })
    }

    fn request_reload_active_document(&self, force: bool) -> Option<ClientConnectionEvent> {
        let Some(queue) = &self.edit_queue else {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "client.reload.unavailable",
                    "Cannot reload because this editor is not connected to a Clay server.",
                ),
            ));
        };
        if !self.has_opened_document {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "client.reload.no_document",
                    "Cannot reload because no document is open.",
                ),
            ));
        }
        let document = self.editor.document_state();
        queue
            .enqueue_reload_document(document.document_id, document.document_version, force)
            .err()
            .map(|error| {
                ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                    "client.reload.queue_failed",
                    format!("Failed to send reload request to the Clay server: {error}"),
                ))
            })
    }

    fn handle_save_conflict_menu_action(&mut self, action: &TransientMenuAction) -> bool {
        match action.command_id.as_str() {
            "documents.serverSaveDocument" => {
                if let Err(diagnostic) = self.request_save_active_document() {
                    let _ = self.apply_connection_event(ClientConnectionEvent::RuntimeDiagnostic(
                        diagnostic,
                    ));
                }
                true
            }
            "documents.serverReloadDocument" => {
                let force = action
                    .arguments
                    .get("force")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if let Some(diagnostic) = self.request_reload_active_document(force) {
                    let _ = self.apply_connection_event(diagnostic);
                }
                true
            }
            "editor.clientKeepUnsavedEdits" => true,
            "editor.clientDeferConflictCompare" => {
                let mut next_status = self.status.clone();
                next_status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(
                    "file.conflict_deferred",
                    "Save conflict deferred — unsaved edits kept; compare later.",
                ));
                let _ = self.set_status(next_status);
                true
            }
            _ => false,
        }
    }

    fn apply_edit_rejected(&mut self, document_id: DocumentId, reason: EditRejection) -> bool {
        if document_id != self.editor.document_state().document_id {
            return false;
        }
        let code = edit_rejection_diagnostic_code(&reason);
        let auto_resync = edit_rejection_requests_resync(&reason);
        let message = if auto_resync {
            format!("{} — requesting resync", edit_rejection_summary(&reason))
        } else {
            format!(
                "{} — choose Resync or Dismiss",
                edit_rejection_summary(&reason)
            )
        };
        let mut next_status = self.status.clone();
        next_status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(code, message));
        let status_changed = self.set_status(next_status);
        let opened_menu = !auto_resync && self.show_edit_rejection_recovery_menu(&reason);
        status_changed || opened_menu
    }

    fn apply_disconnect(&mut self, error_message: Option<&str>) -> bool {
        // Omit raw transport strings from chrome — they may include host paths/endpoints.
        let _ = error_message;
        let mut next_status = EditorStatus {
            connection: EditorConnectionStatus::Disconnected,
            ..self.status.clone().with_document_values(
                self.editor.document_state().document_id,
                self.editor.document_state().document_version,
                self.editor.document_state().access.clone(),
            )
        };
        next_status.runtime_diagnostic = Some(RuntimeDiagnostic::error(
            "client.disconnect",
            "Disconnected (connection lost). Reconnecting…; local unsaved edits stay in this window until then.",
        ));
        let status_changed = self.set_status(next_status);
        let opened_menu = self.show_disconnect_recovery_menu();
        status_changed || opened_menu
    }

    fn apply_server_error(&mut self, code: ProtocolErrorCode, message: String) -> bool {
        let sanitized = crate::editor::accessibility::sanitize_recovery_summary(&message)
            .unwrap_or_else(|| "server error".to_string());
        let mut next_status = self.status.clone();
        next_status.runtime_diagnostic = Some(RuntimeDiagnostic::error(
            format!("server.error.{code:?}"),
            format!("{sanitized}. Use Resync or Dismiss."),
        ));
        let status_changed = self.set_status(next_status);
        let opened_menu = self.show_sync_recovery_menu(
            "Server error — recover sync",
            "Server reported an error. Request a resync or dismiss this prompt.",
        );
        status_changed || opened_menu
    }

    fn show_edit_rejection_recovery_menu(&mut self, reason: &EditRejection) -> bool {
        let prompt = format!(
            "Edit rejected ({}) — recover sync",
            edit_rejection_label(reason)
        );
        self.show_sync_recovery_menu(
            &prompt,
            "Request a canonical resync or dismiss this recovery prompt.",
        )
    }

    fn show_disconnect_recovery_menu(&mut self) -> bool {
        if !self.has_opened_document && self.sessions.is_empty() {
            return false;
        }
        let session_id = self.next_menu_session_id();
        let items = vec![
            crate::shell::TransientMenuItem::new(
                "recovery.dismiss",
                "Dismiss",
                TransientMenuAction::new("editor.clientDismissRecovery"),
            )
            .with_accessibility_label("Dismiss disconnect recovery guidance"),
        ];
        let menu = TransientMenuSession::new(
            TransientMenuSessionId(session_id),
            "Disconnected — reconnect guidance",
        )
        .with_items(items);
        self.push_menu(Some(menu));
        true
    }

    fn show_sync_recovery_menu(&mut self, prompt: &str, accessibility_hint: &str) -> bool {
        if !self.has_opened_document {
            return false;
        }
        let session_id = self.next_menu_session_id();
        let items = vec![
            crate::shell::TransientMenuItem::new(
                "recovery.resync",
                "Request resync",
                TransientMenuAction::new("editor.clientRequestResync"),
            )
            .with_accessibility_label(format!("{accessibility_hint} Request resync")),
            crate::shell::TransientMenuItem::new(
                "recovery.dismiss",
                "Dismiss",
                TransientMenuAction::new("editor.clientDismissRecovery"),
            )
            .with_accessibility_label("Dismiss recovery prompt"),
        ];
        let menu =
            TransientMenuSession::new(TransientMenuSessionId(session_id), prompt).with_items(items);
        self.push_menu(Some(menu));
        true
    }

    fn clear_sync_recovery_menu(&mut self) {
        if let Some(menu) = self.menu_sync.menu.as_ref()
            && menu.is_active()
        {
            let prompt = menu.prompt().to_ascii_lowercase();
            if prompt.contains("recover")
                || prompt.contains("rejected")
                || prompt.contains("disconnected")
                || prompt.contains("server error")
            {
                self.push_menu(None);
            }
        }
    }

    pub fn request_resync_active_document(&mut self) -> Option<ClientConnectionEvent> {
        let Some(queue) = &self.edit_queue else {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "editor.resync_unavailable",
                    "Resync requires an active server connection.",
                ),
            ));
        };
        if matches!(
            self.status.connection,
            EditorConnectionStatus::Disconnected | EditorConnectionStatus::LocalFallback
        ) {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "editor.resync_unavailable",
                    "Resync requires an active server connection. The tab reconnects automatically.",
                ),
            ));
        }
        let document = self.editor.document_state();
        if let Err(error) =
            queue.enqueue_request_resync(document.document_id, document.document_version)
        {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "editor.resync_enqueue_failed",
                    format!("Failed to request resync: {error}"),
                ),
            ));
        }
        let mut next_status = self.status.clone();
        next_status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(
            "editor.resync_requested",
            "Resync requested — waiting for canonical snapshot.",
        ));
        let _ = self.set_status(next_status);
        None
    }

    pub fn dismiss_recovery(&mut self) -> bool {
        let cleared_menu = self
            .menu_sync
            .menu
            .as_ref()
            .is_some_and(|menu| menu.is_active());
        self.push_menu(None);
        let mut next_status = self.status.clone();
        let cleared_diagnostic = next_status.runtime_diagnostic.take().is_some();
        let status_changed = self.set_status(next_status);
        cleared_menu || status_changed || cleared_diagnostic
    }

    fn handle_sync_recovery_menu_action(&mut self, action: &TransientMenuAction) -> bool {
        match action.command_id.as_str() {
            "editor.clientRequestResync" => {
                if let Some(diagnostic) = self.request_resync_active_document() {
                    let _ = self.apply_connection_event(diagnostic);
                }
                true
            }
            "editor.clientDismissRecovery" => self.dismiss_recovery(),
            _ => false,
        }
    }

    pub fn retained_session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn status_text(&self) -> String {
        self.status_observation().status_text
    }

    pub(crate) fn status_observation(&self) -> SduiStatusObservation {
        let mut observation = self.status.observation();
        observation.theme_label = self.editor.theme_label();
        observation.composing = self.editor.is_composing();
        observation.pending_edit_count = self
            .edit_queue
            .as_ref()
            .map(|queue| queue.sync_snapshot_for(self.document_id()).pending.len())
            .unwrap_or(0);
        observation.recovery_summary = self.recovery_summary();
        let open_count = self
            .sessions
            .len()
            .saturating_add(usize::from(self.has_opened_document));
        if open_count > 1 && !observation.status_text.contains("Open docs:") {
            observation
                .status_text
                .push_str(&format!(" — Open docs: {open_count}"));
        }
        if let Some(pending) =
            crate::editor::accessibility::pending_edits_summary(observation.pending_edit_count)
            && !observation.status_text.contains("Pending edits:")
        {
            observation.status_text.push_str(&format!(" — {pending}"));
        }
        if let Some(recovery) = observation.recovery_summary.as_deref()
            && !observation.status_text.contains("Recovery:")
        {
            observation
                .status_text
                .push_str(&format!(" — Recovery: {recovery}"));
        }
        observation
    }

    fn recovery_summary(&self) -> Option<String> {
        if let Some(menu) = self.menu_sync.menu.as_ref()
            && menu.is_active()
            && let Some(summary) =
                crate::editor::accessibility::sanitize_recovery_summary(menu.prompt())
        {
            return Some(summary);
        }
        if let Some(diagnostic) = self.status.runtime_diagnostic.as_ref() {
            let code = diagnostic.code.as_str();
            if code.contains("DirtyDocument")
                || code.contains("StaleFileMetadata")
                || code.contains("file.")
                || code.contains("clipboard")
                || code.contains("resync")
                || code.contains("disconnect")
                || code.contains("edit.rejected")
                || code.contains("server.error")
            {
                return crate::editor::accessibility::sanitize_recovery_summary(
                    &diagnostic.message,
                );
            }
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn visible_text_for_test(&self) -> String {
        self.editor.visible_text()
    }

    pub fn decoration_span_count(&self) -> usize {
        self.editor.decoration_span_count()
    }

    pub fn diagnostic_span_count(&self) -> usize {
        self.editor.diagnostic_span_count()
    }

    pub fn copy_selection_to_system_clipboard(&self) -> Option<ClientConnectionEvent> {
        let mut clipboard = SystemClipboard;
        self.copy_selection_to_clipboard_with(&mut clipboard)
    }

    pub(crate) fn copy_selection_to_clipboard_with(
        &self,
        clipboard: &mut impl ClipboardSink,
    ) -> Option<ClientConnectionEvent> {
        let text = self.editor.selected_text()?;
        clipboard.set_text(text).err().map(|error| {
            ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                "client.clipboard.write_failed",
                format!("Failed to copy selection to the system clipboard: {error}"),
            ))
        })
    }

    pub fn cut_selection_to_system_clipboard(&mut self) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let mut clipboard = SystemClipboard;
        self.cut_selection_to_clipboard_with(&mut clipboard)
    }

    pub(crate) fn cut_selection_to_clipboard_with(
        &mut self,
        clipboard: &mut impl ClipboardSink,
    ) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let Some(text) = self.editor.selected_text() else {
            return ClipboardCommandOutcome::unchanged();
        };
        if let Err(error) = clipboard.set_text(text) {
            return ClipboardCommandOutcome::diagnostic(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "client.clipboard.write_failed",
                    format!("Failed to cut selection to the system clipboard: {error}"),
                ),
            ));
        }
        let outcome = self.editor.command_with_event(EditorCommand::DeleteForward);
        ClipboardCommandOutcome {
            changed: self.apply_local_edit_outcome(outcome),
            diagnostic: None,
        }
    }

    pub fn paste_from_system_clipboard(&mut self) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let mut clipboard = SystemClipboard;
        self.paste_from_clipboard_with(&mut clipboard)
    }

    /// Paste already-fetched clipboard text (Masonry `TextEvent::ClipboardPaste`).
    pub(crate) fn paste_provided_clipboard_text(&mut self, text: &str) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let outcome = self.editor.paste_text_with_event(text);
        ClipboardCommandOutcome {
            changed: self.apply_local_edit_outcome(outcome),
            diagnostic: None,
        }
    }

    pub(crate) fn paste_from_clipboard_with(
        &mut self,
        clipboard: &mut impl ClipboardSink,
    ) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let text = match clipboard.get_text() {
            Ok(text) => text,
            Err(error) => {
                return ClipboardCommandOutcome::diagnostic(
                    ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                        "client.clipboard.read_failed",
                        format!("Failed to read text from the system clipboard: {error}"),
                    )),
                );
            }
        };
        self.paste_provided_clipboard_text(&text)
    }

    pub fn undo(&mut self) -> bool {
        let cancelled = self.editor.is_composing();
        let outcome = self.editor.undo_with_event();
        self.apply_local_edit_outcome(outcome) || cancelled
    }

    pub fn redo(&mut self) -> bool {
        let cancelled = self.editor.is_composing();
        let outcome = self.editor.redo_with_event();
        self.apply_local_edit_outcome(outcome) || cancelled
    }

    pub(crate) fn apply_editor_command_request(&mut self, request: EditorCommandRequest) -> bool {
        if !request.validate() {
            return false;
        }
        if let Some(query) = crate::protocol::SelectionQuery::from_command_id(&request.command_id) {
            if let Some(event) = self.editor.selection_query_request_for(query) {
                self.enqueue_selection_query_request(event);
                return true;
            }
            return false;
        }
        let Some(command) = EditorClientCommand::from_command_id(&request.command_id) else {
            return false;
        };
        self.apply_editor_client_command(command)
    }

    pub fn apply_editor_client_command(&mut self, command: EditorClientCommand) -> bool {
        let editor_command = match command {
            EditorClientCommand::MoveWordStartForward => EditorCommand::MoveWordStart {
                forward: true,
                long: false,
                extend: false,
            },
            EditorClientCommand::MoveWordStartBackward => EditorCommand::MoveWordStart {
                forward: false,
                long: false,
                extend: false,
            },
            EditorClientCommand::MoveParagraphForward => EditorCommand::MoveParagraph {
                forward: true,
                to_end: false,
                extend: false,
            },
            EditorClientCommand::MoveParagraphBackward => EditorCommand::MoveParagraph {
                forward: false,
                to_end: false,
                extend: false,
            },
            EditorClientCommand::SelectWord => EditorCommand::SelectWord,
            EditorClientCommand::SelectLine => EditorCommand::SelectLine,
            EditorClientCommand::AddCursorBelow => EditorCommand::AddCursor {
                direction: CursorSelectDirection::Down,
            },
            EditorClientCommand::AddCursorAbove => EditorCommand::AddCursor {
                direction: CursorSelectDirection::Up,
            },
            EditorClientCommand::ColumnSelectDown => EditorCommand::ColumnSelect {
                direction: CursorSelectDirection::Down,
            },
            EditorClientCommand::ColumnSelectUp => EditorCommand::ColumnSelect {
                direction: CursorSelectDirection::Up,
            },
            EditorClientCommand::ColumnSelectLeft => EditorCommand::ColumnSelect {
                direction: CursorSelectDirection::Left,
            },
            EditorClientCommand::ColumnSelectRight => EditorCommand::ColumnSelect {
                direction: CursorSelectDirection::Right,
            },
            EditorClientCommand::SelectNextMatch => EditorCommand::SelectNextMatch,
            EditorClientCommand::SelectPrevMatch => EditorCommand::SelectPrevMatch,
            EditorClientCommand::SelectAllMatches => EditorCommand::SelectAllMatches,
            EditorClientCommand::CancelMultipleSelections => {
                EditorCommand::CancelMultipleSelections
            }
            EditorClientCommand::KeepSelection => EditorCommand::KeepSelection,
            EditorClientCommand::RemoveSelection => EditorCommand::RemoveSelection,
            EditorClientCommand::UndoCursorMove => EditorCommand::UndoCursorMove,
        };
        self.editor.command(editor_command)
    }

    /// Discard unfinished IME preedit without committing.
    pub fn cancel_composition(&mut self) -> bool {
        self.editor.cancel_composition()
    }

    fn sync_ime_area(&self, ctx: &mut EventCtx<'_>) {
        let rect = self.editor_rect;
        let local = self.editor.ime_cursor_area(rect.width(), rect.height());
        ctx.set_ime_area(Rect::new(
            rect.x0 + local.x0,
            rect.y0 + local.y0,
            rect.x0 + local.x1,
            rect.y0 + local.y1,
        ));
    }

    fn apply_local_edit_outcome(&mut self, outcome: EditorCommandOutcome) -> bool {
        if let Some(edit_queue) = &self.edit_queue {
            for event in outcome.edit_events {
                let transaction_id = self.requests.next_transaction_id();
                let _ = edit_queue.enqueue_edit_event(event, transaction_id);
            }
        }
        if outcome.changed {
            if !self.status.dirty {
                self.status.dirty = true;
            }
            self.requests.clear_active();
            self.push_menu(None);
            self.enqueue_decoration_viewport_request();
        }
        outcome.changed
    }

    fn set_status(&mut self, status: EditorStatus) -> bool {
        if self.status == status {
            return false;
        }
        self.status = status;
        true
    }

    fn apply_completion_result(&mut self, result: CompletionResultSet) -> bool {
        if self.requests.active_completion_request_id != Some(result.request_id) {
            return false;
        }
        let document = self.editor.document_state();
        if result.document_id != document.document_id
            || result.document_version != document.document_version
            || result.behavior_version != document.behavior_version
        {
            return false;
        }
        self.push_menu(Some(completion_result_to_menu_session(&result)));
        true
    }

    fn apply_language_intelligence_result(&mut self, result: LanguageIntelligenceResult) -> bool {
        if self.requests.active_language_intelligence_request_id != Some(result.request_id) {
            return false;
        }
        let document = self.editor.document_state();
        if result.document_id != document.document_id
            || result.document_version != document.document_version
            || result.behavior_version != document.behavior_version
        {
            return false;
        }
        self.push_menu(Some(language_intelligence_result_to_menu_session(&result)));
        true
    }

    fn take_pending_definition_navigation_for_path(
        &mut self,
        path: &str,
    ) -> Option<PendingDefinitionNavigation> {
        let pending = self.pending_definition_navigation.as_ref()?;
        let matches = path == pending.relative_path
            || path.ends_with(&pending.relative_path)
            || path
                .replace('\\', "/")
                .ends_with(&pending.relative_path.replace('\\', "/"));
        if matches {
            self.pending_definition_navigation.take()
        } else {
            None
        }
    }

    fn local_command(&mut self, ctx: &mut EventCtx<'_>, command: EditorCommand<'_>) {
        let _ = self.editor.cancel_composition();
        let outcome = self.editor.command_with_event(command);
        if let Some(edit_queue) = &self.edit_queue {
            for event in outcome.edit_events {
                let transaction_id = self.requests.next_transaction_id();
                let _ = edit_queue.enqueue_edit_event(event, transaction_id);
            }
        }
        if outcome.changed {
            self.requests.clear_active();
            self.push_menu(None);
            self.enqueue_decoration_viewport_request();
            ctx.request_render();
            ctx.request_accessibility_update();
        }
        ctx.set_handled();
    }

    fn local_key(&mut self, ctx: &mut EventCtx<'_>, key: KeyStroke) {
        if self.route_menu_key(ctx, &key) {
            // The menu's local state (selection/query/cancel) may have changed
            // via the keyboard; re-sync the hosted overlay through the action
            // loop (the reconcile needs a `MutateCtx` `EventCtx` can't reach).
            ctx.submit_action::<EditorAction>(EditorAction::MenuStateChanged);
            return;
        }
        let outcome = self.editor.route_key_with_event(&key);
        let changed = outcome.command_outcome.changed;
        self.finish_local_outcome(ctx, outcome.command_outcome);
        if let Some(completion) = outcome.completion_request {
            self.enqueue_completion_request(completion);
            ctx.set_handled();
        } else if let Some(language_intelligence) = outcome.language_intelligence_request {
            self.enqueue_language_intelligence_request(language_intelligence);
            ctx.set_handled();
        } else if changed {
            self.requests.clear_active();
            self.push_menu(None);
        }
        if let Some(command) = outcome.client_ui_command {
            ctx.submit_action::<EditorAction>(EditorAction::ClientUiCommand(command));
            ctx.set_handled();
        } else if let Some(intent) = outcome.server_intent {
            if let Some(feature) =
                crate::client::behavior::language_intelligence_feature_for_command(
                    &intent.command_id,
                )
            {
                if let Some(event) = self
                    .editor
                    .language_intelligence_request_for_feature(feature)
                {
                    self.enqueue_language_intelligence_request(event);
                }
            } else if intent.command_id == "documents.serverSaveDocument" {
                if let Err(diagnostic) = self.request_save_active_document() {
                    let _ = self.apply_connection_event(ClientConnectionEvent::RuntimeDiagnostic(
                        diagnostic,
                    ));
                    ctx.request_render();
                }
            } else if intent.command_id == "documents.serverReloadDocument" {
                if let Some(diagnostic) = self.request_reload_active_document(false) {
                    let _ = self.apply_connection_event(diagnostic);
                    ctx.request_render();
                }
            } else if let Some(query) =
                crate::protocol::SelectionQuery::from_command_id(&intent.command_id)
            {
                // Plan 071 task 10: text-object/smart-select commands capture
                // the selection set locally and query the server read-only.
                if let Some(event) = self.editor.selection_query_request_for(query) {
                    self.enqueue_selection_query_request(event);
                }
            } else if let Some(edit_queue) = &self.edit_queue {
                let document = self.editor.document_state();
                let _ = edit_queue.enqueue_command_intent(
                    document.document_id,
                    document.behavior_version,
                    intent.command_id,
                );
            }
            ctx.set_handled();
        }
    }

    /// Routes keys to the active transient menu when one exists. Returns `true`
    /// if the key was consumed by the menu, keeping editor hot paths free of
    /// command execution or IPC work.
    fn route_menu_key(&mut self, ctx: &mut EventCtx<'_>, key: &KeyStroke) -> bool {
        if self.menu_sync.menu.is_none() {
            return false;
        }
        match key.key {
            KeyCode::ArrowUp => {
                self.menu_select_previous();
                ctx.request_render();
                ctx.set_handled();
                true
            }
            KeyCode::ArrowDown => {
                self.menu_select_next();
                ctx.request_render();
                ctx.set_handled();
                true
            }
            KeyCode::Enter | KeyCode::Tab => {
                self.activate_menu_selection(ctx, None);
                true
            }
            KeyCode::Escape => {
                self.menu_cancel();
                self.push_menu(None);
                self.requests.clear_active();
                ctx.request_render();
                ctx.set_handled();
                true
            }
            KeyCode::Character(ref text) => {
                let Some(completion) = self.menu_activate_completion() else {
                    return false;
                };
                if completion.commit_characters.contains(text) {
                    self.activate_menu_selection(ctx, Some(text));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn activate_menu_selection(&mut self, ctx: &mut EventCtx<'_>, commit_character: Option<&str>) {
        if let Some(completion) = self.menu_activate_completion() {
            let outcome = self
                .editor
                .accept_completion_with_event(&completion, commit_character);
            self.finish_local_outcome(ctx, outcome);
            self.requests.active_completion_request_id = None;
        } else if let Some(local_action) = self.menu_selected_action() {
            if local_action.command_id.starts_with("shell.clientTabClose") {
                // Phase 22.4: the tab-close confirm session is driver-owned
                // orchestration (save all dirty panes then close, or discard,
                // or cancel) — hand the selection back to the driver, which
                // owns the tab's queues and the `TabCommand::Close` path. The
                // `clientId` argument distinguishes the tab; the command IDs
                // are driver-local (never declared, never server-routed), so
                // tab-confirm and per-view save-conflict sessions cannot
                // cross-route.
                let _ = self.menu_activate_selected();
                let client_id = local_action
                    .arguments
                    .get("clientId")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                ctx.submit_action::<EditorAction>(EditorAction::TabCloseMenuAction {
                    client_id,
                    command_id: local_action.command_id.clone(),
                });
            } else if local_action.command_id == "editor.clientActivateDocument" {
                let document_id = local_action
                    .arguments
                    .get("documentId")
                    .and_then(|value| value.as_u64());
                let _ = self.menu_activate_selected();
                if let Some(document_id) = document_id {
                    if self.contains_document(document_id) {
                        // Own session: local switch (stashes the prior
                        // session as before).
                        let _ = self.activate_document(document_id);
                    } else if let Some(pane_id) = local_action
                        .arguments
                        .get("paneId")
                        .and_then(|value| value.as_u64())
                    {
                        // Another pane's document: hand the activation to the
                        // app driver, which switches the owning pane to this
                        // document and focuses it (no duplicate views).
                        ctx.submit_action::<EditorAction>(EditorAction::ActivateDocumentInPane {
                            document_id,
                            pane_id: crate::shell::PaneId(pane_id),
                        });
                    }
                }
            } else if self.handle_save_conflict_menu_action(&local_action)
                || self.handle_sync_recovery_menu_action(&local_action)
            {
                let _ = self.menu_activate_selected();
                ctx.request_render();
            } else if self.handle_language_intelligence_menu_action(&local_action) {
                let _ = self.menu_activate_selected();
            } else if let Some(feature) =
                crate::client::behavior::language_intelligence_feature_for_command(
                    &local_action.command_id,
                )
            {
                let _ = self.menu_activate_selected();
                if let Some(event) = self
                    .editor
                    .language_intelligence_request_for_feature(feature)
                {
                    self.enqueue_language_intelligence_request(event);
                }
            } else if let Some(intent) = self.menu_activate_selected()
                && let Some(edit_queue) = &self.edit_queue
            {
                if intent.command_id == "workspace.openFile"
                    && intent.arguments.iter().any(|arg| {
                        arg.name == "languageIntelligenceNavigation"
                            && matches!(arg.value, crate::protocol::SduiActionValue::Bool(true))
                    })
                {
                    let relative_path =
                        intent
                            .arguments
                            .iter()
                            .find_map(|arg| match (&arg.name, &arg.value) {
                                (name, crate::protocol::SduiActionValue::String(value))
                                    if name == "relativePath" =>
                                {
                                    Some(value.clone())
                                }
                                _ => None,
                            });
                    let byte_start =
                        intent
                            .arguments
                            .iter()
                            .find_map(|arg| match (&arg.name, &arg.value) {
                                (name, crate::protocol::SduiActionValue::U64(value))
                                    if name == "byteStart" =>
                                {
                                    Some(*value)
                                }
                                _ => None,
                            });
                    if let (Some(relative_path), Some(byte_start)) = (relative_path, byte_start) {
                        self.pending_definition_navigation = Some(PendingDefinitionNavigation {
                            relative_path,
                            byte_start,
                        });
                    }
                }
                // Phase 22.2: workspace open intents dispatched from a pane
                // view's menu (definition navigation) bypass the driver's
                // `route_sdui_intent`; attribute the active pane so the
                // answering DocumentOpened lands in the requesting pane.
                let root_id = intent.arguments.iter().find_map(|arg| {
                    if arg.name == "workspaceRootId"
                        && let crate::protocol::SduiActionValue::U64(root_id) = arg.value
                    {
                        return Some(root_id);
                    }
                    None
                });
                let relative_path = intent.arguments.iter().find_map(|arg| {
                    if arg.name == "relativePath"
                        && let crate::protocol::SduiActionValue::String(path) = &arg.value
                    {
                        return Some(path.clone());
                    }
                    None
                });
                if let (Some(root_id), Some(relative_path)) = (root_id, relative_path) {
                    ctx.submit_action::<EditorAction>(EditorAction::RecordPendingOpenIntent {
                        root_id,
                        relative_path,
                    });
                }
                let ui_version = self.sdui_ui_version.get();
                let _ = edit_queue.enqueue_sdui_action(ui_version, intent);
            }
        }
        self.push_menu(None);
        self.requests.active_language_intelligence_request_id = None;
        ctx.request_render();
        ctx.set_handled();
    }

    pub(crate) fn handle_language_intelligence_menu_action(
        &mut self,
        intent: &TransientMenuAction,
    ) -> bool {
        match intent.command_id.as_str() {
            "language.dismissResult" => true,
            "language.previewEdit" => {
                let title = intent
                    .arguments
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("edit preview");
                let mut next_status = self.status.clone();
                next_status.runtime_diagnostic = Some(RuntimeDiagnostic::error(
                    "language.preview_only",
                    format!("Code action edit preview is display-only in Phase 18.20: {title}"),
                ));
                let _ = self.set_status(next_status);
                true
            }
            "language.navigateDefinition" => {
                let kind = intent
                    .arguments
                    .get("kind")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                if kind != "openDocument" {
                    return true;
                }
                let Some(document_id) = intent
                    .arguments
                    .get("documentId")
                    .and_then(|value| value.as_u64())
                else {
                    return true;
                };
                let Some(byte_start) = intent
                    .arguments
                    .get("byteStart")
                    .and_then(|value| value.as_u64())
                else {
                    return true;
                };
                if self.editor.document_state().document_id != document_id {
                    // Other open documents are not focus-switched in Phase 18.20;
                    // external/unknown targets stay non-navigable from this path.
                    return true;
                }
                let _ = self.editor.navigate_to_byte_offset(byte_start);
                true
            }
            _ => false,
        }
    }

    fn finish_local_outcome(&mut self, ctx: &mut EventCtx<'_>, outcome: EditorCommandOutcome) {
        if let Some(edit_queue) = &self.edit_queue {
            for event in outcome.edit_events {
                let transaction_id = self.requests.next_transaction_id();
                let _ = edit_queue.enqueue_edit_event(event, transaction_id);
            }
        }
        if outcome.changed {
            self.enqueue_decoration_viewport_request();
            ctx.request_render();
            ctx.request_accessibility_update();
            ctx.set_handled();
        }
    }

    fn enqueue_completion_request(&mut self, event: crate::editor::EditorCompletionRequestEvent) {
        let Some(edit_queue) = &self.edit_queue else {
            return;
        };
        let request_id = self.requests.next_completion_request_id();
        self.requests.active_completion_request_id = Some(request_id);
        let _ = edit_queue.enqueue_completion_request(event, request_id);
    }

    fn enqueue_language_intelligence_request(
        &mut self,
        event: crate::editor::EditorLanguageIntelligenceRequestEvent,
    ) {
        let Some(edit_queue) = &self.edit_queue else {
            return;
        };
        let request_id = self.requests.next_language_intelligence_request_id();
        self.requests.active_language_intelligence_request_id = Some(request_id);
        let _ = edit_queue.enqueue_language_intelligence_request(event, request_id);
    }

    fn enqueue_selection_query_request(
        &mut self,
        event: crate::editor::EditorSelectionQueryRequestEvent,
    ) {
        let Some(edit_queue) = &self.edit_queue else {
            return;
        };
        let request_id = self.requests.next_selection_query_request_id();
        self.requests.pending_selection_query = Some((request_id, event.selections.clone()));
        let _ = edit_queue.enqueue_selection_query_request(event, request_id);
    }

    /// Applies read-only text-object/smart-select ranges as selections
    /// (multi-cursor aware). Stale results drop without touching the selection.
    fn apply_selection_query_result(
        &mut self,
        result: crate::protocol::SelectionQueryResult,
    ) -> bool {
        let Some((request_id, requested_cursors)) = self.requests.pending_selection_query.take()
        else {
            return false;
        };
        if request_id != result.request_id {
            return false;
        }
        let document = self.editor.document_state();
        if document.document_id != result.document_id
            || document.document_version != result.document_version
        {
            return true;
        }
        let mut selections: Vec<crate::editor::selection::Selection> = Vec::new();
        for (index, cursor) in requested_cursors.iter().enumerate() {
            match result.ranges.get(index) {
                Some(Some(range)) => {
                    let start = usize::try_from(range.start.min(range.end)).unwrap_or(0);
                    let end = usize::try_from(range.end.max(range.start)).unwrap_or(0);
                    selections.push(if cursor.anchor > cursor.focus {
                        crate::editor::selection::Selection::new(end, start)
                    } else {
                        crate::editor::selection::Selection::new(start, end)
                    });
                }
                // No object for this caret: keep the requested selection.
                _ => selections.push(crate::editor::selection::Selection::new(
                    usize::try_from(cursor.anchor).unwrap_or(0),
                    usize::try_from(cursor.focus).unwrap_or(0),
                )),
            }
        }
        self.editor.apply_selection_query_result(selections);
        true
    }

    fn enqueue_decoration_viewport_request(&mut self) {
        let Some(edit_queue) = &self.edit_queue else {
            return;
        };
        let document = self.editor.document_state();
        let range = self.editor.visible_byte_range();
        let viewport = (
            document.document_id,
            document.document_version,
            range.start,
            range.end,
        );
        if self.last_decoration_viewport == Some(viewport) {
            return;
        }
        if edit_queue
            .enqueue_decoration_viewport_request(
                document.document_id,
                document.document_version,
                range.start,
                range.end,
            )
            .is_ok()
        {
            self.last_decoration_viewport = Some(viewport);
        }
    }

    // -- menu session helpers (ported from `SduiNativeState`) --

    fn menu_select_next(&mut self) {
        if let Some(menu) = &mut self.menu_sync.menu {
            menu.select_next();
            self.menu_sync.pending = Some(Some(menu.clone()));
        }
    }

    fn menu_select_previous(&mut self) {
        if let Some(menu) = &mut self.menu_sync.menu {
            menu.select_previous();
            self.menu_sync.pending = Some(Some(menu.clone()));
        }
    }

    fn menu_activate_selected(&mut self) -> Option<SduiActionIntent> {
        let menu = self.menu_sync.menu.as_ref()?;
        let action = menu.activate_selected()?;
        if action.completion_accept.is_some() {
            return None;
        }
        Some(SduiActionIntent {
            command_id: action.command_id.clone(),
            source: crate::protocol::SduiActionSource::ListItem {
                node_id: crate::protocol::SduiNodeId(menu.session_id().0),
                item_id: menu.selected_index().to_string(),
            },
            arguments: crate::masonry_sdui::json_object_to_sdui_arguments(&action.arguments),
        })
    }

    fn menu_selected_action(&self) -> Option<TransientMenuAction> {
        self.menu_sync
            .menu
            .as_ref()
            .and_then(crate::shell::TransientMenuSession::activate_selected)
            .cloned()
    }

    fn menu_activate_completion(&mut self) -> Option<crate::shell::CompletionMenuAcceptAction> {
        let menu = self.menu_sync.menu.as_ref()?;
        menu.activate_selected()?.completion_accept.clone()
    }

    fn menu_cancel(&mut self) {
        if let Some(menu) = &mut self.menu_sync.menu {
            menu.cancel();
        }
    }

    #[cfg(test)]
    pub(crate) fn editor_mut(&mut self) -> &mut EditorSurface {
        &mut self.editor
    }

    #[cfg(test)]
    pub(crate) fn editor_state_for_test(&self) -> &crate::editor::surface::EditorDocumentState {
        self.editor.document_state()
    }

    // -- widget handlers (called both by `Widget` impl and by the chrome's
    //    delegation for pane 1) --

    pub(crate) fn handle_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        ctx.request_focus();

        let (changed, handled) = match event {
            PointerEvent::Down(button_event)
                if button_event.button == Some(masonry::core::PointerButton::Primary) =>
            {
                let point = ctx.local_position(button_event.state.position);
                self.editor.set_pointer_pos(Some(point));
                self.editor.set_pointer_pressed(true);
                if self
                    .editor
                    .scrollbar_thumb_rect(self.editor_rect)
                    .is_some_and(|thumb| thumb.contains(point))
                {
                    // ponytail: thumb-drag scrolling is deferred; the press
                    // only sets the Active visual state for now.
                    ctx.request_render();
                    (false, true)
                } else if let Some(local_point) = self.editor_local_point(point) {
                    ctx.capture_pointer();
                    let composition_cancelled = self.editor.cancel_composition();
                    (
                        self.editor.place_caret_at_point(local_point) || composition_cancelled,
                        true,
                    )
                } else {
                    (false, true)
                }
            }
            PointerEvent::Move(pointer_update) => {
                let point = ctx.local_position(pointer_update.current.position);
                self.editor.set_pointer_pos(Some(point));
                ctx.request_render();
                if ctx.is_active() {
                    if let Some(local_point) = self.editor_local_point(point) {
                        (self.editor.extend_selection_to_point(local_point), true)
                    } else {
                        (false, true)
                    }
                } else {
                    (false, false)
                }
            }
            PointerEvent::Up(_) => {
                self.editor.set_pointer_pressed(false);
                ctx.request_render();
                if ctx.is_active() {
                    (false, true)
                } else {
                    (false, false)
                }
            }
            PointerEvent::Cancel(_) => {
                self.editor.clear_pointer_chrome_state();
                ctx.request_render();
                if ctx.is_active() {
                    (false, true)
                } else {
                    (false, false)
                }
            }
            PointerEvent::Leave(_) => {
                self.editor.clear_pointer_chrome_state();
                ctx.request_render();
                (false, false)
            }
            PointerEvent::Scroll(PointerScrollEvent { delta, state, .. }) => {
                let _point = ctx.local_position(state.position);
                let changed = match delta {
                    ScrollDelta::LineDelta(_, y) => {
                        self.editor.scroll_lines((-*y).round() as isize)
                    }
                    ScrollDelta::PixelDelta(position) => {
                        let logical = position.to_logical::<f64>(ctx.get_scale_factor());
                        self.editor.scroll_vertical_pixels(-logical.y)
                    }
                    ScrollDelta::PageDelta(_, y) => {
                        self.editor.scroll_lines((-*y).round() as isize)
                    }
                };
                if changed {
                    self.enqueue_decoration_viewport_request();
                }
                (changed, changed)
            }
            _ => (false, false),
        };

        if changed {
            ctx.request_render();
            ctx.request_accessibility_update();
        }
        if handled {
            ctx.set_handled();
        }
    }

    pub(crate) fn handle_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        use masonry::core::keyboard::{Key, KeyState, NamedKey};
        match event {
            TextEvent::Keyboard(key_event)
                if key_event.state == KeyState::Down && !key_event.is_composing =>
            {
                match &key_event.key {
                    Key::Named(NamedKey::Escape) => {
                        self.local_key(ctx, key_stroke(KeyCode::Escape, key_event));
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.local_command(ctx, EditorCommand::Backspace);
                    }
                    Key::Named(NamedKey::Delete) => {
                        self.local_command(ctx, EditorCommand::DeleteForward);
                    }
                    Key::Named(NamedKey::Enter) => {
                        self.local_key(ctx, key_stroke(KeyCode::Enter, key_event));
                    }
                    Key::Named(NamedKey::Tab) => {
                        self.local_key(ctx, key_stroke(KeyCode::Tab, key_event));
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        let command = if key_event.modifiers.alt() && key_event.modifiers.shift() {
                            EditorCommand::ColumnSelect {
                                direction: CursorSelectDirection::Left,
                            }
                        } else if key_event.modifiers.ctrl() {
                            EditorCommand::MoveWordStart {
                                forward: false,
                                long: false,
                                extend: key_event.modifiers.shift(),
                            }
                        } else if key_event.modifiers.shift() {
                            EditorCommand::SelectLeft
                        } else {
                            EditorCommand::MoveLeft
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        let command = if key_event.modifiers.alt() && key_event.modifiers.shift() {
                            EditorCommand::ColumnSelect {
                                direction: CursorSelectDirection::Right,
                            }
                        } else if key_event.modifiers.ctrl() {
                            EditorCommand::MoveWordStart {
                                forward: true,
                                long: false,
                                extend: key_event.modifiers.shift(),
                            }
                        } else if key_event.modifiers.shift() {
                            EditorCommand::SelectRight
                        } else {
                            EditorCommand::MoveRight
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        let command = if key_event.modifiers.alt() && key_event.modifiers.shift() {
                            EditorCommand::ColumnSelect {
                                direction: CursorSelectDirection::Up,
                            }
                        } else if key_event.modifiers.alt() {
                            EditorCommand::AddCursor {
                                direction: CursorSelectDirection::Up,
                            }
                        } else if key_event.modifiers.ctrl() {
                            EditorCommand::MoveParagraph {
                                forward: false,
                                to_end: false,
                                extend: key_event.modifiers.shift(),
                            }
                        } else {
                            EditorCommand::MoveUp
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        let command = if key_event.modifiers.alt() && key_event.modifiers.shift() {
                            EditorCommand::ColumnSelect {
                                direction: CursorSelectDirection::Down,
                            }
                        } else if key_event.modifiers.alt() {
                            EditorCommand::AddCursor {
                                direction: CursorSelectDirection::Down,
                            }
                        } else if key_event.modifiers.ctrl() {
                            EditorCommand::MoveParagraph {
                                forward: true,
                                to_end: false,
                                extend: key_event.modifiers.shift(),
                            }
                        } else {
                            EditorCommand::MoveDown
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Named(NamedKey::Home) => {
                        let command = if key_event.modifiers.ctrl() || key_event.modifiers.meta() {
                            EditorCommand::DocumentStart
                        } else {
                            EditorCommand::LineStart
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Named(NamedKey::End) => {
                        let command = if key_event.modifiers.ctrl() || key_event.modifiers.meta() {
                            EditorCommand::DocumentEnd
                        } else {
                            EditorCommand::LineEnd
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Character(_) if is_select_all_matches_shortcut(key_event) => {
                        if self.editor.command(EditorCommand::SelectAllMatches) {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_select_line_shortcut(key_event) => {
                        if self.editor.command(EditorCommand::SelectLine) {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_select_next_match_shortcut(key_event) => {
                        if self.editor.command(EditorCommand::SelectNextMatch) {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_undo_cursor_move_shortcut(key_event) => {
                        if self.editor.command(EditorCommand::UndoCursorMove) {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_copy_shortcut(key_event) => {
                        if let Some(event) = self.copy_selection_to_system_clipboard() {
                            ctx.submit_action::<EditorAction>(EditorAction::ClientConnection(
                                event,
                            ));
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_cut_shortcut(key_event) => {
                        let outcome = self.cut_selection_to_system_clipboard();
                        if let Some(event) = outcome.diagnostic {
                            ctx.submit_action::<EditorAction>(EditorAction::ClientConnection(
                                event,
                            ));
                        }
                        if outcome.changed {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_paste_shortcut(key_event) => {
                        let outcome = self.paste_from_system_clipboard();
                        if let Some(event) = outcome.diagnostic {
                            ctx.submit_action::<EditorAction>(EditorAction::ClientConnection(
                                event,
                            ));
                        }
                        if outcome.changed {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_redo_shortcut(key_event) => {
                        if self.redo() {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_undo_shortcut(key_event) => {
                        if self.undo() {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) => {
                        if let Some(stroke) = character_key_stroke(key_event) {
                            self.local_key(ctx, stroke);
                        }
                    }
                    _ => {}
                }
            }
            // masonry_winit intercepts Ctrl/Cmd+V and emits ClipboardPaste with the
            // clipboard contents instead of forwarding a Keyboard event.
            TextEvent::ClipboardPaste(text) => {
                let outcome = self.paste_provided_clipboard_text(text);
                if let Some(event) = outcome.diagnostic {
                    ctx.submit_action::<EditorAction>(EditorAction::ClientConnection(event));
                }
                if outcome.changed {
                    ctx.request_render();
                    ctx.request_accessibility_update();
                }
                ctx.set_handled();
            }
            TextEvent::Ime(ime) => {
                use masonry::core::Ime;
                match ime {
                    Ime::Enabled => {
                        self.sync_ime_area(ctx);
                        ctx.set_handled();
                    }
                    Ime::Preedit(text, cursor) => {
                        let changed = if text.is_empty() {
                            self.editor.cancel_composition()
                        } else {
                            self.editor.set_preedit(text.clone(), *cursor)
                        };
                        if changed {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        self.sync_ime_area(ctx);
                        ctx.set_handled();
                    }
                    Ime::Commit(text) => {
                        let _ = self.editor.cancel_composition();
                        if !text.is_empty() {
                            self.local_command(ctx, EditorCommand::Insert(text));
                        } else {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        self.sync_ime_area(ctx);
                        ctx.set_handled();
                    }
                    Ime::Disabled => {
                        if self.editor.cancel_composition() {
                            ctx.request_render();
                            ctx.request_accessibility_update();
                        }
                        ctx.clear_ime_area();
                        ctx.set_handled();
                    }
                }
            }
            TextEvent::WindowFocusChange(false) => {
                if self.editor.cancel_composition() {
                    ctx.request_render();
                    ctx.request_accessibility_update();
                }
                ctx.clear_ime_area();
            }
            _ => {}
        }
    }

    pub(crate) fn handle_update(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &Update,
    ) {
        // Phase 22.2: pane activation follows Masonry focus; the app driver
        // syncs the shell's active pane and moves focus to the pane's content.
        if let Update::FocusChanged(true) = event {
            ctx.submit_action::<EditorAction>(EditorAction::PaneFocused(self.pane_id));
        }
        // Kick off the caret-blink animation loop when the editor gains focus
        // and the effective caret style animates. The loop self-perpetuates in
        // `handle_anim_frame` and stops when focus is lost or the style is Solid.
        if let Update::FocusChanged(true) = event
            && self.editor.caret_animates()
        {
            ctx.request_anim_frame();
        }
    }

    pub(crate) fn handle_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        let delta_ms = interval / 1_000_000;
        if self.editor.advance_blink(delta_ms) {
            ctx.request_paint_only();
        }
        if ctx.is_focus_target() && self.editor.caret_animates() {
            ctx.request_anim_frame();
        }
    }

    pub(crate) fn layout_in(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        bc: &BoxConstraints,
        rect: Rect,
    ) -> Size {
        let size = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(900.0, 600.0))
        };
        self.editor_rect = rect;
        let local = self.editor.ime_cursor_area(rect.width(), rect.height());
        ctx.set_ime_area(Rect::new(
            rect.x0 + local.x0,
            rect.y0 + local.y0,
            rect.x0 + local.x1,
            rect.y0 + local.y1,
        ));
        size
    }

    pub(crate) fn paint_in(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        let size = ctx.size();
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }
        let rect = self.editor_rect;
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.editor.theme().base.shell_bg,
            None,
            &rect,
        );
        self.editor.paint_in_rect(ctx, scene, rect);
    }

    pub(crate) fn post_paint_in(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        // Zero-size panes must overlay nothing. Inactive-tab hosts and
        // pending orphans stay in the tree at Size::ZERO / Point::ZERO so they
        // keep receiving connection events, and `post_paint` is unclipped
        // (Masonry appends the postfix scene after the clip layer pops), so an
        // unguarded status line would leak into the top-left corner and paint
        // over the tab bar.
        let size = ctx.size();
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }
        let recorder = global_recorder();
        let _scope = recorder.scope("masonry.render_prepare.post_paint");
        self.paint_status_line(ctx, scene);
    }

    pub(crate) fn accessibility_label(&self) -> String {
        let observation = self.status_observation();
        crate::editor::accessibility::compose_editor_accessibility_label(
            crate::editor::accessibility::EditorAccessibilityLabelParts {
                status_text: &observation.status_text,
                theme_label: &observation.theme_label,
                composing: observation.composing,
                recovery_summary: None,
                visible_text: &self.editor.visible_text(),
                empty_placeholder: "Clay native text canvas.",
            },
        )
    }

    fn editor_local_point(&self, point: Point) -> Option<Point> {
        self.editor_rect
            .contains(point)
            .then(|| Point::new(point.x - self.editor_rect.x0, point.y - self.editor_rect.y0))
    }

    fn paint_status_line(&self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        let rect = self.editor_rect;
        let metrics = self
            .editor
            .typography()
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Status);
        let y0 = (rect.y1 - metrics.status_height()).max(rect.y0);
        let status_rect = masonry::kurbo::Rect::new(rect.x0, y0, rect.x1.max(rect.x0), rect.y1);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.editor.theme().base.status_bg,
            None,
            &status_rect,
        );

        let ui_theme = self.editor.ui_theme();
        let inset =
            ui_theme.scalar_f64("spacing.sm").unwrap_or(12.0) * ui_theme.spacing_scale() as f64;
        crate::shell::primitives::paint_divider(
            scene,
            status_rect,
            crate::shell::primitives::Axis::Horizontal,
            ui_theme,
        );

        let status = self.status_text();
        let max_width = (rect.width() - inset * 2.0).max(1.0) as f32;
        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, &status, 1.0, true);
        builder.push_default(StyleProperty::FontStack(
            self.editor.typography().profile(FontRole::Ui).font_stack(),
        ));
        builder.push_default(StyleProperty::FontSize(metrics.font_size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            UiTextMetrics::LINE_HEIGHT_MULTIPLIER as f32,
        )));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        let mut layout = builder.build(&status);
        layout.break_all_lines(Some(max_width));
        render_text(
            scene,
            Affine::translate((
                rect.x0 + inset,
                y0 + (metrics.status_height() - metrics.line_height) / 2.0,
            )),
            &layout,
            &[self.editor.theme().base.status_text.into()],
            true,
        );
    }
}

impl Widget for PaneDocumentView {
    type Action = EditorAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        self.handle_pointer_event(ctx, props, event);
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        self.handle_text_event(ctx, props, event);
    }

    fn on_access_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, props: &mut PropertiesMut<'_>, event: &Update) {
        self.handle_update(ctx, props, event);
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        self.handle_anim_frame(ctx, props, interval);
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

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
        self.layout_in(ctx, bc, Rect::new(0.0, 0.0, size.width, size.height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        self.paint_in(ctx, scene);
    }

    fn post_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        scene: &mut Scene,
    ) {
        self.post_paint_in(ctx, scene);
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
        let metrics = self
            .editor
            .typography()
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Status);
        let rect = self.editor_rect;
        let status_id = NodeId::from(WidgetId::next());
        let observation = self.status_observation();
        let mut status = Node::new(Role::Status);
        status.set_label(
            crate::editor::accessibility::compose_status_accessibility_label(
                &observation.status_text,
                None,
            ),
        );
        status.set_bounds(masonry::accesskit::Rect {
            x0: rect.x0,
            y0: (rect.y1 - metrics.status_height()).max(rect.y0),
            x1: rect.x1.max(rect.x0),
            y1: rect.y1.max(rect.y0),
        });
        ctx.tree_update().nodes.push((status_id, status));
        node.set_children(vec![status_id]);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn accepts_text_input(&self) -> bool {
        true
    }
}

// -- key/command helpers (ported from `masonry_editor.rs`) --

pub(crate) fn key_stroke(
    key: KeyCode,
    key_event: &masonry::core::keyboard::KeyboardEvent,
) -> KeyStroke {
    KeyStroke {
        key,
        modifiers: KeyModifiers {
            shift: key_event.modifiers.shift(),
            control: key_event.modifiers.ctrl(),
            alt: key_event.modifiers.alt(),
            super_key: key_event.modifiers.meta(),
        },
    }
}

pub(crate) fn character_key_stroke(
    key_event: &masonry::core::keyboard::KeyboardEvent,
) -> Option<KeyStroke> {
    match &key_event.key {
        masonry::core::keyboard::Key::Character(text) => {
            Some(key_stroke(KeyCode::Character(text.to_string()), key_event))
        }
        _ => None,
    }
}

fn is_copy_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "c")
}

fn is_cut_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "x")
}

fn is_paste_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "v")
}

fn is_undo_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "z") && !key_event.modifiers.shift()
}

fn is_redo_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    if is_primary_character_shortcut(key_event, "z") && key_event.modifiers.shift() {
        return true;
    }
    // Common Windows/Linux redo chord; macOS keeps Cmd+Shift+Z.
    !cfg!(target_os = "macos")
        && is_primary_character_shortcut(key_event, "y")
        && !key_event.modifiers.shift()
}

/// Ctrl/Cmd+L selects the current line (VSCode `expandLineSelection`).
fn is_select_line_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "l") && !key_event.modifiers.shift()
}

/// Ctrl/Cmd+Shift+L selects every occurrence of the current selection/word
/// (VSCode `selectHighlights`).
fn is_select_all_matches_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "l") && key_event.modifiers.shift()
}

/// Ctrl/Cmd+D selects the next occurrence of the current selection/word as a
/// new caret (VSCode `addSelectionToNextFindMatch`).
fn is_select_next_match_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "d") && !key_event.modifiers.shift()
}

/// Ctrl/Cmd+U restores the previous selection set (VSCode `cursorUndo`).
fn is_undo_cursor_move_shortcut(key_event: &masonry::core::keyboard::KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "u") && !key_event.modifiers.shift()
}

fn is_primary_character_shortcut(
    key_event: &masonry::core::keyboard::KeyboardEvent,
    character: &str,
) -> bool {
    let masonry::core::keyboard::Key::Character(text) = &key_event.key else {
        return false;
    };
    if !text.eq_ignore_ascii_case(character) {
        return false;
    }

    if cfg!(target_os = "macos") {
        key_event.modifiers.meta() && !key_event.modifiers.ctrl() && !key_event.modifiers.alt()
    } else {
        key_event.modifiers.ctrl() && !key_event.modifiers.meta() && !key_event.modifiers.alt()
    }
}

fn edit_rejection_requests_resync(reason: &EditRejection) -> bool {
    matches!(
        reason,
        EditRejection::StaleVersion { .. }
            | EditRejection::FutureVersion { .. }
            | EditRejection::LeaseRequired
            | EditRejection::LeaseExpired { .. }
            | EditRejection::ReadOnlyDocument
            | EditRejection::RegionLocked { .. }
            | EditRejection::InvalidBehaviorVersion { .. }
    )
}

fn edit_rejection_label(reason: &EditRejection) -> &'static str {
    match reason {
        EditRejection::StaleVersion { .. } => "stale",
        EditRejection::FutureVersion { .. } => "future version",
        EditRejection::LeaseRequired => "lease required",
        EditRejection::LeaseExpired { .. } => "lease expired",
        EditRejection::ReadOnlyDocument => "read-only",
        EditRejection::RegionLocked { .. } => "region locked",
        EditRejection::InvalidDocument { .. } => "invalid document",
        EditRejection::InvalidRange { .. } => "invalid range",
        EditRejection::InvalidBehaviorVersion { .. } => "stale behavior",
    }
}

fn edit_rejection_diagnostic_code(reason: &EditRejection) -> String {
    let kind = match reason {
        EditRejection::StaleVersion { .. } => "StaleVersion",
        EditRejection::FutureVersion { .. } => "FutureVersion",
        EditRejection::LeaseRequired => "LeaseRequired",
        EditRejection::LeaseExpired { .. } => "LeaseExpired",
        EditRejection::ReadOnlyDocument => "ReadOnlyDocument",
        EditRejection::RegionLocked { .. } => "RegionLocked",
        EditRejection::InvalidDocument { .. } => "InvalidDocument",
        EditRejection::InvalidRange { .. } => "InvalidRange",
        EditRejection::InvalidBehaviorVersion { .. } => "InvalidBehaviorVersion",
    };
    format!("edit.rejected.{kind}")
}

fn edit_rejection_summary(reason: &EditRejection) -> String {
    match reason {
        EditRejection::StaleVersion {
            client_base_version,
            server_version,
        } => format!(
            "Edit rejected (stale): local base v{client_base_version}, server v{server_version}"
        ),
        EditRejection::FutureVersion {
            client_base_version,
            server_version,
        } => format!(
            "Edit rejected (future version): local base v{client_base_version}, server v{server_version}"
        ),
        EditRejection::LeaseRequired => "Edit rejected (lease required)".to_string(),
        EditRejection::LeaseExpired { .. } => "Edit rejected (lease expired)".to_string(),
        EditRejection::ReadOnlyDocument => "Edit rejected (read-only document)".to_string(),
        EditRejection::RegionLocked { .. } => "Edit rejected (region locked)".to_string(),
        EditRejection::InvalidDocument { document_id } => {
            format!("Edit rejected (invalid document {document_id})")
        }
        EditRejection::InvalidRange { message } => {
            let sanitized = crate::editor::accessibility::sanitize_recovery_summary(message)
                .unwrap_or_else(|| "invalid range".to_string());
            format!("Edit rejected (invalid range): {sanitized}")
        }
        EditRejection::InvalidBehaviorVersion {
            behavior_version,
            server_behavior_version,
        } => format!(
            "Edit rejected (stale behavior): local bv{behavior_version}, server bv{server_behavior_version}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ClientEditQueue;
    use crate::protocol::{ClientMessage, DocumentAccess, DocumentMetadata, RuntimeStateSnapshot};

    fn metadata(document_id: DocumentId, version: DocumentVersion, path: &str) -> DocumentMetadata {
        DocumentMetadata {
            document_id,
            version,
            access: DocumentAccess::Editable {
                lease_id: document_id,
            },
            lease_id: Some(document_id),
            dirty: false,
            workspace_root_id: 77,
            path: path.to_string(),
        }
    }

    fn view_with_queue(queue: ClientEditQueue) -> PaneDocumentView {
        PaneDocumentView::new(PaneId(1), Rc::new(Cell::new(1)), Rc::new(Cell::new(0)))
            .with_edit_queue(queue)
    }

    fn open(view: &mut PaneDocumentView, document_id: DocumentId, text: &str) {
        assert!(
            view.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: metadata(document_id, 1, &format!("doc-{document_id}.md")),
                text: text.to_string(),
            })
        );
    }

    #[test]
    fn active_document_identity_reflects_only_the_active_document() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        // Blank view (or an open still in flight): no identity.
        assert_eq!(view.active_document_identity(), None);
        // The installed active document's identity: (root id, relative path).
        open(&mut view, 7, "alpha");
        assert_eq!(
            view.active_document_identity(),
            Some((77, "doc-7.md".to_string()))
        );
    }

    fn mode_manifest(
        version: crate::protocol::BehaviorVersion,
        document_id: DocumentId,
        mode_id: &str,
        font_role: crate::protocol::DocumentFontRole,
    ) -> crate::protocol::BehaviorManifest {
        let mut manifest = crate::protocol::BehaviorManifest::minimal_text_editing(version);
        manifest.manifest_id = format!("{mode_id}.{mode_id}");
        manifest.scope = crate::protocol::BehaviorScope::Document { document_id };
        manifest.document_font_role = font_role;
        manifest
    }

    #[test]
    fn behavior_manifest_matching_own_document_installs_mode_content() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");

        view.apply_behavior_manifest(&mode_manifest(
            3,
            7,
            "markdown",
            crate::protocol::DocumentFontRole::Proportional,
        ));

        let state = view.editor_mut().document_state();
        assert_eq!(
            state.behavior_manifest.as_ref().unwrap().manifest_id,
            "markdown.markdown"
        );
        assert_eq!(state.behavior_version, 3);
        assert_eq!(
            view.editor_mut()
                .document_state()
                .behavior_manifest
                .as_ref()
                .unwrap()
                .document_font_role,
            crate::protocol::DocumentFontRole::Proportional
        );
    }

    #[test]
    fn behavior_manifest_for_foreign_document_bumps_version_without_importing_content() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        view.apply_behavior_manifest(&mode_manifest(
            2,
            7,
            "markdown",
            crate::protocol::DocumentFontRole::Proportional,
        ));

        // Another document's mode activation must not swap this pane's
        // keymaps/editor rules/font role — only its version stamp advances.
        view.apply_behavior_manifest(&mode_manifest(
            4,
            9,
            "rust",
            crate::protocol::DocumentFontRole::Monospace,
        ));

        let state = view.editor_mut().document_state();
        assert_eq!(
            state.behavior_version, 4,
            "version stamp must track the connection"
        );
        let installed = state.behavior_manifest.as_ref().unwrap();
        assert_eq!(
            installed.manifest_id, "markdown.markdown",
            "content must stay this pane's mode"
        );
        assert_eq!(
            installed.document_font_role,
            crate::protocol::DocumentFontRole::Proportional,
            "font role must not leak from the other mode"
        );
    }

    #[test]
    fn stale_or_repeated_behavior_manifests_are_ignored() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        view.apply_behavior_manifest(&mode_manifest(
            5,
            7,
            "markdown",
            crate::protocol::DocumentFontRole::Proportional,
        ));

        // Same version re-broadcast: ignored.
        view.apply_behavior_manifest(&mode_manifest(
            5,
            7,
            "rust",
            crate::protocol::DocumentFontRole::Monospace,
        ));
        // Older version (re-sent layer from another document's open): ignored.
        view.apply_behavior_manifest(&mode_manifest(
            3,
            7,
            "rust",
            crate::protocol::DocumentFontRole::Monospace,
        ));

        let state = view.editor_mut().document_state();
        assert_eq!(state.behavior_version, 5);
        assert_eq!(
            state.behavior_manifest.as_ref().unwrap().manifest_id,
            "markdown.markdown"
        );
    }

    #[test]
    fn global_scope_behavior_manifest_installs_in_every_view() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");

        let mut manifest = crate::protocol::BehaviorManifest::minimal_text_editing(6);
        manifest.manifest_id = "runtime.configuration".to_string();
        view.apply_behavior_manifest(&manifest);

        let state = view.editor_mut().document_state();
        assert_eq!(state.behavior_version, 6);
        assert_eq!(
            state.behavior_manifest.as_ref().unwrap().manifest_id,
            "runtime.configuration"
        );
    }

    #[test]
    fn runtime_snapshot_part_restores_own_mode_layer_under_connection_version() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        // The pane is running markdown at connection version 4.
        view.apply_behavior_manifest(&mode_manifest(
            4,
            7,
            "markdown",
            crate::protocol::DocumentFontRole::Proportional,
        ));

        // A recovery snapshot arrives: connection-wide manifest is now code
        // mode at version 7; doc 7's part carries its own (older-versioned)
        // markdown layer.
        let mut global = crate::protocol::BehaviorManifest::minimal_text_editing(7);
        global.manifest_id = "rust.rust".to_string();
        global.scope = crate::protocol::BehaviorScope::GlobalDefault;
        global.document_font_role = crate::protocol::DocumentFontRole::Monospace;
        let snapshot = RuntimeStateSnapshot {
            runtime_generation_id: 1,
            client_id: 1,
            behavior: global,
            active_theme: crate::protocol::ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: Default::default(),
            sdui_tree: crate::protocol::SduiTree {
                ui_version: 1,
                root_id: crate::protocol::SduiNodeId(1),
                nodes: Vec::new(),
            },
            package_ui: Default::default(),
            documents: vec![crate::protocol::DocumentRuntimeRenderState {
                document_id: 7,
                document_version: 1,
                reset_decorations: false,
                reset_diagnostics: false,
                initial_decorations: None,
                initial_diagnostics: None,
                behavior_manifest: Some(mode_manifest(
                    5,
                    7,
                    "markdown",
                    crate::protocol::DocumentFontRole::Proportional,
                )),
            }],
            diagnostics: Vec::new(),
        };

        assert!(view.apply_runtime_snapshot(&snapshot));

        let state = view.editor_mut().document_state();
        assert_eq!(
            state.behavior_manifest.as_ref().unwrap().manifest_id,
            "markdown.markdown",
            "recovery must restore this document's own mode content"
        );
        assert_eq!(
            state.behavior_manifest.as_ref().unwrap().document_font_role,
            crate::protocol::DocumentFontRole::Proportional
        );
        assert_eq!(
            state.behavior_version, 7,
            "outbound stamps stay at the connection-wide version"
        );
    }

    #[test]
    fn failed_open_leaves_pane_state_unchanged_with_diagnostic() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        view.editor_mut().set_caret_for_test(2);

        // Failed open (no document_id yet): the view keeps its document and
        // caret; the failure surfaces as a status diagnostic.
        assert!(
            view.apply_connection_event(ClientConnectionEvent::FileOperationFailed {
                code: crate::protocol::FileErrorCode::NotFound,
                message: "no such file".to_string(),
                workspace_root_id: Some(77),
                document_id: None,
            })
        );
        assert_eq!(view.visible_text_for_test(), "alpha");
        assert_eq!(view.editor_mut().caret_for_test(), 2);
        assert!(
            view.status_text().contains("no such file"),
            "diagnostic visible: {}",
            view.status_text()
        );
    }

    #[test]
    fn duplicate_document_open_is_a_noop_on_the_owning_view() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        assert_eq!(view.visible_text_for_test(), "alpha");
        view.editor_mut().set_caret_for_test(3);

        // The server answers a duplicate open (same canonical path, existing
        // lease) with a fresh DocumentOpened; the owning view must NOT
        // reinstall the snapshot over the live surface.
        assert!(
            !view.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: metadata(7, 1, "doc-7.md"),
                text: "server-side snapshot".to_string(),
            }),
            "same-document open is a no-op"
        );
        assert_eq!(
            view.visible_text_for_test(),
            "alpha",
            "live buffer keeps its content"
        );
        assert_eq!(view.editor_mut().caret_for_test(), 3, "caret survives");
        assert!(
            view.status_text().contains("v1"),
            "status keeps the live document version"
        );
    }

    #[test]
    fn cross_pane_open_documents_menu_lists_every_pane_and_routes_activation() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        // Pane 2 owns doc 42 (active) plus a retained session 55.
        let other = vec![CrossPaneDocumentEntry {
            pane: crate::shell::PaneId(2),
            document_id: 42,
            display_name: "b.rs".to_string(),
            dirty: true,
            retained: vec![(55, "c.md".to_string(), false)],
        }];
        assert!(view.show_open_documents_menu(&other));

        let menu = view
            .take_pending_menu()
            .expect("menu pushed")
            .expect("some menu");
        let items = menu.items();
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        let label = |text: &str| labels.contains(&text);
        assert!(
            label("doc-7.md (active)"),
            "own active document listed: {labels:?}"
        );
        assert!(
            label("pane 2: b.rs •"),
            "other pane's dirty document listed with marker: {labels:?}"
        );
        assert!(
            label("pane 2: c.md"),
            "other pane's retained session listed: {labels:?}"
        );

        // Activation payloads: own entries carry only documentId; cross-pane
        // entries carry documentId + the owning paneId.
        let own = items
            .iter()
            .find(|item| item.label == "doc-7.md (active)")
            .expect("own entry");
        assert!(own.action.arguments.get("documentId").is_some());
        assert!(own.action.arguments.get("paneId").is_none());
        let cross = items
            .iter()
            .find(|item| item.label == "pane 2: b.rs •")
            .expect("cross entry");
        assert_eq!(
            cross.action.arguments.get("documentId"),
            Some(&serde_json::json!(42))
        );
        assert_eq!(
            cross.action.arguments.get("paneId"),
            Some(&serde_json::json!(2))
        );
    }

    #[test]
    fn active_and_retained_document_info_feed_cross_pane_aggregation() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        assert_eq!(
            view.active_document_info(),
            None,
            "placeholder pane has none"
        );
        open(&mut view, 7, "alpha");
        // Switch to another document so 7 becomes a retained session.
        open(&mut view, 9, "beta");
        let (document_id, name, dirty) = view.active_document_info().expect("active doc");
        assert_eq!(document_id, 9);
        assert!(!dirty);
        assert!(name.contains("doc-9.md"));
        let retained = view.retained_documents();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].0, 7);
    }

    #[test]
    fn view_ignores_events_for_foreign_documents() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        open(&mut view, 42, "beta");

        // A decoration set for the OTHER document must not land in this view.
        let foreign = crate::protocol::DecorationSet {
            document_id: 99,
            document_version: 1,
            package_prefix: "core".to_string(),
            kind: crate::protocol::DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 16,
            spans: vec![crate::protocol::DecorationSpan {
                byte_start: 0,
                byte_end: 5,
                kind: crate::protocol::DecorationKind::Syntax,
                token_type: crate::protocol::TokenType::CodeSpan,
                modifiers: crate::protocol::Modifiers::default(),
                scope: None,
                font_role: None,
                priority: 1,
                provenance: crate::protocol::DecorationProvenance {
                    package_name: "test".to_string(),
                    package_version: "1.0.0".to_string(),
                    package_prefix: "test".to_string(),
                },
            }],
        };
        assert!(!view.apply_connection_event(ClientConnectionEvent::DecorationSet(foreign)));
        assert_eq!(view.decoration_span_count(), 0);

        // EditAck for a foreign document is a no-op on the active surface.
        assert!(
            !view.apply_connection_event(ClientConnectionEvent::EditAck {
                document_id: 99,
                version: 9,
                transaction_id: 1,
            })
        );
        assert!(view.status_text().contains("v1"));

        // EditAck for the active document updates its version.
        assert!(view.apply_connection_event(ClientConnectionEvent::EditAck {
            document_id: 42,
            version: 2,
            transaction_id: 1,
        }));
        assert!(view.status_text().contains("v2"));
    }

    #[test]
    fn same_document_reopen_does_not_reinstall_over_live_view() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");

        // Re-open of the pane's active document: no-op (keeps caret/pending).
        assert!(
            !view.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: metadata(7, 3, "doc-7.md"),
                text: "server text changed".to_string(),
            })
        );
        assert_eq!(view.visible_text_for_test(), "alpha");
    }

    #[test]
    fn view_stashes_and_activates_retained_sessions() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        open(&mut view, 42, "beta");
        assert_eq!(view.visible_text_for_test(), "beta");
        assert_eq!(view.retained_session_count(), 1);

        assert!(view.activate_document(7));
        assert_eq!(view.visible_text_for_test(), "alpha");
        assert!(view.contains_document(42));

        // Activation of a session this view does not own: diagnostic only.
        assert!(view.activate_document(99));
        assert!(view.status_text().contains("document_session.missing"));
        assert_eq!(view.visible_text_for_test(), "alpha");
    }

    #[test]
    fn close_pane_releases_active_and_retained_documents_and_resets() {
        let (queue, mut receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        open(&mut view, 42, "beta");

        view.close_pane();

        let mut closes = Vec::new();
        while let Ok(message) = receiver.try_recv() {
            if let ClientMessage::CloseDocument {
                document_id, force, ..
            } = message
            {
                closes.push((document_id, force));
            }
        }
        closes.sort();
        // Retained session force-closed; active document clean-closed.
        assert_eq!(closes, vec![(7, true), (42, false)]);

        assert!(!view.has_opened_document());
        assert_eq!(view.retained_session_count(), 0);
        assert_eq!(
            view.status_text(),
            "Clay — Local Fallback — Editable — doc 0 — v0"
        );
        assert!(!view.contains_document(7));
        assert!(!view.contains_document(42));
    }

    #[test]
    fn per_document_edits_enqueue_with_document_lease_and_version() {
        let (queue, mut receiver) = ClientEditQueue::bounded(8);
        let mut view_a = view_with_queue(queue.clone());
        let mut view_b = view_with_queue(queue.clone());
        open(&mut view_a, 7, "aaa");
        open(&mut view_b, 42, "bbb");

        // Both views share the queue; each document tracks its own base
        // version and lease (Phase 22.2 per-document sync states).
        let _ = view_a.paste_provided_clipboard_text("x");
        let _ = view_b.paste_provided_clipboard_text("y");

        let mut edits = Vec::new();
        while let Ok(message) = receiver.try_recv() {
            if let ClientMessage::Edit {
                document_id,
                lease_id,
                base_version,
                ..
            } = message
            {
                edits.push((document_id, lease_id, base_version));
            }
        }
        edits.sort_by_key(|(document_id, _, _)| *document_id);
        assert_eq!(edits, vec![(7, Some(7), 1), (42, Some(42), 1)]);
    }

    #[test]
    fn view_applies_runtime_snapshot_parts_for_its_own_document() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");

        let snapshot = RuntimeStateSnapshot {
            runtime_generation_id: 5,
            client_id: 11,
            behavior: BehaviorManifest::minimal_text_editing(5),
            active_theme: ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: ActiveTypography {
                revision: 1,
                ..ActiveTypography::default()
            },
            sdui_tree: crate::protocol::SduiTree {
                ui_version: 1,
                root_id: crate::protocol::SduiNodeId(1),
                nodes: Vec::new(),
            },
            package_ui: crate::protocol::PackageUiSnapshot::default(),
            documents: vec![],
            diagnostics: Vec::new(),
        };
        assert!(view.apply_runtime_snapshot(&snapshot));
        // Stale or duplicate generations are accepted idempotently by the
        // view (the chrome gates generations); re-applying is a no-op change.
        assert!(!view.apply_runtime_snapshot(&snapshot));
    }

    #[test]
    fn reconnect_swaps_queue_and_reopens_documents() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        open(&mut view, 9, "beta"); // 7 becomes retained

        let (reconnect_queue, _receiver2) = ClientEditQueue::bounded(8);
        view.reconnect(reconnect_queue);
        // The disconnect recovery menu is cleared (auto-reconnect replaces the
        // "restart Clay" guidance).
        assert!(matches!(view.take_pending_menu(), Some(None)));

        // Active + retained documents are the re-open list (root 77 from the
        // metadata helper; deduped by path).
        let documents = view.documents_for_reopen();
        assert_eq!(documents.len(), 2);
        assert!(documents.contains(&(77, "doc-7.md".to_string())));
        assert!(documents.contains(&(77, "doc-9.md".to_string())));

        // The next DocumentOpened for the ACTIVE document reinstalls fresh
        // server state instead of the Phase 22.2 duplicate-open no-op.
        assert!(
            view.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: metadata(9, 2, "doc-9.md"),
                text: "gamma".to_string(),
            })
        );
        assert_eq!(view.visible_text_for_test(), "gamma");

        // A retained copy is replaced by its re-opened snapshot.
        assert!(
            view.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: metadata(7, 1, "doc-7.md"),
                text: "delta".to_string(),
            })
        );
        assert_eq!(view.visible_text_for_test(), "delta");

        // Each reconnect re-arms the reinstall: a fresh reconnect followed by
        // a DocumentOpened reinstalls again.
        let (third_queue, _receiver3) = ClientEditQueue::bounded(8);
        view.reconnect(third_queue);
        assert!(
            view.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: metadata(7, 1, "doc-7.md"),
                text: "epsilon".to_string(),
            })
        );
        assert_eq!(view.visible_text_for_test(), "epsilon");
        // Without a reconnect, the Phase 22.2 duplicate-open no-op applies.
        assert!(
            !view.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: metadata(7, 1, "doc-7.md"),
                text: "zeta".to_string(),
            })
        );
        assert_eq!(view.visible_text_for_test(), "epsilon");
    }

    #[test]
    fn request_save_active_document_enqueues_save_and_reports_document_id() {
        let (queue, mut receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");

        // Phase 22.4: the driver asks each dirty pane to save before closing
        // its tab; the document id is the ack-tracking key.
        let document_id = view.request_save_active_document().expect("save enqueued");
        assert_eq!(document_id, 7);
        let message = receiver.try_recv().expect("save request sent");
        assert!(
            matches!(message, ClientMessage::SaveDocument { document_id: 7, .. }),
            "save request carries the active document"
        );
    }

    #[test]
    fn request_save_active_document_fails_without_connection_or_document() {
        let view = view_with_queue(ClientEditQueue::bounded(8).0);
        // Blank view: no document to save.
        let err = view.request_save_active_document().unwrap_err();
        assert_eq!(err.code, "client.save.no_document");

        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");
        view.edit_queue = None;
        // No connection: the driver must learn the save cannot be enqueued so
        // the close flow cancels instead of closing unsaved work.
        let err = view.request_save_active_document().unwrap_err();
        assert_eq!(err.code, "client.save.unavailable");
    }

    #[test]
    fn pane_close_gate_blocks_dirty_documents_with_conflict_menu() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");

        // Clean document: close allowed.
        assert!(view.guard_pane_close());
        assert!(view.take_pending_menu().is_none());

        // Dirty document: close blocked, conflict menu pushed (existing
        // dirty-document close flow — Save/Discard/Keep).
        let _ = view.paste_provided_clipboard_text("x");
        assert!(view.is_dirty());
        assert!(!view.guard_pane_close());
        let menu = view.take_pending_menu().expect("menu push");
        assert!(menu.expect("session").prompt().contains("unsaved edits"));
        // The document is still open (never silently dropped).
        assert_eq!(view.visible_text_for_test(), "xalpha");
        assert!(view.has_opened_document());
    }

    #[test]
    fn view_pushes_and_drains_pending_menu_sync() {
        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "alpha");

        assert!(view.show_open_documents_menu(&[]));
        let menu = view.take_pending_menu().expect("menu push");
        assert_eq!(menu.expect("menu session").prompt(), "Open documents");
        assert!(view.take_pending_menu().is_none());

        // Dismissing a recovery prompt pushes a clear.
        view.dismiss_recovery();
        let cleared = view.take_pending_menu().expect("clear push");
        assert!(cleared.is_none());
    }

    fn render_view_scene(width: u32, height: u32) -> Scene {
        use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
        use masonry::core::NewWidget;
        use masonry::dpi::PhysicalSize;
        use masonry::theme::default_property_set;

        let (queue, _receiver) = ClientEditQueue::bounded(8);
        let mut view = view_with_queue(queue);
        open(&mut view, 7, "hello");
        let options = RenderRootOptions {
            default_properties: default_property_set().into(),
            use_system_fonts: false,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(width, height),
            scale_factor: 1.0,
            test_font: None,
        };
        let mut render_root = RenderRoot::new(NewWidget::new(view), |_| {}, options);
        let (scene, _) = render_root.redraw();
        scene
    }

    #[test]
    fn zero_size_pane_overlays_nothing() {
        // Sanity: a real-sized pane paints (background + status line).
        let visible = render_view_scene(900, 600);
        assert!(
            !visible.encoding().is_empty(),
            "a sized pane must paint content"
        );
        // A zero-size pane (retained inactive-tab host / pending orphan placed
        // at Point::ZERO) must overlay nothing: Masonry `post_paint` is
        // unclipped, so an unguarded status line would leak over the tab bar.
        let zero = render_view_scene(0, 0);
        assert!(
            zero.encoding().is_empty(),
            "a zero-size pane must not paint the status line"
        );
    }

    #[test]
    fn request_bookkeeping_allocates_unique_ids() {
        let mut b = PaneRequestBookkeeping::default();
        // First allocation from the default seed is 1; ids are monotonic.
        assert_eq!(b.next_transaction_id(), 1);
        assert_eq!(b.next_transaction_id(), 2);
        let c1 = b.next_completion_request_id();
        let c2 = b.next_completion_request_id();
        assert!(c2 > c1);
        let l1 = b.next_language_intelligence_request_id();
        let l2 = b.next_language_intelligence_request_id();
        assert!(l2 > l1);
        let s1 = b.next_selection_query_request_id();
        let s2 = b.next_selection_query_request_id();
        assert!(s2 > s1);
        // In-flight tracking: only the matching request clears.
        assert!(b.active_completion_request_id.is_none());
        b.active_completion_request_id = Some(c1);
        assert!(b.take_completion_if_current(c1));
        assert!(!b.take_completion_if_current(c1));
        assert!(b.active_completion_request_id.is_none());
        b.active_language_intelligence_request_id = Some(l1);
        b.pending_selection_query = Some((s1, Vec::new()));
        b.clear_active();
        assert!(b.active_language_intelligence_request_id.is_none());
        assert!(
            b.pending_selection_query.is_some(),
            "clear_active keeps the pending query"
        );
        b.reset();
        assert!(b.pending_selection_query.is_none());
    }

    #[test]
    fn menu_sync_pending_semantics() {
        let mut sync = PaneMenuSync::default();
        // Unchanged tri-state: nothing pending initially.
        assert_eq!(sync.take_pending(), None);
        // Show: push remembers the session and marks a one-shot pending push.
        let session = TransientMenuSession::new(TransientMenuSessionId(1), "test");
        sync.push(Some(session.clone()));
        assert_eq!(sync.menu, Some(session.clone()));
        assert_eq!(sync.take_pending(), Some(Some(session.clone())));
        assert_eq!(sync.take_pending(), None, "pending push is one-shot");
        // Clear: Some(None) means an explicit clear, not no-pending.
        sync.push(None);
        assert_eq!(sync.menu, None);
        assert_eq!(sync.take_pending(), Some(None));
        // Session ids allocate monotonically from the shared cell.
        let id = sync.next_session_id();
        assert!(id >= 1);
        let next = sync.next_session_id();
        assert!(next > id);
    }
}
