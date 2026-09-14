# React Shell, Component Registry, and Theme Runtime

## Source

- `frontend/src/app/{App,router,use-clay-session}.tsx`
- `frontend/src/app/layout/{app-shell,tab-bar,working-area}.tsx`
- `frontend/src/components/*`
- `frontend/src/shell/{PaneTree,WorkspacePanes,workspace-controller,layout-state,use-shell-chords}.ts`
- `frontend/src/routes/{workspace,WorkspaceRail,fixture}.tsx`
- `frontend/src/theme/{adapter,types}.ts`
- `frontend/src/state/{theme-store,stores,connection-store}.ts`
- `frontend/src/styles/{global,tokens}.css`
- `frontend/src/routes/{workspace,fixture}.tsx`
- `src/shell/theme.rs` (`resolve_theme_token_snapshot`, `CORE_TOKEN_NAMES`)
- `src-tauri/src/bridge/dto.rs` (`ThemeSnapshotDto`, `TypographySnapshotDto`)
- `frontend/src/test/{theme-adapter,components,shell,performance,surface-adoption,design-system-consumption}.test.ts*`

## Overview

Plan 097 Phase 4 React host: memory-routed application shell, cataloged
token-styled components, and a one-shot theme adapter that writes
Rust-resolved snapshots onto `--clay-*` CSS custom properties.

## Responsibilities

- Top-level routes (`/workspace`, DEV `/fixture/:id`) only. Tabs, panes,
  documents, menus, and overlays stay application state.
- Landmarks: one `header` / `main` / `footer` per window.
- Project catalog kinds (`button`, `list`, `dropdown`, `collapse`, `modal`,
  `textInput`, label/text, badge/kbd/divider) through React Aria + CSS Modules.
- Install theme/typography once per snapshot revision. Never re-resolve in
  paint or per keystroke.
- Reject raw package CSS, scripts, and unsafe URLs. Theme contrast stays a
  Rust authority check (`validate_active_theme_contrast`).

Non-responsibility: editor text (see [React CodeMirror Editor](react-codemirror-editor.md)), live split trees
and tabs (see [React Tabs, Splits, and Layout Persistence](react-tabs-and-splits.md)), package SDUI projection (Phase 7).

## How It Works

1. `useClaySession` bootstraps the Tauri bridge, installs
   `bootstrap.activeTheme` / `activeTypography` into `themeStore`, and
   forwards later `themeSnapshot` envelopes.
2. `createThemeStore` calls `themeCssVariables` / `typographyCssVariables`
   and writes the result onto `document.documentElement`.
3. Naming rule: `token.name.sub` → `--clay-token-name-sub`. Spacing scalars
   are pre-multiplied by `densityScale`. `motion.*` emits `ms`. `z.*` levels
   map to stacking integers (`base=0`, `panel=10`, `overlay=20`, `modal=40`,
   `tooltip=50`).
4. Components read only those variables. Fallback values in
   `frontend/src/styles/tokens.css` cover the frame before the snapshot
   lands.
5. `App` memoizes the memory router on session generation so connection
   updates do not remount the tree. Production routes subscribe to the
   session store; tests may inject a connection snapshot.

## Plan 118 shipped composition (Quiet Instrument)

The design-system migration re-laid the host chrome instead of re-skinning it
(`DESIGN.md` §12/§16, approved set in
`design-artifacts/approved/quiet-instrument-migration/`; recipes come from
`@clay/design-instrument`, see [UI Design System Runtime](ui-design-system-runtime.md)):

- **Shell** (`frontend/src/app/layout/shell.module.css`): a three-row grid —
  40px title bar, `1fr` working area, 28px status bar — with hairline zone
  separators. The title bar hosts the brand, the tab strip and the
  `Palette` / `Files` / `Outline` actions; `ClayTabStrip` gained an `inline`
  variant (no bottom border, centred items) so the strip does not double the
  title bar's own hairline. The status bar renders mono hints through `ClayKbd`
  and consumes the `statusItem` recipe.
