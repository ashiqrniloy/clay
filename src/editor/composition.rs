//! Client-local IME/composition preedit state.
//!
//! Preedit text is paint-only until `Ime::Commit`. It is never canonical
//! document text and never enqueues edits or IPC. See
//! `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`.

/// Paint-only IME composition overlay for the active editor surface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompositionState {
    /// Preedit string shown at the caret. Empty means no active composition.
    text: String,
    /// Optional byte-indexed cursor span within `text` `(begin, end)`.
    cursor: Option<(usize, usize)>,
}

impl CompositionState {
    /// True when a non-empty preedit overlay is active.
    pub fn is_active(&self) -> bool {
        !self.text.is_empty()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor_span(&self) -> Option<(usize, usize)> {
        self.cursor
    }

    /// Replace preedit contents. Empty `text` clears the overlay.
    ///
    /// Cursor byte offsets are clamped to the preedit UTF-8 length and ordered
    /// so `begin <= end`. Invalid offsets outside the string become `None`.
    pub fn set_preedit(&mut self, text: String, cursor: Option<(usize, usize)>) -> bool {
        if text.is_empty() {
            return self.clear();
        }

        let len = text.len();
        let cursor = cursor.and_then(|(begin, end)| {
            if begin > len || end > len {
                return None;
            }
            let (begin, end) = if begin <= end {
                (begin, end)
            } else {
                (end, begin)
            };
            // Keep spans on char boundaries when possible; otherwise drop the
            // cursor hint rather than splitting a scalar.
            if !text.is_char_boundary(begin) || !text.is_char_boundary(end) {
                return None;
            }
            Some((begin, end))
        });

        let changed = self.text != text || self.cursor != cursor;
        self.text = text;
        self.cursor = cursor;
        changed
    }

    /// Discard unfinished composition without committing.
    pub fn clear(&mut self) -> bool {
        if self.text.is_empty() && self.cursor.is_none() {
            return false;
        }
        self.text.clear();
        self.cursor = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::CompositionState;

    #[test]
    fn empty_preedit_clears_overlay() {
        let mut composition = CompositionState::default();
        assert!(composition.set_preedit("あ".into(), Some((0, 3))));
        assert!(composition.is_active());
        assert!(composition.set_preedit(String::new(), None));
        assert!(!composition.is_active());
        assert_eq!(composition.text(), "");
        assert_eq!(composition.cursor_span(), None);
    }

    #[test]
    fn set_preedit_orders_cursor_span_and_is_idempotent() {
        let mut composition = CompositionState::default();
        assert!(composition.set_preedit("abc".into(), Some((3, 1))));
        assert_eq!(composition.cursor_span(), Some((1, 3)));
        assert!(!composition.set_preedit("abc".into(), Some((1, 3))));
    }

    #[test]
    fn invalid_cursor_outside_text_is_dropped() {
        let mut composition = CompositionState::default();
        assert!(composition.set_preedit("hi".into(), Some((0, 8))));
        assert_eq!(composition.cursor_span(), None);
        assert_eq!(composition.text(), "hi");
    }

    #[test]
    fn clear_is_idempotent() {
        let mut composition = CompositionState::default();
        assert!(!composition.clear());
        assert!(composition.set_preedit("x".into(), None));
        assert!(composition.clear());
        assert!(!composition.clear());
    }
}
