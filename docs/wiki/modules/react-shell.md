# React Shell, Component Registry, and Theme Runtime

## Source

- `frontend/src/app/{App,router,use-clay-session}.tsx`
- `frontend/src/app/layout/{app-shell,tab-bar,working-area}.tsx`
- `frontend/src/components/*`
- `frontend/src/shell/{PaneTree,WorkspacePanes,AgentLane,workspace-controller,layout-state,use-shell-chords}.ts`
- `frontend/src/command-centre/{CommandPalette.tsx,CommandPalette.test.tsx}`
- `frontend/src/coding-agent/{AgentView,CodingAgentPanel,Composer}.tsx`
- `frontend/src/components/text-field.tsx`
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
  separators. The title bar hosts the brand, the tab strip (tabs plus their
  new-tab button, hugging the strip) and, right-aligned at the bar's edge, the
  Control Center icon trigger and the tab's `Workspace | Agent` switcher;
  `ClayTabStrip` gained an `inline`
  variant (no bottom border, centred items) so the strip does not double the
  title bar's own hairline. The status bar renders mono hints through `ClayKbd`
  and consumes the `statusItem` recipe.
- **Workspace** (`frontend/src/routes/workspace.module.css`): a three-column
  grid — sidebar, editor, rail — where the sidebar is a **flush zone** (canvas
  fill, one leading hairline, no radius and no veil; the SDUI file browser no
  longer wraps itself in a `Panel`), the editor column is **full-bleed and
  left-aligned** (no centred measure, no line-number gutter, zero `.cm-content`
  padding), and the rail is
  `340px` (`312px` at `≤1240px`, a fixed drawer at `≤1000px`).
  The grid now has three tracks and two rows: files rail · view pane · inspector
  rail, with `WorkspacePanes` and its host at `display: contents`. The pane
  (views) occupies `grid-column: 2`, row 1; the lane occupies `grid-column: 2`,
  row 2 — the view pane's own strip, so it never crosses into either rail (plan
  125, superseding plan 124's `grid-column: 1 / -1`); both rails span `grid-row:
  1 / -1`, so they run the working area's full height instead of ending at the
  lane's hairline. The workspace sidebar is a **rail of this grid** (plan 126,
  closing plan 125's D7): the SDUI tree carries its region and names the token
  that sizes it (`dimension.sidebar.default`, 244/224px), but the *shell* renders
  that region — `hostRailRegionId()` + `SduiRegion` in `sdui/renderer.tsx`, the
  region omitted from the pane's tree — in `.side` (`frontend/src/shell/workspace-panes.module.css`:
  rail fill, one inner hairline, no shadow), sized by its own token and, below
  1000px, a fixed drawer from the left edge like the inspector's. The pane's own
  `PackageWorkspace` keeps its `slot: left`/`right` panels as pane columns: a
  package panel is pane content, the workspace sidebar is a rail.
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
  material attribute). Plan 124 splits the menu surface: command/path sessions use
  `CommandPalette` as a child of the composer's `ClayTextField` menu slot, and
  plan 125 retired the second renderer, so picker stages are the same sheet
  keyed by the server's `mode`.
  The sheet is full field width, rises 6px above the field, and the `.veil` is
  a grid item over the whole working area (`grid-area: 1 / 1 / -1 / -1` in plan
  125, so the rail's full height is veiled too) at z-index 40 while the lane
  stays at z-index 41.
- **Consumption rule.** Component CSS must not carry geometry or colour
  literals: radius, border width, shadow, colour and duration come from
  `var(--clay-ds-*)` recipes or `var(--clay-*)` roles, with translucency
  composed through `color-mix(… calc(var(--clay-ds-…-background-opacity, 1) * 100%), transparent)`.
  `frontend/src/test/surface-adoption.test.tsx` asserts that for the editor,
  SDUI and package-workspace surfaces (no literals, recipes wired, fixed-slot
  panels flattened), and `frontend/src/test/design-system-consumption.test.ts`
  keeps every consumed variable backed by a recipe or a host fallback.

## Plan 124/125: persistent agent lane and composer palette

`WorkspacePanes` is the tab-owned composition root for both the workspace and
agent views. It creates/adopts one `AgentSessionModule` per `TabRuntime`, runs
`listSessions`, `requestBinding`, and `setUiVersion` bootstrap work there, and
makes that store the shared owner for `AgentLane` and `AgentView`. The lane is mounted for
every tab, remains mounted across view switches, and uses the per-tab
`agentLane` visibility store; `hidden` removes it from paint and accessibility
while preserving its draft and picker state. `Ctrl+X Ctrl+P` routes through
`shell.toggleAgentLane`, a client-local command that persists `laneVisible` in
`layout.json`.

The lane contains the optional approval strip, the composer, and the session
foot. Agent controls (agent type, model, effort, token meter) are the
composer field's toolbar. The composer is keyboard-submit only: Enter submits,
Stop appears only while streaming, and a missing provider leaves the field
typable. `@` mentions remain a narrow field menu; their rows include skill
descriptions or file directories.

The `/` palette is different from `@`: its query is the lane's composer field,
not a second input. `CommandPalette` is rendered through the field's `menu`
slot, so it is positioned 6px above and exactly as wide as the field. The
server-owned menu snapshot supplies rows, scope (`All`, `Session`, `Shell`,
`Files`), binding chips, and (protocol v32) the session `mode`, which selects the
catalogue/path/picker/secret/url/oauth presentation. `WorkspacePanes` renders the
shared modal scrim as a row-1 grid item over panes and the inspector rail; it
excludes the lane, whose higher stacking level keeps the query and sheet
interactive. When the lane hides, the palette and veil are both removed rather
than leaving an orphaned scrim.

All server-owned sessions — catalogue, path, and every picker stage — render
through this one `CommandPalette`; the centered `CommandCentre` is deleted. The
palette's root carries the plan-125 halo (two zero-offset `text.primary` layers)
instead of a drop shadow, and its stage transitions never re-anchor or add a
second veil pass. The palette, the Path Browser, and the pickers all use
`TransientMenuOrigin::CommandPalette`, run no package JavaScript on query or paint
paths, and carry no new package layout or authority surface.

Plan 125 also confined the lane: the working-area grid puts the lane in the pane's
column (`grid-column: 2`, `grid-row: 2`) with both rails spanning `grid-row: 1 / -1`,
so the palette sheet is exactly the lane's inner width and the veil covers the
rails' full height. Plan 126 closed the deviation that remained — the lane still
reached the workspace sidebar's column, because the sidebar rendered inside the
pane — by making the sidebar the shell's own left rail
(`test-plan/13-window-splits.md` S47 now passes; evidence in
`code-reviews/screenshots/2026-09-18-plan126-rails/review-log.md`).

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
- Production gzip budgets: 180 kB shell / 404 kB total
  (`frontend/scripts/bundle-budget.mjs`); plan 118 measured 169.8 / 390.9 kB. The
  total ceiling moved 400 → 404 kB on 2026-09-15 with the plan-119 evidence
  (`decision-logs/2026-09-15-1153-frontend-total-bundle-ceiling-404-kb.md`).

## Tests

- `frontend/src/test/theme-adapter.test.ts`: naming, density, motion units,
  z-index mapping, typography sizes.
- `frontend/src/test/components.test.tsx`: keyboard/focus for button, field,
  list, collapse, modal.
- `frontend/src/test/shell.test.tsx`: landmarks, fixture states, separator.
- `frontend/src/test/workspace-composition.test.tsx`: shell grid rows, the
  lane's pane-column placement with the rail at full height, veil
  stacking/coverage, and the single brand-dot run signal.
- `frontend/src/shell/{AgentLane,WorkspacePanes,shell-chords}.test.tsx`:
  lane composition, palette lifecycle, client command routing, and chord
  catalogue behavior.
- `frontend/src/coding-agent/Composer.test.tsx` and
  `frontend/src/command-centre/CommandPalette.test.tsx`: keyboard-only
  submission/Stop, agent-control states, slash scopes/binding chips, mentions,
  and palette activation.
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
