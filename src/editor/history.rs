//! Per-document client undo/redo history for ordinary inverse edits.
//!
//! History is client-local. Inverse applications are emitted as normal
//! optimistic `Edit` transactions; the server remains unaware of undo/redo as
//! distinct operations. See
//! `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`.

use std::collections::VecDeque;

use crate::perf::budgets::{EDIT_HISTORY_MAX_DEPTH, EDIT_HISTORY_MAX_ENTRY_BYTES};
use crate::protocol::EditOperation;

/// Caret/selection restore metadata for one history edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistorySelection {
    /// Focus / caret byte offset after the corresponding edge.
    pub caret: usize,
    /// Selection anchor when a range existed; `None` means collapsed.
    pub anchor: Option<usize>,
}

impl HistorySelection {
    #[allow(dead_code)]
    pub fn collapsed(caret: usize) -> Self {
        Self {
            caret,
            anchor: None,
        }
    }
}

/// One coherent local edit and its inverse, with caret/selection restore points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub forward: EditOperation,
    pub inverse: EditOperation,
    pub selection_before: HistorySelection,
    pub selection_after: HistorySelection,
}

impl HistoryEntry {
    pub fn entry_payload_bytes(&self) -> usize {
        operation_text_bytes(&self.forward).saturating_add(operation_text_bytes(&self.inverse))
    }
}

/// Bounded per-document undo/redo stacks.
#[derive(Debug, Clone, Default)]
pub struct EditHistory {
    undo: VecDeque<HistoryEntry>,
    redo: VecDeque<HistoryEntry>,
}

impl EditHistory {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[allow(dead_code)]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    #[allow(dead_code)]
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Record a new divergent user edit. Clears redo. Drops the oldest undo
    /// entry when depth exceeds [`EDIT_HISTORY_MAX_DEPTH`]. Oversized entries
    /// clear history instead of retaining an unbounded payload.
    pub fn record(&mut self, entry: HistoryEntry) {
        self.redo.clear();
        if entry.entry_payload_bytes() > EDIT_HISTORY_MAX_ENTRY_BYTES {
            self.clear();
            return;
        }
        self.undo.push_back(entry);
        while self.undo.len() > EDIT_HISTORY_MAX_DEPTH {
            self.undo.pop_front();
        }
    }

    /// Pop the latest undo entry, push it onto redo, and return it.
    pub fn undo(&mut self) -> Option<HistoryEntry> {
        let entry = self.undo.pop_back()?;
        self.redo.push_back(entry.clone());
        while self.redo.len() > EDIT_HISTORY_MAX_DEPTH {
            self.redo.pop_front();
        }
        Some(entry)
    }

    /// Pop the latest redo entry, push it onto undo, and return it.
    pub fn redo(&mut self) -> Option<HistoryEntry> {
        let entry = self.redo.pop_back()?;
        self.undo.push_back(entry.clone());
        while self.undo.len() > EDIT_HISTORY_MAX_DEPTH {
            self.undo.pop_front();
        }
        Some(entry)
    }
}

/// Build the inverse of a forward operation using the pre-edit prior text.
pub fn invert_edit_operation(forward: &EditOperation, prior_text: &str) -> EditOperation {
    match forward {
        EditOperation::Insert { byte_offset, text } => EditOperation::Delete {
            start: *byte_offset,
            end: byte_offset.saturating_add(text.len() as u64),
        },
        EditOperation::Delete { start, .. } => EditOperation::Insert {
            byte_offset: *start,
            text: prior_text.to_string(),
        },
        EditOperation::Replace { start, text, .. } => EditOperation::Replace {
            start: *start,
            end: start.saturating_add(text.len() as u64),
            text: prior_text.to_string(),
        },
    }
}

fn operation_text_bytes(operation: &EditOperation) -> usize {
    match operation {
        EditOperation::Insert { text, .. } | EditOperation::Replace { text, .. } => text.len(),
        EditOperation::Delete { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{EditHistory, HistoryEntry, HistorySelection, invert_edit_operation};
    use crate::perf::budgets::{EDIT_HISTORY_MAX_DEPTH, EDIT_HISTORY_MAX_ENTRY_BYTES};
    use crate::protocol::EditOperation;

    fn insert_entry(offset: u64, text: &str) -> HistoryEntry {
        let forward = EditOperation::Insert {
            byte_offset: offset,
            text: text.to_string(),
        };
        let inverse = invert_edit_operation(&forward, "");
        HistoryEntry {
            forward,
            inverse,
            selection_before: HistorySelection::collapsed(offset as usize),
            selection_after: HistorySelection::collapsed(offset as usize + text.len()),
        }
    }

    #[test]
    fn undo_insert_produces_delete_inverse_and_redo_restores() {
        let mut history = EditHistory::new();
        history.record(insert_entry(0, "hi"));

        let undone = history.undo().expect("undo entry");
        assert_eq!(undone.inverse, EditOperation::Delete { start: 0, end: 2 });
        assert!(history.can_redo());
        assert!(!history.can_undo());

        let redone = history.redo().expect("redo entry");
        assert_eq!(
            redone.forward,
            EditOperation::Insert {
                byte_offset: 0,
                text: "hi".to_string(),
            }
        );
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut history = EditHistory::new();
        history.record(insert_entry(0, "a"));
        let _ = history.undo();
        assert!(history.can_redo());
        history.record(insert_entry(0, "b"));
        assert!(!history.can_redo());
        assert_eq!(history.undo_len(), 1);
    }

    #[test]
    fn stack_depth_ceiling_drops_oldest_entries() {
        let mut history = EditHistory::new();
        for index in 0..(EDIT_HISTORY_MAX_DEPTH + 3) {
            history.record(insert_entry(0, &format!("x{index}")));
        }
        assert_eq!(history.undo_len(), EDIT_HISTORY_MAX_DEPTH);
        assert_eq!(history.redo_len(), 0);
        let oldest_kept = history.undo().expect("oldest kept after drops");
        // After pushing depth+3 and popping once from the back, the back entry
        // is the newest remaining (index depth+2). Depth drops oldest first.
        assert!(matches!(
            oldest_kept.forward,
            EditOperation::Insert { text, .. } if text == format!("x{}", EDIT_HISTORY_MAX_DEPTH + 2)
        ));
        assert_eq!(history.undo_len(), EDIT_HISTORY_MAX_DEPTH - 1);
    }

    #[test]
    fn oversized_entry_clears_history() {
        let mut history = EditHistory::new();
        history.record(insert_entry(0, "keep"));
        let huge = "a".repeat(EDIT_HISTORY_MAX_ENTRY_BYTES + 1);
        history.record(insert_entry(0, &huge));
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn invert_replace_and_delete_restore_prior_text() {
        let replace = EditOperation::Replace {
            start: 1,
            end: 4,
            text: "XY".to_string(),
        };
        assert_eq!(
            invert_edit_operation(&replace, "abc"),
            EditOperation::Replace {
                start: 1,
                end: 3,
                text: "abc".to_string(),
            }
        );

        let delete = EditOperation::Delete { start: 2, end: 5 };
        assert_eq!(
            invert_edit_operation(&delete, "def"),
            EditOperation::Insert {
                byte_offset: 2,
                text: "def".to_string(),
            }
        );
    }
}
