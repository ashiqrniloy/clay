use std::ops::Range;

use super::buffer::EditorBuffer;
use super::cursor::CursorState;

/// A single selection: a fixed `anchor` end and a movable `focus` (the caret)
/// carried as a [`CursorState`] so the focus shares the same `preferred_x`
/// column-preservation as the legacy single caret. `Copy` because both halves
/// are trivially copyable; the multi-cursor set ([`SelectionState`]) owns the
/// heap allocation.
///
/// Plan 071 task 8: this is the per-cursor unit of the unified selection set.
/// `anchor == focus` means the selection is collapsed (a bare caret, no range);
/// callers treat a collapsed primary selection as "no selection" to preserve
/// the legacy `Option<SelectionState>` bit-for-bit semantics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    anchor: usize,
    cursor: CursorState,
}

impl Selection {
    /// Build a selection with an explicit anchor and focus.
    pub fn new(anchor: usize, focus: usize) -> Self {
        Self {
            anchor,
            cursor: CursorState::new(focus),
        }
    }

    /// Build a collapsed selection (a bare caret, anchor == focus).
    pub fn collapsed(focus: usize) -> Self {
        Self::new(focus, focus)
    }

    /// Mutable access to the underlying cursor. Movement closures borrow this
    /// so the existing CursorState move_to_* API drives the focus without a
    /// parallel caret store.
    pub fn cursor_mut(&mut self) -> &mut CursorState {
        &mut self.cursor
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn focus(&self) -> usize {
        self.cursor.caret()
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.cursor.caret()
    }

    /// Replace the focus (clears `preferred_x`, mirroring `CursorState::set_caret`).
    pub fn set_focus(&mut self, focus: usize) {
        self.cursor.set_caret(focus);
    }

    /// Set the anchor without disturbing the focus or `preferred_x`. Used by
    /// extend-selection, which moves the focus via the cursor and then fixes
    /// the anchor separately.
    pub fn set_anchor(&mut self, anchor: usize) {
        self.anchor = anchor;
    }

    pub fn normalized_range(&self) -> Range<usize> {
        self.anchor.min(self.focus())..self.anchor.max(self.focus())
    }
}

/// Unified selection set with a primary index. Replaces the legacy pair of a
/// single `CursorState` plus an optional `SelectionState` range so multi-cursor
/// state is one store from day one (Plan 071 task 8 decision: unified over
/// parallel). The invariant is `selections` is non-empty and `primary <
/// selections.len()`; a "no selection" caret is a single collapsed `Selection`
/// at the primary index, preserving the old `Option::None` semantics exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionState {
    selections: Vec<Selection>,
    primary: usize,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selections: vec![Selection::new(0, 0)],
            primary: 0,
        }
    }
}

impl SelectionState {
    /// All selections, primary first is NOT guaranteed; use `primary` for the
    /// active caret. Paint iterates this to render every range + caret.
    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    pub fn primary(&self) -> &Selection {
        &self.selections[self.primary]
    }

    pub fn primary_mut(&mut self) -> &mut Selection {
        &mut self.selections[self.primary]
    }

    pub fn primary_index(&self) -> usize {
        self.primary
    }

    /// Number of selections in the set (Plan 071 task 9 multi-cursor).
    pub fn selection_count(&self) -> usize {
        self.selections.len()
    }

    /// Mutable access to the selection at `index` (e.g. to move one caret of
    /// a multi-cursor set independently).
    pub fn selection_mut(&mut self, index: usize) -> &mut Selection {
        &mut self.selections[index]
    }

    /// Active caret byte offset (the primary selection's focus).
    pub fn primary_focus(&self) -> usize {
        self.primary().focus()
    }

    pub fn primary_anchor(&self) -> usize {
        self.primary().anchor()
    }

    /// True when the primary selection is an active range (not collapsed).
    /// Replaces the legacy `self.selection.is_some()`.
    pub fn has_selection(&self) -> bool {
        !self.primary().is_collapsed()
    }

