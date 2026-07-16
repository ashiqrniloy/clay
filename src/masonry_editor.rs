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
    ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientUiCommandRoute,
    ClipboardSink, SystemClipboard,
};
use crate::editor::{
    EditorCommand, EditorCommandOutcome, EditorSurface,
    typography::{UiTextMetrics, UiTextVariant},
};
use crate::masonry_sdui::{SduiNativeState, editor_region_for_document};
use crate::perf::metrics::global_recorder;
use crate::protocol::{
    BehaviorManifest, CompletionRequestId, CompletionResultSet, DocumentAccess, DocumentId,
    DocumentVersion, FontRole, KeyCode, KeyModifiers, KeyStroke, LanguageIntelligenceRequestId,
    LanguageIntelligenceResult, RuntimeDiagnostic,
};

#[allow(
    clippy::large_enum_variant,
    reason = "editor event channel is low-volume; boxing would add churn without measured benefit"
)]
#[derive(Debug, PartialEq)]
pub enum EditorAction {
    ExitRequested,
    ClientConnection(ClientConnectionEvent),
    ClientUiCommand(ClientUiCommandRoute),
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
}

impl EditorStatus {
    pub fn connecting() -> Self {
        Self {
            connection: EditorConnectionStatus::Connecting,
            document_id: None,
            version: None,
            access: None,
            runtime_diagnostic: None,
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
        }
    }

    pub fn local_fallback() -> Self {
        Self {
            connection: EditorConnectionStatus::LocalFallback,
            document_id: None,
            version: None,
            access: None,
            runtime_diagnostic: None,
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
        self.document_id
            .map(|document_id| format!("doc {document_id}"))
            .unwrap_or_else(|| "local document".to_string())
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
        editor.set_theme(crate::editor::theme::StyleRegistry::from_active_theme(
            &initial_state.active_theme,
        ));
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
                let version_changed = self.editor.note_confirmed_version(document_id, version);
                let next_status = EditorStatus::connected(
                    document_id,
                    version,
                    self.editor.document_state().access.clone(),
                );
                let status_changed = self.set_status(next_status);
                version_changed || status_changed
            }
            ClientConnectionEvent::ResyncSnapshot(snapshot) => {
                self.editor.load_resync_snapshot(
                    snapshot.document_id,
                    snapshot.version,
                    snapshot.text,
                    snapshot.access.clone(),
                );
                if let Some(queue) = self.edit_queue.as_mut() {
                    queue.update_opened_document_authority(&snapshot.access, snapshot.version);
                }
                self.set_status(EditorStatus::connected(
                    snapshot.document_id,
                    snapshot.version,
                    snapshot.access,
                ));
                true
            }
            ClientConnectionEvent::DocumentOpened { metadata, text } => {
                self.editor.load_snapshot(
                    metadata.document_id,
                    metadata.version,
                    text,
                    metadata.access.clone(),
                );
                if let Some(queue) = self.edit_queue.as_mut() {
                    queue.update_opened_document_authority(&metadata.access, metadata.version);
                }
                let jumped = self
                    .take_pending_definition_navigation_for_path(&metadata.path)
                    .map(|pending| self.editor.navigate_to_byte_offset(pending.byte_start))
                    .unwrap_or(false);
                let status_changed = self.set_status(EditorStatus::connected(
                    metadata.document_id,
                    metadata.version,
                    metadata.access,
                ));
                jumped || status_changed
            }
            ClientConnectionEvent::FileOperationFailed { code, message, .. } => {
                let mut next_status = self.status.clone();
                next_status.runtime_diagnostic = Some(RuntimeDiagnostic::error(
                    format!("clay.file.{code:?}"),
                    message,
                ));
                self.set_status(next_status)
            }
            ClientConnectionEvent::BehaviorManifestInstalled { manifest, .. } => {
                self.editor.install_behavior_manifest(manifest);
                false
            }
            ClientConnectionEvent::ActiveTheme(theme) => {
                self.editor
                    .set_theme(crate::editor::theme::StyleRegistry::from_active_theme(
                        &theme,
                    ));
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
            ClientConnectionEvent::Disconnected | ClientConnectionEvent::ConnectionError(_) => {
                let next_status = EditorStatus {
                    connection: EditorConnectionStatus::Disconnected,
                    ..self.status.clone().with_document_values(
                        self.editor.document_state().document_id,
                        self.editor.document_state().document_version,
                        self.editor.document_state().access.clone(),
                    )
                };
                self.set_status(next_status)
            }
            _ => false,
        }
    }

    pub fn status_text(&self) -> String {
        self.status_observation().status_text
    }

    pub(crate) fn status_observation(&self) -> SduiStatusObservation {
        self.status.observation()
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
            if self.handle_language_intelligence_menu_action(&local_action) {
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
        let status = self.status_text();
        let text = self.editor.visible_text();
        if text.is_empty() {
            format!("Clay native text canvas. {status}")
        } else {
            format!("{status}. {text}")
        }
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
    let Key::Character(text) = &key_event.key else {
        return false;
    };
    if !text.eq_ignore_ascii_case("c") {
        return false;
    }

    if cfg!(target_os = "macos") {
        key_event.modifiers.meta()
    } else {
        key_event.modifiers.ctrl()
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
                    (self.editor.place_caret_at_point(local_point), true)
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
                    Key::Character(_) => {
                        if let Some(stroke) = character_key_stroke(key_event) {
                            self.local_key(ctx, stroke);
                        }
                    }
                    _ => {}
                }
            }
            TextEvent::Ime(masonry::core::Ime::Commit(text)) => {
                self.local_command(ctx, EditorCommand::Insert(text));
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
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(900.0, 600.0))
        }
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
        let mut status = Node::new(Role::Status);
        status.set_label(self.status_text());
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

#[cfg(test)]
mod tests {
    use super::{EditorStatus, EditorWidget, SduiStatusObservation, character_key_stroke};
    use crate::client::{
        ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientResyncSnapshot,
        ClipboardError, ClipboardSink,
    };
    use crate::editor::EditorCommand;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, CompletionItem, CompletionItemTextFormat,
        CompletionProvenance, CompletionReplacementRange, CompletionResultSet, CompletionStatus,
        DocumentAccess, DocumentMetadata, EditOperation, FontRole, KeyCode, KeyModifiers,
        LanguageIntelligenceResult, RuntimeDiagnostic, SduiEditorBinding, SduiFlexDirection,
        SduiNode, SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation, SduiTreeUpdate,
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

        assert!(
            widget
                .accessibility_label()
                .starts_with("Clay native text canvas. Clay — Local Fallback")
        );
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
            "Clay — Connected — Editable — doc 42 — v5"
        );
    }

    #[test]
    fn opening_second_file_browser_file_replaces_editor_snapshot() {
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
        assert_eq!(widget.editor.document_state().document_version, 1);
        assert_eq!(
            widget.status_text(),
            "Clay — Connected — Editable — doc 43 — v1"
        );
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
}
