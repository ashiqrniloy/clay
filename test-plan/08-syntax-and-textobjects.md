# 08 — Syntax Highlighting, Text Objects, Smart Select

Grammar-backed highlighting, tree-sitter textobjects, smart select, engine
tier preference, advisory degradation. Deep reference:
`docs/development/manual-editor-capabilities-test-plan.md` (section F).

## Setup

- Load `@clay/rust` (+ optionally `@clay/typescript`, `@clay/javascript`,
  `@clay/markdown`) in init.js.
- Textobject/smart-select keys are bound via init.js (no defaults ship):

```js
bindKey("Alt+I", "clay.editor.clientSelectTextobject.function.inner.current", { scope: "editor" });
bindKey("Alt+O", "clay.editor.clientSelectTextobject.function.around.current", { scope: "editor" });
bindKey("Alt+A", "clay.editor.clientSelectTextobject.argument.inner.current", { scope: "editor" });
bindKey("Alt+C", "clay.editor.clientSelectTextobject.comment.around.current", { scope: "editor" });
bindKey("Alt+E", "clay.editor.clientSmartSelect.expand", { scope: "editor" });
bindKey("Alt+R", "clay.editor.clientSmartSelect.shrink", { scope: "editor" });
```

Open `/tmp/clay-manual/test.rs` (from module 05 setup).

## Highlighting

| # | Action | Expected |
|---|--------|----------|
| S1 | Open `test.rs` | Keywords/types/strings/comments colored per theme grammar styles |
| S2 | Edit inside a function | Highlighting updates incrementally, no full-file flicker |
| S3 | Scroll a long file fast | Windowed re-highlight keeps typing smooth |
| S4 | Open `plain.txt` (no grammar) | Plain text, no crash, core.text fallback mode |

## Text objects (Rust grammar)

| # | Action | Expected |
|---|--------|----------|
| S5 | Caret inside `compute_total_value` body, `Alt+I` | Selects function body (inner) |
| S6 | `Alt+O` | Selection grows to include the `fn` signature line (around) |
| S7 | Caret on `firstItem` in signature, `Alt+A` | Selects that parameter only |
| S8 | Caret in the leading comment, `Alt+C` | Selects the whole comment block |
| S9 | Bind a `.next` variant, press repeatedly | Walks to following functions |

## Smart select

| # | Action | Expected |
|---|--------|----------|
| S10 | Caret in a statement, `Alt+E` repeatedly | Grows: expression → statement → block → function → file |
| S11 | `Alt+R` after S10 | Shrinks back down the same chain |

## Advisory degradation (must never block editing)

| # | Action | Expected |
|---|--------|----------|
| S12 | Repeat S5 in `test.md` / `plain.txt` (no textobjects grammar) | No crash, no selection change — carets untouched |
| S13 | Smart select in a file with no grammar | Same: silent no-op |

## Engine tiers

| # | Action | Expected |
|---|--------|----------|
| S14 | `setSyntaxEnginePreference("rust", "native")` (default) | Grammar works |
| S15 | `setSyntaxEnginePreference("rust", "turbo")` (unknown tier) | Diagnostic/rejection; highlighting unaffected |

## Negative checks

- Parsing/decoration publication never happens in keystroke/paint hot paths
  (typing stays smooth while a large file re-parses in background).
- Decoration payloads stay bounded (large-file case → module 11).

## Known ceilings

- Textobjects.scm ships for rust/typescript/javascript only; other languages
  degrade silently.
- Third-party packages cannot contribute grammars/textobjects today
  (first-party-only contribution rule).
