# Frontend Theme Runtime

## What it is

`frontend/src/theme/` projects one Rust-resolved theme/typography snapshot
into CSS custom properties on the app root. Rust resolves tokens (core +
package remaps + density + contrast enforcement); the frontend only writes
finished values. There is no theme logic, palette math, or fallback
resolution in JavaScript.

| File | Responsibility |
| --- | --- |
| `types.ts` | `ThemeSnapshot` (`tokens`, `densityScale`, `editorStyles`), `TypographySnapshot` (font roles, hierarchy, ligatures), `ThemeTokenValue` tagged union. |
| `adapter.ts` | `themeCssVariables`, `typographyCssVariables`, `installVariables`, `tokenToCssName`, `variantSize`. |

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

## Invariants and tradeoffs

- **CSS custom properties are the styling currency**; components and CSS
  modules read `var(--clay-*)` only — no hardcoded colors, sizes, or shadows.
- Variant sizes are computed once in the adapter; components consume finished
  sizes so typography changes cannot desync between components.
- Contrast floors are enforced server-side at resolution; the adapter cannot
  weaken them because it never edits values.

## Tests

- Adapter mapping/density/z-level/editor-style tests live with the Phase 4
  shell suites under `frontend/src/test/`.

## Related

- [React Shell, Component Registry, and Theme Runtime](react-shell.md).
- Theme tokens reference: `docs/reference/primitives/tokens.md`.
