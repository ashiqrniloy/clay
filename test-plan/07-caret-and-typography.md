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
  hierarchy: {
    display: 1.5, title: 14 / 12, section: 13 / 12, body: 1,
    status: 1, detail: 10 / 12, caption: 0.75,
  },
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

Negative: `clientSetCursorStyle({ shape: "triangle" })` → `editor.invalid_set_cursor_style`
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

## Plan 088 typography/layout steps

| # | Config/action | Expected |
|---|---------------|----------|
| T14 | Apply valid UI sizes at the supported low/high edges and a complete hierarchy | Editor roles remain readable; UI row, status, tab, and hit-test geometry stay bounded; hierarchy installs atomically |
| T15 | Apply the canonical large-typography profile (`monospace: 20`, `proportional: 21`, `ui: 24`) | Welcome, status, tab affordances, package controls, and pane chrome reflow without clipping or overlap; accessible names remain complete |
| T16 | Compare dark/light themes with the same typography profile | Typography remains user-owned while contrast/state semantics and focus indicators stay visible in both themes |
| T17 | Remove one hierarchy field or use a non-finite/out-of-range scale, then reload | Entire typography update is rejected; prior valid profiles/hierarchy remain active; no partial scale reaches layout |

## Plan 088 task 12 Linux execution record (2026-08-15)

| Checks | Result | Evidence |
|---|---|---|
| T14/T17 | PASS automated / NOT RUN manually | `typography_protocol` and `invalid_init_typography_reports_actionable_validation_error` cover bounded complete/partial hierarchy validation and atomic rejection; targeted reload input is blocked by the host |
| T15 | PASS | `code-reviews/screenshots/2026-08-14-plan088-modernization/large-typography/` shows the large UI fixture in bounds; accessibility tree remains equivalent to default with named actions/status |
| T16 | PASS | `code-reviews/screenshots/2026-08-14-plan088-modernization/default/` and `light-default/` Task 8 artifacts plus bundled-theme contrast tests cover both palettes; no raw color/fixed-font path was introduced |

## Plan 089 validation steps

| # | Config/action | Expected |
|---|---------------|----------|
| T18 | Run the headless rescale test (`cargo test --lib masonry_shell::tests::rescale_event_recomputes_logical_bounds_from_physical_size`) | Logical bounds remain 900×600 at 2× physical scale; tab bar and pane hosts stay inside bounds |
| T19 | Run `CLAY_LIVE_WINDOW_SMOKE=1 cargo test --test security live_atspi_smoke::live_multi_window_scale_smoke -- --ignored --exact --test-threads=1` on a Wayland host | Two real Clay clients launch with large-typography init.js; AT-SPI exposes two PID-separated frames with positive bounds and scale factors between 0.5 and 4.0 |

## Known ceilings

- `phase`/`smooth` blink use discrete on/off timing (no alpha ramp yet).
- Block width at end-of-line falls back to measured advance heuristics.

## Plan 089 task 9 Linux execution record (2026-08-17)

