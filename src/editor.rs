use crop::Rope;
use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::{Affine, Circle, Rect};
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::{Color, Fill};
use masonry::{TextAlign, TextAlignOptions};

const BACKGROUND_COLOR: Color = Color::from_rgb8(0x18, 0x18, 0x18);
const PANEL_COLOR: Color = Color::from_rgb8(0x24, 0x24, 0x24);
const ACCENT_COLOR: Color = Color::from_rgb8(0x8a, 0x6f, 0xff);
const TEXT_COLOR: Color = Color::from_rgb8(0xf4, 0xf1, 0xff);
const PLACEHOLDER_COLOR: Color = Color::from_rgb8(0x8d, 0x86, 0xa3);
const TEXT_INSET: f64 = 48.0;
const TEXT_FONT_SIZE: f32 = 20.0;
const PLACEHOLDER_TEXT: &str = "Start typing in the Clay native text canvas…";

#[derive(Debug, Default)]
pub struct EditorBuffer {
    rope: Rope,
}

impl EditorBuffer {
    pub fn insert_str(&mut self, text: &str) {
        self.rope.insert(self.rope.byte_len(), text);
    }

    pub fn backspace(&mut self) {
        let Some(last_char) = self.rope.chars().next_back() else {
            return;
        };

        let end = self.rope.byte_len();
        self.rope.delete(end - last_char.len_utf8()..end);
    }

    pub fn visible_text(&self) -> String {
        self.rope.to_string()
    }
}

#[derive(Debug, Default)]
pub struct EditorSurface {
    buffer: EditorBuffer,
}

impl EditorSurface {
    pub fn insert_text(&mut self, text: &str) -> bool {
        if !is_printable_text(text) {
            return false;
        }

        self.buffer.insert_str(text);
        true
    }

    pub fn backspace(&mut self) {
        self.buffer.backspace();
    }

    pub fn visible_text(&self) -> String {
        self.buffer.visible_text()
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut masonry::vello::Scene) {
        let size = ctx.size();
        let width = size.width;
        let height = size.height;

        let canvas = Rect::new(
            24.0,
            24.0,
            (width - 24.0).max(24.0),
            (height - 24.0).max(24.0),
        );
        scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_COLOR, None, &canvas);

        let radius = (width.min(height) * 0.12).clamp(32.0, 96.0);
        let circle = Circle::new((width - 72.0, height - 72.0), radius);
        scene.fill(Fill::NonZero, Affine::IDENTITY, ACCENT_COLOR, None, &circle);

        self.paint_text(ctx, scene, (width - (TEXT_INSET * 2.0)).max(1.0) as f32);
    }

    fn paint_text(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        max_width: f32,
    ) {
        let current_text = self.buffer.visible_text();
        let (display_text, color) = if current_text.is_empty() {
            (PLACEHOLDER_TEXT, PLACEHOLDER_COLOR)
        } else {
            (current_text.as_str(), TEXT_COLOR)
        };

        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, display_text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(TEXT_FONT_SIZE));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.4)));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));

        let mut layout = builder.build(display_text);
        layout.break_all_lines(Some(max_width));
        layout.align(
            Some(max_width),
            TextAlign::Start,
            TextAlignOptions::default(),
        );

        render_text(
            scene,
            Affine::translate((TEXT_INSET, TEXT_INSET)),
            &layout,
            &[color.into()],
            true,
        );
    }
}

pub fn is_printable_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| !character.is_control())
}

pub fn background_color() -> Color {
    BACKGROUND_COLOR
}