    /// The primary selection's range, or `None` when collapsed. Replaces the
    /// legacy `selected_range` over `Option<SelectionState>`.
    pub fn primary_range(&self) -> Option<Range<usize>> {
        let selection = self.primary();
        let range = selection.normalized_range();
        (range.start < range.end).then_some(range)
    }

    /// Move the primary focus, clearing `preferred_x` (mirrors `CursorState::set_caret`).
    /// Does not touch the anchor; pair with [`collapse_primary`] when clearing
    /// the selection.
    pub fn set_primary_focus(&mut self, focus: usize) {
        self.primary_mut().set_focus(focus);
    }

    /// Collapse the primary selection to its current focus (anchor := focus),
    /// i.e. "no selection". Replaces the legacy `self.selection = None`.
    pub fn collapse_primary(&mut self) {
        let focus = self.primary_focus();
        self.primary_mut().set_anchor(focus);
    }

    /// Clamp the primary anchor in place without disturbing the focus or
    /// `preferred_x`. Used by extend-selection after a cursor move.
    pub fn clamp_primary_anchor(&mut self, buffer: &EditorBuffer) {
        let anchor = buffer.clamp_byte_offset(self.primary_anchor());
        self.primary_mut().set_anchor(anchor);
    }

    /// Append a selection and make it the new primary (e.g. add-cursor-below,
    /// select-next-match adding a match). Repeated calls stack so the most
    /// recently added caret is the active one (Plan 071 task 9).
    pub fn push_and_make_primary(&mut self, selection: Selection) {
        self.selections.push(selection);
        self.primary = self.selections.len() - 1;
    }

    /// Append a selection without changing the primary (used by multi-cursor
    /// test helpers; production set-building goes through `set_selections`).
    #[allow(dead_code)]
    pub fn push_selection(&mut self, selection: Selection) {
        self.selections.push(selection);
    }

    /// Replace the whole set. `primary` is clamped into bounds; an empty
    /// `selections` is rejected (the set is never empty).
    pub fn set_selections(&mut self, selections: Vec<Selection>, primary: usize) -> bool {
        if selections.is_empty() {
            return false;
        }
        self.selections = selections;
        self.primary = primary.min(self.selections.len() - 1);
        true
    }

    /// Keep only the primary selection, dropping every other caret/range (Helix
    /// `keep_primary_selection`; VSCode-style collapse-to-primary is this plus
    /// [`collapse_primary`]).
    pub fn keep_only_primary(&mut self) {
        let kept = *self.primary();
        self.selections = vec![kept];
        self.primary = 0;
    }

    /// Remove the primary selection, keeping the rest; the new primary is the
    /// first remaining selection (Helix `remove_primary_selection`). A no-op
    /// when only one selection exists (cannot remove the last caret).
    pub fn remove_primary(&mut self) {
        if self.selections.len() <= 1 {
            return;
        }
        self.selections.remove(self.primary);
        self.primary = self.primary.min(self.selections.len() - 1);
    }

