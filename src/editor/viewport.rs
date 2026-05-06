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
}

#[cfg(test)]
mod tests {
    use super::Viewport;

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
}
