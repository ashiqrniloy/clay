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

## Negative checks

- Editing a read-only observer session must not mutate the document.
- Rapid typing during `Pending edits > 0` stays responsive (local-optimistic).

## Known ceilings

- IME preedit caret blink/shape parity is visual-only; blink phase timing is
  discrete (no alpha ramp) — see module 07 ceilings.
