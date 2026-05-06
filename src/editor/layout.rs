use std::fmt;

use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::Affine;
use masonry::parley::Layout;
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::Color;
use masonry::{TextAlign, TextAlignOptions};

use super::surface::{TEXT_FONT_SIZE, TEXT_INSET};

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
        key: LayoutCacheKey,
    ) {
        if self.should_rebuild(key, ctx.fonts_changed()) {
            self.rebuild(ctx, display_text, max_width, key);
        }

        let cached = self
            .cached
            .as_ref()
            .expect("layout cache must contain a layout after rebuild check");
        render_text(
            scene,
            Affine::translate((TEXT_INSET, TEXT_INSET)),
            &cached.layout,
            &[color.into()],
            true,
        );
    }

    fn should_rebuild(&self, key: LayoutCacheKey, fonts_changed: bool) -> bool {
        fonts_changed || self.cached.as_ref().is_none_or(|cached| cached.key != key)
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
    fn set_cached_key_for_test(&mut self, key: LayoutCacheKey) {
        self.cached = Some(CachedLayout {
            key,
            layout: Layout::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutCacheKey, LayoutState};

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
}
