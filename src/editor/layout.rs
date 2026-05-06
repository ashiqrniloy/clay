use std::fmt;

use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::{Affine, Rect};
use masonry::parley::Layout;
use masonry::parley::layout::{Affinity, Cursor};
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
pub struct CaretGeometry {
    pub rect: Rect,
}

const CARET_WIDTH: f32 = 1.5;

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
    text_len: usize,
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
        caret_visible_byte_offset: Option<usize>,
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
        if let Some(caret_offset) = caret_visible_byte_offset
            && let Some(caret) =
                Self::caret_geometry_in_layout(&cached.layout, cached.text_len, caret_offset)
        {
            Self::ensure_rect_visible(scroll_y, caret.rect, available_height, max_scroll_y);
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

    pub fn hit_test_visible_byte_offset(&self, x: f32, y: f32) -> Option<usize> {
        let cached = self.cached.as_ref()?;
        Some(Cursor::from_point(&cached.layout, x, y).index())
    }

    pub fn caret_geometry_for_visible_byte_offset(
        &self,
        byte_offset: usize,
        width: f32,
    ) -> Option<CaretGeometry> {
        let cached = self.cached.as_ref()?;
        Self::caret_geometry_in_layout(&cached.layout, cached.text_len, byte_offset).map(|caret| {
            if width == CARET_WIDTH {
                caret
            } else {
                let x0 = caret.rect.x0;
                CaretGeometry {
                    rect: Rect::new(x0, caret.rect.y0, x0 + width as f64, caret.rect.y1),
                }
            }
        })
    }

    fn caret_geometry_in_layout(
        layout: &Layout<BrushIndex>,
        text_len: usize,
        byte_offset: usize,
    ) -> Option<CaretGeometry> {
        let byte_offset = byte_offset.min(text_len);
        let cursor = Cursor::from_byte_index(layout, byte_offset, Affinity::Downstream);
        let geometry = cursor.geometry(layout, CARET_WIDTH);
        Some(CaretGeometry {
            rect: Rect::new(
                geometry.x0 as f64,
                geometry.y0 as f64,
                geometry.x1 as f64,
                geometry.y1 as f64,
            ),
        })
    }

    fn ensure_rect_visible(
        scroll_y: &mut f64,
        rect: Rect,
        available_height: f64,
        max_scroll_y: f64,
    ) {
        if available_height <= 0.0 {
            *scroll_y = scroll_y.clamp(0.0, max_scroll_y);
            return;
        }

        if rect.y0 < *scroll_y {
            *scroll_y = rect.y0;
        } else if rect.y1 > *scroll_y + available_height {
            *scroll_y = rect.y1 - available_height;
        }
        *scroll_y = scroll_y.clamp(0.0, max_scroll_y);
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

        self.cached = Some(CachedLayout {
            key,
            layout,
            text_len: display_text.len(),
        });
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
            text_len: 0,
        });
    }

    #[cfg(test)]
    pub(super) fn set_cached_layout_for_test(&mut self, display_text: &str, max_width: f32) {
        self.cached = Some(CachedLayout {
            key: LayoutCacheKey::new(0, 0, max_width),
            layout: Self::build_layout_for_test(display_text, max_width),
            text_len: display_text.len(),
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

    #[test]
    fn hit_test_clamps_before_and_after_text() {
        let mut cache = LayoutState::default();
        cache.set_cached_layout_for_test("abc", 300.0);

        let before = cache
            .hit_test_visible_byte_offset(-100.0, 0.0)
            .expect("cached layout should hit-test");
        let after = cache
            .hit_test_visible_byte_offset(10_000.0, 0.0)
            .expect("cached layout should hit-test");

        assert!(before <= "abc".len());
        assert!(after <= "abc".len());
    }

    #[test]
    fn caret_geometry_is_available_for_visible_caret() {
        let mut cache = LayoutState::default();
        cache.set_cached_layout_for_test("abc", 300.0);

        let geometry = cache
            .caret_geometry_for_visible_byte_offset(1, 1.5)
            .expect("cached layout should return caret geometry");

        assert!(geometry.rect.x0.is_finite());
        assert!(geometry.rect.y0.is_finite());
        assert!(geometry.rect.height().is_finite());
    }

    #[test]
    fn ensure_caret_visible_scrolls_to_caret_rect() {
        let mut scroll_y = 0.0;
        let caret = masonry::kurbo::Rect::new(0.0, 90.0, 1.5, 118.0);

        LayoutState::ensure_rect_visible(&mut scroll_y, caret, 56.0, 100.0);

        assert_eq!(scroll_y, 62.0);
    }

    #[test]
    fn ensure_caret_visible_preserves_visible_rect() {
        let mut scroll_y = 50.0;
        let caret = masonry::kurbo::Rect::new(0.0, 60.0, 1.5, 80.0);

        LayoutState::ensure_rect_visible(&mut scroll_y, caret, 56.0, 100.0);

        assert_eq!(scroll_y, 50.0);
    }
}
