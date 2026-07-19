use std::path::PathBuf;

use masonry::accesskit::{Node, NodeId, Role};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, BrushIndex, ChildrenIds, EventCtx, KeyboardEvent,
    LayoutCtx, PaintCtx, PointerButton, PointerEvent, PointerScrollEvent, PropertiesMut,
    PropertiesRef, RegisterCtx, ScrollDelta, TextEvent, Widget, render_text,
};
use masonry::kurbo::{Affine, Point, Rect, Size};
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::Fill;
use masonry::vello::Scene;

use crate::client::{
    ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientRuntimeStateCandidate,
    ClientRuntimeStateInstallError, ClientUiCommandRoute, ClipboardSink, SystemClipboard,
};
use crate::editor::{
    EditorCommand, EditorCommandOutcome, EditorSurface,
    document_session::{DocumentSessionStore, RetainedDocumentSession},
    typography::{UiTextMetrics, UiTextVariant},
};
use crate::masonry_sdui::{SduiNativeState, editor_region_for_document};
use crate::perf::metrics::global_recorder;
use crate::protocol::{
    BehaviorManifest, CompletionRequestId, CompletionResultSet, DocumentAccess, DocumentId,
    DocumentMetadata, DocumentVersion, EditRejection, FileErrorCode, FontRole, KeyCode,
    KeyModifiers, KeyStroke, LanguageIntelligenceRequestId, LanguageIntelligenceResult,
    ProtocolErrorCode, RuntimeDiagnostic, RuntimeGenerationId,
};

#[derive(Debug, Default, PartialEq)]
pub struct ClipboardCommandOutcome {
    pub changed: bool,
    pub diagnostic: Option<ClientConnectionEvent>,
}

impl ClipboardCommandOutcome {
    fn unchanged() -> Self {
        Self {
            changed: false,
            diagnostic: None,
        }
    }

    fn diagnostic(diagnostic: ClientConnectionEvent) -> Self {
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
#[derive(Debug, PartialEq)]
pub enum EditorAction {
    ExitRequested,
    ClientConnection(ClientConnectionEvent),
    ClientUiCommand(ClientUiCommandRoute),
    /// Native file dialog finished on a background thread (Linux portal must not
    /// block the Wayland/UI event loop or the chooser never appears).
    OpenSelectedFile(std::path::PathBuf),
    /// Native folder dialog finished on a background thread.
    OpenSelectedFolder(std::path::PathBuf),
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
    connection: EditorConnectionStatus,
    document_id: Option<DocumentId>,
    version: Option<DocumentVersion>,
    access: Option<DocumentAccess>,
    runtime_diagnostic: Option<RuntimeDiagnostic>,
    /// Server/client dirty bit for the active document (optimistic after local edits).
    dirty: bool,
    /// Sanitized basename-only title for status/accessibility (never an absolute path).
    document_display_name: Option<String>,
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

    fn with_document_values(
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

    fn observation(&self) -> SduiStatusObservation {
        SduiStatusObservation {
            status_text: self.text(),
            connection_label: self.connection_label().to_string(),
            access_label: self.access_label().to_string(),
            sync_version: self.version,
            diagnostic_text: self.diagnostic_text(),
            // Filled by EditorWidget::status_observation from the active theme / chrome.
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingDefinitionNavigation {
    relative_path: String,
    byte_start: u64,
}

#[derive(Debug)]
pub struct EditorWidget {
    editor: EditorSurface,
    edit_queue: Option<ClientEditQueue>,
    next_transaction_id: u64,
    next_completion_request_id: u64,
    active_completion_request_id: Option<CompletionRequestId>,
    next_language_intelligence_request_id: u64,
    active_language_intelligence_request_id: Option<LanguageIntelligenceRequestId>,
    pending_definition_navigation: Option<PendingDefinitionNavigation>,
    last_decoration_viewport: Option<(DocumentId, DocumentVersion, u64, u64)>,
    status: EditorStatus,
    sdui: SduiNativeState,
    layout_invalidated: bool,
    /// Last successfully installed runtime generation (0 before any live snapshot).
    runtime_generation_id: RuntimeGenerationId,
    /// Inactive document sessions retained for multi-document switching (Phase 20).
    sessions: DocumentSessionStore,
    /// True after the first real document snapshot/open replaces the empty bootstrap buffer.
    has_opened_document: bool,
    next_document_menu_session_id: u64,
}

impl Default for EditorWidget {
    fn default() -> Self {
        let mut editor = EditorSurface::default();
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(0));
        let status = EditorStatus::local_fallback().with_document_values(
            editor.document_state().document_id,
            editor.document_state().document_version,
            editor.document_state().access.clone(),
        );
        Self {
            editor,
            edit_queue: None,
            next_transaction_id: 1,
            next_completion_request_id: 1,
            active_completion_request_id: None,
            next_language_intelligence_request_id: 1,
            active_language_intelligence_request_id: None,
            pending_definition_navigation: None,
            last_decoration_viewport: None,
            status,
            sdui: SduiNativeState::empty(),
            layout_invalidated: false,
            runtime_generation_id: 0,
            sessions: DocumentSessionStore::default(),
            has_opened_document: false,
            next_document_menu_session_id: 1,
        }
    }
}

impl EditorWidget {
    pub fn with_initial_state(initial_state: ClientInitialState) -> Self {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            initial_state.document_id,
            initial_state.document_version,
            initial_state.text,
            initial_state.access.clone(),
        );
        editor.install_behavior_manifest(initial_state.behavior_manifest);
        editor.set_active_theme(&initial_state.active_theme);
        let _ = editor.set_typography(initial_state.active_typography.clone());
        let status = EditorStatus::connected(
            initial_state.document_id,
            initial_state.document_version,
            initial_state.access,
        );
        let mut sdui = SduiNativeState::empty();
        sdui.set_typography(editor.typography().clone());
        Self {
            editor,
            edit_queue: None,
            next_transaction_id: 1,
            next_completion_request_id: 1,
            active_completion_request_id: None,
            next_language_intelligence_request_id: 1,
            active_language_intelligence_request_id: None,
            pending_definition_navigation: None,
            last_decoration_viewport: None,
            status,
            sdui,
            layout_invalidated: false,
            runtime_generation_id: 0,
            sessions: DocumentSessionStore::default(),
            has_opened_document: true,
            next_document_menu_session_id: 1,
        }
    }

    /// Return and clear a layout request caused by a typography profile change.
    /// Other connection events retain the existing render-only behavior.
    pub fn take_layout_invalidation(&mut self) -> bool {
        std::mem::take(&mut self.layout_invalidated)
    }

    pub fn with_status(mut self, status: EditorStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_edit_queue(mut self, edit_queue: ClientEditQueue) -> Self {
        self.edit_queue = Some(edit_queue);
        self
    }

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
                        session.pending.retain(|pending| {
                            // Keep unmatched pending; specific transaction removal happens when
                            // the connection layer tracks the active document only.
                            pending.document_id == document_id
                        });
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
                self.editor.install_behavior_manifest(manifest);
                false
            }
            ClientConnectionEvent::ActiveTheme(theme) => {
                self.editor.set_active_theme(&theme);
                true
            }
            ClientConnectionEvent::ActiveTypography(typography) => {
                let changed = self.editor.set_typography(typography);
                if changed {
                    self.sdui.set_typography(self.editor.typography().clone());
                }
                self.layout_invalidated |= changed;
                changed
            }
            ClientConnectionEvent::SduiSnapshot { tree, .. } => {
                self.sdui.apply_snapshot(tree);
                true
            }
            ClientConnectionEvent::SduiUpdate(update) => self.sdui.apply_update(update),
            ClientConnectionEvent::DecorationSet(set) => self.editor.apply_decoration_set(set),
            ClientConnectionEvent::DiagnosticSet(set) => self.editor.apply_diagnostic_set(set),
            ClientConnectionEvent::CompletionResult(result) => self.apply_completion_result(result),
            ClientConnectionEvent::LanguageIntelligenceResult(result) => {
                self.apply_language_intelligence_result(result)
            }
            ClientConnectionEvent::LanguageIntelligenceRejected { request_id, .. } => {
                if self.active_language_intelligence_request_id == Some(request_id) {
                    self.active_language_intelligence_request_id = None;
                    self.sdui.clear_active_menu();
                    true
                } else {
                    false
                }
            }
            ClientConnectionEvent::CompletionRejected { request_id, .. } => {
                if self.active_completion_request_id == Some(request_id) {
                    self.active_completion_request_id = None;
                    self.sdui.clear_active_menu();
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
            ClientConnectionEvent::RuntimeStateSnapshot(snapshot) => {
                self.install_runtime_state_snapshot(*snapshot)
            }
            ClientConnectionEvent::Disconnected => self.apply_disconnect(None),
            ClientConnectionEvent::ConnectionError(message) => {
                self.apply_disconnect(Some(message.as_str()))
            }
            _ => false,
        }
    }

    /// Validate and atomically install one complete runtime-generation snapshot.
    ///
    /// On success the widget acknowledges the generation through the edit queue.
    /// On validation failure no partial state remains, no acknowledgement is
    /// sent, and the connection status becomes disconnected so the shell can
    /// rebootstrap into the latest authoritative state.
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
                let next_status = EditorStatus {
                    connection: EditorConnectionStatus::Disconnected,
                    runtime_diagnostic: Some(RuntimeDiagnostic::error(
                        "clay.runtime.invalid_snapshot",
                        "Runtime state snapshot failed client validation and was rejected.",
                    )),
                    ..self.status.clone().with_document_values(
                        self.editor.document_state().document_id,
                        self.editor.document_state().document_version,
                        self.editor.document_state().access.clone(),
                    )
                };
                self.set_status(next_status);
                return true;
            }
        };

        self.editor
            .install_behavior_manifest(candidate.behavior.clone());
        self.editor.set_active_theme(&candidate.active_theme);
        let typography_changed = self
            .editor
            .install_runtime_typography(candidate.active_typography.clone());
        if typography_changed {
            self.sdui.set_typography(self.editor.typography().clone());
        }
        self.sdui.apply_snapshot(candidate.sdui_tree.clone());
        self.sdui.install_package_ui_snapshot(&candidate.package_ui);

        let open_document_id = self.editor.document_state().document_id;
        let open_document_version = self.editor.document_state().document_version;
        for document in &candidate.documents {
            if document.document_id != open_document_id {
                continue;
            }
            if document.reset_decorations {
                self.editor.clear_decorations();
            }
            if document.reset_diagnostics {
                self.editor.clear_diagnostics();
            }
            if let Some(set) = document.initial_decorations.clone()
                && set.document_version == open_document_version
            {
                let _ = self.editor.apply_decoration_set(set);
            }
            if let Some(set) = document.initial_diagnostics.clone()
                && set.document_version == open_document_version
            {
                let _ = self.editor.apply_diagnostic_set(set);
            }
        }

        if let Some(diagnostic) = candidate.diagnostics.last().cloned() {
            let mut next_status = self.status.clone();
            next_status.runtime_diagnostic = Some(diagnostic);
            let _ = self.set_status(next_status);
        }

        self.runtime_generation_id = candidate.runtime_generation_id;
        self.layout_invalidated = true;

        if let Some(queue) = &self.edit_queue {
            let _ = queue.enqueue_runtime_generation_installed(candidate.runtime_generation_id);
        }
        true
    }

    fn open_document_session(
        &mut self,
        metadata: crate::protocol::DocumentMetadata,
        text: String,
    ) -> bool {
        let incoming_id = metadata.document_id;
        let active_id = self.editor.document_state().document_id;
        let mut eviction_notice = None;

        if self.has_opened_document && incoming_id != active_id {
            eviction_notice = self.stash_active_session();
            // Server-authored open replaces any stale retained copy for this id.
            let _ = self.sessions.remove(incoming_id);
        } else if self.has_opened_document && incoming_id == active_id {
            // Same-document hard open/replace: keep session map, replace active buffer.
            self.editor.cancel_composition();
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
                "clay.editor.document_session.evicted",
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
        let access = self.editor.document_state().access.clone();
        let sync = self
            .edit_queue
            .as_ref()
            .map(|queue| queue.sync_snapshot())
            .unwrap_or_else(|| crate::client::ClientSyncSnapshot {
                confirmed_version: self.editor.document_state().document_version,
                optimistic_version: self.editor.document_state().document_version,
                pending: Vec::new(),
                last_resync: None,
            });
        let pending: Vec<_> = sync
            .pending
            .into_iter()
            .filter(|pending| pending.document_id == document_id)
            .collect();

        let theme = self.editor.theme();
        let theme_specifier = self.editor.theme_specifier().to_string();
        let typography = self.editor.typography().clone();
        let behavior = self.editor.document_state().behavior_manifest.clone();
        let outgoing = std::mem::take(&mut self.editor);
        // Leave a blank surface with shared theme/typography/behavior until caller loads.
        self.editor = EditorSurface::default();
        self.editor.set_theme(theme);
        self.editor.set_theme_specifier(theme_specifier);
        self.editor.set_typography_registry(typography);
        if let Some(manifest) = behavior {
            self.editor.install_behavior_manifest(manifest);
        }
        let _ = access;

        let session = RetainedDocumentSession {
            surface: outgoing,
            dirty: self.status.dirty,
            document_display_name: self.status.document_display_name.clone(),
            confirmed_version: sync.confirmed_version,
            pending,
            last_activated_order: 0,
        };
        self.sessions.insert(document_id, session)
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
                "clay.editor.document_session.missing",
                format!("No retained client session for document {document_id}."),
            ));
            return self.set_status(next_status);
        };

