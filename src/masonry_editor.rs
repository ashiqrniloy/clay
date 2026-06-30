use std::path::PathBuf;

use masonry::accesskit::{Node, Role};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, BrushIndex, ChildrenIds, EventCtx, KeyboardEvent,
    LayoutCtx, PaintCtx, PointerButton, PointerEvent, PointerScrollEvent, PropertiesMut,
    PropertiesRef, RegisterCtx, ScrollDelta, TextEvent, Widget, render_text,
};
use masonry::kurbo::{Affine, Point, Rect, Size};
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;

use crate::client::{
    ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientUiCommandRoute,
};
use crate::editor::{EditorCommand, EditorCommandOutcome, EditorSurface, background_color};
use crate::masonry_sdui::{SduiNativeState, editor_region_for_document};
use crate::perf::metrics::global_recorder;
use crate::protocol::{
    BehaviorManifest, DocumentAccess, DocumentId, DocumentVersion, KeyCode, KeyModifiers,
    KeyStroke, RuntimeDiagnostic,
};

const STATUS_BAR_HEIGHT: f64 = 28.0;
const STATUS_TEXT_SIZE: f32 = 12.0;
const STATUS_BACKGROUND: Color = Color::from_rgb8(0x18, 0x18, 0x1f);
const STATUS_TEXT_COLOR: Color = Color::from_rgb8(0xd7, 0xd2, 0xe8);

#[allow(
    clippy::large_enum_variant,
    reason = "editor event channel is low-volume; boxing would add churn without measured benefit"
)]
#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct EditorWidget {
    editor: EditorSurface,
    edit_queue: Option<ClientEditQueue>,
    next_transaction_id: u64,
    status: EditorStatus,
    sdui: SduiNativeState,
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
            status,
            sdui: SduiNativeState::empty(),
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
        let status = EditorStatus::connected(
            initial_state.document_id,
            initial_state.document_version,
            initial_state.access,
        );
        Self {
            editor,
            edit_queue: None,
            next_transaction_id: 1,
            status,
            sdui: SduiNativeState::empty(),
        }
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
                self.editor.load_snapshot(
                    snapshot.document_id,
                    snapshot.version,
                    snapshot.text,
                    snapshot.access.clone(),
                );
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
                self.set_status(EditorStatus::connected(
                    metadata.document_id,
                    metadata.version,
                    metadata.access,
                ));
                true
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
            ClientConnectionEvent::SduiSnapshot { tree, .. } => {
                self.sdui.apply_snapshot(tree);
                true
            }
            ClientConnectionEvent::SduiUpdate(update) => self.sdui.apply_update(update),
            ClientConnectionEvent::DecorationSet(set) => self.editor.apply_decoration_set(set),
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

    fn set_status(&mut self, status: EditorStatus) -> bool {
        if self.status == status {
            return false;
        }
        self.status = status;
        true
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
        self.finish_local_outcome(ctx, outcome.command_outcome);
        if let Some(command) = outcome.client_ui_command {
            ctx.submit_action::<EditorAction>(EditorAction::ClientUiCommand(command));
            ctx.set_handled();
        } else if let Some(intent) = outcome.server_intent {
            if let Some(edit_queue) = &self.edit_queue {
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
            KeyCode::Enter => {
                if let Some(intent) = self.sdui.menu_activate_selected() {
                    if let Some(edit_queue) = &self.edit_queue {
                        let document = self.editor.document_state();
                        let _ = edit_queue.enqueue_command_intent(
                            document.document_id,
                            document.behavior_version,
                            intent.command_id,
                        );
                    }
                }
                self.sdui.clear_active_menu();
                ctx.request_render();
                ctx.set_handled();
                true
            }
            KeyCode::Escape => {
                self.sdui.menu_cancel();
                self.sdui.clear_active_menu();
                ctx.request_render();
                ctx.set_handled();
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
            ctx.request_render();
            ctx.request_accessibility_update();
            ctx.set_handled();
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
        let y0 = (size.height - STATUS_BAR_HEIGHT).max(0.0);
        let rect = masonry::kurbo::Rect::new(0.0, y0, size.width.max(0.0), size.height.max(y0));
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            STATUS_BACKGROUND,
            None,
            &rect,
        );

        let status = self.status_text();
        let max_width = (size.width - 24.0).max(1.0) as f32;
        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, &status, 1.0, true);
        builder.push_default(StyleProperty::FontSize(STATUS_TEXT_SIZE));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.2)));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        let mut layout = builder.build(&status);
        layout.break_all_lines(Some(max_width));
        render_text(
            scene,
            Affine::translate((12.0, y0 + 7.0)),
            &layout,
            &[STATUS_TEXT_COLOR.into()],
            true,
        );
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
            PointerEvent::Scroll(PointerScrollEvent { delta, .. }) => {
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
                        ctx.submit_action::<Self::Action>(EditorAction::ExitRequested);
                        ctx.set_handled();
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
            background_color(),
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
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.accessibility_label());
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
    };
    use crate::editor::EditorCommand;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, DocumentAccess, DocumentMetadata, EditOperation, KeyCode,
        KeyModifiers, RuntimeDiagnostic, SduiEditorBinding, SduiFlexDirection, SduiNode,
        SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation, SduiTreeUpdate,
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
        }
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
