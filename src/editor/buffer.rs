use std::ops::Range;

use crop::Rope;

use crate::protocol::{ParagraphStyle, WordSeparatorPolicy};

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

    pub fn text_range(&self, range: Range<usize>) -> String {
        let start = self.clamp_byte_offset(range.start);
        let end = self.clamp_byte_offset(range.end);
        if start >= end {
            return String::new();
        }

        self.rope.byte_slice(start..end).to_string()
    }

    pub fn line_text_before_byte(&self, offset: usize) -> String {
        let offset = self.clamp_byte_offset(offset);
        let start = self.line_start_byte(offset);
        self.rope.byte_slice(start..offset).to_string()
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

    /// Character at `caret` (the char starting at that byte), or `None` at EOF.
    pub fn char_at(&self, caret: usize) -> Option<char> {
        let caret = self.clamp_byte_offset(caret);
        self.rope.byte_slice(caret..).chars().next()
    }

    /// Character immediately before `caret`, or `None` at document start.
    pub fn char_before(&self, caret: usize) -> Option<char> {
        let caret = self.clamp_byte_offset(caret);
        self.rope.byte_slice(..caret).chars().next_back()
    }

    /// Shared word classifier: `long` (WORD) overrides the policy with a
    /// whitespace-only boundary so punctuation joins the current word.
    /// Combining marks (U+0300–U+036F) continue a word so word motion does not
    /// split a grapheme.
    /// `ponytail:` basic combining-diacritical range only; full grapheme
    /// clustering would need the unicode-segmentation crate.
    fn classify_word(
        policy: &WordSeparatorPolicy,
        underscore: bool,
        long: bool,
        character: char,
    ) -> bool {
        if long {
            return !character.is_whitespace();
        }
        if policy.is_word_char(character, underscore) {
            return true;
        }
        (0x0300..=0x036F).contains(&(character as u32))
    }

    /// Byte offset of the start of the next word run strictly after `caret`.
    /// `None` when no further word exists (caller falls back to document end).
    pub fn next_word_start(
        &self,
        caret: usize,
        policy: &WordSeparatorPolicy,
        underscore: bool,
        long: bool,
    ) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        let end = self.rope.byte_len();
        if caret >= end {
            return None;
        }
        let mut chars = self.rope.byte_slice(caret..end).chars();
        let mut offset = caret;
        // Phase 1: if the current char is a word char, skip the whole run.
        if let Some(first) = chars.next() {
            offset += first.len_utf8();
            if Self::classify_word(policy, underscore, long, first) {
                for character in chars.by_ref() {
                    offset += character.len_utf8();
                    if !Self::classify_word(policy, underscore, long, character) {
                        break;
                    }
                }
            }
        }
        // Phase 2: skip separators, land on the first word char.
        for character in chars {
            if Self::classify_word(policy, underscore, long, character) {
                return Some(offset);
            }
            offset += character.len_utf8();
        }
        None
    }

    /// Byte offset just past the last char of the current/next word run
    /// (forward word-end). `None` at EOF.
    pub fn next_word_end(
        &self,
        caret: usize,
        policy: &WordSeparatorPolicy,
        underscore: bool,
        long: bool,
        stop_at_eol: bool,
    ) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        let end = if stop_at_eol {
            self.line_end_byte(caret)
        } else {
            self.rope.byte_len()
        };
        if caret >= end {
            return None;
        }
        let mut chars = self.rope.byte_slice(caret..end).chars();
        let mut offset = caret;
        let first = chars.next()?;
        offset += first.len_utf8();
        if Self::classify_word(policy, underscore, long, first) {
            // Skip the rest of the current word; return just past its last char.
            for character in chars.by_ref() {
                if !Self::classify_word(policy, underscore, long, character) {
                    return Some(offset);
                }
                offset += character.len_utf8();
            }
            return Some(offset);
        }
        // On a separator: skip separators, then return the end of the next word.
        for character in chars.by_ref() {
            if !Self::classify_word(policy, underscore, long, character) {
                offset += character.len_utf8();
                continue;
            }
            let mut word_end = offset + character.len_utf8();
            for next in chars.by_ref() {
                if !Self::classify_word(policy, underscore, long, next) {
                    return Some(word_end);
                }
                word_end += next.len_utf8();
            }
            return Some(word_end);
        }
        None
    }

    /// Byte offset of the start of the previous word run strictly before
    /// `caret`. `None` at document start.
    pub fn prev_word_start(
        &self,
        caret: usize,
        policy: &WordSeparatorPolicy,
        underscore: bool,
        long: bool,
    ) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        if caret == 0 {
            return None;
        }
        let mut chars = self.rope.byte_slice(..caret).chars().rev();
        let mut pos = caret;
        // Phase 1: skip trailing separators before the caret.
        loop {
            let character = chars.next()?;
            pos -= character.len_utf8();
            if Self::classify_word(policy, underscore, long, character) {
                break;
            }
        }
        // `pos` is now the start of the last char of the previous word.
        // Phase 2: skip the word backward to its start.
        for character in chars {
            if !Self::classify_word(policy, underscore, long, character) {
                return Some(pos);
            }
            pos -= character.len_utf8();
        }
        Some(pos)
    }

    /// Byte offset just past the last char of the previous word run (backward
    /// word-end). `None` when no previous word exists.
    pub fn prev_word_end(
        &self,
        caret: usize,
        policy: &WordSeparatorPolicy,
        underscore: bool,
        long: bool,
    ) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        if caret == 0 {
            return None;
        }
        let mut chars = self.rope.byte_slice(..caret).chars().rev();
        let mut pos = caret;
        // Phase A: skip the word tail the caret sits in/adjacent to.
        for character in chars.by_ref() {
            if !Self::classify_word(policy, underscore, long, character) {
                break;
            }
            pos -= character.len_utf8();
        }
        // Phase B: skip separators, then the previous word's last char end.
        for character in chars {
            if Self::classify_word(policy, underscore, long, character) {
                pos -= character.len_utf8();
                return Some(pos + character.len_utf8());
            }
            pos -= character.len_utf8();
        }
        None
    }

    /// Sub-word start boundary (camelCase / underscore / digit) strictly after
    /// `caret`. `camel = false` disables case/digit transitions, leaving only
    /// underscore and separator boundaries.
    pub fn next_sub_word_start(&self, caret: usize, camel: bool) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        let end = self.rope.byte_len();
        if caret >= end {
            return None;
        }
        let mut prev = self.rope.byte_slice(..caret).chars().next_back();
        let mut offset = caret;
        let mut first = true;
        for character in self.rope.byte_slice(caret..end).chars() {
            if !first && Self::is_sub_word_start(prev, character, camel) {
                return Some(offset);
            }
            first = false;
            prev = Some(character);
            offset += character.len_utf8();
        }
        None
    }

    /// Sub-word start boundary strictly before `caret`.
    pub fn prev_sub_word_start(&self, caret: usize, camel: bool) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        if caret == 0 {
            return None;
        }
        // Scan backward; `pending` holds the char closer to the caret from the
        // previous reverse iteration. When the current char `c` is the char
        // before `pending`'s char (document order), `pending`'s position is a
        // sub-word start iff is_sub_word_start(Some(c), pending_char).
        let mut pending: Option<(usize, char)> = None;
        let mut pos = caret;
        for character in self.rope.byte_slice(..caret).chars().rev() {
            pos -= character.len_utf8();
            if let Some((pending_pos, pending_char)) = pending
                && Self::is_sub_word_start(Some(character), pending_char, camel)
            {
                return Some(pending_pos);
            }
            pending = Some((pos, character));
        }
        // The first document char (last pending) is a sub-word start iff it is
        // alphanumeric with no predecessor.
        if let Some((pending_pos, pending_char)) = pending
            && Self::is_sub_word_start(None, pending_char, camel)
        {
            return Some(pending_pos);
        }
        None
    }

    fn is_sub_word_start(prev: Option<char>, current: char, camel: bool) -> bool {
        if !current.is_alphanumeric() {
            return false;
        }
        match prev {
            None => true,
            Some(p) if !p.is_alphanumeric() => true,
            Some(p) if camel => {
                (p.is_lowercase() && current.is_uppercase())
                    || (p.is_ascii_digit() && current.is_alphabetic())
                    || (p.is_alphabetic() && current.is_ascii_digit())
            }
            Some(_) => false,
        }
    }

    /// Byte offset of the start of the next blank line strictly after the
    /// caret's line (paragraph boundary). `None` when no such line exists.
    pub fn next_paragraph(&self, caret: usize, style: ParagraphStyle) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        let current_line = self.line_of_byte(caret);
        let line_count = self.line_len();
        let mut line = current_line + 1;
        while line < line_count {
            if self.is_blank_line(line, style) {
                return Some(self.byte_of_line(line));
            }
            line += 1;
        }
        None
    }

    /// Byte offset of the start of the previous blank line strictly before the
    /// caret's line. `None` at document start.
    pub fn prev_paragraph(&self, caret: usize, style: ParagraphStyle) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        let current_line = self.line_of_byte(caret);
        let mut line = current_line;
        while line > 0 {
            line -= 1;
            if self.is_blank_line(line, style) {
                return Some(self.byte_of_line(line));
            }
        }
        None
    }

    /// End of the paragraph containing `caret` (start of the trailing blank
    /// line, or document end).
    pub fn paragraph_end_byte(&self, caret: usize, style: ParagraphStyle) -> usize {
        let current_line = self.line_of_byte(caret);
        let line_count = self.line_len();
        let mut line = current_line;
        while line < line_count && self.is_blank_line(line, style) {
            line += 1;
        }
        while line < line_count && !self.is_blank_line(line, style) {
            line += 1;
        }
        if line >= line_count {
            self.document_end_byte()
        } else {
            self.byte_of_line(line)
        }
    }

    fn is_blank_line(&self, line: usize, style: ParagraphStyle) -> bool {
        let start = self.byte_of_line(line);
        let next = self.byte_of_line((line + 1).min(self.line_len()));
        let content = self.rope.byte_slice(start..next).to_string();
        let trimmed = content.trim_end_matches(['\n', '\r']);
        match style {
            ParagraphStyle::BlankLine => trimmed.is_empty(),
            ParagraphStyle::BlankLineOrWhitespace => trimmed.chars().all(|c| c.is_whitespace()),
        }
    }

    /// First non-whitespace byte on the caret's line (line start if all blank).
    pub fn first_non_blank_byte(&self, caret: usize) -> usize {
        let line = self.line_of_byte(caret);
        let start = self.byte_of_line(line);
        let next = self.byte_of_line((line + 1).min(self.line_len()));
        let mut offset = start;
        for character in self.rope.byte_slice(start..next).chars() {
            if character == '\n' || character == '\r' {
                break;
            }
            if !character.is_whitespace() {
                return offset;
            }
            offset += character.len_utf8();
        }
        start
    }

    /// Byte offset just past the last non-whitespace char on the caret's line
    /// (line end if all blank).
    pub fn last_non_blank_byte(&self, caret: usize) -> usize {
        let line = self.line_of_byte(caret);
        let start = self.byte_of_line(line);
        let next = self.byte_of_line((line + 1).min(self.line_len()));
        let mut last_end = None;
        let mut offset = start;
        for character in self.rope.byte_slice(start..next).chars() {
            if character == '\n' || character == '\r' {
                break;
            }
            if !character.is_whitespace() {
                last_end = Some(offset + character.len_utf8());
            }
            offset += character.len_utf8();
        }
        last_end.unwrap_or(self.line_end_byte(caret))
    }

    /// `(start, end)` byte range of the word run containing `caret`. Returns
    /// `None` when the caret is not on a word character (separator/whitespace);
    /// callers may no-op or scan forward. Combining marks continue a word.
    /// `ponytail:` between-words caret no-ops; VSCode selects the next word —
    /// deferred until a `count`-aware select op needs it.
    pub fn word_range_at(
        &self,
        caret: usize,
        policy: &WordSeparatorPolicy,
        underscore: bool,
        long: bool,
    ) -> Option<(usize, usize)> {
        let caret = self.clamp_byte_offset(caret);
        let current = self.char_at(caret)?;
        if !Self::classify_word(policy, underscore, long, current) {
            return None;
        }
        let mut start = caret;
        while let Some(prev) = self.char_before(start)
            && Self::classify_word(policy, underscore, long, prev)
        {
            start -= prev.len_utf8();
        }
        let mut end = caret;
        while let Some(next) = self.char_at(end)
            && Self::classify_word(policy, underscore, long, next)
        {
            end += next.len_utf8();
        }
        Some((start, end))
    }

    /// `(start, end)` byte range of the caret's line content (excludes the line
    /// terminator). Whole-line selection for `Ctrl+L`.
    pub fn line_range(&self, caret: usize) -> (usize, usize) {
        (self.line_start_byte(caret), self.line_end_byte(caret))
    }

    /// `(start, end)` byte range of the paragraph containing `caret`, where a
    /// paragraph is a maximal run of non-blank lines. A blank caret line yields
    /// that line's range. Used by `Ctrl+Shift+L`-style paragraph selection.
    pub fn paragraph_range(&self, caret: usize, style: ParagraphStyle) -> (usize, usize) {
        let line = self.line_of_byte(caret);
        let total = self.line_len();
        let mut start_line = line;
        while start_line > 0 && !self.is_blank_line(start_line - 1, style) {
            start_line -= 1;
        }
        let mut end_line = line;
        while end_line + 1 < total && !self.is_blank_line(end_line + 1, style) {
            end_line += 1;
        }
        (
            self.byte_of_line(start_line),
            self.line_end_byte_for_line(end_line),
        )
    }

    /// Matching bracket offset for a single-char `open`/`close` pair. Detects
    /// the bracket at `caret` (or just before it), then scans forward (open) or
    /// backward (close) with depth counting. `None` if no bracket is present or
    /// the match is unbalanced.
    /// `ponytail:` single-char distinct open/close pairs only; same-char and
    /// multi-char pairs are not supported for motion.
    pub fn matching_pair_byte(&self, caret: usize, open: char, close: char) -> Option<usize> {
        self.matching_pair_byte_within(caret, open, close, usize::MAX)
    }

    /// Like [`Self::matching_pair_byte`], but stop after `max_bytes` from the
    /// detected bracket. Chrome paint uses a 64 KiB ceiling so a missing closer
    /// cannot walk the whole document on every frame.
    pub fn matching_pair_byte_within(
        &self,
        caret: usize,
        open: char,
        close: char,
        max_bytes: usize,
    ) -> Option<usize> {
        let caret = self.clamp_byte_offset(caret);
        let end = self.rope.byte_len();
        let current = self.char_at(caret);
        let (anchor, forward) = match current {
            Some(c) if c == open => (caret, true),
            Some(c) if c == close => (caret, false),
            _ => {
                let prev = self.char_before(caret)?;
                if prev == open {
                    (caret - prev.len_utf8(), true)
                } else if prev == close {
                    (caret - prev.len_utf8(), false)
                } else {
                    return None;
                }
            }
        };
        if forward {
            let mut depth = 1usize;
            let mut offset = anchor + open.len_utf8();
            for character in self
                .rope
                .byte_slice((anchor + open.len_utf8())..end)
                .chars()
            {
                if offset.saturating_sub(anchor) > max_bytes {
                    return None;
                }
                if character == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(offset);
                    }
                } else if character == open {
                    depth += 1;
                }
                offset += character.len_utf8();
            }
            None
        } else {
            let mut depth = 1usize;
            let mut offset = anchor;
            for character in self.rope.byte_slice(..anchor).chars().rev() {
                offset -= character.len_utf8();
                if anchor.saturating_sub(offset) > max_bytes {
                    return None;
                }
                if character == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some(offset);
                    }
                } else if character == close {
                    depth += 1;
                }
            }
            None
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

        let rejected = buffer.delete_range(std::ops::Range { start: 3, end: 1 });
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

