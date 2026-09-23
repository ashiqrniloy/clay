---
id: theme.setTheme
kind: clay-js-api
js_module: "clay:theme"
js_export: setTheme
js_facade: runtime/js/theme.js::setTheme
backing_rust: src/server/ops/theme.rs::op_clay_theme_set_theme; src/editor/theme.rs::StyleRegistry::from_active_theme
deno_op: op_clay_theme_set_theme
deno_op_path: src/server/ops/theme.rs::op_clay_theme_set_theme
name: setTheme
user_facing_name: Set Theme
summary: Select one first-party inert theme for editor syntax colors, base UI chrome colors, and typed UI design-token overrides.
owner: server
phase: Phase 18.15
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: specifier
    type: string
    default: required
    description: Bundled first-party theme package specifier.
security: Accepts only bundled first-party @clay/* theme specifiers, reads static package.json textStyles and optional designTokens contributions, validates typed UI overrides against core token types and domain bounds, and sends inert RGBA/style-attribute data plus validated typed UI token overrides to the client; does not grant filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, raw CSS, or renderer callback authority.
agent_guidance: Use a bundled specifier from init.js — the four shipped themes are `@clay/theme-modus-operandi` and `@clay/theme-modus-vivendi` (the canonical light/dark defaults) plus `@clay/theme-gruvbox-material-dark` and `@clay/theme-gruvbox-material-light`. Every shipped theme declares the language's theme-side roles and must pass the composited contrast gate, so never hand-edit a palette to "fix" contrast, and never suggest raw CSS, theme code execution, raw color ops, or third-party theme loading through this API.
lookup_tags: [theme, syntax, colors, gruvbox, init]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setTheme

## Summary

Select one first-party inert theme for editor syntax colors, base UI chrome colors, and typed UI design-token overrides.

## Description

`setTheme` resolves a bundled first-party theme package, validates its static `textStyles` contribution and optional `designTokens` typed UI overrides, and stores an `ActiveTheme` snapshot that the server sends to the native client during bootstrap. The editor builds a `StyleRegistry` for syntax/base-UI colors and a `ResolvedUiTheme` for typed UI tokens (dimensions, elevations, motion durations, z-levels, density, color roles, spacings, radii, and opacity) before first paint. Resolution is additive: a theme that ships no `designTokens` still resolves through Clay core fallbacks with no manifest change, and the four shipped themes now declare the Quiet Instrument language's theme-side roles (`border.*`, `surface.hover`/`active`/`selected`/`scrim`, `accent.*`, `focus.ring`, `border.focus`, `text.muted`/`disabled`) so no role falls back to the core palette.

## When to use

Use from `~/.clay/init.js` to choose the editor theme at startup.

## JavaScript usage

```ts
import { setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
// or
setTheme({ specifier: "@clay/theme-gruvbox-material-light" });
```

`setTheme` returns `{ specifier, overrideCount, designTokenCount }`.

## Example

```ts
import { setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
```

## Options

Pass either a theme specifier string or `{ specifier }`. Only bundled first-party theme packages are accepted, for example `@clay/theme-gruvbox-material-dark`.
- `specifier` (`string`, default `required`): Installed or canonical theme package specifier to activate (for example `@clay/theme-mono`).

## Shipped choice set

Clay ships four content themes, all first-party `@clay/theme-*` packages with the same manifest shape:

| Specifier | Appearance | Role |
| --- | --- | --- |
| `@clay/theme-modus-operandi` | light | canonical default for `light` |
| `@clay/theme-modus-vivendi` | dark | canonical default for `dark` |
| `@clay/theme-gruvbox-material-light` | light | opt-in, explicit `setTheme` only |
| `@clay/theme-gruvbox-material-dark` | dark | opt-in, explicit `setTheme` only |

The canonical pair is selected by [`theme.setAppearance`](set-appearance.md) (`light` → Modus Operandi, `dark` → Modus Vivendi, `system` → the observed OS signal with a dark fallback); the Gruvbox pair is never auto-selected. All four declare the same theme-side roles the design system maps (`border.hairline`/`subtle`/`strong`, `surface.hover`/`active`/`selected`/`scrim`, `accent.primary`/`muted`, `focus.ring`, `border.focus`, `text.muted`/`disabled`), so no role silently falls back to the core palette. The Settings panel's Theme dropdown renders the server-enumerated `ui_choices.themes` list (every enabled bundled `@clay/theme-*` package) rather than a hardcoded set, so the selectable list and this API accept the same specifiers. Per-theme measured values live in [`design-artifacts/approved/quiet-instrument-migration/theme-values.md`](../../../../design-artifacts/approved/quiet-instrument-migration/theme-values.md).

## Contrast gate (Phase 118)

A theme is installed only if its palette clears the accessibility floors, measured **composited**: each role is alpha-blended over the surface it is painted on before the ratio is computed, because a 34 % hairline or a 75 % accent is not the opaque color its bytes describe.

- **Prose** (`text.*` on the surface it is drawn on, including `text.disabled`): 4.5:1 — WCAG 2.1 SC 1.4.3.
- **Affordances** (`accent.primary`, `accent.muted`, `focus.ring`, `border.focus` on the canvas): 3.0:1 — SC 1.4.11.
- **Structural boundaries** (`border.subtle`, `border.strong` against both canvas and panel): 3.0:1. A zone edge that cannot be seen is a broken surface, not a style choice.
- **State fills** (`surface.hover`, `surface.active`, `surface.selected` composited over the canvas, then `text.primary` over that): 3.0:1.
- **`border.hairline`** keeps a 1.2:1 visibility floor only: it is the decorative zone separator (the theme's border grey at 34 %) and is deliberately exempt from 3:1 — it has to stay quieter than `border.subtle`, which it could not be at 3:1 on either surface — but it must never vanish.
- **Monotonic ladder:** `border.hairline` must stay strictly below `border.subtle` on the same surface, so a theme cannot buy the hairline floor by flattening the boundary ladder.

The gate is atomic: a failing theme is refused and the previously active theme stays installed, with one bounded `theme.contrast` diagnostic naming the specifier, the failing pair, the measured ratio and the threshold. A canonical default that fails the gate (a build invariant violation) records the same diagnostic and leaves the Clay core default palette active instead of installing a low-contrast theme at startup. `tests/theme_packages.rs` re-derives every shipped value from the theme packages and asserts the floors and the ladder; the floors themselves live in `src/shell/theme.rs` (`REQUIRED_CONTRAST_PAIRS`, `REQUIRED_FILL_PAIRS`, `HAIRLINE_VISIBILITY_MIN`).

## Return and async behavior

Synchronous. Returns `{ specifier, overrideCount, designTokenCount }` after the theme package is resolved and validated. `overrideCount` counts text-style overrides (`textStyles`); `designTokenCount` counts typed UI design-token overrides (`designTokens`, zero for packages without them).

## Errors

Throws `theme.invalid_request` for missing specifiers, `theme.unauthorized` for non-`@clay/*` specifiers, package load/validation errors if the theme package is invalid, and `theme.contrast` when the palette fails the contrast gate (the diagnostic names the specifier, the failing role pair, the composited ratio and the threshold). A rejected theme installs nothing: the previously active theme stays in effect, and the Clay core default palette remains the fallback when nothing was active.

## Permissions and security

Authority not granted: no raw CSS, renderer callbacks, client hooks, raw `Deno.core.ops`, arbitrary package theme loading, filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, native widget, client-side JavaScript, WASM authority, or ability to mutate core UI tokens from package JavaScript during client paint/layout. Package `designTokens` are validated against core token types and domain bounds; raw values that bypass typed validation are rejected.

## Agent guidance

Prefer the canonical default for the user's appearance (`@clay/theme-modus-operandi` for light, `@clay/theme-modus-vivendi` for dark) or an explicit Gruvbox choice when the user asks for one. All four shipped themes clear the contrast gate; if a palette looks wrong, fix the role values in the theme package and re-run `tests/theme_packages.rs` instead of hand-editing colors in place. Do not suggest raw CSS or arbitrary theme code execution.

## Backing implementation

`runtime/js/theme.js::setTheme` calls `op_clay_theme_set_theme`, which resolves package `textStyles` into `ActiveTheme`; the React theme adapter (`frontend/src/theme/adapter.ts`) converts it into CSS custom properties before paint.

## Lookup metadata

Tags: theme, syntax, colors, gruvbox, init.

## Authority

Only first-party `@clay/*` theme packages are accepted. Theme packages contribute static `textStyles` data for editor syntax/base-UI colors, plus optional `designTokens` typed UI overrides for dimensions, elevations, motion durations, z-levels, density, color roles, spacings, radii, and opacity. Clay validates tokens, hex colors, and domain-specific bounds (dimension ordering, opacity `[0,1]`, motion-duration `[0,1000]`, valid level names), then ships an `ActiveTheme` snapshot containing both text-style overrides and design-token overrides to the native client during bootstrap. The client resolves overrides into a `StyleRegistry` (color paint paths) and a `ResolvedUiTheme` (typed UI token hot-path reads).

## Denied

Authority not granted: no raw CSS, renderer callbacks, client hooks, raw `Deno.core.ops`, arbitrary package theme loading, filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, native widget, client-side JavaScript, WASM authority, or ability to mutate core UI tokens from package JavaScript during client paint/layout. Package `designTokens` are validated against core token types and domain bounds; raw values that bypass typed validation are rejected.

## Key bindings

No default key bindings. This API is meant for startup configuration in `init.js`, not key routing.

## Custom properties

- name: specifier
- `specifier` (string, required): bundled first-party theme package specifier, for example `@clay/theme-gruvbox-material-dark`.

## Phase 20.1 typed UI design-token overrides

When a theme package's `package.json` includes `clay.contributions.designTokens`, those typed UI overrides are validated server-side (type match against core token, domain bounds) and shipped to the client inside `ActiveTheme.design_tokens`. The client builds a `ResolvedUiTheme` that serves cached hot-path reads for dimensions, elevations, motion durations, z-levels, density, color roles, spacings, radii, and opacity. A theme that ships no `designTokens` resolves through Clay core fallbacks unchanged; all four shipped themes declare the language's theme-side color roles, so their palettes are their own. The Phase 20.1 typed token catalog (`.agents/skills/clay-execution/references/tokens.md`) documents all ten typed domains and their core fallback tokens.
