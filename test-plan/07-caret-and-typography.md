# 07 — Caret Styling and Typography (Ligatures)

Caret shape/blink configuration and per-role ligature policies. Deep
reference: `docs/development/manual-editor-capabilities-test-plan.md`
(sections C/D). Requires: Fira Code font installed, `@clay/rust` loaded.

## Setup

`/tmp/clay-manual/test.rs` first line:

```rust
// Ligature sample: =>  !=  ==  ->  ::  ||  >=
```

init.js typography pin (all three profiles required by setTypography):

```js
setTypography({
  monospace: { families: ["FiraCode Nerd Font Mono", "monospace"], size: 16 },
  proportional: { families: ["sans-serif"], size: 17 },
  ui: { families: ["system-ui"], size: 13 },
});
```

Apply caret changes via `clientSetCursorStyle` in init.js, then reload
(module 02, C9) or restart.

## Caret shapes

| # | Config | Expected |
|---|--------|----------|
| T1 | `{ shape: "block", blink: "blink" }` | Filled block over next character; blinks when idle |
| T2 | Type, then wait | Blink stops while typing, resumes when idle (`stopBlinkOnTyping` default) |
| T3 | `{ shape: "underline", blink: "phase", stopBlinkOnTyping: false }` | Underline under the cell; keeps blinking through typing |
| T4 | `{ shape: "bar", blink: "solid", widthPx: 2.5 }` | Thick non-blinking bar |
| T5 | `{ shape: "block", hollow: true }` | Outlined block |
| T6 | Delete all caret config | Theme default (thin bar, standard blink) returns |
| T7 | Multi-caret session (module 06) | Primary carets blink per config; secondaries render solid |
| T8 | IME preedit (if IME available) | Preedit caret matches active shape |

Negative: `clientSetCursorStyle({ shape: "triangle" })` → `clay.editor.invalid_set_cursor_style`
diagnostic, editor unaffected (deny-by-default enum).

## Ligatures

| # | Config (monospace.ligatures) | Expected on the sample line |
|---|------------------------------|------------------------------|
| T9 | omitted or `{ enableStandard: true, enableContextual: true }` | Fira Code ligatures render (joined `=>` `!=` `==`) — Fira Code ligatures are `calt` |
| T10 | `{ enableStandard: true, enableContextual: false }` | Ligatures OFF — individual glyphs |
| T11 | `{ disableFeatures: ["calt"] }` | OFF — escape hatch overrides semantic toggle (last-wins merge) |
| T12 | `{ rawFeatures: '"calt" 0' }` | OFF via raw CSS-format features |
| T13 | `{ discretionaryFeatures: ["zero"] }` + a `0O` sample | Slashed zero if the font has `zero` |

Negative: oversized `rawFeatures` (>256 bytes) → init.js diagnostic, previous
typography kept (all-or-nothing replacement).

## Negative checks

- Caret color always theme-owned — no config path changes it except theme.
- Ligature policy is per font ROLE: markdown and code share the monospace
  policy; there is no per-mode ligature override (by design).

## Known ceilings

- `phase`/`smooth` blink use discrete on/off timing (no alpha ramp yet).
- Block width at end-of-line falls back to measured advance heuristics.