#[cfg(test)]
mod movement_tests {
    use super::EditorBuffer;
    use crate::protocol::{ParagraphStyle, WordSeparatorPolicy};

    const CODE: WordSeparatorPolicy = WordSeparatorPolicy::Code;

    #[test]
    fn word_start_lands_on_next_word_in_code_policy() {
        // foo.bar_baz: dot is a separator; underscore joins (code policy).
        let buffer = EditorBuffer::from_text("foo.bar_baz");
        // From `f`: skip `foo`, skip `.`, land on `b` of `bar` (offset 4).
        assert_eq!(buffer.next_word_start(0, &CODE, true, false), Some(4));
        // `bar_baz` is one word (underscore joins), so from `b` of `bar` there
        // is no further word start.
        assert_eq!(buffer.next_word_start(4, &CODE, true, false), None);
    }

    #[test]
    fn word_start_underscore_as_separator_splits_snake_case() {
        let buffer = EditorBuffer::from_text("foo.bar_baz");
        // underscore NOT a word char: `bar` and `baz` are separate words.
        assert_eq!(buffer.next_word_start(0, &CODE, false, false), Some(4));
        assert_eq!(buffer.next_word_start(4, &CODE, false, false), Some(8));
    }

    #[test]
    fn long_word_skips_punctuation() {
        // `a, b, c`: long-WORD treats only whitespace as a separator, so `a,`
        // is one word and the next start is `b` (offset 3).
        let buffer = EditorBuffer::from_text("a, b, c");
        assert_eq!(buffer.next_word_start(0, &CODE, true, false), Some(3));
    }

