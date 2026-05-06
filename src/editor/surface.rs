use masonry::core::PaintCtx;
use masonry::kurbo::{Affine, Circle, Rect};
use masonry::peniko::{Color, Fill};

use super::buffer::{EditResult, EditorBuffer};
use super::cursor::CursorState;
use super::is_printable_text;
use super::layout::{LayoutCacheKey, LayoutState};
use super::viewport::{Viewport, visible_line_count_from_height};

const PANEL_COLOR: Color = Color::from_rgb8(0x24, 0x24, 0x24);
const ACCENT_COLOR: Color = Color::from_rgb8(0x8a, 0x6f, 0xff);
const TEXT_COLOR: Color = Color::from_rgb8(0xf4, 0xf1, 0xff);
const PLACEHOLDER_COLOR: Color = Color::from_rgb8(0x8d, 0x86, 0xa3);
pub(super) const TEXT_INSET: f64 = 48.0;
pub(super) const TEXT_FONT_SIZE: f32 = 20.0;
const PLACEHOLDER_TEXT: &str = "Start typing in the Clay native text canvas…";
const LINE_HEIGHT_MULTIPLIER: f64 = 1.4;

#[derive(Debug, Default)]
pub struct EditorSurface {
    buffer: EditorBuffer,
    cursor: CursorState,
    viewport: Viewport,
    layout: LayoutState,
    visual_scroll_y: f64,
    last_visual_max_scroll_y: f64,
    follow_visual_end: bool,
}

impl EditorSurface {
    pub fn insert_text(&mut self, text: &str) -> bool {
        if !is_printable_text(text) {
            return false;
        }

        let result = self.buffer.insert_at(self.cursor.caret(), text);
        self.finish_edit(result)
    }

    pub fn insert_newline(&mut self) -> bool {
        let result = self.buffer.insert_newline_at(self.cursor.caret());
        self.finish_edit(result)
    }

    pub fn backspace(&mut self) -> bool {
        let result = self.buffer.backspace_at(self.cursor.caret());
        self.finish_edit(result)
    }

    pub fn delete_forward(&mut self) -> bool {
        let result = self.buffer.delete_after(self.cursor.caret());
        self.finish_edit(result)
    }