- **Workspace** (`frontend/src/routes/workspace.module.css`): a three-column
  grid — sidebar, editor, rail — where the sidebar is a **flush zone** (canvas
  fill, one leading hairline, no radius and no veil; the SDUI file browser no
  longer wraps itself in a `Panel`), the editor column centres its text at a
  92ch measure (`max-width: calc(92ch + 4rem)` on `.canvas`), and the rail is
  `340px` (`312px` at `≤1240px`, a fixed drawer at `≤1000px`).
  `WorkspaceRail.tsx` derives the outline from `## HH:MM — title` headings plus
  a facts list (file, revision, state, entries, words) and navigates through
  `DocumentSession.revealLine`; its visibility lives in
  `frontend/src/shell/layout-state.ts` and toggles from the title bar or the
  client-local `Ctrl+I` chord (`use-shell-chords.ts`). The editor's relative-path
  field is an on-demand strip (docbar button, not `Ctrl+O` — that stays the
  native file dialog) and consumes `textInput.default.input` so the single-line
  boundary keeps the `r8` contract.
- **Overlays** (`frontend/src/components/modal.tsx` + `modal.module.css`):
  `ClayModal` takes `flush` (the palette sheet paints its own surface) and
  `footer` (head / body / foot with hairline separations), the scrim uses the
  theme's `surface.scrim` role, and the reduced-transparency fallback in
  `frontend/src/styles/global.css` paints veil-bearing components opaque
  (`[data-clay-component]` / `[data-clay-slot]` selectors, not the retired
  material attribute).
- **Consumption rule.** Component CSS must not carry geometry or colour
  literals: radius, border width, shadow, colour and duration come from
  `var(--clay-ds-*)` recipes or `var(--clay-*)` roles, with translucency
  composed through `color-mix(… calc(var(--clay-ds-…-background-opacity, 1) * 100%), transparent)`.
  `frontend/src/test/surface-adoption.test.tsx` asserts that for the editor,
  SDUI and package-workspace surfaces (no literals, recipes wired, fixed-slot
  panels flattened), and `frontend/src/test/design-system-consumption.test.ts`
  keeps every consumed variable backed by a recipe or a host fallback.

## Code Examples

```ts
themeStore.setTheme(bootstrap.activeTheme);
// writes --clay-surface-main, --clay-spacing-md, --clay-z-modal, …
```

```ts
const router = createMemoryRouter(routes, {
  initialEntries: ["/workspace"],
});
```

## Invariants and Constraints

- Token names and component kinds are additive-only; schema changes need a
  migration test.
- Geometry comes from the active design system: chrome and fixed slots stay
  radius-free because they are flush zones, interactive controls take the
  language's control radius (see
  [Design Artifact Gate](design-artifact-gate.md) for the frozen values, and
  `DESIGN.md` §11 for the recipes).
- Modal scrim alpha-multiplies `surface.scrim` × `opacity.scrim` on the fill,
  not the overlay element, so the dialog stays opaque.
- Fixture routes exist only when `import.meta.env.DEV`.
- Production gzip budgets: 180 kB shell / 400 kB total
  (`frontend/scripts/bundle-budget.mjs`); plan 118 measured 169.8 / 390.9 kB.

## Tests

- `frontend/src/test/theme-adapter.test.ts`: naming, density, motion units,
  z-index mapping, typography sizes.
- `frontend/src/test/components.test.tsx`: keyboard/focus for button, field,
  list, collapse, modal.
- `frontend/src/test/shell.test.tsx`: landmarks, fixture states, separator.
- `frontend/src/test/performance.test.tsx`: store notify count, reducer
  identity for non-lifecycle envelopes.
- `src/shell/theme.rs` + `tests/theme_packages.rs`: 91-token snapshot and
  bundled-theme contrast.
- Commands: `cd frontend && npm test && npm run build && npm run check:budget`

## Related

- [Desktop Typed Bridge](desktop-typed-bridge.md)
- [Phase 20.1 UI Design Language](../modules/ui-design-language-primitive-review.md)
- `docs/development/react-ui-catalog-mapping.md`
- `.agents/skills/clay-execution/references/{components,tokens}.md`