    #[test]
    fn word_end_lands_just_past_last_word_char() {
        let buffer = EditorBuffer::from_text("foo.bar");
        // From `f`: end of `foo` is offset 3 (just past `o`, before `.`).
        assert_eq!(buffer.next_word_end(0, &CODE, true, false, false), Some(3));
        // From `.`: end of `bar` is offset 7.
        assert_eq!(buffer.next_word_end(3, &CODE, true, false, false), Some(7));
    }

    #[test]
    fn prev_word_start_skips_back_to_previous_word() {
        let buffer = EditorBuffer::from_text("foo.bar");
        // From end (offset 7): previous word start is `b` of `bar` (offset 4).
        assert_eq!(buffer.prev_word_start(7, &CODE, true, false), Some(4));
        // From `b` (offset 4): previous word start is `f` of `foo` (offset 0).
        assert_eq!(buffer.prev_word_start(4, &CODE, true, false), Some(0));
    }

    #[test]
    fn sub_word_splits_camel_case_and_underscore() {
        // fooBar_baz: sub-word starts at `B` of `Bar` (3) and `b` of `baz` (7).
        let buffer = EditorBuffer::from_text("fooBar_baz");
        assert_eq!(buffer.next_sub_word_start(0, true), Some(3));
        assert_eq!(buffer.next_sub_word_start(3, true), Some(7));
        // Backward from end: `b` of `baz` (7), then `B` of `Bar` (3), then `f` (0).
        assert_eq!(buffer.prev_sub_word_start(10, true), Some(7));
        assert_eq!(buffer.prev_sub_word_start(7, true), Some(3));
        assert_eq!(buffer.prev_sub_word_start(3, true), Some(0));
    }

