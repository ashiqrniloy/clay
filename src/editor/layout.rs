use std::fmt;

use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::{Affine, Rect};
use masonry::parley::Layout;
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::Color;
use masonry::{TextAlign, TextAlignOptions};

use super::surface::{TEXT_FONT_SIZE, TEXT_INSET};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualLayoutMetrics {
    pub visual_line_count: usize,
    pub height: f32,
}

impl VisualLayoutMetrics {
    pub fn max_scroll_y(self, available_height: f64) -> f64 {
        (self.height as f64 - available_height.max(0.0)).max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutCacheKey {
    text_revision: u64,
    viewport_revision: u64,
    max_width: f32,
}

impl LayoutCacheKey {
    pub fn new(text_revision: u64, viewport_revision: u64, max_width: f32) -> Self {
        Self {
            text_revision,
            viewport_revision,
            max_width,
        }
    }
}

#[derive(Default)]
pub struct LayoutState {
    cached: Option<CachedLayout>,
}

struct CachedLayout {
    key: LayoutCacheKey,
    layout: Layout<BrushIndex>,
}

impl fmt::Debug for LayoutState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LayoutState")
            .field("cached_key", &self.cached.as_ref().map(|cached| cached.key))
            .finish()
    }
}

impl LayoutState {
    pub fn paint_text(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        display_text: &str,
        color: Color,
        max_width: f32,
        scroll_y: &mut f64,
        follow_visual_end: bool,
        available_height: f64,
        key: LayoutCacheKey,
    ) -> VisualLayoutMetrics {
        if self.should_rebuild(key, ctx.fonts_changed()) {
            self.rebuild(ctx, display_text, max_width, key);
        }

        let cached = self
            .cached
            .as_ref()
            .expect("layout cache must contain a layout after rebuild check");
        let metrics = Self::visual_metrics(&cached.layout);
        let max_scroll_y = metrics.max_scroll_y(available_height);
        if follow_visual_end {
            *scroll_y = max_scroll_y;
        } else {
            *scroll_y = scroll_y.clamp(0.0, max_scroll_y);
        }

        let clip = Rect::new(
            TEXT_INSET,
            TEXT_INSET,
            TEXT_INSET + max_width as f64,
            TEXT_INSET + available_height,
        );
        scene.push_clip_layer(Affine::IDENTITY, &clip);
        render_text(
            scene,
            Affine::translate((TEXT_INSET, TEXT_INSET - *scroll_y)),
            &cached.layout,
            &[color.into()],
            true,
        );
        scene.pop_layer();
        metrics
    }

    fn should_rebuild(&self, key: LayoutCacheKey, fonts_changed: bool) -> bool {
        fonts_changed || self.cached.as_ref().is_none_or(|cached| cached.key != key)
    }

    fn visual_metrics(layout: &Layout<BrushIndex>) -> VisualLayoutMetrics {
        VisualLayoutMetrics {
            visual_line_count: layout.len(),
            height: layout.height(),
        }
    }

    fn rebuild(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        display_text: &str,
        max_width: f32,
        key: LayoutCacheKey,
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

        self.cached = Some(CachedLayout { key, layout });
    }

    #[cfg(test)]
    fn build_layout_for_test(display_text: &str, max_width: f32) -> Layout<BrushIndex> {
        let mut font_context = masonry::parley::FontContext::new();
        let mut layout_context = masonry::parley::LayoutContext::new();
        let mut builder = layout_context.ranged_builder(&mut font_context, display_text, 1.0, true);
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
        layout
    }

    #[cfg(test)]
    fn set_cached_key_for_test(&mut self, key: LayoutCacheKey) {
        self.cached = Some(CachedLayout {
            key,
            layout: Layout::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutCacheKey, LayoutState, VisualLayoutMetrics};

    #[test]
    fn layout_cache_reuses_unchanged_key() {
        let key = LayoutCacheKey::new(1, 2, 300.0);
        let mut cache = LayoutState::default();
        cache.set_cached_key_for_test(key);

        assert!(!cache.should_rebuild(key, false));
    }

    #[test]
    fn layout_cache_invalidates_on_text_revision() {
        let mut cache = LayoutState::default();
        cache.set_cached_key_for_test(LayoutCacheKey::new(1, 2, 300.0));

        assert!(cache.should_rebuild(LayoutCacheKey::new(2, 2, 300.0), false));
    }

    #[test]
    fn layout_cache_invalidates_on_width_change() {
        let mut cache = LayoutState::default();
        cache.set_cached_key_for_test(LayoutCacheKey::new(1, 2, 300.0));

        assert!(cache.should_rebuild(LayoutCacheKey::new(1, 2, 320.0), false));
    }

    #[test]
    fn layout_cache_invalidates_on_viewport_revision() {
        let mut cache = LayoutState::default();
        cache.set_cached_key_for_test(LayoutCacheKey::new(1, 2, 300.0));

        assert!(cache.should_rebuild(LayoutCacheKey::new(1, 3, 300.0), false));
    }

    #[test]
    fn layout_cache_invalidates_when_fonts_change() {
        let key = LayoutCacheKey::new(1, 2, 300.0);
        let mut cache = LayoutState::default();
        cache.set_cached_key_for_test(key);

        assert!(cache.should_rebuild(key, true));
    }

    #[test]
    fn layout_reports_wrapped_visual_line_overflow() {
        let layout = LayoutState::build_layout_for_test(
            "this long line should wrap into multiple visual lines in a narrow layout",
            90.0,
        );
        let metrics = LayoutState::visual_metrics(&layout);

        assert!(metrics.visual_line_count > 1);
        assert!(metrics.max_scroll_y(28.0) > 0.0);
    }

    #[test]
    fn visual_layout_metrics_clamps_scroll_to_overflow() {
        let metrics = VisualLayoutMetrics {
            visual_line_count: 3,
            height: 84.0,
        };

        assert_eq!(metrics.max_scroll_y(56.0), 28.0);
        assert_eq!(metrics.max_scroll_y(100.0), 0.0);
    }
}
