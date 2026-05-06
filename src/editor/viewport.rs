use std::ops::Range;

const DEFAULT_VISIBLE_LINE_COUNT: usize = 64;
const DEFAULT_OVERSCAN_LINES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewport {
    first_visible_line: usize,
    visible_line_count: usize,
    overscan_lines: usize,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            first_visible_line: 0,
            visible_line_count: DEFAULT_VISIBLE_LINE_COUNT,
            overscan_lines: DEFAULT_OVERSCAN_LINES,
        }
    }
}

impl Viewport {
    #[cfg(test)]
    pub fn new(
        first_visible_line: usize,
        visible_line_count: usize,
        overscan_lines: usize,
    ) -> Self {
        Self {
            first_visible_line,
            visible_line_count: visible_line_count.max(1),
            overscan_lines,
        }
    }

    #[cfg(test)]
    pub fn first_visible_line(&self) -> usize {
        self.first_visible_line
    }

    pub fn visible_range(&self, document_line_count: usize) -> Range<usize> {
        if document_line_count == 0 {
            return 0..0;
        }

        let start = self.first_visible_line.min(document_line_count);
        let requested_len = self
            .visible_line_count
            .saturating_add(self.overscan_lines)
            .max(1);
        let end = start.saturating_add(requested_len).min(document_line_count);
        start..end
    }

    pub fn set_visible_line_count(
        &mut self,
        visible_line_count: usize,
        document_line_count: usize,
    ) -> bool {
        let visible_line_count = visible_line_count.max(1);
        if self.visible_line_count == visible_line_count {
            return false;
        }

        self.visible_line_count = visible_line_count;
        self.clamp_first_visible_line(document_line_count);
        true
    }

    pub fn scroll_lines(&mut self, delta_lines: isize, document_line_count: usize) -> bool {
        if delta_lines == 0 {
            return false;
        }

        let previous = self.first_visible_line;
        if delta_lines.is_negative() {
            self.first_visible_line = self
                .first_visible_line
                .saturating_sub(delta_lines.unsigned_abs());
        } else {
            self.first_visible_line = self.first_visible_line.saturating_add(delta_lines as usize);
        }
        self.clamp_first_visible_line(document_line_count);
        self.first_visible_line != previous
    }

    fn clamp_first_visible_line(&mut self, document_line_count: usize) {
        let max_first_visible_line = document_line_count.saturating_sub(self.visible_line_count);
        self.first_visible_line = self.first_visible_line.min(max_first_visible_line);
    }
}

pub fn visible_line_count_from_height(available_height: f64, line_height: f64) -> usize {
    if available_height <= 0.0 || line_height <= 0.0 {
        return 1;
    }

    (available_height / line_height).ceil().max(1.0) as usize
}

#[cfg(test)]
mod tests {
    use super::{Viewport, visible_line_count_from_height};

    #[test]
    fn viewport_visible_range_includes_overscan() {
        let viewport = Viewport::new(3, 2, 1);

        assert_eq!(viewport.visible_range(10), 3..6);
    }

    #[test]
    fn viewport_visible_range_clamps_to_document_end() {
        let viewport = Viewport::new(8, 4, 4);

        assert_eq!(viewport.visible_range(10), 8..10);
    }

    #[test]
    fn viewport_visible_range_is_empty_for_empty_documents() {
        let viewport = Viewport::default();

        assert_eq!(viewport.visible_range(0), 0..0);
    }

    #[test]
    fn viewport_scroll_clamps_to_document_start() {
        let mut viewport = Viewport::new(0, 3, 1);

        let changed = viewport.scroll_lines(-5, 10);

        assert!(!changed);
        assert_eq!(viewport.first_visible_line(), 0);
    }

    #[test]
    fn viewport_scroll_clamps_to_document_end() {
        let mut viewport = Viewport::new(0, 3, 1);

        let changed = viewport.scroll_lines(50, 10);

        assert!(changed);
        assert_eq!(viewport.first_visible_line(), 7);
    }

    #[test]
    fn viewport_visible_line_count_updates_from_height() {
        assert_eq!(visible_line_count_from_height(0.0, 28.0), 1);
        assert_eq!(visible_line_count_from_height(56.0, 28.0), 2);
        assert_eq!(visible_line_count_from_height(57.0, 28.0), 3);
    }

    #[test]
    fn viewport_visible_line_count_update_clamps_first_line() {
        let mut viewport = Viewport::new(7, 3, 1);

        let changed = viewport.set_visible_line_count(8, 10);

        assert!(changed);
        assert_eq!(viewport.first_visible_line(), 2);
    }
}