    #[test]
    fn sub_word_disabled_camel_falls_back_to_underscore_only() {
        // camel = false: `B` of `Bar` is NOT a boundary (no case transition),
        // but `_` still splits, so the only sub-word start after `foo` is `b` of
        // `baz` (offset 7).
        let buffer = EditorBuffer::from_text("fooBar_baz");
        assert_eq!(buffer.next_sub_word_start(0, false), Some(7));
    }

    #[test]
    fn paragraph_next_prev_and_end_use_blank_lines() {
        // a, blank, b, blank, c
        let buffer = EditorBuffer::from_text("a\n\nb\n\nc");
        let style = ParagraphStyle::BlankLineOrWhitespace;
        // Next paragraph from `a` (line 0): start of the blank line at offset 2.
        assert_eq!(buffer.next_paragraph(0, style), Some(2));
        // Previous paragraph from `c` (line 4): start of the blank line at
        // offset 5 (after `b\n`).
        assert_eq!(
            buffer.prev_paragraph(buffer.document_end_byte(), style),
            Some(5)
        );
        // Paragraph end from `a`: end of the `a` paragraph = offset 2.
        assert_eq!(buffer.paragraph_end_byte(0, style), 2);
        // Paragraph end from `b` (offset 3): end of the `b` paragraph = offset 5.
        assert_eq!(buffer.paragraph_end_byte(3, style), 5);
    }

