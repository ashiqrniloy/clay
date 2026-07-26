---
id: clay.theme.setAppearance
kind: clay-js-api
js_module: "clay:theme"
js_export: setAppearance
js_facade: runtime/js/theme.js::setAppearance
backing_rust: src/server/ops/theme.rs::op_clay_theme_set_appearance; src/server/ops/theme.rs::apply_appearance; src/server/ops/theme.rs::canonical_default_specifier
deno_op: op_clay_theme_set_appearance
deno_op_path: src/server/ops/theme.rs::op_clay_theme_set_appearance
name: setAppearance
user_facing_name: Set Appearance
summary: Set the bounded appearance preference (light/dark/system) that drives the canonical default theme; an explicit setTheme always wins.
owner: server
phase: Phase 20.6
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: appearance
    type: enum
    default: required
    description: One of `light`, `dark`, or `system`. `system` follows the OS color-scheme signal with a dark fallback when no signal is available.
security: Accepts only the bounded `light` | `dark` | `system` enum, resolves the canonical default from bundled first-party `@clay/theme-modus-*` packages via the same `ensure_first_party_record` trust path as explicit setTheme, and stores an inert `ActiveTheme` snapshot; does not grant filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, raw CSS, third-party theme loading, or promotion-by-naming authority.
agent_guidance: Use setAppearance("light" | "dark" | "system") from init.js to drive the canonical default theme. An explicit setTheme("@clay/theme-*") always wins over the appearance-derived default.
lookup_tags: [theme, appearance, light-dark, system, init]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setAppearance

## Summary

Set the bounded appearance preference (`light` | `dark` | `system`) that drives the canonical default theme. An explicit `setTheme` always wins: once a theme is explicitly selected, `setAppearance` no longer re-resolves over it.

## Description

`setAppearance` stores the appearance preference and, when no explicit theme is active, resolves the canonical default theme for the resolved appearance: `light` → `@clay/theme-modus-operandi`, `dark` → `@clay/theme-modus-vivendi`, `system` → the observed OS color-scheme signal with a `dark` fallback when no signal is available. Canonical-default resolution is a bundled-inventory `ensure_first_party_record` lookup — no `loadPackage` call is required and there is no extra load cost. The Modus packages are regular first-party `@clay/theme-*` packages (identical manifest shape, inert `textStyles`) and remain explicitly selectable by `setTheme`.

The appearance preference is a `clay:theme` Clay JS API usable from `~/.config/clay/init.js` (source `init-js`). A UI-session appearance choice made through the `@clay/settings` panel is persisted to `~/.config/clay/preferences.json` (source `ui-session`) and overrides `init.js` on every reload. See [Configuration: Phase 20.6 precedence and persistence](../configuration.md#phase-206-themetypographyappearance-precedence-and-persistence) for the full source-order model.

## When to use

Use from `~/.config/clay/init.js` to drive the canonical default theme from a light/dark/system preference without pinning a specific theme package. Use `setTheme` instead when you want a specific theme regardless of appearance.

## JavaScript usage

```ts
import { setAppearance } from "clay:theme";

setAppearance("system");
// or
setAppearance({ appearance: "light" });
```

`setAppearance` returns `{ appearance, resolvedTheme }`.

## Example

```ts
import { setAppearance } from "clay:theme";

const summary = setAppearance("dark");
// summary.appearance === "dark"
// summary.resolvedTheme === "@clay/theme-modus-vivendi"
```

## Options

Pass either an appearance string or `{ appearance }`.

## Return and async behavior

Synchronous. Returns `{ appearance, resolvedTheme }`. `appearance` is the stored value (`light` | `dark` | `system`). `resolvedTheme` is the canonical default theme specifier resolved for the resolved appearance, or `null` if an explicit theme is already active (an earlier `setTheme` in `init.js`) or the canonical package is unavailable.

## Errors

Throws `clay.theme.invalid_request` for a missing `appearance` field and for unknown appearance values (anything outside `light` | `dark` | `system`).

## Permissions and security

Authority not granted: no raw CSS, renderer callbacks, client hooks, raw `Deno.core.ops`, arbitrary package theme loading, filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, native widget, client-side JavaScript, WASM authority, or promotion-by-naming. The `appearance` input is bounded to the closed `light` | `dark` | `system` enum; the resolved canonical theme is a bundled first-party `@clay/theme-*` package validated through the same `ensure_first_party_record` trust path as explicit `setTheme`.

## Agent guidance

Prefer `setAppearance("system")` for an OS-following default, or `"light"` / `"dark"` to pin the canonical default. Do not suggest arbitrary appearance strings or third-party theme loading through this API. Remember that an explicit `setTheme` always wins over the appearance-derived default.

## Backing implementation

`runtime/js/theme.js::setAppearance` calls `op_clay_theme_set_appearance`, which parses the bounded enum, calls `apply_appearance` (shared with the persisted-preference apply path), and returns the resolved canonical theme specifier. `canonical_default_specifier` maps the resolved appearance to the canonical Modus package.

## Lookup metadata

Tags: theme, appearance, light-dark, system, init.

## Authority

Only the bounded `light` | `dark` | `system` enum is accepted. Canonical-default resolution reuses the bundled first-party trust classification (`ensure_first_party_record`); selecting a canonical default grants no authority that an explicit `setTheme` of the same package would not. No theme JavaScript, package parser, or raw IPC runs in paint, layout, scroll, keypress, text-event, or edit-ack hot paths.

## Denied

Authority not granted: no raw CSS, renderer callbacks, client hooks, raw `Deno.core.ops`, arbitrary package theme loading, filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, native widget, client-side JavaScript, WASM authority, or promotion-by-naming. The `appearance` input is bounded to the closed `light` | `dark` | `system` enum; out-of-enum values are rejected with `clay.theme.invalid_request`.

## Key bindings

No default key bindings. This API is meant for startup configuration in `init.js` (and the settings UI), not key routing.

## Custom properties

- name: appearance
- `appearance` (enum, required): one of `light`, `dark`, or `system`. `system` follows the OS color-scheme signal with a `dark` fallback when no signal is available.

## Phase 20.6 canonical defaults and precedence

`setAppearance` is the programmatic surface for the Phase 20.6 appearance preference. The canonical default themes are the segregated `@clay/theme-modus-operandi` (light) and `@clay/theme-modus-vivendi` (dark) packages; Gruvbox themes stay opt-in via explicit `setTheme`. The precedence model is: canonical/package default < `init.js` (`setAppearance` / `setTheme`) < UI session (`preferences.json` written by `@clay/settings`). See [Package authoring: Phase 20.6 canonical defaults vs opt-in themes](../../packages/creating-packages.md#phase-206-canonical-defaults-vs-opt-in-themes) for the default-vs-opt-in loading contract.