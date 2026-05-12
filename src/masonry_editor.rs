use masonry::accesskit::{Node, Role};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, KeyboardEvent, LayoutCtx,
    PaintCtx, PointerButton, PointerEvent, PointerScrollEvent, PropertiesMut, PropertiesRef,
    RegisterCtx, ScrollDelta, TextEvent, Widget,
};
use masonry::kurbo::Size;
use masonry::peniko::Fill;
use masonry::vello::Scene;

use crate::client::{ClientConnectionEvent, ClientEditQueue, ClientInitialState};
use crate::editor::{EditorCommand, EditorCommandOutcome, EditorSurface, background_color};
use crate::protocol::{BehaviorManifest, KeyCode, KeyModifiers, KeyStroke};

#[derive(Debug)]
pub enum EditorAction {
    ExitRequested,
}

#[derive(Debug)]
pub struct EditorWidget {
    editor: EditorSurface,
    edit_queue: Option<ClientEditQueue>,
    next_transaction_id: u64,
}

impl Default for EditorWidget {
    fn default() -> Self {
        let mut editor = EditorSurface::default();
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(0));
        Self {
            editor,
            edit_queue: None,
            next_transaction_id: 1,
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
            initial_state.access,
        );
        editor.install_behavior_manifest(initial_state.behavior_manifest);
        Self {
            editor,
            edit_queue: None,
            next_transaction_id: 1,
        }
    }

    pub fn with_edit_queue(mut self, edit_queue: ClientEditQueue) -> Self {
        self.edit_queue = Some(edit_queue);
        self
    }

    pub fn apply_connection_event(&mut self, event: ClientConnectionEvent) -> bool {
        match event {
            ClientConnectionEvent::ResyncSnapshot(snapshot) => {
                self.editor.load_snapshot(
                    snapshot.document_id,
                    snapshot.version,
                    snapshot.text,
                    snapshot.access,
                );
                true
            }
            ClientConnectionEvent::BehaviorManifestInstalled { manifest, .. } => {
                self.editor.install_behavior_manifest(manifest);
                false
            }
            _ => false,
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
        let text = self.editor.visible_text();
        if text.is_empty() {
            "Clay native text canvas".to_string()
        } else {
            text
        }
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
    use super::EditorWidget;
    use crate::client::{ClientConnectionEvent, ClientResyncSnapshot};
    use crate::editor::EditorCommand;
    use crate::protocol::DocumentAccess;

    #[test]
    fn accessibility_label_uses_placeholder_for_empty_editor() {
        let widget = EditorWidget::default();

        assert_eq!(widget.accessibility_label(), "Clay native text canvas");
    }

    #[test]
    fn accessibility_label_updates_after_caret_edit() {
        let mut widget = EditorWidget::default();
        widget.editor.command(EditorCommand::Insert("abc"));
        widget.editor.command(EditorCommand::MoveLeft);
        widget.editor.command(EditorCommand::Insert("X"));

        assert_eq!(widget.accessibility_label(), "abXc");
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
