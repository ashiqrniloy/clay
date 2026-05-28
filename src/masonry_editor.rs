use masonry::accesskit::{Node, Role};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, BrushIndex, ChildrenIds, EventCtx, KeyboardEvent,
    LayoutCtx, PaintCtx, PointerButton, PointerEvent, PointerScrollEvent, PropertiesMut,
    PropertiesRef, RegisterCtx, ScrollDelta, TextEvent, Widget, render_text,
};
use masonry::kurbo::{Affine, Size};
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;

use crate::client::{ClientConnectionEvent, ClientEditQueue, ClientInitialState};
use crate::editor::{EditorCommand, EditorCommandOutcome, EditorSurface, background_color};
use crate::protocol::{
    BehaviorManifest, DocumentAccess, DocumentId, DocumentVersion, KeyCode, KeyModifiers, KeyStroke,
};

const STATUS_BAR_HEIGHT: f64 = 28.0;
const STATUS_TEXT_SIZE: f32 = 12.0;
const STATUS_BACKGROUND: Color = Color::from_rgb8(0x18, 0x18, 0x1f);
const STATUS_TEXT_COLOR: Color = Color::from_rgb8(0xd7, 0xd2, 0xe8);

#[derive(Debug, PartialEq, Eq)]
pub enum EditorAction {
    ExitRequested,
    ClientConnection(ClientConnectionEvent),
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
}

impl EditorStatus {
    pub fn connecting() -> Self {
        Self {
            connection: EditorConnectionStatus::Connecting,
            document_id: None,
            version: None,
            access: None,
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
        }
    }

    pub fn local_fallback() -> Self {
        Self {
            connection: EditorConnectionStatus::LocalFallback,
            document_id: None,
            version: None,
            access: None,
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

    fn text(&self) -> String {
        let connection = match self.connection {
            EditorConnectionStatus::Connecting => "Connecting",
            EditorConnectionStatus::Connected => "Connected",
            EditorConnectionStatus::LocalFallback => "Local Fallback",
            EditorConnectionStatus::Disconnected => "Disconnected",
        };
        let access = match &self.access {
            Some(DocumentAccess::Editable { .. }) => "Editable",
            Some(DocumentAccess::ReadOnly) => "Read-only Observer",
            None => "No Server",
        };
        let version = self
            .version
            .map(|version| format!("v{version}"))
            .unwrap_or_else(|| "version unknown".to_string());
        let document = self
            .document_id
            .map(|document_id| format!("doc {document_id}"))
            .unwrap_or_else(|| "local document".to_string());

        format!("Clay — {connection} — {access} — {document} — {version}")
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
            ClientConnectionEvent::BehaviorManifestInstalled { manifest, .. } => {
                self.editor.install_behavior_manifest(manifest);
                false
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
        self.status.text()
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
        let outcome = self.editor.route_key_with_event(&key);
        self.finish_local_outcome(ctx, outcome.command_outcome);
        if outcome.server_intent.is_some() {
            ctx.set_handled();
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
                ctx.capture_pointer();
                (self.editor.place_caret_at_point(point), true)
            }
            PointerEvent::Move(pointer_update) if ctx.is_active() => {
                let point = ctx.local_position(pointer_update.current.position);
                (self.editor.extend_selection_to_point(point), true)
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
                    Key::Character(text)
                        if !key_event.modifiers.ctrl() && !key_event.modifiers.meta() =>
                    {
                        self.local_key(
                            ctx,
                            key_stroke(KeyCode::Character(text.to_string()), key_event),
                        );
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
        let rect = ctx.size().to_rect();
        scene.fill(
            Fill::NonZero,
            masonry::kurbo::Affine::IDENTITY,
            background_color(),
            None,
            &rect,
        );
        self.editor.paint(ctx, scene);
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
    use super::{EditorStatus, EditorWidget};
    use crate::client::{ClientConnectionEvent, ClientInitialState, ClientResyncSnapshot};
    use crate::editor::EditorCommand;
    use crate::protocol::{BehaviorManifest, DocumentAccess};

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
}
