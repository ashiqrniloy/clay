use std::ops::Range;

use super::buffer::EditorBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionState {
    anchor: usize,
    focus: usize,
}

impl SelectionState {
    pub fn new(anchor: usize, focus: usize) -> Self {
        Self { anchor, focus }
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    #[cfg(test)]
    pub fn focus(&self) -> usize {
        self.focus
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    pub fn set_focus(&mut self, focus: usize) {
        self.focus = focus;
    }

    pub fn normalized_range(&self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    pub fn clamped(self, buffer: &EditorBuffer) -> Self {
        Self {
            anchor: buffer.clamp_byte_offset(self.anchor),
            focus: buffer.clamp_byte_offset(self.focus),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SelectionState;

    #[test]
    fn selection_normalizes_anchor_and_focus() {
        let forward = SelectionState::new(2, 7);
        let backward = SelectionState::new(7, 2);

        assert_eq!(forward.normalized_range(), 2..7);
        assert_eq!(backward.normalized_range(), 2..7);
    }

    #[test]
    fn selection_reports_collapsed_anchor_focus() {
        let collapsed = SelectionState::new(3, 3);

        assert!(collapsed.is_collapsed());
        assert_eq!(collapsed.anchor(), 3);
        assert_eq!(collapsed.focus(), 3);
    }
}