        let eviction_notice = self.stash_active_session();
        let theme = self.editor.theme();
        let theme_specifier = self.editor.theme_specifier().to_string();
        let typography = self.editor.typography().clone();
        self.editor = retained.surface;
        self.editor.set_theme(theme);
        self.editor.set_theme_specifier(theme_specifier);
        self.editor.set_typography_registry(typography);
        self.editor.cancel_composition();

        if let Some(queue) = self.edit_queue.as_mut() {
            queue.install_document_sync_state(
                document_id,
                &self.editor.document_state().access,
                retained.confirmed_version,
                retained.pending,
            );
        }

        let mut status = EditorStatus::connected_with_metadata(
            self.editor.document_state().document_id,
            self.editor.document_state().document_version,
            self.editor.document_state().access.clone(),
            retained.dirty,
            retained.document_display_name,
        );
        if let Some(message) = eviction_notice {
            status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(
                "clay.editor.document_session.evicted",
                message,
            ));
        }
        let _ = self.set_status(status);
        true
    }

    pub fn show_open_documents_menu(&mut self) -> bool {
        if !self.has_opened_document {
            return false;
        }
        let entries = self.sessions.list_with_active(
            self.editor.document_state().document_id,
            self.status.document_display_name.as_deref(),
            self.status.dirty,
        );
        let session_id = self.next_document_menu_session_id;
        self.next_document_menu_session_id = self.next_document_menu_session_id.saturating_add(1);
        let items = entries
            .into_iter()
            .map(|entry| {
                let mut label = entry.display_name.clone();
                if entry.dirty {
                    label.push_str(" •");
                }
                if entry.active {
                    label.push_str(" (active)");
                }
                let action =
                    crate::shell::TransientMenuAction::new("clay.editor.clientActivateDocument")
                        .with_arguments(serde_json::json!({ "documentId": entry.document_id }));
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
        let menu = crate::shell::TransientMenuSession::new(
            crate::shell::TransientMenuSessionId(session_id),
            "Open documents",
        )
        .with_items(items);
        self.sdui.set_active_menu(menu);
        true
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
            if self.sdui.active_menu().is_some_and(|menu| {
                let prompt = menu.prompt();
                prompt.contains("conflict")
                    || prompt.contains("unsaved edits")
                    || prompt.contains("Reload")
                    || prompt.contains("reload")
            }) {
                self.sdui.clear_active_menu();
            }
        }
        version_changed || self.set_status(next_status)
    }

    fn apply_document_reloaded(&mut self, metadata: DocumentMetadata, text: String) -> bool {
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
        self.sdui.clear_active_menu();
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
            format!("clay.file.{code:?}"),
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
        let session_id = self.next_document_menu_session_id;
        self.next_document_menu_session_id = self.next_document_menu_session_id.saturating_add(1);
        let (prompt, items) = match code {
            FileErrorCode::StaleFileMetadata => {
                let prompt = "File changed on disk — resolve save conflict".to_string();
                let items = vec![
                    crate::shell::TransientMenuItem::new(
                        "conflict.reload",
                        "Reload from disk (discard local edits)",
                        crate::shell::TransientMenuAction::new(
                            "clay.documents.serverReloadDocument",
                        )
                        .with_arguments(serde_json::json!({ "force": true })),
                    )
                    .with_accessibility_label("Reload from disk and discard unsaved local edits"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.keep",
                        "Keep unsaved edits",
                        crate::shell::TransientMenuAction::new(
                            "clay.editor.clientKeepUnsavedEdits",
                        ),
                    )
                    .with_accessibility_label("Keep unsaved edits and dismiss conflict menu"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.defer",
                        "Compare later",
                        crate::shell::TransientMenuAction::new(
                            "clay.editor.clientDeferConflictCompare",
                        ),
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
                        crate::shell::TransientMenuAction::new("clay.documents.serverSaveDocument"),
                    )
                    .with_accessibility_label("Save the document before reloading"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.reload",
                        "Discard edits and reload",
                        crate::shell::TransientMenuAction::new(
                            "clay.documents.serverReloadDocument",
                        )
                        .with_arguments(serde_json::json!({ "force": true })),
                    )
                    .with_accessibility_label("Discard unsaved edits and reload from disk"),
                    crate::shell::TransientMenuItem::new(
                        "conflict.keep",
                        "Keep unsaved edits",
                        crate::shell::TransientMenuAction::new(
                            "clay.editor.clientKeepUnsavedEdits",
                        ),
                    )
                    .with_accessibility_label("Keep unsaved edits and dismiss reload prompt"),
                ];
                (prompt, items)
            }
            _ => return false,
        };
        let _ = message;
        let menu = crate::shell::TransientMenuSession::new(
            crate::shell::TransientMenuSessionId(session_id),
            prompt,
        )
        .with_items(items);
        self.sdui.set_active_menu(menu);
        true
    }

    fn request_save_active_document(&self) -> Option<ClientConnectionEvent> {
        let Some(queue) = &self.edit_queue else {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.client.save.unavailable",
                    "Cannot save because this editor is not connected to a Clay server.",
                ),
            ));
        };
        if !self.has_opened_document {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.client.save.no_document",
                    "Cannot save because no document is open.",
                ),
            ));
        }
        let document = self.editor.document_state();
        queue
            .enqueue_save_document(document.document_id, document.document_version)
            .err()
            .map(|error| {
                ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                    "clay.client.save.queue_failed",
                    format!("Failed to send save request to the Clay server: {error}"),
                ))
            })
    }

    fn request_reload_active_document(&self, force: bool) -> Option<ClientConnectionEvent> {
        let Some(queue) = &self.edit_queue else {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.client.reload.unavailable",
                    "Cannot reload because this editor is not connected to a Clay server.",
                ),
            ));
        };
        if !self.has_opened_document {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.client.reload.no_document",
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
                    "clay.client.reload.queue_failed",
                    format!("Failed to send reload request to the Clay server: {error}"),
                ))
            })
    }

    fn handle_save_conflict_menu_action(
        &mut self,
        action: &crate::shell::TransientMenuAction,
    ) -> bool {
        match action.command_id.as_str() {
            "clay.documents.serverSaveDocument" => {
                if let Some(diagnostic) = self.request_save_active_document() {
                    let _ = self.apply_connection_event(diagnostic);
                }
                true
            }
            "clay.documents.serverReloadDocument" => {
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
            "clay.editor.clientKeepUnsavedEdits" => true,
            "clay.editor.clientDeferConflictCompare" => {
                let mut next_status = self.status.clone();
                next_status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(
                    "clay.file.conflict_deferred",
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
            "clay.client.disconnect",
            "Disconnected (connection lost). Restart Clay to reconnect; local unsaved edits stay in this window until then.",
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
            format!("clay.server.error.{code:?}"),
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
        let session_id = self.next_document_menu_session_id;
        self.next_document_menu_session_id = self.next_document_menu_session_id.saturating_add(1);
        let items = vec![
            crate::shell::TransientMenuItem::new(
                "recovery.dismiss",
                "Dismiss",
                crate::shell::TransientMenuAction::new("clay.editor.clientDismissRecovery"),
            )
            .with_accessibility_label("Dismiss disconnect recovery guidance"),
        ];
        let menu = crate::shell::TransientMenuSession::new(
            crate::shell::TransientMenuSessionId(session_id),
            "Disconnected — reconnect guidance",
        )
        .with_items(items);
        self.sdui.set_active_menu(menu);
        true
    }

    fn show_sync_recovery_menu(&mut self, prompt: &str, accessibility_hint: &str) -> bool {
        if !self.has_opened_document {
            return false;
        }
        let session_id = self.next_document_menu_session_id;
        self.next_document_menu_session_id = self.next_document_menu_session_id.saturating_add(1);
        let items = vec![
            crate::shell::TransientMenuItem::new(
                "recovery.resync",
                "Request resync",
                crate::shell::TransientMenuAction::new("clay.editor.clientRequestResync"),
            )
            .with_accessibility_label(format!("{accessibility_hint} Request resync")),
            crate::shell::TransientMenuItem::new(
                "recovery.dismiss",
                "Dismiss",
                crate::shell::TransientMenuAction::new("clay.editor.clientDismissRecovery"),
            )
            .with_accessibility_label("Dismiss recovery prompt"),
        ];
        let menu = crate::shell::TransientMenuSession::new(
            crate::shell::TransientMenuSessionId(session_id),
            prompt,
        )
        .with_items(items);
        self.sdui.set_active_menu(menu);
        true
    }

    fn clear_sync_recovery_menu(&mut self) {
        if let Some(menu) = self.sdui.active_menu()
            && menu.is_active()
        {
            let prompt = menu.prompt().to_ascii_lowercase();
            if prompt.contains("recover")
                || prompt.contains("rejected")
                || prompt.contains("disconnected")
                || prompt.contains("server error")
            {
                self.sdui.clear_active_menu();
            }
        }
    }

    pub fn request_resync_active_document(&mut self) -> Option<ClientConnectionEvent> {
        let Some(queue) = &self.edit_queue else {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.editor.resync_unavailable",
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
                    "clay.editor.resync_unavailable",
                    "Resync requires an active server connection. Restart Clay to reconnect.",
                ),
            ));
        }
        let document = self.editor.document_state();
        if let Err(error) =
            queue.enqueue_request_resync(document.document_id, document.document_version)
        {
            return Some(ClientConnectionEvent::RuntimeDiagnostic(
                RuntimeDiagnostic::error(
                    "clay.editor.resync_enqueue_failed",
                    format!("Failed to request resync: {error}"),
                ),
            ));
        }
        let mut next_status = self.status.clone();
        next_status.runtime_diagnostic = Some(RuntimeDiagnostic::warning(
            "clay.editor.resync_requested",
            "Resync requested — waiting for canonical snapshot.",
        ));
        let _ = self.set_status(next_status);
        None
    }

    pub fn dismiss_recovery(&mut self) -> bool {
        let cleared_menu = self.sdui.active_menu().is_some_and(|menu| menu.is_active());
        self.sdui.clear_active_menu();
        let mut next_status = self.status.clone();
        let cleared_diagnostic = next_status.runtime_diagnostic.take().is_some();
        let status_changed = self.set_status(next_status);
        cleared_menu || status_changed || cleared_diagnostic
    }

    fn handle_sync_recovery_menu_action(
        &mut self,
        action: &crate::shell::TransientMenuAction,
    ) -> bool {
        match action.command_id.as_str() {
            "clay.editor.clientRequestResync" => {
                if let Some(diagnostic) = self.request_resync_active_document() {
                    let _ = self.apply_connection_event(diagnostic);
                }
                true
            }
            "clay.editor.clientDismissRecovery" => self.dismiss_recovery(),
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
            .map(|queue| queue.sync_snapshot().pending.len())
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
        if let Some(menu) = self.sdui.active_menu()
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

    pub fn sdui_visible_texts(&self) -> Vec<String> {
        self.sdui.visible_texts()
    }

    pub fn sdui_ui_version(&self) -> u64 {
        self.sdui.ui_version()
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
        let mut clipboard = SystemClipboard;
        self.copy_selection_to_clipboard_with(&mut clipboard)
    }

    fn copy_selection_to_clipboard_with(
        &self,
        clipboard: &mut impl ClipboardSink,
    ) -> Option<ClientConnectionEvent> {
        let text = self.editor.selected_text()?;
        clipboard.set_text(text).err().map(|error| {
            ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                "clay.client.clipboard.write_failed",
                format!("Failed to copy selection to the system clipboard: {error}"),
            ))
        })
    }

    pub fn cut_selection_to_system_clipboard(&mut self) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let mut clipboard = SystemClipboard;
        self.cut_selection_to_clipboard_with(&mut clipboard)
    }

    fn cut_selection_to_clipboard_with(
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
                    "clay.client.clipboard.write_failed",
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
    ///
    /// `masonry_winit` intercepts Ctrl/Cmd+V and delivers `ClipboardPaste` instead of a
    /// keyboard event, so this path is the production native-paste entry point.
    fn paste_provided_clipboard_text(&mut self, text: &str) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let outcome = self.editor.paste_text_with_event(text);
        ClipboardCommandOutcome {
            changed: self.apply_local_edit_outcome(outcome),
            diagnostic: None,
        }
    }

    fn paste_from_clipboard_with(
        &mut self,
        clipboard: &mut impl ClipboardSink,
    ) -> ClipboardCommandOutcome {
        let _ = self.editor.cancel_composition();
        let text = match clipboard.get_text() {
            Ok(text) => text,
            Err(error) => {
                return ClipboardCommandOutcome::diagnostic(
                    ClientConnectionEvent::RuntimeDiagnostic(RuntimeDiagnostic::error(
                        "clay.client.clipboard.read_failed",
                        format!("Failed to read text from the system clipboard: {error}"),
                    )),
                );
            }
        };
        self.paste_provided_clipboard_text(&text)
    }

    pub fn undo(&mut self) -> bool {
        // Surface undo/redo also clears composition; track it for render.
        let cancelled = self.editor.is_composing();
        let outcome = self.editor.undo_with_event();
        self.apply_local_edit_outcome(outcome) || cancelled
    }

    pub fn redo(&mut self) -> bool {
        let cancelled = self.editor.is_composing();
        let outcome = self.editor.redo_with_event();
        self.apply_local_edit_outcome(outcome) || cancelled
    }

    /// Discard unfinished IME preedit without committing.
    pub fn cancel_composition(&mut self) -> bool {
        self.editor.cancel_composition()
    }

    fn sync_ime_area(&self, ctx: &mut EventCtx<'_>, size: Size) {
        let editor_rect = self.editor_main_rect(size);
        let local = self
            .editor
            .ime_cursor_area(editor_rect.width(), editor_rect.height());
        let area = Rect::new(
            editor_rect.x0 + local.x0,
            editor_rect.y0 + local.y0,
            editor_rect.x0 + local.x1,
            editor_rect.y0 + local.y1,
        );
        ctx.set_ime_area(area);
    }

    fn apply_local_edit_outcome(&mut self, outcome: EditorCommandOutcome) -> bool {
        if let Some(event) = outcome.edit_event
            && let Some(edit_queue) = &self.edit_queue
        {
            let transaction_id = self.next_transaction_id;
            self.next_transaction_id = self.next_transaction_id.saturating_add(1).max(1);
            let _ = edit_queue.enqueue_edit_event(event, transaction_id);
        }
        if outcome.changed {
            if !self.status.dirty {
                self.status.dirty = true;
            }
            self.active_completion_request_id = None;
            self.active_language_intelligence_request_id = None;
            self.sdui.clear_active_menu();
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
        if self.active_completion_request_id != Some(result.request_id) {
            return false;
        }
        let document = self.editor.document_state();
        if result.document_id != document.document_id
            || result.document_version != document.document_version
            || result.behavior_version != document.behavior_version
        {
            return false;
        }
        self.sdui
            .set_active_menu(crate::shell::completion_result_to_menu_session(&result));
        true
    }

    fn apply_language_intelligence_result(&mut self, result: LanguageIntelligenceResult) -> bool {
        if self.active_language_intelligence_request_id != Some(result.request_id) {
            return false;
        }
        let document = self.editor.document_state();
        if result.document_id != document.document_id
            || result.document_version != document.document_version
            || result.behavior_version != document.behavior_version
        {
            return false;
        }
        self.sdui
            .set_active_menu(crate::shell::language_intelligence_result_to_menu_session(
                &result,
            ));
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
        if let Some(event) = outcome.edit_event
            && let Some(edit_queue) = &self.edit_queue
        {
            let transaction_id = self.next_transaction_id;
            self.next_transaction_id = self.next_transaction_id.saturating_add(1).max(1);
            let _ = edit_queue.enqueue_edit_event(event, transaction_id);
        }
        if outcome.changed {
            self.active_completion_request_id = None;
            self.active_language_intelligence_request_id = None;
            self.sdui.clear_active_menu();
            ctx.request_render();
            ctx.request_accessibility_update();
        }
        ctx.set_handled();
    }

    fn local_key(&mut self, ctx: &mut EventCtx<'_>, key: KeyStroke) {
        if self.route_menu_key(ctx, &key) {
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
            self.active_completion_request_id = None;
            self.active_language_intelligence_request_id = None;
            self.sdui.clear_active_menu();
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
            } else if intent.command_id == "clay.documents.serverSaveDocument" {
                if let Some(diagnostic) = self.request_save_active_document() {
                    let _ = self.apply_connection_event(diagnostic);
                    ctx.request_render();
                }
            } else if intent.command_id == "clay.documents.serverReloadDocument" {
                if let Some(diagnostic) = self.request_reload_active_document(false) {
                    let _ = self.apply_connection_event(diagnostic);
                    ctx.request_render();
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
        if self.sdui.active_menu().is_none() {
            return false;
        }
        match key.key {
            KeyCode::ArrowUp => {
                self.sdui.menu_select_previous();
                ctx.request_render();
                ctx.set_handled();
                true
            }
            KeyCode::ArrowDown => {
                self.sdui.menu_select_next();
                ctx.request_render();
                ctx.set_handled();
                true
            }
            KeyCode::Enter | KeyCode::Tab => {
                self.activate_menu_selection(ctx, None);
                true
            }
            KeyCode::Escape => {
                self.sdui.menu_cancel();
                self.sdui.clear_active_menu();
                self.active_completion_request_id = None;
                self.active_language_intelligence_request_id = None;
                ctx.request_render();
                ctx.set_handled();
                true
            }
            KeyCode::Character(ref text) => {
                let Some(completion) = self.sdui.menu_activate_completion() else {
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
        if let Some(completion) = self.sdui.menu_activate_completion() {
            let outcome = self
                .editor
                .accept_completion_with_event(&completion, commit_character);
            self.finish_local_outcome(ctx, outcome);
            self.active_completion_request_id = None;
        } else if let Some(local_action) = self.sdui.menu_selected_action() {
            if local_action.command_id == "clay.editor.clientActivateDocument" {
                let document_id = local_action
                    .arguments
                    .get("documentId")
                    .and_then(|value| value.as_u64());
                let _ = self.sdui.menu_activate_selected();
                if let Some(document_id) = document_id {
                    let _ = self.activate_document(document_id);
                }
            } else if self.handle_save_conflict_menu_action(&local_action)
                || self.handle_sync_recovery_menu_action(&local_action)
            {
                let _ = self.sdui.menu_activate_selected();
                ctx.request_render();
            } else if self.handle_language_intelligence_menu_action(&local_action) {
                let _ = self.sdui.menu_activate_selected();
            } else if let Some(feature) =
                crate::client::behavior::language_intelligence_feature_for_command(
                    &local_action.command_id,
                )
            {
                let _ = self.sdui.menu_activate_selected();
                if let Some(event) = self
                    .editor
                    .language_intelligence_request_for_feature(feature)
                {
                    self.enqueue_language_intelligence_request(event);
                }
            } else if let Some(intent) = self.sdui.menu_activate_selected()
                && let Some(edit_queue) = &self.edit_queue
            {
                if intent.command_id == "clay.workspace.openFile"
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
                let _ = edit_queue.enqueue_sdui_action(self.sdui.ui_version(), intent);
            }
        }
        self.sdui.clear_active_menu();
        self.active_language_intelligence_request_id = None;
        ctx.request_render();
        ctx.set_handled();
    }

    fn handle_language_intelligence_menu_action(
        &mut self,
        intent: &crate::shell::transient_menu::TransientMenuAction,
    ) -> bool {
        match intent.command_id.as_str() {
            "clay.language.dismissResult" => true,
            "clay.language.previewEdit" => {
                let title = intent
                    .arguments
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("edit preview");
                let mut next_status = self.status.clone();
                next_status.runtime_diagnostic = Some(RuntimeDiagnostic::error(
                    "clay.language.preview_only",
                    format!("Code action edit preview is display-only in Phase 18.20: {title}"),
                ));
                let _ = self.set_status(next_status);
                true
            }
            "clay.language.navigateDefinition" => {
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
        if let Some(event) = outcome.edit_event
            && let Some(edit_queue) = &self.edit_queue
        {
            let transaction_id = self.next_transaction_id;
            self.next_transaction_id = self.next_transaction_id.saturating_add(1).max(1);
            let _ = edit_queue.enqueue_edit_event(event, transaction_id);
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
        let request_id = self.next_completion_request_id;
        self.next_completion_request_id = self.next_completion_request_id.saturating_add(1).max(1);
        self.active_completion_request_id = Some(request_id);
        let _ = edit_queue.enqueue_completion_request(event, request_id);
    }

    fn enqueue_language_intelligence_request(
        &mut self,
        event: crate::editor::EditorLanguageIntelligenceRequestEvent,
    ) {
        let Some(edit_queue) = &self.edit_queue else {
            return;
        };
        let request_id = self.next_language_intelligence_request_id;
        self.next_language_intelligence_request_id = self
            .next_language_intelligence_request_id
            .saturating_add(1)
            .max(1);
        self.active_language_intelligence_request_id = Some(request_id);
        let _ = edit_queue.enqueue_language_intelligence_request(event, request_id);
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

    fn accessibility_label(&self) -> String {
        let observation = self.status_observation();
        // Recovery/pending markers already live in status_text for chrome consistency.
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

    fn editor_main_rect(&self, size: Size) -> Rect {
        let document_id = self.editor.document_state().document_id;
        editor_region_for_document(size, &self.sdui, document_id)
    }

    fn editor_local_point(&self, size: Size, point: Point) -> Option<Point> {
        let rect = self.editor_main_rect(size);
        rect.contains(point)
            .then(|| Point::new(point.x - rect.x0, point.y - rect.y0))
    }

    fn paint_status_line(&self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        let size = ctx.size();
        let metrics = self
            .editor
            .typography()
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Status);
        let y0 = (size.height - metrics.status_height()).max(0.0);
        let rect = masonry::kurbo::Rect::new(0.0, y0, size.width.max(0.0), size.height.max(y0));
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            self.editor.theme().base.status_bg,
            None,
            &rect,
        );

        let status = self.status_text();
        let max_width = (size.width - 24.0).max(1.0) as f32;
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
                12.0,
                y0 + (metrics.status_height() - metrics.line_height) / 2.0,
            )),
            &layout,
            &[self.editor.theme().base.status_text.into()],
            true,
        );
    }
}

fn is_copy_shortcut(key_event: &KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "c")
}

fn is_cut_shortcut(key_event: &KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "x")
}

fn is_paste_shortcut(key_event: &KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "v")
}

fn is_undo_shortcut(key_event: &KeyboardEvent) -> bool {
    is_primary_character_shortcut(key_event, "z") && !key_event.modifiers.shift()
}

fn is_redo_shortcut(key_event: &KeyboardEvent) -> bool {
    if is_primary_character_shortcut(key_event, "z") && key_event.modifiers.shift() {
        return true;
    }
    // Common Windows/Linux redo chord; macOS keeps Cmd+Shift+Z.
    !cfg!(target_os = "macos")
        && is_primary_character_shortcut(key_event, "y")
        && !key_event.modifiers.shift()
}

fn is_primary_character_shortcut(key_event: &KeyboardEvent, character: &str) -> bool {
    let Key::Character(text) = &key_event.key else {
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

fn key_stroke(key: KeyCode, key_event: &KeyboardEvent) -> KeyStroke {
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

fn character_key_stroke(key_event: &KeyboardEvent) -> Option<KeyStroke> {
    match &key_event.key {
        Key::Character(text) => Some(key_stroke(KeyCode::Character(text.to_string()), key_event)),
        _ => None,
    }
}

impl Widget for EditorWidget {
    type Action = EditorAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        ctx.request_focus();

        let (changed, handled) = match event {
            PointerEvent::Down(button_event)
                if button_event.button == Some(PointerButton::Primary) =>
            {
                let point = ctx.local_position(button_event.state.position);
                if let Some(intent) = self.sdui.action_for_point(point) {
                    if let Some(edit_queue) = &self.edit_queue {
                        let _ = edit_queue.enqueue_sdui_action(self.sdui.ui_version(), intent);
                    }
                    (false, true)
                } else if let Some(local_point) = self.editor_local_point(ctx.size(), point) {
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
            PointerEvent::Move(pointer_update) if ctx.is_active() => {
                let point = ctx.local_position(pointer_update.current.position);
                if let Some(local_point) = self.editor_local_point(ctx.size(), point) {
                    (self.editor.extend_selection_to_point(local_point), true)
                } else {
                    (false, true)
                }
            }
            PointerEvent::Up(_) | PointerEvent::Cancel(_) if ctx.is_active() => (false, true),
            PointerEvent::Scroll(PointerScrollEvent { delta, state, .. }) => {
                let point = ctx.local_position(state.position);
                if self.sdui.scrolls_point(ctx.size(), point) {
                    let changed = match delta {
                        ScrollDelta::LineDelta(_, y) => {
                            self.sdui.scroll_lines(ctx.size(), (-*y).round() as isize)
                        }
                        ScrollDelta::PixelDelta(position) => {
                            let logical = position.to_logical::<f64>(ctx.get_scale_factor());
                            self.sdui.scroll_vertical_pixels(ctx.size(), -logical.y)
                        }
                        ScrollDelta::PageDelta(_, y) => {
                            self.sdui.scroll_lines(ctx.size(), (-*y).round() as isize)
                        }
                    };
                    (changed, changed)
                } else {
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

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(key_event)
                if key_event.state == KeyState::Down && !key_event.is_composing =>
            {
                match &key_event.key {
                    Key::Named(NamedKey::Escape) => {
                        if self.editor.has_active_snippet_session() {
                            self.local_key(ctx, key_stroke(KeyCode::Escape, key_event));
                        } else {
                            ctx.submit_action::<Self::Action>(EditorAction::ExitRequested);
                            ctx.set_handled();
                        }
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
                        let command = if key_event.modifiers.shift() {
                            EditorCommand::SelectLeft
                        } else {
                            EditorCommand::MoveLeft
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        let command = if key_event.modifiers.shift() {
                            EditorCommand::SelectRight
                        } else {
                            EditorCommand::MoveRight
                        };
                        self.local_command(ctx, command);
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.local_command(ctx, EditorCommand::MoveUp);
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.local_command(ctx, EditorCommand::MoveDown);
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
                    Key::Character(_) if is_copy_shortcut(key_event) => {
                        if let Some(event) = self.copy_selection_to_system_clipboard() {
                            ctx.submit_action::<Self::Action>(EditorAction::ClientConnection(
                                event,
                            ));
                        }
                        ctx.set_handled();
                    }
                    Key::Character(_) if is_cut_shortcut(key_event) => {
                        let outcome = self.cut_selection_to_system_clipboard();
                        if let Some(event) = outcome.diagnostic {
                            ctx.submit_action::<Self::Action>(EditorAction::ClientConnection(
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
                        // Alternate path for tests / non-winit hosts. Production
                        // masonry_winit converts Ctrl/Cmd+V into ClipboardPaste.
                        let outcome = self.paste_from_system_clipboard();
                        if let Some(event) = outcome.diagnostic {
                            ctx.submit_action::<Self::Action>(EditorAction::ClientConnection(
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
                    ctx.submit_action::<Self::Action>(EditorAction::ClientConnection(event));
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
                        // Ready for composition; publish candidate-window geometry.
                        self.sync_ime_area(ctx, ctx.size());
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
                        self.sync_ime_area(ctx, ctx.size());
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
                        self.sync_ime_area(ctx, ctx.size());
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

    fn on_access_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
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
        let editor_rect = self.editor_main_rect(size);
        let local = self
            .editor
            .ime_cursor_area(editor_rect.width(), editor_rect.height());
        ctx.set_ime_area(Rect::new(
            editor_rect.x0 + local.x0,
            editor_rect.y0 + local.y0,
            editor_rect.x0 + local.x1,
            editor_rect.y0 + local.y1,
        ));
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let recorder = global_recorder();
        let _scope = recorder.scope("masonry.render_prepare.paint");
        let rect = ctx.size().to_rect();
        scene.fill(
            Fill::NonZero,
            masonry::kurbo::Affine::IDENTITY,
            self.editor.theme().base.shell_bg,
            None,
            &rect,
        );
        self.editor
            .paint_in_rect(ctx, scene, self.editor_main_rect(ctx.size()));
        self.sdui.paint(ctx, scene);
        self.paint_status_line(ctx, scene);
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
        let mut children = self.sdui.append_accessibility_children(ctx);
        let metrics = self
            .editor
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
        ChildrenIds::new()
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn accepts_text_input(&self) -> bool {
        true
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
    format!("clay.edit.rejected.{kind}")
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
    use super::{
        ClipboardCommandOutcome, EditorStatus, EditorWidget, SduiStatusObservation,
        character_key_stroke,
    };
    use crate::client::{
        ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientResyncSnapshot,
        ClipboardError, ClipboardSink,
    };
    use crate::editor::EditorCommand;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, CompletionItem, CompletionItemTextFormat,
        CompletionProvenance, CompletionReplacementRange, CompletionResultSet, CompletionStatus,
        DocumentAccess, DocumentMetadata, EditOperation, EditRejection, FileErrorCode, FontRole,
        KeyCode, KeyModifiers, LanguageIntelligenceResult, RuntimeDiagnostic, SduiEditorBinding,
        SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation,
        SduiTreeUpdate,
    };
    use crate::shell::{
        FixedPackagePanel, FixedSlotId, FixedSlotState, PackagePanelVisibility,
        PackageUiComponentTree, PackageUiRuntimeUpdate, PaneSlotLayout,
    };
    use masonry::core::keyboard::{Code, Key, KeyState, KeyboardEvent, Modifiers};

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
            },
            active_typography: crate::protocol::ActiveTypography::default(),
        }
    }

    #[derive(Default)]
    struct FakeClipboard {
        text: Option<String>,
        fail: bool,
    }

    impl ClipboardSink for FakeClipboard {
        fn set_text(&mut self, text: String) -> Result<(), ClipboardError> {
            if self.fail {
                return Err(ClipboardError::new("no display"));
            }
            self.text = Some(text);
            Ok(())
        }

        fn get_text(&mut self) -> Result<String, ClipboardError> {
            if self.fail {
                return Err(ClipboardError::new("no display"));
            }
            Ok(self.text.clone().unwrap_or_default())
        }
    }

    fn completion_result(request_id: u64) -> CompletionResultSet {
        CompletionResultSet {
            request_id,
            client_id: 11,
            document_id: 7,
            document_version: 12,
            behavior_version: 3,
            provider_generation: 1,
            replacement_range: CompletionReplacementRange::new(0, 3),
            status: CompletionStatus::Ok,
            items: vec![CompletionItem {
                label: "println".to_string(),
                insert_text: "println!".to_string(),
                detail: "macro".to_string(),
                commit_characters: ";".to_string(),
                text_format: CompletionItemTextFormat::PlainText,
                provenance: CompletionProvenance::builtin_core(),
            }],
            provenance: CompletionProvenance::builtin_core(),
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
    fn completion_result_installs_bottom_transient_menu_for_active_request() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.active_completion_request_id = Some(4);

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::CompletionResult(
                completion_result(4)
            ))
        );

        let menu = widget
            .sdui
            .active_menu()
            .expect("completion menu installed");
        assert_eq!(menu.prompt(), "Completion");
        assert_eq!(menu.items()[0].label, "println");
        assert_eq!(menu.selected_index(), 0);
    }

    #[test]
    fn stale_completion_result_is_ignored_after_newer_request() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.active_completion_request_id = Some(5);

        assert!(
            !widget.apply_connection_event(ClientConnectionEvent::CompletionResult(
                completion_result(4)
            ))
        );
        assert!(widget.sdui.active_menu().is_none());
    }

    fn language_intelligence_hover_result(request_id: u64) -> LanguageIntelligenceResult {
        use crate::protocol::{
            CompletionProvenance, HoverResult, LanguageIntelligenceFeature,
            LanguageIntelligencePayload, LanguageIntelligenceStatus,
        };
        LanguageIntelligenceResult {
            request_id,
            client_id: 1,
            document_id: 7,
            document_version: 12,
            behavior_version: 3,
            provider_generation: 0,
            feature: LanguageIntelligenceFeature::Hover,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::Hover(HoverResult {
                range: None,
                markdown: "**symbol** <em>docs</em>".to_string(),
            }),
            provenance: CompletionProvenance::builtin_core(),
        }
    }

    #[test]
    fn language_intelligence_result_installs_bottom_transient_menu_for_active_request() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.active_language_intelligence_request_id = Some(4);

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::LanguageIntelligenceResult(
                language_intelligence_hover_result(4)
            ))
        );

        let menu = widget
            .sdui
            .active_menu()
            .expect("language intelligence menu installed");
        assert_eq!(menu.prompt(), "Hover");
        assert!(!menu.items()[0].label.contains("<"));
    }

    #[test]
    fn stale_language_intelligence_result_is_ignored_after_newer_request() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.active_language_intelligence_request_id = Some(5);

        assert!(
            !widget.apply_connection_event(ClientConnectionEvent::LanguageIntelligenceResult(
                language_intelligence_hover_result(4)
            ))
        );
        assert!(widget.sdui.active_menu().is_none());
    }

    #[test]
    fn definition_menu_navigation_jumps_current_document_and_rejects_foreign_document() {
        use crate::protocol::{
            CompletionProvenance, GoToDefinitionResult, LanguageIntelligenceFeature,
            LanguageIntelligencePayload, LanguageIntelligenceStatus, TextByteRange, TextLocation,
        };
        use crate::shell::transient_menu::TransientMenuAction;

        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "abcdefghij".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        widget.editor.set_caret_for_test(0);

        let result = LanguageIntelligenceResult {
            request_id: 8,
            client_id: 1,
            document_id: 7,
            document_version: 12,
            behavior_version: 3,
            provider_generation: 0,
            feature: LanguageIntelligenceFeature::GoToDefinition,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult {
                locations: vec![TextLocation::OpenDocument {
                    document_id: 7,
                    range: TextByteRange {
                        byte_start: 4,
                        byte_end: 6,
                    },
                }],
            }),
            provenance: CompletionProvenance::builtin_core(),
        };
        widget.active_language_intelligence_request_id = Some(8);
        assert!(
            widget
                .apply_connection_event(ClientConnectionEvent::LanguageIntelligenceResult(result))
        );

        let action = widget
            .sdui
            .menu_selected_action()
            .expect("definition action");
        assert!(widget.handle_language_intelligence_menu_action(&action));
        assert_eq!(widget.editor.caret_for_test(), 4);

        // Foreign document targets are accepted as handled but do not move the caret.
        let foreign = TransientMenuAction::new("clay.language.navigateDefinition").with_arguments(
            serde_json::json!({
                "kind": "openDocument",
                "documentId": 99,
                "byteStart": 1,
                "byteEnd": 2,
            }),
        );
        assert!(widget.handle_language_intelligence_menu_action(&foreign));
        assert_eq!(widget.editor.caret_for_test(), 4);
    }

    #[test]
    fn code_action_edit_preview_does_not_mutate_document_text() {
        use crate::shell::transient_menu::TransientMenuAction;

        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "hello".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        let before = widget.editor.document_state().document_version;
        let action = TransientMenuAction::new("clay.language.previewEdit").with_arguments(
            serde_json::json!({
                "title": "Inline preview",
                "previewOnly": true,
            }),
        );
        assert!(widget.handle_language_intelligence_menu_action(&action));
        assert_eq!(widget.editor.document_state().document_version, before);
        assert!(widget.status.runtime_diagnostic.is_some());
    }

    #[test]
    fn copy_selection_writes_selected_text_without_edit_event() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha 🦀 beta".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        widget.editor.command_with_event(EditorCommand::SelectRight);
        widget.editor.command_with_event(EditorCommand::SelectRight);
        let mut clipboard = FakeClipboard::default();

        let event = widget.copy_selection_to_clipboard_with(&mut clipboard);

        assert_eq!(event, None);
        assert_eq!(clipboard.text.as_deref(), Some("al"));
        assert_eq!(widget.editor.visible_text(), "alpha 🦀 beta");
        assert_eq!(widget.next_transaction_id, 1);
    }

    #[test]
    fn copy_selection_is_noop_when_selection_is_collapsed() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        let mut clipboard = FakeClipboard::default();

        let event = widget.copy_selection_to_clipboard_with(&mut clipboard);

        assert_eq!(event, None);
        assert_eq!(clipboard.text, None);
        assert_eq!(widget.editor.visible_text(), "alpha");
    }

    #[test]
    fn copy_selection_failure_reports_runtime_diagnostic() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        widget.editor.command_with_event(EditorCommand::SelectRight);
        let mut clipboard = FakeClipboard {
            fail: true,
            ..FakeClipboard::default()
        };

        let event = widget.copy_selection_to_clipboard_with(&mut clipboard);

        match event {
            Some(ClientConnectionEvent::RuntimeDiagnostic(diagnostic)) => {
                assert_eq!(diagnostic.code, "clay.client.clipboard.write_failed");
                assert!(diagnostic.message.contains("Failed to copy selection"));
            }
            message => panic!("expected clipboard runtime diagnostic, got {message:?}"),
        }
        assert_eq!(widget.editor.visible_text(), "alpha");
    }

    #[test]
    fn cut_selection_copies_and_deletes_selection() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ))
        .with_edit_queue(
            queue
                .with_authority(11, &DocumentAccess::Editable { lease_id: 99 })
                .with_confirmed_version(12),
        );
        widget.editor.load_snapshot(
            7,
            12,
            "alpha 🦀 beta".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        widget.editor.command_with_event(EditorCommand::SelectRight);
        widget.editor.command_with_event(EditorCommand::SelectRight);
        let mut clipboard = FakeClipboard::default();

        let outcome = widget.cut_selection_to_clipboard_with(&mut clipboard);

        assert_eq!(outcome.diagnostic, None);
        assert!(outcome.changed);
        assert_eq!(clipboard.text.as_deref(), Some("al"));
        assert_eq!(widget.editor.visible_text(), "pha 🦀 beta");
        assert_eq!(widget.next_transaction_id, 2);
        let message = receiver.try_recv().expect("cut should enqueue delete edit");
        match message {
            crate::protocol::ClientMessage::Edit {
                operation: crate::protocol::EditOperation::Delete { start, end },
                ..
            } => {
                assert_eq!((start, end), (0, 2));
            }
            other => panic!("expected delete edit, got {other:?}"),
        }
    }

    #[test]
    fn cut_selection_is_noop_when_selection_is_collapsed() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        let mut clipboard = FakeClipboard::default();

        let outcome = widget.cut_selection_to_clipboard_with(&mut clipboard);

        assert_eq!(outcome, ClipboardCommandOutcome::unchanged());
        assert_eq!(clipboard.text, None);
        assert_eq!(widget.editor.visible_text(), "alpha");
    }

    #[test]
    fn cut_selection_failure_reports_runtime_diagnostic_without_deleting() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        widget.editor.command_with_event(EditorCommand::SelectRight);
        let mut clipboard = FakeClipboard {
            fail: true,
            ..FakeClipboard::default()
        };

        let outcome = widget.cut_selection_to_clipboard_with(&mut clipboard);

        match outcome.diagnostic {
            Some(ClientConnectionEvent::RuntimeDiagnostic(diagnostic)) => {
                assert_eq!(diagnostic.code, "clay.client.clipboard.write_failed");
                assert!(diagnostic.message.contains("Failed to cut selection"));
            }
            message => panic!("expected clipboard runtime diagnostic, got {message:?}"),
        }
        assert!(!outcome.changed);
        assert_eq!(widget.editor.visible_text(), "alpha");
    }

    #[test]
    fn paste_clipboard_inserts_and_replaces_selection() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        let mut clipboard = FakeClipboard {
            text: Some("XY".to_string()),
            fail: false,
        };

        let inserted = widget.paste_from_clipboard_with(&mut clipboard);
        assert!(inserted.changed);
        assert_eq!(inserted.diagnostic, None);
        assert_eq!(widget.editor.visible_text(), "XYalpha");

        widget.editor.set_selection_for_test(0, 2);
        clipboard.text = Some("Z".to_string());
        let replaced = widget.paste_from_clipboard_with(&mut clipboard);
        assert!(replaced.changed);
        assert_eq!(widget.editor.visible_text(), "Zalpha");
    }

    #[test]
    fn paste_clipboard_empty_text_is_noop() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        let mut clipboard = FakeClipboard::default();

        let outcome = widget.paste_from_clipboard_with(&mut clipboard);

        assert_eq!(outcome, ClipboardCommandOutcome::unchanged());
        assert_eq!(widget.editor.visible_text(), "alpha");
    }

    #[test]
    fn paste_provided_clipboard_text_inserts_without_rereading_system_clipboard() {
        // masonry_winit delivers Ctrl/Cmd+V as TextEvent::ClipboardPaste(text).
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );

        let inserted = widget.paste_provided_clipboard_text("XY\nZ");
        assert!(inserted.changed);
        assert_eq!(inserted.diagnostic, None);
        assert_eq!(widget.editor.visible_text(), "XY\nZalpha");

        widget.editor.set_selection_for_test(0, 2);
        let replaced = widget.paste_provided_clipboard_text("Q");
        assert!(replaced.changed);
        assert_eq!(widget.editor.visible_text(), "Q\nZalpha");
    }

    #[test]
    fn paste_clipboard_failure_reports_runtime_diagnostic() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        widget.editor.load_snapshot(
            7,
            12,
            "alpha".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        let mut clipboard = FakeClipboard {
            fail: true,
            ..FakeClipboard::default()
        };

        let outcome = widget.paste_from_clipboard_with(&mut clipboard);

        match outcome.diagnostic {
            Some(ClientConnectionEvent::RuntimeDiagnostic(diagnostic)) => {
                assert_eq!(diagnostic.code, "clay.client.clipboard.read_failed");
                assert!(diagnostic.message.contains("Failed to read text"));
            }
            message => panic!("expected clipboard runtime diagnostic, got {message:?}"),
        }
        assert!(!outcome.changed);
        assert_eq!(widget.editor.visible_text(), "alpha");
    }

    #[test]
    fn undo_and_redo_enqueue_ordinary_inverse_edits() {
        let (queue, mut receiver) = ClientEditQueue::bounded(8);
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ))
        .with_edit_queue(
            queue
                .with_authority(11, &DocumentAccess::Editable { lease_id: 99 })
                .with_confirmed_version(12),
        );
        widget.editor.load_snapshot(
            7,
            12,
            "ab".to_string(),
            DocumentAccess::Editable { lease_id: 99 },
        );
        widget
            .editor
            .install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        widget.editor.set_caret_for_test(2);
        let insert_outcome = widget.editor.insert_text_with_event("x");
        assert!(widget.apply_local_edit_outcome(insert_outcome));
        loop {
            match receiver.try_recv() {
                Ok(crate::protocol::ClientMessage::Edit { .. }) => break,
                Ok(_) => continue,
                Err(_) => panic!("insert should enqueue edit"),
            }
        }

        assert!(widget.undo());
        assert_eq!(widget.editor.visible_text(), "ab");
        let undo_message = loop {
            match receiver.try_recv() {
                Ok(message @ crate::protocol::ClientMessage::Edit { .. }) => break message,
                Ok(_) => continue,
                Err(_) => panic!("undo should enqueue delete edit"),
            }
        };
        match undo_message {
            crate::protocol::ClientMessage::Edit {
                operation: EditOperation::Delete { start, end },
                ..
            } => assert_eq!((start, end), (2, 3)),
            other => panic!("expected undo delete, got {other:?}"),
        }

        assert!(widget.redo());
        assert_eq!(widget.editor.visible_text(), "abx");
        let redo_message = loop {
            match receiver.try_recv() {
                Ok(message @ crate::protocol::ClientMessage::Edit { .. }) => break message,
                Ok(_) => continue,
                Err(_) => panic!("redo should enqueue insert edit"),
            }
        };
        match redo_message {
            crate::protocol::ClientMessage::Edit {
                operation: EditOperation::Insert { byte_offset, text },
                ..
            } => {
                assert_eq!(byte_offset, 2);
                assert_eq!(text, "x");
            }
            other => panic!("expected redo insert, got {other:?}"),
        }
    }

    #[test]
    fn read_only_observer_undo_is_noop() {
        let mut widget =
            EditorWidget::with_initial_state(initial_state(DocumentAccess::ReadOnly, 12));
        widget
            .editor
            .load_snapshot(7, 12, "ab".to_string(), DocumentAccess::ReadOnly);
        assert!(!widget.undo());
        assert!(!widget.redo());
        assert_eq!(widget.editor.visible_text(), "ab");
    }

    #[test]
    fn control_character_key_is_available_for_manifest_routing() {
        let event = KeyboardEvent {
            state: KeyState::Down,
            key: Key::Character("o".into()),
            code: Code::KeyO,
            modifiers: Modifiers::CONTROL,
            ..KeyboardEvent::default()
        };

        let stroke = character_key_stroke(&event)
            .expect("control-modified character should produce a routeable stroke");

        assert_eq!(stroke.key, KeyCode::Character("o".to_string()));
        assert_eq!(
            stroke.modifiers,
            KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            }
        );
    }

    #[test]
    fn accessibility_label_uses_placeholder_for_empty_editor() {
        let widget = EditorWidget::default();

        let label = widget.accessibility_label();
        assert!(label.starts_with("Clay native text canvas. Theme default. Clay — Local Fallback"));
        assert!(label.contains("Theme default."));
    }

    #[test]
    fn accessibility_label_updates_after_caret_edit() {
        let mut widget = EditorWidget::default();
        widget.editor.command(EditorCommand::Insert("abc"));
        widget.editor.command(EditorCommand::MoveLeft);
        widget.editor.command(EditorCommand::Insert("X"));

        assert!(widget.accessibility_label().ends_with(". abXc"));
    }

    #[test]
    fn accessibility_label_marks_composing_without_preedit_text() {
        let mut widget = EditorWidget::default();
        widget.editor.command(EditorCommand::Insert("hi"));
        assert!(widget.editor.set_preedit("漢".into(), Some((0, 3))));
        let label = widget.accessibility_label();
        assert!(label.contains("Composing."));
        assert!(!label.contains("漢"));
        assert!(widget.editor.cancel_composition());
        assert!(!widget.accessibility_label().contains("Composing."));
    }

    #[test]
    fn accessibility_label_includes_dirty_and_sanitized_display_name() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 7,
                    version: 12,
                    access: DocumentAccess::Editable { lease_id: 99 },
                    lease_id: Some(99),
                    dirty: true,
                    workspace_root_id: 77,
                    path: "/home/alice/secret/note.md".to_string(),
                },
                text: "hello".to_string(),
            })
        );
        let observation = widget.status_observation();
        assert_eq!(
            observation.document_display_name.as_deref(),
            Some("note.md")
        );
        assert!(observation.dirty);
        assert!(observation.status_text.contains("note.md"));
        assert!(observation.status_text.contains("Dirty"));
        let label = widget.accessibility_label();
        assert!(label.contains("note.md"));
        assert!(label.contains("Dirty"));
        assert!(!label.contains("/home/alice"));
        assert!(label.contains(&observation.status_text));
    }

    #[test]
    fn accessibility_recovery_summary_uses_active_menu_prompt() {
        use crate::shell::transient_menu::{
            TransientMenuAction, TransientMenuItem, TransientMenuSession, TransientMenuSessionId,
        };

        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        let menu = TransientMenuSession::new(TransientMenuSessionId(42), "Reload dirty document?")
            .with_items(vec![
                TransientMenuItem::new(
                    "reload",
                    "Reload",
                    TransientMenuAction::new("clay.documents.serverReloadDocument"),
                )
                .with_accessibility_label("Reload from disk"),
            ]);
        widget.sdui.set_active_menu(menu);
        let observation = widget.status_observation();
        assert_eq!(
            observation.recovery_summary.as_deref(),
            Some("Reload dirty document?")
        );
        assert!(
            observation
                .status_text
                .contains("Recovery: Reload dirty document?")
        );
        assert!(
            widget
                .accessibility_label()
                .contains("Recovery: Reload dirty document?")
        );
    }

    #[test]
    fn local_edit_marks_status_dirty_for_accessibility() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 7,
                    version: 12,
                    access: DocumentAccess::Editable { lease_id: 99 },
                    lease_id: Some(99),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: String::new(),
            })
        );
        assert!(!widget.status_observation().dirty);
        let outcome = widget.editor.insert_text_with_event("y");
        assert!(widget.apply_local_edit_outcome(outcome));
        assert!(widget.status_observation().dirty);
        assert!(widget.accessibility_label().contains("Dirty"));
    }

    #[test]
    fn commit_after_preedit_inserts_once_and_clears_overlay() {
        let mut widget = EditorWidget::default();
        assert!(widget.editor.set_preedit("ni".into(), Some((0, 2))));
        assert!(widget.editor.is_composing());
        // Simulate Ime::Commit semantics used by on_text_event.
        assert!(widget.editor.cancel_composition());
        assert!(widget.editor.command(EditorCommand::Insert("你")));
        assert!(!widget.editor.is_composing());
        assert_eq!(widget.editor.visible_text(), "你");
    }

    #[test]
    fn undo_cancels_unfinished_composition() {
        let mut widget = EditorWidget::default();
        assert!(widget.editor.command(EditorCommand::Insert("ab")));
        assert!(widget.editor.set_preedit("x".into(), None));
        assert!(widget.undo());
        assert!(!widget.editor.is_composing());
        assert_eq!(widget.editor.visible_text(), "");
    }

    #[test]
    fn status_reflects_connecting_state() {
        let widget = EditorWidget::default().with_status(EditorStatus::connecting());

        assert_eq!(
            widget.status_text(),
            "Clay — Connecting — No Server — local document — version unknown"
        );
    }

    #[test]
    fn status_reflects_connected_editable_initial_state() {
        let widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));

        assert_eq!(
            widget.status_text(),
            "Clay — Connected — Editable — doc 7 — v12"
        );
    }

    #[test]
    fn status_reflects_read_only_observer() {
        let widget = EditorWidget::with_initial_state(initial_state(DocumentAccess::ReadOnly, 12));

        assert_eq!(
            widget.status_text(),
            "Clay — Connected — Read-only Observer — doc 7 — v12"
        );
    }

    #[test]
    fn status_reflects_local_fallback_when_no_server() {
        let widget = EditorWidget::default().with_status(EditorStatus::local_fallback());

        assert_eq!(
            widget.status_text(),
            "Clay — Local Fallback — No Server — local document — version unknown"
        );
    }

    #[test]
    fn status_updates_after_edit_ack_or_resync() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::EditAck {
                document_id: 7,
                version: 13,
                transaction_id: 1,
            })
        );

        assert_eq!(
            widget.status_text(),
            "Clay — Connected — Editable — doc 7 — v13"
        );
        assert_eq!(widget.editor.document_state().document_version, 13);
    }

    #[test]
    fn runtime_diagnostic_updates_status_text() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        let diagnostic = RuntimeDiagnostic::error(
            "clay.runtime.syntax_error",
            "JavaScript syntax error while evaluating server-side configuration.",
        );

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
        );

        assert_eq!(
            widget.status_text(),
            "Clay — Connected — Editable — doc 7 — v12 — Runtime clay.runtime.syntax_error: JavaScript syntax error while evaluating server-side configuration."
        );
    }

    #[test]
    fn status_observation_local_fallback_state() {
        let widget = EditorWidget::default().with_status(EditorStatus::local_fallback());

        assert_eq!(
            widget.status_observation(),
            SduiStatusObservation {
                status_text: "Clay — Local Fallback — No Server — local document — version unknown"
                    .to_string(),
                connection_label: "Local Fallback".to_string(),
                access_label: "No Server".to_string(),
                sync_version: None,
                diagnostic_text: None,
                theme_label: "default".to_string(),
                dirty: false,
                document_display_name: None,
                composing: false,
                pending_edit_count: 0,
                recovery_summary: None,
            }
        );
    }

    #[test]
    fn status_observation_connected_editable_with_version() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            4,
        ));

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::EditAck {
                document_id: 7,
                version: 5,
                transaction_id: 1,
            })
        );

        assert_eq!(widget.status_observation().connection_label, "Connected");
        assert_eq!(widget.status_observation().access_label, "Editable");
        assert_eq!(widget.status_observation().sync_version, Some(5));
    }

    #[test]
    fn status_observation_diagnostic_present_after_runtime_diagnostic_event() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        let diagnostic = RuntimeDiagnostic::error(
            "clay.runtime.syntax_error",
            "JavaScript syntax error while evaluating server-side configuration.",
        );

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
        );

        assert_eq!(
            widget.status_observation().diagnostic_text,
            Some(
                "Runtime clay.runtime.syntax_error: JavaScript syntax error while evaluating server-side configuration."
                    .to_string()
            )
        );
    }

    #[test]
    fn status_observation_does_not_regress_accessibility_label() {
        let widget = EditorWidget::with_initial_state(initial_state(DocumentAccess::ReadOnly, 12));
        let observation = widget.status_observation();

        assert!(
            widget
                .accessibility_label()
                .contains(&observation.status_text)
        );
        assert!(
            observation
                .status_text
                .contains(&observation.connection_label)
        );
        assert!(observation.status_text.contains(&observation.access_label));
    }

    #[test]
    fn status_observation_exposes_active_theme_label() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 99 },
            12,
        ));
        assert_eq!(widget.status_observation().theme_label, "default");
        assert!(widget.accessibility_label().contains("Theme default."));

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::ActiveTheme(
                crate::protocol::ActiveTheme {
                    specifier: "@clay/theme-gruvbox-material-dark".to_string(),
                    overrides: Vec::new(),
                },
            ))
        );
        assert_eq!(
            widget.status_observation().theme_label,
            "theme-gruvbox-material-dark"
        );
        assert!(
            widget
                .accessibility_label()
                .contains("Theme theme-gruvbox-material-dark.")
        );
        assert!(crate::editor::theme::status_chrome_meets_contrast(
            &widget.editor.theme()
        ));
    }

    #[test]
    fn resync_event_replaces_editor_snapshot() {
        let mut widget = EditorWidget::default();
        widget.editor.command(EditorCommand::Insert("local"));

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::ResyncSnapshot(
                ClientResyncSnapshot {
                    document_id: 7,
                    version: 12,
                    text: "server 🦀".to_string(),
                    access: DocumentAccess::ReadOnly,
                    lease_id: None,
                },
            ))
        );

        assert_eq!(widget.editor.visible_text(), "server 🦀");
        assert_eq!(widget.editor.document_state().document_id, 7);
        assert_eq!(widget.editor.document_state().document_version, 12);
        assert_eq!(
            widget.editor.document_state().access,
            DocumentAccess::ReadOnly
        );
    }

    #[test]
    fn same_document_resync_preserves_caret_and_updates_edit_authority() {
        let (queue, _receiver) = ClientEditQueue::bounded(2);
        let queue = queue
            .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(3);
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ))
        .with_edit_queue(queue);
        widget.editor.set_caret_for_test(7);

        widget.apply_connection_event(ClientConnectionEvent::ResyncSnapshot(
            ClientResyncSnapshot {
                document_id: 7,
                version: 9,
                text: "server text updated".to_string(),
                access: DocumentAccess::Editable { lease_id: 4 },
                lease_id: Some(4),
            },
        ));

        assert_eq!(widget.editor.caret_for_test(), 7);
        assert_eq!(
            widget
                .edit_queue
                .as_ref()
                .expect("edit queue")
                .sync_snapshot()
                .confirmed_version,
            9
        );
    }

    #[test]
    fn document_opened_event_replaces_editor_snapshot() {
        let mut widget = EditorWidget::default();
        widget.editor.command(EditorCommand::Insert("local"));

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "# opened\n".to_string(),
            })
        );

        assert_eq!(widget.editor.visible_text(), "# opened\n");
        assert_eq!(widget.editor.document_state().document_id, 42);
        assert_eq!(widget.editor.document_state().document_version, 5);
        assert_eq!(
            widget.editor.document_state().access,
            DocumentAccess::Editable { lease_id: 8 }
        );
        assert_eq!(
            widget.status_text(),
            "Clay — Connected — Editable — note.md — doc 42 — v5"
        );
    }

    #[test]
    fn opening_second_file_retains_prior_session_and_switches_active_document() {
        let mut widget = EditorWidget::default();

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "first.md".to_string(),
                },
                text: "# first\n".to_string(),
            })
        );
        widget.status.dirty = true;

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 43,
                    version: 1,
                    access: DocumentAccess::Editable { lease_id: 9 },
                    lease_id: Some(9),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "src/main.rs".to_string(),
                },
                text: "fn main() {}\n".to_string(),
            })
        );

        assert_eq!(widget.editor.visible_text(), "fn main() {}\n");
        assert_eq!(widget.editor.document_state().document_id, 43);
        assert_eq!(widget.retained_session_count(), 1);
        assert!(
            widget.status_text().contains("main.rs — doc 43 — v1"),
            "{}",
            widget.status_text()
        );
        assert!(
            widget.status_text().contains("Open docs: 2"),
            "{}",
            widget.status_text()
        );

        assert!(widget.activate_document(42));
        assert_eq!(widget.editor.document_state().document_id, 42);
        assert!(
            widget.editor.visible_text().contains("# first"),
            "{}",
            widget.editor.visible_text()
        );
        assert_eq!(widget.retained_session_count(), 1);
        assert_eq!(
            widget.status.document_display_name.as_deref(),
            Some("first.md")
        );
    }

    #[test]
    fn activate_document_restores_caret_and_history() {
        let mut widget = EditorWidget::default();
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 1,
                    version: 1,
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    dirty: false,
                    workspace_root_id: 1,
                    path: "a.txt".to_string(),
                },
                text: "abc".to_string(),
            })
        );
        assert!(widget.editor.insert_text_with_event("X").changed);
        let edited = widget.editor.visible_text();
        assert!(edited.contains('X') && edited.contains("abc"), "{edited}");

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 2,
                    version: 1,
                    access: DocumentAccess::Editable { lease_id: 2 },
                    lease_id: Some(2),
                    dirty: false,
                    workspace_root_id: 1,
                    path: "b.txt".to_string(),
                },
                text: "zzz".to_string(),
            })
        );
        assert_eq!(widget.editor.visible_text(), "zzz");

        assert!(widget.activate_document(1));
        assert_eq!(widget.editor.visible_text(), edited);
        assert!(widget.editor.undo_with_event().changed);
        assert_eq!(widget.editor.visible_text(), "abc");
    }

    #[test]
    fn show_open_documents_menu_lists_active_and_retained() {
        let mut widget = EditorWidget::default();
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 10,
                    version: 1,
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    dirty: false,
                    workspace_root_id: 1,
                    path: "one.md".to_string(),
                },
                text: "one".to_string(),
            })
        );
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 11,
                    version: 1,
                    access: DocumentAccess::ReadOnly,
                    lease_id: None,
                    dirty: false,
                    workspace_root_id: 1,
                    path: "two.md".to_string(),
                },
                text: "two".to_string(),
            })
        );
        assert!(widget.show_open_documents_menu());
        let menu = widget.sdui.active_menu().expect("menu");
        assert_eq!(menu.prompt(), "Open documents");
        assert_eq!(menu.items().len(), 2);
        let labels: Vec<_> = menu.items().iter().map(|item| item.label.clone()).collect();
        assert!(
            labels
                .iter()
                .any(|label| label.contains("two.md") && label.contains("active"))
        );
        assert!(labels.iter().any(|label| label.contains("one.md")));
    }

    #[test]
    fn document_saved_clears_dirty_and_keeps_status_chrome_clean() {
        let mut widget = EditorWidget::default();
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "hello".to_string(),
            })
        );
        widget.status.dirty = true;
        assert!(widget.status_observation().dirty);

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentSaved {
                document_id: 42,
                version: 5,
                dirty: false,
            })
        );

        let observation = widget.status_observation();
        assert!(!observation.dirty);
        assert!(!observation.status_text.contains("Dirty"));
        assert_eq!(observation.sync_version, Some(5));
    }

    #[test]
    fn stale_save_conflict_keeps_dirty_and_opens_recovery_menu() {
        let mut widget = EditorWidget::default();
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "local edits".to_string(),
            })
        );
        widget.status.dirty = true;

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::FileOperationFailed {
                code: FileErrorCode::StaleFileMetadata,
                message: "workspace file note.md changed on disk since it was loaded".to_string(),
                workspace_root_id: Some(77),
                document_id: Some(42),
            })
        );

        let observation = widget.status_observation();
        assert!(observation.dirty);
        assert!(observation.status_text.contains("Dirty"));
        assert!(observation.diagnostic_text.as_deref().is_some_and(|text| {
            text.contains("StaleFileMetadata") && text.contains("changed on disk")
        }));
        let menu = widget.sdui.active_menu().expect("conflict menu");
        assert!(menu.prompt().contains("save conflict"));
        let labels: Vec<_> = menu.items().iter().map(|item| item.label.clone()).collect();
        assert!(
            labels
                .iter()
                .any(|label| label.contains("Reload from disk"))
        );
        assert!(
            labels
                .iter()
                .any(|label| label.contains("Keep unsaved edits"))
        );
        assert!(labels.iter().any(|label| label.contains("Compare later")));
        assert_eq!(
            observation.recovery_summary.as_deref(),
            Some("File changed on disk — resolve save conflict")
        );
        assert_eq!(widget.editor.visible_text(), "local edits");
    }

    #[test]
    fn dirty_reload_conflict_offers_save_first_and_keeps_local_text() {
        let mut widget = EditorWidget::default();
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 9,
                    version: 2,
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    dirty: true,
                    workspace_root_id: 3,
                    path: "selected.md".to_string(),
                },
                text: "unsaved".to_string(),
            })
        );
        assert!(widget.status_observation().dirty);

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::FileOperationFailed {
                code: FileErrorCode::DirtyDocument,
                message: "workspace document 9 has unsaved edits".to_string(),
                workspace_root_id: None,
                document_id: Some(9),
            })
        );

        assert!(widget.status_observation().dirty);
        assert_eq!(widget.editor.visible_text(), "unsaved");
        let menu = widget.sdui.active_menu().expect("dirty reload menu");
        assert!(menu.prompt().contains("unsaved edits"));
        let labels: Vec<_> = menu.items().iter().map(|item| item.label.clone()).collect();
        assert!(labels.iter().any(|label| label.contains("Save first")));
        assert!(
            labels
                .iter()
                .any(|label| label.contains("Discard edits and reload"))
        );
    }

    #[test]
    fn document_reloaded_replaces_text_and_clears_dirty() {
        let mut widget = EditorWidget::default();
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "old".to_string(),
            })
        );
        widget.status.dirty = true;

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentReloaded {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 6,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "from disk".to_string(),
            })
        );

        assert_eq!(widget.editor.visible_text(), "from disk");
        assert!(!widget.status_observation().dirty);
        assert_eq!(widget.status_observation().sync_version, Some(6));
        assert!(widget.sdui.active_menu().is_none());
    }

    #[tokio::test]
    async fn save_and_reload_command_intents_enqueue_protocol_file_messages() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(8, &DocumentAccess::Editable { lease_id: 8 })
            .with_confirmed_version(5);
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 8 },
            5,
        ))
        .with_edit_queue(queue);
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "hello".to_string(),
            })
        );

        assert!(widget.request_save_active_document().is_none());
        assert!(widget.request_reload_active_document(true).is_none());

        let save = receiver.recv().await.expect("save message");
        assert!(matches!(
            save,
            ClientMessage::SaveDocument {
                document_id: 42,
                known_version: 5,
                ..
            }
        ));
        let reload = receiver.recv().await.expect("reload message");
        assert!(matches!(
            reload,
            ClientMessage::ReloadDocument {
                document_id: 42,
                known_version: 5,
                force: true,
                ..
            }
        ));
    }

    #[test]
    fn pending_edit_count_increments_on_enqueue_and_decrements_on_ack() {
        let (queue, _receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(8, &DocumentAccess::Editable { lease_id: 8 })
            .with_confirmed_version(5);
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 8 },
            5,
        ))
        .with_edit_queue(queue);

        assert_eq!(widget.status_observation().pending_edit_count, 0);
        assert!(!widget.status_text().contains("Pending edits:"));

        let outcome = widget.editor.insert_text_with_event("x");
        assert!(widget.apply_local_edit_outcome(outcome));
        assert_eq!(widget.status_observation().pending_edit_count, 1);
        assert!(widget.status_text().contains("Pending edits: 1"));

        // Connection task acknowledges before forwarding EditAck; unit-test the same order.
        widget
            .edit_queue
            .as_ref()
            .expect("queue")
            .acknowledge_for_test(7, 6, 1);
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::EditAck {
                document_id: 7,
                version: 6,
                transaction_id: 1,
            })
        );
        assert_eq!(widget.status_observation().pending_edit_count, 0);
        assert!(!widget.status_text().contains("Pending edits:"));
    }

    #[test]
    fn disconnect_updates_status_accessibility_and_opens_recovery_prompt() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 8 },
            5,
        ));
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: true,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "local".to_string(),
            })
        );
        widget.status.dirty = true;

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::ConnectionError(
                "/home/secret/path pipe broken".to_string()
            ))
        );

        let observation = widget.status_observation();
        assert_eq!(observation.connection_label, "Disconnected");
        assert!(observation.status_text.contains("Disconnected"));
        assert!(
            observation
                .status_text
                .contains("Restart Clay to reconnect")
        );
        assert!(observation.recovery_summary.is_some());
        assert!(
            observation
                .diagnostic_text
                .as_deref()
                .is_some_and(|text| text.contains("clay.client.disconnect"))
        );
        // Path sanitization: host path fragments must not leak.
        assert!(!observation.status_text.contains("/home/secret"));
        let menu = widget.sdui.active_menu().expect("disconnect recovery menu");
        assert!(menu.prompt().contains("reconnect"));
        assert!(
            menu.items()
                .iter()
                .any(|item| item.label.contains("Dismiss"))
        );
        assert!(widget.dismiss_recovery());
        assert!(widget.sdui.active_menu().is_none());
        assert!(widget.status.runtime_diagnostic.is_none());
    }

    #[test]
    fn stale_edit_rejection_shows_status_without_blocking_menu_while_auto_resync_runs() {
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 8 },
            5,
        ));
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "hello".to_string(),
            })
        );

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::EditRejected {
                document_id: 42,
                transaction_id: 9,
                reason: EditRejection::StaleVersion {
                    client_base_version: 5,
                    server_version: 8,
                },
            })
        );

        let observation = widget.status_observation();
        assert!(observation.diagnostic_text.as_deref().is_some_and(|text| {
            text.contains("StaleVersion") && text.contains("requesting resync")
        }));
        assert!(observation.recovery_summary.is_some());
        assert!(widget.sdui.active_menu().is_none());
    }

    #[test]
    fn actionable_invalid_range_rejection_opens_resync_dismiss_recovery_menu() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(8, &DocumentAccess::Editable { lease_id: 8 })
            .with_confirmed_version(5);
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 8 },
            5,
        ))
        .with_edit_queue(queue);
        assert!(
            widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
                metadata: DocumentMetadata {
                    document_id: 42,
                    version: 5,
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                    dirty: false,
                    workspace_root_id: 77,
                    path: "note.md".to_string(),
                },
                text: "hello".to_string(),
            })
        );

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::EditRejected {
                document_id: 42,
                transaction_id: 3,
                reason: EditRejection::InvalidRange {
                    message: "byte range is not UTF-8 aligned".to_string(),
                },
            })
        );

        let observation = widget.status_observation();
        assert!(
            observation
                .diagnostic_text
                .as_deref()
                .is_some_and(|text| text.contains("InvalidRange"))
        );
        let menu = widget.sdui.active_menu().expect("recovery menu");
        assert!(menu.prompt().contains("invalid range"));
        let labels: Vec<_> = menu.items().iter().map(|item| item.label.clone()).collect();
        assert!(labels.iter().any(|label| label.contains("Request resync")));
        assert!(labels.iter().any(|label| label.contains("Dismiss")));

        assert!(widget.request_resync_active_document().is_none());
        let message = receiver.try_recv().expect("resync request");
        assert!(matches!(
            message,
            ClientMessage::RequestResync {
                document_id: 42,
                known_version: 5,
                ..
            }
        ));
        assert!(
            widget
                .status
                .runtime_diagnostic
                .as_ref()
                .is_some_and(|diagnostic| diagnostic.code == "clay.editor.resync_requested")
        );

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::ResyncSnapshot(
                ClientResyncSnapshot {
                    document_id: 42,
                    version: 9,
                    text: "canonical".to_string(),
                    access: DocumentAccess::Editable { lease_id: 8 },
                    lease_id: Some(8),
                }
            ))
        );
        assert_eq!(widget.editor.visible_text(), "canonical");
        assert!(widget.status.runtime_diagnostic.is_none());
        assert!(widget.sdui.active_menu().is_none());
    }

    #[tokio::test]
    async fn scrolling_enqueues_new_decoration_viewport_once() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue.with_authority(11, &DocumentAccess::ReadOnly);
        let mut widget = EditorWidget::default().with_edit_queue(queue);
        let text = (0..200)
            .map(|line| format!("const value{line} = {line};\n"))
            .collect::<String>();
        widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
            metadata: DocumentMetadata {
                document_id: 42,
                version: 5,
                access: DocumentAccess::ReadOnly,
                lease_id: None,
                dirty: false,
                workspace_root_id: 77,
                path: "main.ts".to_string(),
            },
            text,
        });

        assert!(widget.editor.scroll_lines(80));
        widget.enqueue_decoration_viewport_request();
        widget.enqueue_decoration_viewport_request();

        assert!(matches!(
            receiver.recv().await.unwrap(),
            ClientMessage::DecorationViewportRequest {
                client_id: 11,
                document_id: 42,
                document_version: 5,
                byte_start,
                byte_end,
            } if byte_start > 0 && byte_end > byte_start
        ));
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn opened_file_edits_continue_as_deltas() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(3);
        let mut widget = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ))
        .with_edit_queue(queue);

        widget.apply_connection_event(ClientConnectionEvent::DocumentOpened {
            metadata: DocumentMetadata {
                document_id: 42,
                version: 5,
                access: DocumentAccess::Editable { lease_id: 8 },
                lease_id: Some(8),
                dirty: false,
                workspace_root_id: 77,
                path: "note.md".to_string(),
            },
            text: "# opened\n".to_string(),
        });
        let outcome = widget.editor.command_with_event(EditorCommand::Insert("!"));
        let edit_event = outcome.edit_event.expect("insert emits an edit event");
        widget
            .edit_queue
            .as_ref()
            .unwrap()
            .enqueue_edit_event(edit_event, 99)
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::Edit {
                document_id: 42,
                client_id: 11,
                lease_id: Some(8),
                base_version: 5,
                behavior_version: 3,
                transaction_id: 99,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "!".to_string(),
                },
            }
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
        let before_text = widget.editor.visible_text();
        let before_version = widget.editor.document_state().document_version;

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

        assert_eq!(widget.editor.visible_text(), before_text);
        assert_eq!(
            widget.editor.document_state().document_version,
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
        widget.editor.command(EditorCommand::Insert(" local"));
        widget.apply_connection_event(ClientConnectionEvent::SduiSnapshot {
            client_id: 11,
            tree: sdui_tree("Ready"),
        });
        let before_text = widget.editor.visible_text();
        let before_document = widget.editor.document_state().clone();

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

        assert_eq!(widget.editor.visible_text(), before_text);
        assert_eq!(widget.editor.document_state(), &before_document);
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
                        kind: "panel".to_string(),
                        font_role: FontRole::Ui,
                        text_variant: None,
                        title: Some("Preview".to_string()),
                        text: None,
                        label: None,
                        action_command_id: None,
                        items: Vec::new(),
                        children: Vec::new(),
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
            .editor
            .command(EditorCommand::Insert("\nsecond line\nthird line"));
        widget.editor.set_caret_for_test(6);
        widget.editor.set_visual_scroll_bounds_for_test(120.0);
        assert!(widget.editor.scroll_vertical_pixels(40.0));
        let before_text = widget.editor.visible_text();
        let before_caret = widget.editor.caret_for_test();
        let before_scroll_y = widget.editor.visual_scroll_y();
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
        assert_eq!(widget.editor.visible_text(), before_text);
        assert_eq!(widget.editor.caret_for_test(), before_caret);
        assert_eq!(widget.editor.visual_scroll_y(), before_scroll_y);
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
                        kind: "panel".to_string(),
                        font_role: FontRole::Ui,
                        text_variant: None,
                        title: Some("old".to_string()),
                        text: None,
                        label: None,
                        action_command_id: None,
                        items: Vec::new(),
                        children: Vec::new(),
                    },
                    Vec::new(),
                )],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .expect("seed package ui");
        assert_eq!(widget.sdui.package_ui_version(), 1);

        let g1_behavior = widget.editor.document_state().behavior_version;
        assert_eq!(widget.runtime_generation_id, 0);

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeStateSnapshot(Box::new(
                runtime_snapshot(2, 11, 7)
            )))
        );

        assert_eq!(widget.runtime_generation_id, 2);
        assert_eq!(widget.editor.document_state().behavior_version, 2);
        assert_ne!(widget.editor.document_state().behavior_version, g1_behavior);
        assert_eq!(widget.editor.typography().revision(), 2);
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

        let before_behavior = widget.editor.document_state().behavior_manifest.clone();
        let before_theme = widget.editor.theme();
        let before_typography = widget.editor.typography().revision();
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
            widget.editor.document_state().behavior_manifest,
            before_behavior
        );
        assert_eq!(widget.editor.theme(), before_theme);
        assert_eq!(widget.editor.typography().revision(), before_typography);
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
            .editor
            .command(EditorCommand::Insert("alpha beta gamma"));
        widget.editor.set_selection_for_test(6, 10);
        widget.editor.set_visual_scroll_bounds_for_test(80.0);
        assert!(widget.editor.scroll_vertical_pixels(24.0));

        let before_caret = widget.editor.caret_for_test();
        let before_selection = widget.editor.selection_for_test();
        let before_scroll = widget.editor.visual_scroll_y();
        let before_connection = widget.status_observation().connection_label.clone();

        assert!(
            widget.apply_connection_event(ClientConnectionEvent::RuntimeStateSnapshot(Box::new(
                runtime_snapshot(4, 11, 7)
            )))
        );

        assert_eq!(widget.runtime_generation_id, 4);
        assert_eq!(widget.editor.caret_for_test(), before_caret);
        assert_eq!(widget.editor.selection_for_test(), before_selection);
        assert_eq!(widget.editor.visual_scroll_y(), before_scroll);
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
