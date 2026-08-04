# 05 — Movement and Selection

Word/paragraph/line movement, sticky column, selection commands, prose-vs-code
word policies. Deep reference:
`docs/development/manual-editor-capabilities-test-plan.md` (sections A/B).

## Setup

Use `/tmp/clay-manual/test.rs` and `/tmp/clay-manual/test.md` from the setup
in `docs/development/manual-editor-capabilities-test-plan.md` (section 0.1),
with packages `@clay/rust` + `@clay/markdown` loaded.

## Movement (test.rs — code movement policy)

| # | Action | Expected |
|---|--------|----------|
| M1 | Caret at line start of `compute_total_value`, `Ctrl+Right` repeatedly | ONE stop per identifier — underscores join into one word in code mode |
| M2 | `Ctrl+Left` from identifier end | Mirrors M1 backwards |
| M3 | Mid-file `Ctrl+Down` / `Ctrl+Up` | Jumps across blank line to paragraph boundaries |
| M4 | `Home` / `End` on indented line | First non-whitespace / line end |
| M5 | `Ctrl+Home` / `Ctrl+End` | Document start / end |
| M6 | Column 20 → `ArrowUp` onto short line → `ArrowUp` onto long line → `ArrowDown` back | Column restored (sticky column) |
| M7 | `Shift+Arrow*` variants | Selection extends in movement direction |

## Selection commands

| # | Action | Expected |
|---|--------|----------|
| M8 | `Ctrl+L` | Whole current line selected |
| M9 | `Ctrl+D` on collapsed caret | Word at caret selected (first press of match-selection) |

## Prose vs code contrast (test.md — prose policy via @clay/markdown)

| # | Action | Expected |
|---|--------|----------|
| M10 | `Ctrl+Right` across `snake_case_words` in `.md` | Stops at EVERY underscore segment (`snake`, `case`, `words`) |
| M11 | Same text in `.rs` | ONE stop — same text, different policy per mode (the proof) |
| M12 | `Ctrl+Right` across `camelCaseWords` in `.md` | ONE stop — whole identifier, no sub-word stops |
| M13 | `Ctrl+Down` in `.md` | Blank-line paragraph jumping works in prose too |

## Negative checks

- Movement never edits the buffer (pure caret/selection state).
- Movement at document edges is a no-op, not an error/crash.

## Known ceilings

- camelCase sub-word movement exists in the core but has no command-ID/key
  surface yet — identifiers are single words in all modes.
- `line_movement: screenLine` falls back to character movement (no soft wrap).