    #[test]
    fn non_blank_first_and_last_on_a_line() {
        // `  foo  bar  ` with a trailing newline.
        let buffer = EditorBuffer::from_text("  foo  bar  \n");
        // First non-blank: `f` at offset 2.
        assert_eq!(buffer.first_non_blank_byte(0), 2);
        // Last non-blank: just past `r` of `bar` at offset 10.
        assert_eq!(buffer.last_non_blank_byte(0), 10);
    }

    #[test]
    fn matching_pair_toggles_between_brackets() {
        let buffer = EditorBuffer::from_text("({[]})");
        // `(` at 0 matches `)` at 5.
        assert_eq!(buffer.matching_pair_byte(0, '(', ')'), Some(5));
        // `)` at 5 matches `(` at 0.
        assert_eq!(buffer.matching_pair_byte(5, '(', ')'), Some(0));
        // `{` at 1 matches `}` at 4.
        assert_eq!(buffer.matching_pair_byte(1, '{', '}'), Some(4));
        // `]` at 3 matches `[` at 2 (backward).
        assert_eq!(buffer.matching_pair_byte(3, '[', ']'), Some(2));
    }

    #[test]
    fn matching_pair_handles_caret_after_close() {
        let buffer = EditorBuffer::from_text("(a)");
        // Caret just after `)` (offset 3): char_before is `)` → backward to `(`.
        assert_eq!(buffer.matching_pair_byte(3, '(', ')'), Some(0));
    }