    /// Clamp every caret/anchor into `buffer` bounds. Cursor-undo snapshots
    /// may predate edits, so restored sets are clamped before install.
    pub fn clamp_to(&mut self, buffer: &EditorBuffer) {
        for selection in &mut self.selections {
            let focus = buffer.clamp_byte_offset(selection.focus());
            let anchor = buffer.clamp_byte_offset(selection.anchor());
            selection.set_focus(focus);
            selection.set_anchor(anchor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorBuffer, Selection, SelectionState};

    #[test]
    fn selection_normalizes_anchor_and_focus() {
        let forward = Selection::new(2, 7);
        let backward = Selection::new(7, 2);

        assert_eq!(forward.normalized_range(), 2..7);
        assert_eq!(backward.normalized_range(), 2..7);
    }

    #[test]
    fn selection_reports_collapsed_anchor_focus() {
        let collapsed = Selection::new(3, 3);

        assert!(collapsed.is_collapsed());
        assert_eq!(collapsed.anchor(), 3);
        assert_eq!(collapsed.focus(), 3);
    }

    #[test]
    fn selection_set_focus_clears_preferred_x() {
        let mut selection = Selection::new(0, 5);
        selection.cursor_mut().set_preferred_x(42.0);
        assert_eq!(selection.cursor_mut().preferred_x(), Some(42.0));

        selection.set_focus(9);

        assert_eq!(selection.focus(), 9);
        assert_eq!(selection.cursor_mut().preferred_x(), None);
    }

    #[test]
    fn selection_set_anchor_preserves_focus_and_preferred_x() {
        let mut selection = Selection::new(0, 5);
        selection.cursor_mut().set_preferred_x(42.0);

        selection.set_anchor(2);

        assert_eq!(selection.anchor(), 2);
        assert_eq!(selection.focus(), 5);
        assert_eq!(selection.cursor_mut().preferred_x(), Some(42.0));
    }

    #[test]
    fn selection_state_defaults_to_single_collapsed_caret() {
        let state = SelectionState::default();

        assert_eq!(state.selection_count(), 1);
        assert_eq!(state.primary_index(), 0);
        assert!(!state.has_selection());
        assert_eq!(state.primary_range(), None);
        assert_eq!(state.primary_focus(), 0);
    }

    #[test]
    fn selection_state_has_selection_only_when_primary_is_expanded() {
        let mut state = SelectionState {
            selections: vec![Selection::new(3, 3)],
            primary: 0,
        };
        assert!(!state.has_selection());

        state.primary_mut().set_anchor(0);
        assert!(state.has_selection());
        assert_eq!(state.primary_range(), Some(0..3));
    }

    #[test]
    fn selection_state_collapse_primary_clears_range() {
        let mut state = SelectionState {
            selections: vec![Selection::new(0, 0)],
            primary: 0,
        };
        state.primary_mut().set_anchor(4);
        assert!(state.has_selection());

        state.collapse_primary();

        assert!(!state.has_selection());
        assert_eq!(state.primary_focus(), 0);
        assert_eq!(state.primary_anchor(), 0);
    }

    #[test]
    fn push_and_make_primary_stacks_newest_caret() {
        let mut state = SelectionState::default();
        state.push_and_make_primary(Selection::collapsed(5));
        state.push_and_make_primary(Selection::collapsed(9));

        assert_eq!(state.selection_count(), 3);
        assert_eq!(state.primary_index(), 2);
        assert_eq!(state.primary_focus(), 9);
    }

    #[test]
    fn set_selections_replaces_and_clamps_primary() {
        let mut state = SelectionState::default();
        let ok = state.set_selections(vec![Selection::new(0, 3), Selection::new(8, 11)], 5);

        assert!(ok);
        assert_eq!(state.selection_count(), 2);
        assert_eq!(state.primary_index(), 1, "primary clamped to last");
        // An empty set is rejected.
        assert!(!state.set_selections(Vec::new(), 0));
        assert_eq!(state.selection_count(), 2);
    }

    #[test]
    fn keep_only_primary_drops_secondaries() {
        let mut state = SelectionState::default();
        state.push_and_make_primary(Selection::new(8, 11));
        assert_eq!(state.selection_count(), 2);

        state.keep_only_primary();

        assert_eq!(state.selection_count(), 1);
        assert_eq!(state.primary_index(), 0);
        assert_eq!(state.primary_range(), Some(8..11), "range survives");
    }

    #[test]
    fn remove_primary_keeps_rest_and_is_noop_on_single() {
        let mut state = SelectionState::default();
        state.push_selection(Selection::new(8, 11));
        state.push_selection(Selection::new(16, 19));
        assert_eq!(state.selection_count(), 3);

        state.remove_primary();
        assert_eq!(state.selection_count(), 2);

        state.remove_primary();
        assert_eq!(state.selection_count(), 1);
        // Cannot remove the last selection.
        state.remove_primary();
        assert_eq!(state.selection_count(), 1);
    }

    #[test]
    fn clamp_to_clamps_every_caret_into_bounds() {
        let buffer = EditorBuffer::from_text("abc");
        let mut state = SelectionState::default();
        state.push_and_make_primary(Selection::new(10, 99));

        state.clamp_to(&buffer);

        let end = buffer.document_end_byte();
        assert_eq!(state.primary_anchor(), end);
        assert_eq!(state.primary_focus(), end);
    }
}
