# 06 — Multi-Cursor Editing

Match selection, column selection, cursor stacking, cursor undo. Deep
reference: `docs/development/manual-editor-capabilities-test-plan.md`
(section E). Setup: `/tmp/clay-manual/test.rs` with default bindings.

## Match selection (Ctrl+D family)

| # | Action | Expected |
|---|--------|----------|
| X1 | Caret inside `camelCaseCounter` (collapsed), `Ctrl+D` | First press selects the word at caret |
| X2 | `Ctrl+D` ×3 more | One selection per next occurrence, wrap-around at EOF, primary advances |
| X3 | Type `_x` | Inserted at ALL carets simultaneously |
| X4 | `Backspace` ×2 | Deleted at all carets |
| X5 | `Ctrl+Shift+L` with a word selected | Selects ALL occurrences at once |
| X6 | Select all occurrences, then one more `Ctrl+D` | Stops cleanly when everything is selected (no runaway) |

## Cursor stacking and columns

| # | Action | Expected |
|---|--------|----------|
| X7 | Caret mid-line, `Ctrl+Alt+Down` ×2, `Ctrl+Alt+Up` | Carets stack on lines below/above at same column |
| X8 | `Ctrl+Alt+Down` when a caret already occupies that line/column | Refused — no stacked duplicate |
| X9 | With stacked carets, `Shift+Alt+Right` ×3, then type | Rectangular column selection grows; typing replaces the block on every line |
| X10 | `Shift+Alt+Left` | Column shrinks from the other side |
| X11 | `Shift+Alt+Up` / `Shift+Alt+Down` on a single caret | Grows the column box one line |

## Set management and history

| # | Action | Expected |
|---|--------|----------|
| X12 | `Ctrl+U` after several cursor moves | Caret/selection set steps back through cursor-move history |
| X13 | `Escape` with multi-selection active | Collapses to single caret at primary |
| X14 | Multi-caret edit then `Ctrl+Z` | Text edits undo (one history step per caret edit — documented compromise) |
| X15 | Completion menu open + multi-selection, `Escape` | Menu closes FIRST; second `Escape` collapses the selection set (priority chain) |

## Negative checks

- Multi-caret typing never corrupts offsets (each caret's insert lands at its
  own position even when adjacent to another caret — test by stacking carets
  on adjacent characters).
- Secondary carets render solid; only primary blinks (visual check, module 07).
- Loading a different document resets the selection set (no caret leakage
  across documents).

## Known ceilings

- Multi-caret undo replays one history entry per caret edit (batched undo
  not implemented).
- `Ctrl+Shift+L` uses document-text matching, not search-engine matching.
