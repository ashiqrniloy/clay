# Visual Baseline Review — Phase 20.9 Task 1

Pre-change baseline screenshot matrix captured using the Vite dev server (`http://localhost:5199/?fixture=`) and headless Chromium CDP automation.

## Environment & Methodology

- **Vite dev server**: `http://localhost:5199/` (`frontend/src/routes/fixture.tsx`, gated under `import.meta.env.DEV`)
- **Browser**: Headless Chromium (`/home/arn/.cache/puppeteer/chrome/linux-152.0.7977.42/chrome-linux64/chrome`)
- **Viewport**: 1280 × 800 (standard desktop)
- **Capture script**: [`capture-baseline.mjs`](file:///home/arn/Projects/clay/.impeccable/reviews/110-baseline/capture-baseline.mjs)
- **Var maps**:
  - [`neobrutal.json`](file:///home/arn/Projects/clay/.impeccable/reviews/110-baseline/neobrutal.json) (generated from `packages/design-neobrutal/package.json` via `gen-ds-vars.mjs`)
  - [`glass.json`](file:///home/arn/Projects/clay/.impeccable/reviews/110-baseline/glass.json) (generated from `packages/design-glass/package.json` via `gen-ds-vars.mjs`)
- **Manifest**: [`manifest.json`](file:///home/arn/Projects/clay/.impeccable/reviews/110-baseline/manifest.json) (36 total states)

## Matrix Dimensions

- **6 Fixtures**:
  1. `controls`: Component catalogue controls (buttons, badges, text field, dropdown, list, collapse, modal)
  2. `splits`: Workspace pane split tree layout
  3. `chat`: Empty-tab chat panel landing surface
  4. `command-centre`: Centered modal Command Centre palette
  5. `settings`: Settings presentation panel
  6. `package-ui`: SDUI package workspace panel integration
- **3 Styles**:
  1. `core`: Host baseline fallback tokens only (no `--clay-ds-*` overrides)
  2. `neobrutal`: Injected `packages/design-neobrutal` recipe custom properties
  3. `glass`: Injected `packages/design-glass` recipe custom properties
- **2 Themes**:
  1. `modus-vivendi`: High-contrast dark theme
  2. `modus-operandi`: High-contrast light theme

Total: 6 fixtures × 3 styles × 2 themes = 36 screenshots.

## Key Baseline Findings

1. **Chrome Surface Bypasses & Non-Coverage**:
   - `splits`, `settings`, `chat`, and `package-ui` exhibit almost zero visual change across `core`, `neobrutal`, and `glass` on their shell chrome (shell header, tab bars, status bar, split dividers, panel frames).
   - Only registered leaf controls (`button`, `badge`, `dropdown`, etc.) change their geometry and borders.
2. **Neobrutal Identity Collapse on Dark Themes**:
   - In `modus-vivendi`, neobrutal offset shadows use `--clay-border-strong` (`#3d385c` against `#100f17`), rendering them nearly invisible.
   - 1px structural borders read as faint hairline rules rather than bold mechanical boundaries.
   - List rows and container surfaces lack solid differentiated backgrounds.
3. **Core Baseline Monotony**:
   - In `core` fallback, controls and field inputs share identical or nearly indistinguishable flat surfaces (`#39354a` / `#292835`), leading to low visual hierarchy.
   - Muted buttons lack framing.
4. **Light Theme Behavior (`modus-operandi`)**:
   - While shadows have marginally more contrast in `modus-operandi`, the 1px borders and soft gray shadows fail to convey true neobrutalism.

This baseline serves as the pre-change reference against which Task 11 (`.impeccable/reviews/110-final/`) will be diffed and evaluated.
