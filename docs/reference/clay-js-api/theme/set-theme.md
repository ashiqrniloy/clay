---
id: clay.theme.setTheme
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
agent_guidance: Use setTheme("@clay/theme-gruvbox-material-dark") or setTheme("@clay/theme-gruvbox-material-light") from init.js. Do not expose arbitrary CSS, theme code execution, raw color ops, or third-party theme loading through this API.
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

`setTheme` resolves a bundled first-party theme package, validates its static `textStyles` contribution and optional `designTokens` typed UI overrides, and stores an `ActiveTheme` snapshot that the server sends to the native client during bootstrap. The editor builds a `StyleRegistry` for syntax/base-UI colors and a `ResolvedUiTheme` for typed UI tokens (dimensions, elevations, motion durations, z-levels, density, color roles, spacings, radii, and opacity) before first paint. Resolution is additive: existing Gruvbox themes without `designTokens` resolve through Clay core fallbacks with no manifest change.

## When to use

Use from `~/.config/clay/init.js` to choose the editor theme at startup.

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

Pass either a theme specifier string or `{ specifier }`.

## Return and async behavior

Synchronous. Returns `{ specifier, overrideCount, designTokenCount }` after the theme package is resolved and validated. `overrideCount` counts text-style overrides (`textStyles`); `designTokenCount` counts typed UI design-token overrides (`designTokens`, zero for packages without them).

## Errors

Throws `clay.theme.invalid_request` for missing specifiers, `clay.theme.unauthorized` for non-`@clay/*` specifiers, and package load/validation errors if the theme package is invalid.

## Permissions and security

Authority not granted: no raw CSS, renderer callbacks, client hooks, raw `Deno.core.ops`, arbitrary package theme loading, filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, native widget, client-side JavaScript, WASM authority, or ability to mutate core UI tokens from package JavaScript during client paint/layout. Package `designTokens` are validated against core token types and domain bounds; raw values that bypass typed validation are rejected.

## Agent guidance

Prefer the shipped `@clay/theme-gruvbox-material-dark` or `@clay/theme-gruvbox-material-light` specifiers. Do not suggest raw CSS or arbitrary theme code execution.

## Backing implementation

`runtime/js/theme.js::setTheme` calls `op_clay_theme_set_theme`, which resolves package `textStyles` into `ActiveTheme`; `EditorSurface` converts it to `StyleRegistry` before paint.

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

When a theme package's `package.json` includes `clay.contributions.designTokens`, those typed UI overrides are validated server-side (type match against core token, domain bounds) and shipped to the client inside `ActiveTheme.design_tokens`. The client builds a `ResolvedUiTheme` that serves cached hot-path reads for dimensions, elevations, motion durations, z-levels, density, color roles, spacings, radii, and opacity. Existing Gruvbox themes carry no `designTokens` and resolve through Clay core fallbacks unchanged. The Phase 20.1 typed token catalog (`.agents/skills/clay-ui/references/tokens.md`) documents all ten typed domains and their core fallback tokens.
