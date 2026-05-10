use std::ops::Range;

use crop::Rope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleSnapshot {
    pub text: String,
    pub line_range: Range<usize>,
    pub start_byte_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditResult {
    pub changed: bool,
    pub caret: usize,
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

    pub fn replace_text(&mut self, text: String) {
        self.rope = Rope::from(text);
        self.bump_revision();
    }

    #[cfg(test)]
    pub fn insert_str(&mut self, text: &str) {
        self.insert_at(self.rope.byte_len(), text);
    }

    pub fn insert_at(&mut self, caret: usize, text: &str) -> EditResult {
        let caret = self.clamp_byte_offset(caret);
        if text.is_empty() {
            return EditResult {
                changed: false,
                caret,
            };
        }

        self.rope.insert(caret, text);
        self.bump_revision();
        EditResult {
            changed: true,
            caret: caret + text.len(),
        }
    }

    pub fn insert_newline_at(&mut self, caret: usize) -> EditResult {
        self.insert_at(caret, "\n")
    }

    pub fn replace_range(&mut self, range: Range<usize>, text: &str) -> EditResult {
        let caret = self.clamp_byte_offset(range.start);
        if range.start > range.end {
            return EditResult {
                changed: false,
                caret,
            };
        }

        let start = caret;
        let end = self.clamp_byte_offset(range.end);
        if start == end && text.is_empty() {
            return EditResult {
                changed: false,
                caret: start,
            };
        }

        self.rope.replace(start..end, text);
        self.bump_revision();
        EditResult {
            changed: true,
            caret: start + text.len(),
        }
    }

    pub fn delete_range(&mut self, range: Range<usize>) -> EditResult {
        let caret = self.clamp_byte_offset(range.start);
        if range.start > range.end {
            return EditResult {
                changed: false,
                caret,
            };
        }

        let start = caret;
        let end = self.clamp_byte_offset(range.end);
        if start >= end {
            return EditResult {
                changed: false,
                caret: start,
            };
        }

        self.rope.delete(start..end);
        self.bump_revision();
        EditResult {
            changed: true,
            caret: start,
        }
    }

    pub fn backspace_at(&mut self, caret: usize) -> EditResult {
        let caret = self.clamp_byte_offset(caret);
        let Some(previous) = self.previous_scalar_boundary(caret) else {
            return EditResult {
                changed: false,
                caret,
            };
        };

        self.delete_range(previous..caret)
    }

    pub fn delete_after(&mut self, caret: usize) -> EditResult {
        let caret = self.clamp_byte_offset(caret);
        let Some(next) = self.next_scalar_boundary(caret) else {
            return EditResult {
                changed: false,
                caret,
            };
        };

        self.delete_range(caret..next)
    }

    #[cfg(test)]
    pub fn backspace(&mut self) -> bool {
        self.backspace_at(self.rope.byte_len()).changed
    }

    pub fn clamp_byte_offset(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.rope.byte_len());
        while offset > 0 && !self.rope.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    pub fn previous_scalar_boundary(&self, caret: usize) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        if caret == 0 {
            return None;
        }

        self.rope
            .byte_slice(..caret)
            .chars()
            .next_back()
            .map(|character| caret - character.len_utf8())
    }

    pub fn next_scalar_boundary(&self, caret: usize) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        if caret == self.rope.byte_len() {
            return None;
        }

        self.rope
            .byte_slice(caret..)
            .chars()
            .next()
            .map(|character| caret + character.len_utf8())
    }

    pub fn document_start_byte(&self) -> usize {
        0
    }

    pub fn document_end_byte(&self) -> usize {
        self.rope.byte_len()
    }

    pub fn line_of_byte(&self, offset: usize) -> usize {
        if self.rope.byte_len() == 0 {
            0
        } else {
            self.rope.line_of_byte(self.clamp_byte_offset(offset))
        }
    }

    pub fn byte_of_line(&self, line: usize) -> usize {
        self.rope.byte_of_line(line.min(self.line_len()))
    }

    pub fn line_start_byte(&self, offset: usize) -> usize {
        self.byte_of_line(self.line_of_byte(offset))
    }

    pub fn line_end_byte(&self, offset: usize) -> usize {
        if self.rope.byte_len() == 0 {
            return 0;
        }

        let line = self.line_of_byte(offset);
        self.line_end_byte_for_line(line)
    }

    pub fn scalar_column_of_byte(&self, offset: usize) -> usize {
        let offset = self.clamp_byte_offset(offset);
        let start = self.line_start_byte(offset);
        self.rope.byte_slice(start..offset).chars().count()
    }

    pub fn byte_for_line_scalar_column(&self, line: usize, column: usize) -> usize {
        if self.rope.byte_len() == 0 {
            return 0;
        }

        let line = line.min(self.line_len().saturating_sub(1));
        let start = self.byte_of_line(line);
        let end = self.line_end_byte_for_line(line);
        let mut offset = start;

        for character in self.rope.byte_slice(start..end).chars().take(column) {
            offset += character.len_utf8();
        }

        offset
    }

    fn line_end_byte_for_line(&self, line: usize) -> usize {
        let line = line.min(self.line_len());
        let start = self.byte_of_line(line);
        let next_line_start = self.byte_of_line(line.saturating_add(1));
        let mut end = next_line_start;
        let slice = self.rope.byte_slice(start..next_line_start);
        let mut chars = slice.chars().rev();

        if let Some('\n') = chars.next() {
            end -= '\n'.len_utf8();
            if let Some('\r') = chars.next() {
                end -= '\r'.len_utf8();
            }
        }

        end
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
                start_byte_offset: if document_line_count == 0 {
                    0
                } else {
                    self.rope.byte_of_line(start_line)
                },
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
            start_byte_offset: start_byte,
        }
    }

    #[cfg(test)]
    pub fn visible_text(&self) -> String {
        self.rope.to_string()
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
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
        assert_eq!(snapshot.start_byte_offset, "zero\n".len());
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
        assert_eq!(snapshot.start_byte_offset, 0);
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
    fn insert_at_caret_updates_buffer_and_caret() {
        let mut buffer = EditorBuffer::from_text("Hello Earth!");

        let result = buffer.insert_at(5, " brave");

        assert!(result.changed);
        assert_eq!(result.caret, 11);
        assert_eq!(buffer.visible_text(), "Hello brave Earth!");
    }

    #[test]
    fn insert_at_invalid_byte_offset_clamps_to_scalar_boundary() {
        let mut buffer = EditorBuffer::from_text("a🦀b");

        let result = buffer.insert_at(2, "X");

        assert!(result.changed);
        assert_eq!(result.caret, 2);
        assert_eq!(buffer.visible_text(), "aX🦀b");
    }

    #[test]
    fn backspace_at_caret_deletes_previous_scalar_boundary() {
        let mut buffer = EditorBuffer::from_text("a🦀b");
        let caret_after_crab = "a🦀".len();

        let result = buffer.backspace_at(caret_after_crab);

        assert!(result.changed);
        assert_eq!(result.caret, 1);
        assert_eq!(buffer.visible_text(), "ab");
    }

    #[test]
    fn delete_at_caret_deletes_next_scalar_boundary() {
        let mut buffer = EditorBuffer::from_text("a🦀b");

        let result = buffer.delete_after(1);

        assert!(result.changed);
        assert_eq!(result.caret, 1);
        assert_eq!(buffer.visible_text(), "ab");
    }

    #[test]
    fn delete_range_clamps_or_rejects_invalid_ranges() {
        let mut buffer = EditorBuffer::from_text("a🦀b");

        let result = buffer.delete_range(2..999);

        assert!(result.changed);
        assert_eq!(result.caret, 1);
        assert_eq!(buffer.visible_text(), "a");

        let rejected = buffer.delete_range(3..1);
        assert!(!rejected.changed);
        assert_eq!(buffer.visible_text(), "a");
    }

    #[test]
    fn replace_range_updates_text_and_caret() {
        let mut buffer = EditorBuffer::from_text("abcdef");

        let result = buffer.replace_range(2..5, "X");

        assert!(result.changed);
        assert_eq!(result.caret, 3);
        assert_eq!(buffer.visible_text(), "abXf");
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
    fn visible_snapshot_includes_start_byte_offset() {
        let buffer = EditorBuffer::from_text("zero\none\ntwo");
        let viewport = Viewport::new(1, 1, 0);

        let snapshot = buffer.visible_snapshot(viewport.visible_range(buffer.line_len()));

        assert_eq!(snapshot.text, "one\n");
        assert_eq!(snapshot.start_byte_offset, "zero\n".len());
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
