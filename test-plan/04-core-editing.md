# 04 — Core Editing

Typing, history, clipboard, newline/indent rules, IME. This is the baseline
module: if anything here breaks, stop and report before running other modules.

## Setup

```bash
mkdir -p /tmp/clay-manual && cd /tmp/clay-manual
printf 'line one\nline two\n' > plain.txt
```

```rust
// /tmp/clay-manual/indent.rs
fn main() {
    let x = 1;
    if x > 0 {
        // caret-here line for indent checks
    }
}
```

Open the workspace, open `plain.txt` then `indent.rs`.

## Typing and history

| # | Action | Expected |
|---|--------|----------|
| E1 | Type characters, spaces, emoji (paste `émoji 🎉`) | Correct insertion at caret; grapheme clusters intact |
| E2 | `Backspace` / `Delete` | Character-granular removal at caret |
| E3 | `Enter` on an indented line in `indent.rs` | New line inherits leading whitespace (indent rule) |
| E4 | `Enter` inside `// comment` line | Comment continuation (`//`) inserted if mode declares it; plain.txt does not |
| E5 | Type `}` after an indented block line | Electric outdent snaps the brace to the block's indent (code modes) |
| E6 | Type `(` then `)` | Pair handling per mode manifest (auto-close / skip-over as configured) |
| E7 | `Tab` | Spaces per mode `tabSpaces` (rust package: 4; markdown: 2) |
| E8 | `Ctrl+Z` / `Ctrl+Shift+Z` (or `Ctrl+Y`) | Undo/redo restores text AND caret position; redo chain survives new edits only as documented |
| E9 | Undo to document start, retype | History branches correctly (no ghost text) |

## Clipboard

| # | Action | Expected |
|---|--------|----------|
| E10 | Select word, `clientCopySelection` binding (bind if not bound), paste elsewhere | Clipboard round-trip exact |
| E11 | Cut selection | Selection removed to clipboard |
| E12 | Paste multi-line text | Line endings normalized consistently |

## IME (only if an IME is installed; skip otherwise)

| # | Action | Expected |
|---|--------|----------|
| E13 | Begin IME composition (e.g. Japanese/German dead keys) | Preedit text overlays with underline; caret renders in active shape |
| E14 | Commit | Composed text replaces preedit exactly once; no duplication |
| E15 | Cancel composition (`Escape`) | Preedit removed, buffer untouched |

## Completion projection (Plan 087)

| # | Action | Expected |
|---|--------|----------|
| E16 | In a markdown/rust document with the fixture binding, press `Ctrl+Space` | A modeless completion popup opens next to the caret: width ≤ 480 logical px, at most 8 visible rows, item rows labeled from the provider; `Recovery: Completion` appears in the entry/status; the editor keeps focus (no Dialog role, no modal trap) |
| E17 | ArrowDown/ArrowUp in the popup | Selected row moves; the virtual MenuItem for the selected row carries the `selected` state; scroll keeps the selected row visible for long lists |
| E18 | `Escape` | Popup closes; editor/status return to normal; no text is inserted or deleted |
| E19 | Type text with no completions (e.g. `zzzz`), then `Ctrl+Space` | Provider returns `Empty`: popup is dismissed, NO blocking `No completions` panel appears, no status diagnostic; typing continues normally |
| E20 | Negative: type text, open completion, then trigger an edit/version change before accepting | Stale completion cannot apply: results for a stale document/version/behavior are consumed and dismissed; no text mutation from a stale accept (automated: `completion_result_rejects_foreign_document_and_behavior_provenance`) |
| E21 | IME (only if an IME is installed): compose while the completion popup is open | IME preedit and the completion popup coexist; committing the preedit dismisses/refreshes completion as documented — completion never blocks IME commit |

## Negative checks

- Editing a read-only observer session must not mutate the document.
- Rapid typing during `Pending edits > 0` stays responsive (local-optimistic).

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — E16:** `Ctrl+Space` (fixture `completion.trigger` binding, `UiReactivePriority` from the installed markdown manifest) opened a modeless `Completion` popup in the live X11 instance: Menu `480x340` logical px at the caret area, 16 `@clay/markdown` items, ≤ 8 visible ListItem rows, virtual MenuItems `Completion # … Completion \`` with the first row `selected`, entry/status `— Recovery: Completion`, editor Entry still `focused` (no modal trap).
- **PASS — E18:** `Escape` dismissed the popup; Menu node gone, status back to `Clay — Connected — Editable — review.md — doc 3 — v1`, no text inserted/deleted.
- **PASS — E19:** typing `zzzz` then `Ctrl+Space` produced `CompletionResult status: Empty`; no popup, no blocking `No completions` panel, no status diagnostic, typing continued (doc v5).
- **Coverage note:** E17 (selection/scroll), E20 (stale accept), E21 (IME coexist) were not re-run live this session; they are covered by automated consumer/unit tests (`menu_selection_keeps_selected_row_in_scroll_viewport`, `completion_result_rejects_foreign_document_and_behavior_provenance`) and E21 is IME-installation dependent. Live completion capture artifacts from plan 087 task 7 (`code-reviews/screenshots/2026-08-14-plan087-ui-foundation/completion/`) remain valid for the same build.

## Known ceilings

- IME preedit caret blink/shape parity is visual-only; blink phase timing is
  discrete (no alpha ramp) — see module 07 ceilings.
