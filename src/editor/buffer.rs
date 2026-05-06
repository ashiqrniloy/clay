use std::ops::Range;

use crop::Rope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleSnapshot {
    pub text: String,
    pub line_range: Range<usize>,
}

#[derive(Debug, Default)]
pub struct EditorBuffer {
    rope: Rope,
    revision: u64,
}

impl EditorBuffer {
    #[cfg(test)]
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from(text),
            revision: 0,
        }
    }

    pub fn insert_str(&mut self, text: &str) {
        self.rope.insert(self.rope.byte_len(), text);
        self.revision = self.revision.saturating_add(1);
    }

    pub fn backspace(&mut self) -> bool {
        let Some(last_char) = self.rope.chars().next_back() else {
            return false;
        };

        let end = self.rope.byte_len();
        self.rope.delete(end - last_char.len_utf8()..end);
        self.revision = self.revision.saturating_add(1);
        true
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn line_len(&self) -> usize {
        if self.rope.byte_len() == 0 {
            0
        } else {
            self.rope.line_len()
        }
    }

    pub fn visible_snapshot(&self, line_range: Range<usize>) -> VisibleSnapshot {
        let document_line_count = self.line_len();
        let start_line = line_range.start.min(document_line_count);
        let end_line = line_range.end.min(document_line_count).max(start_line);

        if start_line == end_line {
            return VisibleSnapshot {
                text: String::new(),
                line_range: start_line..end_line,
            };
        }

        let start_byte = self.rope.byte_of_line(start_line);
        let end_byte = if end_line == document_line_count {
            self.rope.byte_len()
        } else {
            self.rope.byte_of_line(end_line)
        };

        VisibleSnapshot {
            text: self.rope.byte_slice(start_byte..end_byte).to_string(),
            line_range: start_line..end_line,
        }
    }

    #[cfg(test)]
    pub fn visible_text(&self) -> String {
        self.rope.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::EditorBuffer;
    use crate::editor::viewport::Viewport;

    fn generated_lines(line_count: usize) -> String {
        let mut text = String::new();
        for line in 0..line_count {
            writeln!(text, "line {line:05}").expect("writing to String cannot fail");
        }
        text
    }

    #[test]
    fn visible_snapshot_limits_to_requested_lines() {
        let buffer = EditorBuffer::from_text("zero\none\ntwo\nthree\nfour\n");
        let viewport = Viewport::new(1, 2, 1);

        let snapshot = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert_eq!(snapshot.text, "one\ntwo\nthree\n");
        assert_eq!(snapshot.line_range, 1..4);
    }

    #[test]
    fn visible_snapshot_clamps_past_document_end() {
        let buffer = EditorBuffer::from_text("zero\none\ntwo");
        let viewport = Viewport::new(1, 10, 10);

        let snapshot = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert_eq!(snapshot.text, "one\ntwo");
        assert_eq!(snapshot.line_range, 1..3);
    }

    #[test]
    fn visible_snapshot_preserves_utf8_boundaries() {
        let buffer = EditorBuffer::from_text("alpha 🦀\nbéta é\n三\n");
        let viewport = Viewport::new(1, 1, 1);

        let snapshot = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert_eq!(snapshot.text, "béta é\n三\n");
        assert_eq!(snapshot.line_range, 1..3);
    }

    #[test]
    fn empty_buffer_visible_snapshot_is_empty() {
        let buffer = EditorBuffer::default();
        let viewport = Viewport::default();

        let snapshot = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert_eq!(snapshot.text, "");
        assert_eq!(snapshot.line_range, 0..0);
    }

    #[test]
    fn scrolling_viewport_changes_visible_snapshot() {
        let buffer = EditorBuffer::from_text("zero\none\ntwo\nthree\nfour\n");
        let mut viewport = Viewport::new(0, 2, 0);
        let before = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        viewport.scroll_lines(2, buffer.line_len());
        let after = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert_eq!(before.text, "zero\none\n");
        assert_eq!(after.text, "two\nthree\n");
    }

    #[test]
    fn editor_buffer_revision_changes_on_edits() {
        let mut buffer = EditorBuffer::default();

        buffer.insert_str("a");
        let after_insert = buffer.revision();
        buffer.backspace();

        assert!(after_insert > 0);
        assert!(buffer.revision() > after_insert);
    }

    #[test]
    fn newline_insertion_creates_additional_visible_line() {
        let mut buffer = EditorBuffer::default();

        buffer.insert_str("first");
        buffer.insert_str("\n");
        buffer.insert_str("second");

        assert_eq!(buffer.line_len(), 2);
        assert_eq!(buffer.visible_text(), "first\nsecond");
    }

    #[test]
    fn large_buffer_visible_extraction_is_bounded() {
        let text = generated_lines(10_000);
        let buffer = EditorBuffer::from_text(&text);
        let viewport = Viewport::new(5_000, 12, 3);

        let snapshot = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert_eq!(snapshot.line_range, 5_000..5_015);
        assert!(snapshot.text.len() < text.len() / 100);
        assert!(snapshot.text.starts_with("line 05000\n"));
        assert!(snapshot.text.ends_with("line 05014\n"));
    }

    #[test]
    fn large_buffer_scroll_changes_snapshot_without_changing_buffer() {
        let text = generated_lines(10_000);
        let buffer = EditorBuffer::from_text(&text);
        let mut viewport = Viewport::new(0, 3, 0);
        let before = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        let changed = viewport.scroll_lines(7_500, buffer.line_len());
        let after = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert!(changed);
        assert_eq!(before.text, "line 00000\nline 00001\nline 00002\n");
        assert_eq!(after.text, "line 07500\nline 07501\nline 07502\n");
        assert_eq!(buffer.visible_text().len(), text.len());
    }
}
