# React Shell, Component Registry, and Theme Runtime

## Source

- `frontend/src/app/{App,router,use-clay-session}.tsx`
- `frontend/src/app/layout/{app-shell,tab-bar,working-area}.tsx`
- `frontend/src/components/*`
- `frontend/src/theme/{adapter,types}.ts`
- `frontend/src/state/{theme-store,stores,connection-store}.ts`
- `frontend/src/styles/{global,tokens}.css`
- `frontend/src/routes/{workspace,fixture}.tsx`
- `src/shell/theme.rs` (`resolve_theme_token_snapshot`, `CORE_TOKEN_NAMES`)
- `src-tauri/src/bridge/dto.rs` (`ThemeSnapshotDto`, `TypographySnapshotDto`)
- `frontend/src/test/{theme-adapter,components,shell,performance}.test.ts*`

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
- `border-radius` is `0` on chrome (mechanical geometry). Badge/kbd may use
  `radius.xs` from the catalog.
- Modal scrim alpha-multiplies `surface.scrim` × `opacity.scrim` on the fill,
  not the overlay element, so the dialog stays opaque.
- Fixture routes exist only when `import.meta.env.DEV`.
- Production gzip budget: 160 kB (`frontend/scripts/bundle-budget.mjs`).

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
- [Phase 20.1 UI Design Language](phase20.1-ui-design-language-primitive-review.md)
- `docs/development/react-ui-catalog-mapping.md`
- `.agents/skills/clay-ui/references/{components,tokens}.md`
