use std::{fmt, ops::Range};

use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::{Affine, BezPath, Rect, Stroke};
use masonry::parley::Layout;
use masonry::parley::layout::{Affinity, Cursor, Selection};
use masonry::parley::style::{FontStyle, FontWeight, LineHeight, StyleProperty};
use masonry::peniko::{Brush, Color, Fill};
use masonry::{TextAlign, TextAlignOptions};

use crate::perf::metrics::global_recorder;

use crate::protocol::FontRole;

use super::surface::TEXT_INSET;
use super::theme::TextAttributes;
use super::typography::{DOCUMENT_LINE_HEIGHT_MULTIPLIER, TypographyRegistry};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct VisibleTextStyleRun {
    pub range: Range<usize>,
    pub font_role: FontRole,
    pub attributes: TextAttributes,
    pub color: Option<Color>,
}

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

// Clay-owned squiggle geometry — themes supply color only.
const SQUIGGLE_AMPLITUDE: f64 = 1.5;
const SQUIGGLE_PERIOD: f64 = 4.0;
const SQUIGGLE_STROKE_WIDTH: f64 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutCacheKey {
    text_revision: u64,
    viewport_revision: u64,
    max_width: f32,
    typography_revision: u64,
    layout_style_revision: u64,
    document_font_role: FontRole,
}

impl LayoutCacheKey {
    pub fn new(text_revision: u64, viewport_revision: u64, max_width: f32) -> Self {
        Self {
            text_revision,
            viewport_revision,
            max_width,
            typography_revision: 0,
            layout_style_revision: 0,
            document_font_role: FontRole::Proportional,
        }
    }

    pub fn with_presentation(
        mut self,
        typography_revision: u64,
        layout_style_revision: u64,
        document_font_role: FontRole,
    ) -> Self {
        self.typography_revision = typography_revision;
        self.layout_style_revision = layout_style_revision;
        self.document_font_role = document_font_role;
        self
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
    style_runs: Vec<VisibleTextStyleRun>,
    brushes: Vec<Brush>,
}

impl fmt::Debug for LayoutState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LayoutState")
            .field("cached_key", &self.cached.as_ref().map(|cached| cached.key))
            .field(
                "cached_style_run_count",
                &self.cached.as_ref().map(|cached| cached.style_runs.len()),
            )
            .finish()
    }
}

