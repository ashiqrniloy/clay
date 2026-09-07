# Final Visual & Accessibility Review — Plan 110 Task 11

Post-implementation screenshot matrix and accessibility pass, captured with the same
methodology as [`110-baseline`](../110-baseline/README.md): Vite dev server
(`http://localhost:5199/?fixture=`, `frontend/src/routes/fixture.tsx`, DEV-gated) +
headless Chromium via CDP (playwright-core).

## Environment & Method

- **Capture script**: [`capture-final.mjs`](./capture-final.mjs) — 61 captures recorded in
  [`manifest.json`](./manifest.json); accessibility trees in
  [`a11y-snapshots.json`](./a11y-snapshots.json).
- **Var maps**: `neobrutal.json` / `glass.json` regenerated from the current package
  manifests via `gen-ds-vars.mjs` (verified byte-identical to the baseline-era maps —
  recipe JSON unchanged by tasks 2–10; only host CSS consumption changed).
- **Viewports**: 1280×800 base matrix; 900×700 and 1920×1000 for `splits`.

## Matrix

1. **Base matrix (36)**: 6 fixtures (`controls`, `splits`, `chat`, `command-centre`,
   `settings`, `package-ui`) × 3 styles (`core`, `neobrutal`, `glass`) × 2 themes
   (`modus-vivendi`, `modus-operandi`).
2. **Interaction states (14)**: `controls` for `core × modus-operandi` and
   `neobrutal × modus-vivendi`: hover, keyboard focus-visible, open-dropdown,
   collapsed-collapse, modal-open, invalid text field.
   - Fixture change: added an `invalid` `ClayTextField` (`validationState="error"`) to the
     DEV-only controls fixture so the token-driven error state has visual evidence.
3. **Responsive (12)**: `splits` at 900×700 and 1920×1000, all 6 style×theme combos.
4. **A11y**: CDP accessibility tree of the `settings` fixture (39 nodes), settings tab
   order, first-Tab focus style, keyboard flow captures
   (`settings-focus-visible__neobrutal__modus-vivendi.png`).

## Baseline P1/P2 verification (diff vs `110-baseline`)

| # | Baseline finding | Status | Evidence |
|---|------------------|--------|----------|
| 1 | Shell chrome ignores DS (zero visual change across styles on `splits`/`settings`/`chat`/`package-ui`) | **Resolved at control level.** Neobrutal offset shadows + geometry now reach every leaf control inside chrome (Save/Reload/Close/Open, Open file/Open folder/Coding Agent). Chrome *containers* (header/tab-bar/status-bar backgrounds) stay theme-owned by design — DS packages style component recipes, themes own surfaces. | `splits-1920__core__modus-vivendi.png` vs `splits-1920__neobrutal__modus-vivendi.png`, `package-ui__neobrutal__modus-vivendi.png` |
| 2 | Neobrutal identity collapse on dark (shadows near-invisible) | **Resolved.** Offset shadows and solid borders clearly visible on `modus-vivendi` across controls, modal, and panes. | `controls__neobrutal__modus-vivendi.png`, `controls-modal-open__neobrutal__modus-vivendi.png` |
| 3 | Core baseline monotony (input/control fills indistinguishable, muted button unframed) | **Resolved (task 9).** Text inputs render on `surface.main` distinct from gray control/button fills; muted button has the `border.subtle` ghost frame; list rows are transparent with hairline separators. | `controls__core__modus-operandi.png`, `controls__core__modus-vivendi.png` |
| 4 | Light theme neobrutalism weak | **Resolved.** Borders/shadows read on `modus-operandi`; invalid field uses diagnostic error border + message. | `controls__core__modus-operandi.png`, `controls-invalid-field__core__modus-operandi.png` |

## Task 9/10 acceptance evidence

- Muted ghost border, input/control fill differentiation, transparent list rows: see #3 above.
- Settings panel: **Theme**, **Design system** (new), **Appearance** dropdowns render; core
  light + neobrutal dark both clean (`settings__core__modus-operandi.png`,
  `settings__neobrutal__modus-vivendi.png`).
- Dropdown open state: popover with panel surface, selected row accent-highlighted,
  Escape dismisses (`controls-dropdown-open__*.png`; the `listboxVisible:false` value in
  the manifest is a double-listbox selector artifact — screenshots show the open popover).
- Modal: scrim, focus trap, Escape close, offset-shadow dialog
  (`controls-modal-open__*.png`).

## Accessibility findings (settings + controls)

- **Roles/names**: every dropdown trigger is a named `button` (Close, Theme, Design system,
  Appearance, Typography, Apply typography, Reset preferences); tablist/complementary
  landmarks intact; invalid input carries `aria-invalid` + visible error text.
- **Keyboard flow (tab order)**: Close → Theme collapse → Theme dropdown → Design system →
  Appearance → Typography collapse → Apply typography → Reset preferences — logical DOM
  order, no traps, dropdowns/modal close on Escape.
- **Focus visibility**: first Tab lands with `outline: solid 2px`
  (`settings-first-focus` in a11y-snapshots.json; screenshot shows visible focus ring).
- **Result: no accessibility blockers.**

## New issues found

- **P3** — Settings dropdown triggers render the raw label ("Theme", "System") as the
  trigger value when no server snapshot is loaded (fixture/disconnected state). Real
  sessions populate values from `uiChoices`. Cosmetic fallback improvement — recorded in
  Further Actions, not blocking.
- No P1/P2 issues found in any captured state.
