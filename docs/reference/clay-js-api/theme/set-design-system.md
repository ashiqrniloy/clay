---
id: theme.setDesignSystem
kind: clay-js-api
js_module: "clay:theme"
js_export: setDesignSystem
js_facade: runtime/js/theme.js::setDesignSystem
backing_rust: src/server/ops/theme.rs::op_clay_theme_set_design_system; src/server/ops/theme.rs::apply_design_system; src/shell/design_system.rs::resolve_design_system
deno_op: op_clay_theme_set_design_system
deno_op_path: src/server/ops/theme.rs::op_clay_theme_set_design_system
name: setDesignSystem
user_facing_name: Set Design System
summary: Explicitly activate a UI design system (the built-in `@clay/core` baseline or one adopted package's `uiDesignSystem` contribution) for component recipes; color authority always stays with the active theme.
owner: server
phase: Phase 22.4
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: specifier
    type: string
    default: required
    description: "`@clay/core` (built-in baseline) or the name of one loaded package that contributes a `uiDesignSystem` declaration."
security: Activates inert recipe data only: recipes may select among active-theme color roles and set bounded non-color values, but never contain concrete colors, so color authority always stays with the active theme and no raw CSS is possible. `@clay/core` is built in and always selectable; a bundled `@clay/` specifier (the shipped `@clay/design-instrument`, the approved Quiet Instrument language) resolves through the first-party bundled record path with its manifest read and nothing installed, and a third-party specifier must be an installed package that still passes the package service's existing enable/trust validation — selection itself never installs, adopts, or enables a capability, and there is no automatic trust promotion; provenance/trust-domain requirements come from that path. On failure or later package revocation the previous valid design system is preserved and the sanitized diagnostic surfaces; recipes resolve through `@clay/core` fallbacks for missing components. Does not grant filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, raw CSS, or renderer callback authority.
agent_guidance: Use setDesignSystem("@clay/core") from init.js for the built-in baseline, or a loaded package name that contributes a `uiDesignSystem` (the approved default language is Quiet Instrument, specified in DESIGN.md). Colors come only from the active theme; never suggest raw CSS or package-owned colors. If the package is later removed — for example by a migration that deletes a shipped design system — Clay keeps the last valid design system active, records one bounded diagnostic naming the rejected specifier and the system that stays active, and never fails the generation.
lookup_tags: [theme, design-system, recipes, init, core, fallback]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setDesignSystem

## Summary

Explicitly activate a UI design system for component recipes: the built-in `@clay/core` baseline, or one loaded package's `uiDesignSystem` contribution. Color authority always stays with the active theme.

## Description

`setDesignSystem` stores a validated `ActiveDesignSystem` snapshot on the server. `@clay/core` (also accepted as `clay:core` or `core`) activates the built-in core fallback recipes. Any other specifier must resolve to a package record with a `clay.contributions.uiDesignSystem` declaration: a bundled first-party `@clay/*` name resolves through the first-party bundled record path (the manifest is read; nothing is installed), and a third-party name must be an installed package that passes the package service's existing enable/trust validation. Selection itself adopts no trust, enables no capability, and installs nothing. The declaration is validated and resolved with `resolve_design_system`, which fills any missing component recipes from the `@clay/core` fallback set. The resolved snapshot ships to the client inside the runtime state snapshot and the React adapter projects it into CSS custom properties before paint.

## When to use

Use from `~/.clay/init.js` (or a local configuration module) to select the component recipe set at startup, or from a package's own configuration to activate its contributed design system. For interactive changes that persist across restarts, use the Settings panel's "Design system" dropdown or the [`settings.setDesignSystem`](../settings/set-design-system.md) command, which validates the specifier, persists the `designSystem` preference, and reapplies it on every reload.

## JavaScript usage

```ts
import { setDesignSystem } from "clay:theme";

setDesignSystem("@clay/design-instrument"); // Quiet Instrument (shipped default language)
// or
setDesignSystem("@clay/core"); // built-in baseline recipes
```

`setDesignSystem` returns `{ specifier, recipeCount, schemaVersion }`.

## Example

```ts
import { setAppearance, setDesignSystem, setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
setAppearance("dark");
setDesignSystem("@clay/design-instrument");
```

## Options

Pass either a specifier string or `{ specifier }`. `@clay/core`, `clay:core`, and `core` select the built-in baseline; other specifiers name packages that contribute `clay.contributions.uiDesignSystem`.

Shipped choice set: `@clay/core` (baseline, always first) and the bundled `@clay/design-instrument` contributor. A bundled contributor is selectable without a prior `loadPackage` because the bundled manifest is validated and resolved on demand — this is what makes the shipped design system reachable from the Settings dropdown on a fresh install. The Settings panel enumerates the server's `ui_choices` snapshot rather than a hand-maintained list, so removed or unknown specifiers are never offered; a hand-written `init.js` specifier that names one still fails closed with `theme.load_failed`.

## Return and async behavior

Synchronous. Returns `{ specifier, recipeCount, schemaVersion }` after the declaration is resolved and validated.

## Errors

Throws `theme.invalid_request` for a missing or empty specifier, `theme.load_failed` when a package cannot be enabled or resolved (including a specifier whose package was removed), and `theme.invalid_design_system` when the package does not contribute a `uiDesignSystem` or the declaration fails validation. A failed activation installs nothing and preserves the previously active design system for this generation (nothing activated yet means the `@clay/core` baseline is what paints); a persisted preference that fails on reload — the shape a removed design system takes on the next start — is recorded as one bounded diagnostic naming both the rejected specifier and what stays active instead of blocking startup.

## Permissions and security

Recipes are inert data: they may select among active-theme color roles and define bounded non-color values (spacing, radii, typography scales), but concrete colors are rejected, so no raw CSS or color authority exists. Color sourcing is active-theme-only. Package adoption requires the package service's existing trust/approval path; naming a package never promotes trust. Revoking or removing an activated package preserves the last valid design system and surfaces a sanitized diagnostic on the next reload attempt.

## Agent guidance

Use `setDesignSystem("@clay/core")` unless the user asks for a specific design-system package. The approved Clay design language is Quiet Instrument (`DESIGN.md`); its package specifier is `@clay/design-instrument` (the former Neobrutal and Glass packages were removed). Do not suggest raw CSS, package-owned colors, or design systems from untrusted packages. After a package is removed, Clay keeps the last valid design system until a valid reload succeeds.

## Backing implementation

`runtime/js/theme.js::setDesignSystem` calls `op_clay_theme_set_design_system`, which calls `apply_design_system` (`src/server/ops/theme.rs`) to validate and store the `ActiveDesignSystem`; `src/shell/design_system.rs::resolve_design_system` resolves recipes against core fallbacks, and the React adapter (`frontend/src/theme/design-system-adapter.ts`) projects the snapshot into CSS custom properties.

## Lookup metadata

Tags: theme, design-system, recipes, init, core, fallback.

## Authority

Only the built-in `@clay/core` baseline and packages activated through the package service are accepted. Packages contribute a static `uiDesignSystem` declaration (recipe tokens) validated server-side; recipes reference theme color roles by name and never carry concrete colors, so switching themes re-colors an activated design system automatically. Missing component recipes resolve from `@clay/core` fallbacks.

## Denied

Authority not granted: no filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, raw CSS, or renderer callback authority; no automatic trust promotion by naming a package.

## Key bindings

No default key bindings. This API is meant for startup configuration in `init.js`, not key routing.

## Custom properties

- name: specifier
- `specifier` (string, required): `@clay/core` for the built-in baseline, or the name of one loaded package contributing a `uiDesignSystem`.

## Fallback and revocation

Explicit activation records intent; the core fallback recipes cover every component so packages may override only what they declare. If an activated package fails to load on a later reload (removed, revoked, or invalid declaration), the previous valid design system is preserved for the running generation and a sanitized diagnostic is recorded; the next valid reload applies cleanly. A generation that never activated one paints the built-in `@clay/core` baseline — which is the shipped language's host-consumed subset, not a second look — so a removed specifier degrades to the same recipes with one diagnostic instead of an unstyled window.