    #[test]
    fn matching_pair_balances_nested_brackets() {
        let buffer = EditorBuffer::from_text("((a))");
        // Outer `(` at 0 matches the last `)` at 4.
        assert_eq!(buffer.matching_pair_byte(0, '(', ')'), Some(4));
        // Inner `(` at 1 matches the first `)` at 3.
        assert_eq!(buffer.matching_pair_byte(1, '(', ')'), Some(3));
    }

    #[test]
    fn matching_pair_within_stops_at_byte_ceiling() {
        let buffer = EditorBuffer::from_text(&format!("({} )", "x".repeat(16)));
        assert_eq!(buffer.matching_pair_byte_within(0, '(', ')', 4), None);
        assert_eq!(buffer.matching_pair_byte_within(0, '(', ')', 64), Some(18));
    }

    #[test]
    fn combining_marks_do_not_split_a_grapheme() {
        // `e` + combining acute (a 2-byte mark), then ` bar`. Word-end from `e`
        // must land after the grapheme (offset 3, past the 2-byte mark), not at
        // offset 1 (between `e` and the mark).
        let buffer = EditorBuffer::from_text("e\u{0301} bar");
        assert_eq!(buffer.next_word_end(0, &CODE, true, false, false), Some(3));
        // Next word start lands on `b` of `bar` at offset 4 (after the space).
        assert_eq!(buffer.next_word_start(0, &CODE, true, false), Some(4));
    }

    #[test]
    fn crlf_is_treated_as_a_separator_pair() {
        // `a\r\nb`: word motion skips both `\r` and `\n` and never lands between
        // them. Next word start from `a` lands on `b`.
        let buffer = EditorBuffer::from_text("a\r\nb");
        assert_eq!(buffer.next_word_start(0, &CODE, true, false), Some(3));
    }
}