impl LayoutState {
    #[allow(
        clippy::too_many_arguments,
        reason = "editor paint hot path passes render inputs explicitly to avoid per-frame heap context objects"
    )]
    pub fn paint_text<F>(
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
        selection_visible_byte_range: Option<Range<usize>>,
        selection_color: Color,
        diagnostic_visible_byte_ranges: &[(Range<usize>, Color)],
        origin: (f64, f64),
        pin_caret_visible: bool,
        typography: &TypographyRegistry,
        document_font_role: FontRole,
        normalize_style_runs: F,
    ) -> VisualLayoutMetrics
    where
        F: FnOnce() -> Vec<VisibleTextStyleRun>,
    {
        let recorder = global_recorder();
        let _scope = recorder.scope("editor.layout.paint_text");
        if self.should_rebuild(key, ctx.fonts_changed()) {
            let _rebuild_scope = recorder.scope("editor.layout.rebuild");
            recorder.record_counter("editor.layout.cache_miss", 1);
            self.rebuild(
                ctx,
                display_text,
                max_width,
                key,
                typography,
                document_font_role,
                color,
                normalize_style_runs(),
            );
        } else {
            recorder.record_counter("editor.layout.cache_hit", 1);
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
            && pin_caret_visible
            && let Some(caret) =
                Self::caret_geometry_in_layout(&cached.layout, cached.text_len, caret_offset)
        {
            Self::ensure_rect_visible(scroll_y, caret.rect, available_height, max_scroll_y);
        }

        let clip = Rect::new(
            origin.0 + TEXT_INSET,
            origin.1 + TEXT_INSET,
            origin.0 + TEXT_INSET + max_width as f64,
            origin.1 + TEXT_INSET + available_height,
        );
        scene.push_clip_layer(Affine::IDENTITY, &clip);
        if let Some(range) = selection_visible_byte_range {
            for rect in Self::selection_rects_in_layout(&cached.layout, cached.text_len, range) {
                let rect = Rect::new(
                    origin.0 + rect.x0 + TEXT_INSET,
                    origin.1 + rect.y0 + TEXT_INSET - *scroll_y,
                    origin.0 + rect.x1 + TEXT_INSET,
                    origin.1 + rect.y1 + TEXT_INSET - *scroll_y,
                );
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    selection_color,
                    None,
                    &rect,
                );
            }
        }
        render_text(
            scene,
            Affine::translate((origin.0 + TEXT_INSET, origin.1 + TEXT_INSET - *scroll_y)),
            &cached.layout,
            &cached.brushes,
            true,
        );
        for (range, diagnostic_color) in diagnostic_visible_byte_ranges {
            for rect in Self::diagnostic_mark_rects_in_layout(
                &cached.layout,
                cached.text_len,
                range.clone(),
            ) {
                let rect = Rect::new(
                    origin.0 + rect.x0 + TEXT_INSET,
                    origin.1 + rect.y0 + TEXT_INSET - *scroll_y,
                    origin.0 + rect.x1 + TEXT_INSET,
                    origin.1 + rect.y1 + TEXT_INSET - *scroll_y,
                );
                Self::paint_squiggle(scene, rect, *diagnostic_color);
            }
        }
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

    fn selection_rects_in_layout(
        layout: &Layout<BrushIndex>,
        text_len: usize,
        range: Range<usize>,
    ) -> Vec<Rect> {
        let start = range.start.min(text_len);
        let end = range.end.min(text_len);
        if start >= end {
            return Vec::new();
        }

        let anchor = Cursor::from_byte_index(layout, start, Affinity::Downstream);
        let focus = Cursor::from_byte_index(layout, end, Affinity::Upstream);
        Selection::new(anchor, focus)
            .geometry(layout)
            .into_iter()
            .map(|(rect, _)| Rect::new(rect.x0, rect.y0, rect.x1, rect.y1))
            .collect()
    }

    /// Line-local rectangles for a diagnostic range. Zero-width anchors expand
    /// to caret-width so MISSING-node marks still paint.
    fn diagnostic_mark_rects_in_layout(
        layout: &Layout<BrushIndex>,
        text_len: usize,
        range: Range<usize>,
    ) -> Vec<Rect> {
        let start = range.start.min(text_len);
        let end = range.end.min(text_len);
        if start < end {
            return Self::selection_rects_in_layout(layout, text_len, start..end);
        }
        Self::caret_geometry_in_layout(layout, text_len, start)
            .map(|caret| {
                let width = caret.rect.width().max(SQUIGGLE_PERIOD);
                vec![Rect::new(
                    caret.rect.x0,
                    caret.rect.y0,
                    caret.rect.x0 + width,
                    caret.rect.y1,
                )]
            })
            .unwrap_or_default()
    }

    fn paint_squiggle(scene: &mut masonry::vello::Scene, rect: Rect, color: Color) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let baseline = rect.y1 - SQUIGGLE_AMPLITUDE.max(1.0);
        let mut path = BezPath::new();
        let mut x = rect.x0;
        let mut peak = true;
        path.move_to((x, baseline));
        while x < rect.x1 {
            let next = (x + SQUIGGLE_PERIOD * 0.5).min(rect.x1);
            let y = if peak {
                baseline - SQUIGGLE_AMPLITUDE
            } else {
                baseline + SQUIGGLE_AMPLITUDE
            };
            path.line_to((next, y));
            x = next;
            peak = !peak;
        }
        scene.stroke(
            &Stroke::new(SQUIGGLE_STROKE_WIDTH),
            Affine::IDENTITY,
            color,
            None,
            &path,
        );
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
            rect: Rect::new(geometry.x0, geometry.y0, geometry.x1, geometry.y1),
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

    #[expect(
        clippy::too_many_arguments,
        reason = "cached layout rebuild keeps hot-path inputs explicit rather than allocating a transient context"
    )]
    fn rebuild(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        display_text: &str,
        max_width: f32,
        key: LayoutCacheKey,
        typography: &TypographyRegistry,
        document_font_role: FontRole,
        default_color: Color,
        style_runs: Vec<VisibleTextStyleRun>,
    ) {
        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, display_text, 1.0, true);
        let default_profile = typography.profile(document_font_role);
        builder.push_default(StyleProperty::FontStack(default_profile.font_stack()));
        builder.push_default(StyleProperty::FontSize(default_profile.size()));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            DOCUMENT_LINE_HEIGHT_MULTIPLIER as f32,
        )));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        let mut brush_colors = vec![default_color];
        for run in &style_runs {
            let profile = typography.profile(run.font_role);
            builder.push(
                StyleProperty::FontStack(profile.font_stack()),
                run.range.clone(),
            );
            builder.push(StyleProperty::FontSize(profile.size()), run.range.clone());
            if run.attributes.bold {
                builder.push(
                    StyleProperty::FontWeight(FontWeight::BOLD),
                    run.range.clone(),
                );
            }
            if run.attributes.italic {
                builder.push(
                    StyleProperty::FontStyle(FontStyle::Italic),
                    run.range.clone(),
                );
            }
            if run.attributes.underline {
                builder.push(StyleProperty::Underline(true), run.range.clone());
            }
            if run.attributes.strike {
                builder.push(StyleProperty::Strikethrough(true), run.range.clone());
            }
            if let Some(color) = run.color {
                let brush_index = brush_colors
                    .iter()
                    .position(|candidate| *candidate == color)
                    .unwrap_or_else(|| {
                        brush_colors.push(color);
                        brush_colors.len() - 1
                    });
                builder.push(
                    StyleProperty::Brush(BrushIndex(brush_index)),
                    run.range.clone(),
                );
            }
        }

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
            style_runs,
            brushes: brush_colors.into_iter().map(Into::into).collect(),
        });
    }

    #[cfg(test)]
    fn build_layout_for_test(display_text: &str, max_width: f32) -> Layout<BrushIndex> {
        Self::build_layout_with_typography_for_test(
            display_text,
            max_width,
            &TypographyRegistry::default(),
            FontRole::Proportional,
            &[],
        )
    }

    #[cfg(test)]
    fn build_layout_with_typography_for_test(
        display_text: &str,
        max_width: f32,
        typography: &TypographyRegistry,
        document_font_role: FontRole,
        style_runs: &[VisibleTextStyleRun],
    ) -> Layout<BrushIndex> {
        let mut font_context = masonry::parley::FontContext::new();
        let mut layout_context = masonry::parley::LayoutContext::new();
        let mut builder = layout_context.ranged_builder(&mut font_context, display_text, 1.0, true);
        let profile = typography.profile(document_font_role);
        builder.push_default(StyleProperty::FontStack(profile.font_stack()));
        builder.push_default(StyleProperty::FontSize(profile.size()));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            DOCUMENT_LINE_HEIGHT_MULTIPLIER as f32,
        )));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        for (index, run) in style_runs.iter().enumerate() {
            let profile = typography.profile(run.font_role);
            builder.push(
                StyleProperty::FontStack(profile.font_stack()),
                run.range.clone(),
            );
            builder.push(StyleProperty::FontSize(profile.size()), run.range.clone());
            if run.color.is_some() {
                builder.push(
                    StyleProperty::Brush(BrushIndex(index + 1)),
                    run.range.clone(),
                );
            }
        }

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
            style_runs: Vec::new(),
            brushes: Vec::new(),
        });
    }

    #[cfg(test)]
    pub(super) fn set_cached_layout_for_test(&mut self, display_text: &str, max_width: f32) {
        self.cached = Some(CachedLayout {
            key: LayoutCacheKey::new(0, 0, max_width),
            layout: Self::build_layout_for_test(display_text, max_width),
            text_len: display_text.len(),
            style_runs: Vec::new(),
            brushes: Vec::new(),
        });
    }

    #[cfg(test)]
    pub(super) fn set_cached_layout_with_typography_for_test(
        &mut self,
        display_text: &str,
        max_width: f32,
        typography: &TypographyRegistry,
        document_font_role: FontRole,
    ) {
        self.cached = Some(CachedLayout {
            key: LayoutCacheKey::new(0, 0, max_width),
            layout: Self::build_layout_with_typography_for_test(
                display_text,
                max_width,
                typography,
                document_font_role,
                &[],
            ),
            text_len: display_text.len(),
            style_runs: Vec::new(),
            brushes: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LayoutCacheKey, LayoutState, TextAttributes, VisibleTextStyleRun, VisualLayoutMetrics,
    };
    use crate::{
        editor::typography::TypographyRegistry,
        protocol::{ActiveTypography, FontRole},
    };

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
    fn layout_cache_invalidates_on_typography_style_or_document_role_change() {
        let key = LayoutCacheKey::new(1, 2, 300.0).with_presentation(4, 7, FontRole::Proportional);
        let mut cache = LayoutState::default();
        cache.set_cached_key_for_test(key);

        assert!(cache.should_rebuild(
            LayoutCacheKey::new(1, 2, 300.0).with_presentation(5, 7, FontRole::Proportional),
            false,
        ));
        assert!(cache.should_rebuild(
            LayoutCacheKey::new(1, 2, 300.0).with_presentation(4, 8, FontRole::Proportional),
            false,
        ));
        assert!(cache.should_rebuild(
            LayoutCacheKey::new(1, 2, 300.0).with_presentation(4, 7, FontRole::Monospace),
            false,
        ));
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
    fn mixed_role_line_height_keeps_largest_inline_profile_in_bounds() {
        let mut active = ActiveTypography::default();
        active.monospace.size = 40.0;
        active.proportional.size = 10.0;
        let typography = TypographyRegistry::from_active_typography(active).unwrap();
        let layout = LayoutState::build_layout_with_typography_for_test(
            "a b",
            300.0,
            &typography,
            FontRole::Proportional,
            &[VisibleTextStyleRun {
                range: 1..2,
                font_role: FontRole::Monospace,
                attributes: TextAttributes::default(),
                color: None,
            }],
        );

        assert!(
            layout.get(0).unwrap().metrics().line_height
                >= typography.document_line_height() as f32
        );
    }

    #[test]
    fn decoration_range_uses_a_non_default_text_brush() {
        let typography = TypographyRegistry::default();
        let layout = LayoutState::build_layout_with_typography_for_test(
            "let value",
            300.0,
            &typography,
            FontRole::Monospace,
            &[VisibleTextStyleRun {
                range: 0..3,
                font_role: FontRole::Monospace,
                attributes: TextAttributes::default(),
                color: Some(masonry::peniko::color::palette::css::RED),
            }],
        );

        assert!(
            layout
                .styles()
                .iter()
                .any(|style| style.brush == masonry::core::BrushIndex(1))
        );
    }

    #[test]
    fn unicode_and_emoji_shape_with_unavailable_named_font_fallback() {
        let mut active = ActiveTypography::default();
        active.proportional.families = vec![
            "definitely-not-installed".to_string(),
            "sans-serif".to_string(),
        ];
        let typography = TypographyRegistry::from_active_typography(active).unwrap();
        let text = "Hé 🦀 漢字";
        let layout = LayoutState::build_layout_with_typography_for_test(
            text,
            300.0,
            &typography,
            FontRole::Proportional,
            &[],
        );

        assert!(layout.height().is_finite() && layout.height() > 0.0);
        assert_eq!(layout.get(0).unwrap().text_range(), 0..text.len());
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

    #[test]
    fn selection_geometry_is_available_for_visible_range() {
        let layout = LayoutState::build_layout_for_test("abc", 300.0);

        let rects = LayoutState::selection_rects_in_layout(&layout, "abc".len(), 1..3);

        assert!(!rects.is_empty());
        assert!(rects.iter().all(|rect| rect.width().is_finite()));
    }

    #[test]
    fn diagnostic_squiggles_follow_wrapped_parley_range_geometry() {
        let text = "this long line should wrap into multiple visual lines in a narrow layout";
        let layout = LayoutState::build_layout_for_test(text, 90.0);
        assert!(LayoutState::visual_metrics(&layout).visual_line_count > 1);

        let rects =
            LayoutState::diagnostic_mark_rects_in_layout(&layout, text.len(), 0..text.len());
        assert!(
            rects.len() > 1,
            "wrapped diagnostic range must yield line-local rectangles, got {}",
            rects.len()
        );
        assert!(
            rects
                .iter()
                .all(|rect| rect.width().is_finite() && rect.height() > 0.0)
        );

        let zero = LayoutState::diagnostic_mark_rects_in_layout(&layout, text.len(), 10..10);
        assert_eq!(zero.len(), 1);
        assert!(zero[0].width() >= super::SQUIGGLE_PERIOD);
    }
}