| Checks | Result | Evidence |
|---|---|---|
| T15 | PASS live | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/large-typography/` shows the large UI fixture (size 24/20/21) in bounds with named welcome actions and Connected status |
| T18 | PASS headless | `rescale_event_recomputes_logical_bounds_from_physical_size` passes; logical size remains 900×600 at 2× physical scale |
| T19 | PASS live | `CLAY_LIVE_WINDOW_SMOKE=1` multi-window smoke test launched two real Clay clients with large typography; AT-SPI exposed two PID-separated frames with positive bounds and scale factors within 0.5–4.0 |

## Phase 26 document typography, wrap policy, and editor chrome steps

Deep references: `docs/reference/primitives/typography.md` (document size
ladder), `docs/reference/primitives/rendering-strategy.md` (Phase 26 axes),
`docs/reference/clay-js-api/editor/client-set-editor-layout.md`,
`docs/reference/packages/creating-packages.md` (editorRules.chrome,
textStyles background/scale). Setup: open the markdown fixture
(`tests/fixtures/syntax/markdown.md` — six heading levels, quote, fenced
code, inline code) and a code fixture (`tests/fixtures/syntax/rust.rs`).

| # | Config/action | Expected |
|---|---------------|----------|
| T20 | Open the markdown fixture with the default theme | Headings descend H1→H6 (scale ladder 1.50, 1.33, 1.17, 1.08, 1.00, 0.92); inline code renders smaller (0.90); body text stays at the profile size; line height grows with the largest heading on the line |
| T21 | Open the markdown fixture (proportional role) | Prose wraps at the column cap (default 72 average character widths) — no horizontal scrollbar; a long paragraph breaks at word boundaries |
| T22 | Open the rust fixture (monospace role) | NO wrapping — a 180+ character line extends past the pane and horizontal scrolling (`Shift+wheel` or the horizontal scrollbar) reveals the tail; vertical scroll unaffected |
| T23 | Add to init.js: `clientSetEditorLayout({ wrapPolicy: "viewport" })`, reload | Both code and prose wrap at the pane content width; horizontal scrolling is disabled; the override beats the per-mode manifest default |
| T24 | Add to init.js: `clientSetEditorLayout({ wrapPolicy: "column", columnCap: 100 })`, reload | Prose wraps at 100 columns; `columnCap` outside 16–240 is clamped (e.g. 9999 → 240), never rejected; unknown `wrapPolicy` values reject with a diagnostic and the previous layout stays |
| T25 | Code fixture with default chrome (code mode) | Gutter line numbers right-aligned in the left inset; the current line's number emphasized; active-line wash behind the caret line; indent guides at each indent level; the matching bracket pair highlighted when the caret is on a bracket |
| T26 | Markdown fixture (prose mode) | NO gutter, active-line wash, indent guides, or bracket-match highlight — chrome defaults off for proportional documents; explicit `editorRules.chrome` in a package manifest overrides |
| T27 | Theme with `textStyles` `background`/`scale` entries (e.g. gruvbox-dark) | Quote and code-block backgrounds paint behind glyphs; heading scale entries multiply the profile size; a `scale` outside `(0, 4.0]` or non-finite is rejected at theme load with the previous theme kept |

Negative: `clientSetEditorLayout({ wrapPolicy: "galley" })` → deny-by-default
diagnostic, layout unchanged; `clientSetEditorLayout({})` → rejected
(`wrapPolicy` required). Chrome toggles are manifest-only — there is no
runtime chrome override API (by design; packages cannot forge chrome).

## Phase 26 Linux execution record (2026-08-19)

| Checks | Result | Evidence |
|---|---|---|
| T20/T21 | PASS live | `code-reviews/screenshots/2026-08-18-phase26-review/markdown-*/` (4 themes): H1→H6 descend, quote + fence backgrounds, CodeSpan smaller, prose wraps at the column cap with no horizontal scrollbar |
| T22 | PASS live | `code-reviews/screenshots/2026-08-18-phase26-review/rust-longline-default/`: 180-char string clips at the pane edge, no wrap (`WrapPolicy::None`) |
| T23/T24 | PASS automated / NOT RUN live | `set_editor_layout_publishes_runtime_wrap_override`, `set_editor_layout_rejects_unknown_and_clamps_column`, `user_wrap_override_beats_manifest`, `column_wrap_is_narrower_than_viewport` cover override precedence, clamping, and rejection; live reload input is host-blocked (see V9 in the review log) |
| T25 | PASS live (dark/gruvbox) | `rust-default/`, `rust-gruvbox-dark/`, `rust-gruvbox-light/`: right-aligned gutter digits, active-line wash, indent guides, bracket-match highlight |
| T25 (light) | DEFECT — V4 | `*-modus-operandi/` code captures: the current-line gutter digit is invisible — `gutterFgActive` (default 0xf4f1ff) fails contrast against the light `lineHighlight`/panel background. Tracked in `code-reviews/screenshots/2026-08-18-phase26-review/review-log.md` V4; fix = light themes define `gutterFgActive` or the default becomes theme-aware |
| T26 | PASS live | `markdown-*/` captures show no gutter/active-line/indent-guide chrome on proportional documents |
| T27 | PASS automated | `style_for_resolves_theme_owned_backgrounds`, `text_style_overrides_can_set_background_axis`, `size_scale_ladder_descends_headings_and_clamps_theme_overrides`, and `tests/theme_packages.rs` dormant-token distinctness cover background/scale resolution and validation; live theme reload is host-blocked |
