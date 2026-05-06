use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::Affine;
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::Color;
use masonry::{TextAlign, TextAlignOptions};

use super::surface::{TEXT_FONT_SIZE, TEXT_INSET};

#[derive(Debug, Default)]
pub struct LayoutState;

impl LayoutState {
    pub fn paint_text(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        display_text: &str,
        color: Color,
        max_width: f32,
    ) {
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