    pub fn move_left(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_previous_scalar(buffer))
    }

    pub fn move_right(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_next_scalar(buffer))
    }

    pub fn move_to_line_start(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_line_start(buffer))
    }

    pub fn move_to_line_end(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_line_end(buffer))
    }

    pub fn move_to_document_start(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_document_start(buffer))
    }

    pub fn move_to_document_end(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_document_end(buffer))
    }

    pub fn visible_text(&self) -> String {
        self.visible_snapshot_text()
    }

    pub fn scroll_lines(&mut self, delta_lines: isize) -> bool {
        if delta_lines != 0 {
            let line_height = TEXT_FONT_SIZE as f64 * LINE_HEIGHT_MULTIPLIER;
            if self.scroll_visual_pixels(delta_lines as f64 * line_height) {
                return true;
            }
        }

        let changed = self
            .viewport
            .scroll_lines(delta_lines, self.buffer.line_len());
        if changed {
            self.visual_scroll_y = 0.0;
            self.follow_visual_end = false;
        }
        changed
    }

    pub fn scroll_vertical_pixels(&mut self, delta_pixels: f64) -> bool {
        let line_height = TEXT_FONT_SIZE as f64 * LINE_HEIGHT_MULTIPLIER;
        let magnitude = (delta_pixels.abs() / line_height).ceil().max(1.0) as isize;
        let delta_lines = if delta_pixels.is_sign_negative() {
            -magnitude
        } else {
            magnitude
        };
        if self.scroll_visual_pixels(delta_pixels) {
            true
        } else {
            self.scroll_lines(delta_lines)
        }
    }

    pub fn update_visible_line_count_for_height(&mut self, height: f64) -> bool {
        let available_height = (height - (TEXT_INSET * 2.0)).max(0.0);
        let line_height = TEXT_FONT_SIZE as f64 * LINE_HEIGHT_MULTIPLIER;
        let visible_line_count = visible_line_count_from_height(available_height, line_height);
        self.viewport
            .set_visible_line_count(visible_line_count, self.buffer.line_len())
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut masonry::vello::Scene) {
        let size = ctx.size();
        let width = size.width;
        let height = size.height;
        self.update_visible_line_count_for_height(height);

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

        let max_width = (width - (TEXT_INSET * 2.0)).max(1.0) as f32;
        let available_height = (height - (TEXT_INSET * 2.0)).max(0.0);
        self.paint_text(ctx, scene, max_width, available_height);
    }

    fn paint_text(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        max_width: f32,
        available_height: f64,
    ) {
        let current_text = self.visible_snapshot_text();
        let (display_text, color) = if current_text.is_empty() {
            (PLACEHOLDER_TEXT, PLACEHOLDER_COLOR)
        } else {
            (current_text.as_str(), TEXT_COLOR)
        };

        let key = LayoutCacheKey::new(self.buffer.revision(), self.viewport.revision(), max_width);
        let metrics = self.layout.paint_text(
            ctx,
            scene,
            display_text,
            color,
            max_width,
            &mut self.visual_scroll_y,
            self.follow_visual_end && !current_text.is_empty(),
            available_height,
            key,
        );
        if current_text.is_empty() {
            self.visual_scroll_y = 0.0;
            self.last_visual_max_scroll_y = 0.0;
        } else {
            self.last_visual_max_scroll_y = metrics.max_scroll_y(available_height);
        }
        self.follow_visual_end = false;
    }

    fn visible_snapshot_text(&self) -> String {
        let range = self.viewport.visible_range(self.buffer.line_len());
        self.buffer.visible_snapshot(range).text
    }

    fn finish_edit(&mut self, result: EditResult) -> bool {
        self.cursor.set_caret(result.caret);
        if !result.changed {
            return false;
        }

        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        true
    }

    fn ensure_caret_line_visible(&mut self) -> bool {
        let caret_line = self.buffer.line_of_byte(self.cursor.caret());
        self.viewport
            .ensure_line_visible(caret_line, self.buffer.line_len())
    }

    fn move_cursor(
        &mut self,
        movement: impl FnOnce(&mut CursorState, &EditorBuffer) -> bool,
    ) -> bool {
        let changed = movement(&mut self.cursor, &self.buffer);
        if changed {
            self.ensure_caret_line_visible();
            self.follow_visual_end = false;
        }
        changed
    }

    fn scroll_visual_pixels(&mut self, delta_pixels: f64) -> bool {
        if delta_pixels == 0.0 || self.last_visual_max_scroll_y <= 0.0 {
            return false;
        }

        let previous = self.visual_scroll_y;
        self.visual_scroll_y =
            (self.visual_scroll_y + delta_pixels).clamp(0.0, self.last_visual_max_scroll_y);
        self.follow_visual_end = false;
        self.visual_scroll_y != previous
    }

    #[cfg(test)]
    fn set_caret_for_test(&mut self, caret: usize) {
        let caret = self.buffer.clamp_byte_offset(caret);
        self.cursor.set_caret(caret);
    }

    #[cfg(test)]
    fn caret_for_test(&self) -> usize {
        self.cursor.caret()
    }

    #[cfg(test)]
    fn visual_scroll_y(&self) -> f64 {
        self.visual_scroll_y
    }

    #[cfg(test)]
    fn set_visual_scroll_bounds_for_test(&mut self, max_scroll_y: f64) {
        self.last_visual_max_scroll_y = max_scroll_y.max(0.0);
        self.visual_scroll_y = self
            .visual_scroll_y
            .clamp(0.0, self.last_visual_max_scroll_y);
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorSurface, TEXT_INSET};

    #[test]
    fn editor_enter_inserts_newline() {
        let mut editor = EditorSurface::default();

        editor.insert_text("first");
        let changed = editor.insert_newline();
        editor.insert_text("second");

        assert!(changed);
        assert_eq!(editor.visible_text(), "first\nsecond");
    }

    #[test]
    fn editor_insert_text_uses_caret_instead_of_appending() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_caret_for_test(1);

        let changed = editor.insert_text("X");

        assert!(changed);
        assert_eq!(editor.visible_text(), "aXbc");
        assert_eq!(editor.caret_for_test(), 2);
    }

    #[test]
    fn editor_insert_newline_auto_scrolls_to_new_line() {
        let mut editor = EditorSurface::default();
        editor.update_visible_line_count_for_height(TEXT_INSET * 2.0 + 1.0);

        editor.insert_text("first");
        editor.insert_newline();
        editor.insert_text("second");

        assert_eq!(editor.visible_text(), "second");
    }

    #[test]
    fn editor_backspace_keeps_remaining_end_visible() {
        let mut editor = EditorSurface::default();
        editor.update_visible_line_count_for_height(TEXT_INSET * 2.0 + 1.0);
        editor.insert_text("first");
        editor.insert_newline();
        editor.insert_text("second");

        let changed = editor.backspace();

        assert!(changed);
        assert_eq!(editor.visible_text(), "secon");
    }

    #[test]
    fn editor_delete_forward_removes_text_after_caret() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_caret_for_test(1);

        let changed = editor.delete_forward();

        assert!(changed);
        assert_eq!(editor.visible_text(), "ac");
        assert_eq!(editor.caret_for_test(), 1);
    }

    #[test]
    fn editor_cursor_navigation_moves_over_unicode_boundaries() {
        let mut editor = EditorSurface::default();
        editor.insert_text("a🦀b");
        editor.set_caret_for_test("a🦀".len());

        assert!(editor.move_left());
        assert_eq!(editor.caret_for_test(), 1);
        assert!(editor.move_right());
        assert_eq!(editor.caret_for_test(), "a🦀".len());
    }

    #[test]
    fn editor_home_end_navigation_uses_current_line() {
        let mut editor = EditorSurface::default();
        editor.insert_text("zero");
        editor.insert_newline();
        editor.insert_text("one");
        editor.set_caret_for_test("zero\no".len());

        assert!(editor.move_to_line_end());
        assert_eq!(editor.caret_for_test(), "zero\none".len());
        assert!(editor.move_to_line_start());
        assert_eq!(editor.caret_for_test(), "zero\n".len());
    }

    #[test]
    fn editor_scroll_vertical_pixels_uses_visual_overflow_before_logical_lines() {
        let mut editor = EditorSurface::default();
        editor.set_visual_scroll_bounds_for_test(80.0);

        let changed = editor.scroll_vertical_pixels(20.0);

        assert!(changed);
        assert_eq!(editor.visual_scroll_y(), 20.0);
        assert_eq!(editor.visible_text(), "");
    }

    #[test]
    fn editor_visual_scroll_clamps_to_known_overflow() {
        let mut editor = EditorSurface::default();
        editor.set_visual_scroll_bounds_for_test(80.0);

        let changed = editor.scroll_vertical_pixels(200.0);

        assert!(changed);
        assert_eq!(editor.visual_scroll_y(), 80.0);
    }
}
