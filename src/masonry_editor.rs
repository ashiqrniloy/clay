use masonry::accesskit::{Node, Role};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, PaintCtx,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Widget,
};
use masonry::kurbo::Size;
use masonry::peniko::Fill;
use masonry::vello::Scene;

use crate::editor::{EditorSurface, background_color};

#[derive(Debug)]
pub enum EditorAction {
    ExitRequested,
}

#[derive(Debug, Default)]
pub struct EditorWidget {
    editor: EditorSurface,
}

impl EditorWidget {
    fn edit(&mut self, ctx: &mut EventCtx<'_>, changed: bool) {
        if changed {
            ctx.request_render();
            ctx.request_accessibility_update();
            ctx.set_handled();
        }
    }
}

impl Widget for EditorWidget {
    type Action = EditorAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &PointerEvent,
    ) {
        ctx.request_focus();
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(key_event) if key_event.state == KeyState::Down => {
                match &key_event.key {
                    Key::Named(NamedKey::Escape) => {
                        ctx.submit_action::<Self::Action>(EditorAction::ExitRequested);
                        ctx.set_handled();
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.editor.backspace();
                        self.edit(ctx, true);
                    }
                    Key::Character(text) => {
                        let changed = self.editor.insert_text(text);
                        self.edit(ctx, changed);
                    }
                    _ => {}
                }
            }
            TextEvent::Ime(masonry::core::Ime::Commit(text)) => {
                let changed = self.editor.insert_text(text);
                self.edit(ctx, changed);
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
        let text = self.editor.visible_text();
        node.set_label(if text.is_empty() {
            "Clay native text canvas".to_string()
        } else {
            text
        });
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
