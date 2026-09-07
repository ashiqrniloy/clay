# Frontend Theme Runtime

## What it is

`frontend/src/theme/` projects one Rust-resolved theme/typography snapshot
into CSS custom properties on the app root. Rust resolves tokens (core +
package remaps + density + contrast enforcement); the frontend only writes
finished values. There is no theme logic, palette math, or fallback
resolution in JavaScript.

| File | Responsibility |
| --- | --- |
| `types.ts` | `ThemeSnapshot` (`tokens`, `densityScale`, `editorStyles`), `TypographySnapshot` (font roles, hierarchy, ligatures), `ThemeTokenValue` tagged union, `DesignSystemSnapshot` (specifier, generation, recipes → variables). |
| `adapter.ts` | `themeCssVariables`, `typographyCssVariables`, `installVariables`, `tokenToCssName`, `variantSize`. |
| `design-system-adapter.ts` | `designSystemCssVariables` / `installDesignSystemVariables`: projects one resolved `ActiveDesignSystem` snapshot into `--clay-ds-*` custom properties once per generation. Validates bounds, rejects literal colors/package aliases, resolves `ThemeColorRef` roles to host-owned `--clay-*` variables — no raw CSS text, no runtime style injection. |

## How it works

1. The server resolves the active theme into a snapshot and ships it in the
   bootstrap DTO and `themeSnapshot` envelopes.
2. `themeCssVariables` maps every token to a custom property using the locked
   naming rule `token.name.sub` → `--clay-token-name-sub`.
   - Spacing scalars are pre-multiplied by `densityScale` (spacing only —
     density never rescales dimensions or radii).
   - `z.*` levels become numeric stacking integers (base 0, panel 10,
     overlay 20, modal 40, tooltip 50); other level domains keep catalog
     names for diagnostics.
   - Motion durations emit `ms`; spacing/radius/dimension emit logical `px`.
   - `editorStyles` entries become `--clay-editor-<token>-{color,background,
     scale,weight,style,decoration}` variables consumed by the CodeMirror
     theme (see [React CodeMirror Editor](react-codemirror-editor.md)).
3. `typographyCssVariables` emits font-role stacks (`--clay-font-ui`,
   `--clay-font-monospace`, `--clay-font-proportional`) and finished text
   variant sizes: role base × hierarchy scale, with the shared line-height
   multiplier. Ligature policy becomes the UI stack's OpenType feature
   setting.
4. `installVariables` writes the sorted list onto the app root once per
   install — never per frame, never per component.

## Design-system stores and coherent install (Plan 102)

The frontend keeps theme/typography and design-system state in **separate
stores with one coherent install order**:

- `themeStore` holds the theme/typography snapshot; `designSystemStore`
  (`frontend/src/state/design-system-store.ts`) holds the latest
  `DesignSystemSnapshot` plus the exact set of installed `--clay-ds-*` key
  names. Neither store derives from the other.
- `use-clay-session.ts` feeds both: `themeSnapshot` envelopes update theme
  state, bootstrap and runtime snapshots call
  `designSystemStore.setDesignSystem(snapshot.activeDesignSystem)`, and
  disconnects call `designSystemStore.resetToFallback()`.
- `setDesignSystem` is **idempotent**: an unchanged `(specifier, generation,
  schemaVersion)` triple performs zero DOM writes and notifies zero
  subscribers. A change removes exactly the previously installed keys, then
  writes the new set — variables never accumulate across switches.
- Design-system variables resolve color roles to host-owned theme variables
  (`surface.control` → `var(--clay-surface-control)`), so a theme swap alone
  recolors an installed design system with no design-system write.
- Install is once per activation/snapshot (measured: 300 variables ≈ 0.3 ms,
  300 initial root writes, one notification) and is excluded from text-event,
  edit-acknowledgement, and paint hot paths.

## Invariants and tradeoffs

- **CSS custom properties are the styling currency**; components and CSS
  modules read `var(--clay-*)` only — no hardcoded colors, sizes, or shadows.
- **Content themes are the sole color authority**: Content themes own color roles; UI design systems (see [UI Design System Runtime](ui-design-system-runtime.md)) own component geometry, materials, shadows, and motion, referencing active-theme color roles without declaring custom palettes.
- Variant sizes are computed once in the adapter; components consume finished
  sizes so typography changes cannot desync between components.
- Contrast floors are enforced server-side at resolution; the adapter cannot
  weaken them because it never edits values.

## Tests

- Adapter mapping/density/z-level/editor-style tests live with the Phase 4
  shell suites under `frontend/src/test/`.
- `frontend/src/test/design-system-adapter.test.ts` — naming rule, color-role
  resolution, bounds rejection, and install idempotence (identical generation →
  zero writes/notifications; 300-variable fixture ≈ 0.3 ms).

## Related

- [UI Design System Runtime](ui-design-system-runtime.md) — recipe schema, fallback engine, and Plan 102 activation lifecycle.
- [React Shell, Component Registry, and Theme Runtime](react-shell.md).
- Theme tokens reference: `docs/reference/primitives/tokens.md`.

Plan 112: icon rendering consumes the same installed core tokens directly
(`--clay-text-icon`, `--clay-dimension-icon-size`, `--clay-opacity-disabled`)
without waiting for tokens.css fallbacks; the icon size token acts as a floor
with `max(…, 1em)` typography scaling. See [Icon Pack Runtime](icon-pack-runtime.md).
