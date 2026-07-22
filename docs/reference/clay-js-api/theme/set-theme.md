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
summary: Select one first-party inert text-style theme for editor syntax and base UI colors.
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
security: Accepts only bundled first-party @clay/* theme specifiers, reads static package.json textStyles contributions, and sends inert RGBA/style-attribute data to the client; does not grant filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, raw CSS, or renderer callback authority.
agent_guidance: Use setTheme("@clay/theme-gruvbox-material-dark") or setTheme("@clay/theme-gruvbox-material-light") from init.js. Do not expose arbitrary CSS, theme code execution, raw color ops, or third-party theme loading through this API.
lookup_tags: [theme, syntax, colors, gruvbox, init]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setTheme

## Summary

Select one first-party inert text-style theme for editor syntax and base UI colors.

## Description

`setTheme` resolves a bundled first-party theme package, validates its static `textStyles` contribution, and stores an `ActiveTheme` snapshot that the server sends to the native client during bootstrap. The editor then builds a `StyleRegistry` before first paint.

## When to use

Use from `~/.config/clay/init.js` to choose the editor theme at startup.

## JavaScript usage

```ts
import { setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
// or
setTheme({ specifier: "@clay/theme-gruvbox-material-light" });
```

`setTheme` returns `{ specifier, overrideCount }`.

## Example

```ts
import { setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
```

## Options

Pass either a theme specifier string or `{ specifier }`.

## Return and async behavior

Synchronous. Returns `{ specifier, overrideCount }` after the theme package is resolved and validated.

## Errors

Throws `clay.theme.invalid_request` for missing specifiers, `clay.theme.unauthorized` for non-`@clay/*` specifiers, and package load/validation errors if the theme package is invalid.

## Permissions and security

Authority not granted: no raw CSS, renderer callbacks, client hooks, raw `Deno.core.ops`, arbitrary package theme loading, filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, native widget, client-side JavaScript, or WASM authority.

## Agent guidance

Prefer the shipped `@clay/theme-gruvbox-material-dark` or `@clay/theme-gruvbox-material-light` specifiers. Do not suggest raw CSS or arbitrary theme code execution.

## Backing implementation

`runtime/js/theme.js::setTheme` calls `op_clay_theme_set_theme`, which resolves package `textStyles` into `ActiveTheme`; `EditorSurface` converts it to `StyleRegistry` before paint.

## Lookup metadata

Tags: theme, syntax, colors, gruvbox, init.

## Authority

Only bundled `@clay/*` theme packages are accepted. Theme packages contribute static `textStyles` data from `package.json`; Clay validates tokens and hex colors, then ships an `ActiveTheme` snapshot to the native client during bootstrap.

## Denied

Authority not granted: no raw CSS, renderer callbacks, client hooks, raw `Deno.core.ops`, arbitrary package theme loading, filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, native widget, client-side JavaScript, or WASM authority.

## Key bindings

No default key bindings. This API is meant for startup configuration in `init.js`, not key routing.

## Custom properties

- name: specifier
- `specifier` (string, required): bundled first-party theme package specifier, for example `@clay/theme-gruvbox-material-dark`.
