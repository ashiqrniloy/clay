---
id: clay.ui.serverSetLayoutOverride
kind: clay-js-api
js_module: "clay:ui"
js_export: serverSetLayoutOverride
js_facade: runtime/js/ui.js::serverSetLayoutOverride
backing_rust: src/server/ui.rs::PackageUiRegistry::set_layout_override
deno_op: op_clay_ui_set_layout_override
deno_op_path: src/server/ops/ui.rs::op_clay_ui_set_layout_override
name: serverSetLayoutOverride
user_facing_name: Set Layout Override
summary: Set a validated package UI layout/configuration override through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 18.4
visibility: public
permissions: ["package-configuration"]
key_bindings: []
custom_properties:
  - name: targetId
    type: string
    default: package-prefixed
    description: Package-prefixed panel/component/input/token target ID.
  - name: property
    type: enum
    default: required
    description: One of `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, or `fallback`.
  - name: value
    type: json
    default: required
    description: Typed value validated for the selected property.
  - name: source
    type: enum
    default: user-config
    description: One of `user-config`, `active-major-mode`, `compatible-minor-mode`, `global-package`, or `package-default`.
hot_path_policy: Evaluated during configuration/package update work only; Masonry hot paths read already-validated inert state.
security: does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, hidden layout keys, direct Masonry widgets, native widget handles, raw CSS, or renderer callbacks.
agent_guidance: Use only documented typed override/configuration records; never expose hidden keys, raw ops, callbacks, native handles, raw CSS, or client-side JavaScript.
lookup_tags: [ui, package-ui, layout-overrides, configuration, clay-js-api, phase18.4, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverSetLayoutOverride

## Summary

`serverSetLayoutOverride` records a validated package UI layout/configuration override.

## Description

The runtime validates package-prefixed targets, supported properties, typed values, source precedence, known theme tokens, registered input/action defaults, payload size, and prohibited authority.

## When to use

Use this from configuration/package update work to override panel slots, visibility, split ratios, theme-token remaps, input/action defaults, or fallback behavior.

## JavaScript usage

```ts
import { serverSetLayoutOverride } from "clay:ui";
```

## Example

```ts
serverSetLayoutOverride({ targetId: "markdown.preview", property: "visibility", value: "hidden", source: "user-config" });
```

## Options

- `targetId`: package-prefixed target ID.
- `property`: `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, or `fallback`.
- `value`: typed value for the property.
- `source`: precedence source.

## Key bindings

No key bindings are registered by this API.

## Custom properties

- `targetId`
- `property`
- `value`
- `source`

## Return and async behavior

Returns a synchronous registration record; `async: false`.

## Errors

Throws `clay.ui.layout_override_failed` for hidden target IDs, unsupported properties, invalid slots/visibility/ratios, unknown or type-incompatible theme-token remaps, unregistered input/action defaults, raw ops, raw CSS, callbacks, native handles, or client-side JavaScript.

## Permissions and security

Requires: `package-configuration`. server-side validation is required before any layout override is accepted. This API does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, or hidden layout-key authority.

## Agent guidance

Use documented typed overrides only. Do not add Masonry-specific or package-specific Rust branches.

## Backing implementation

- Facade: `runtime/js/ui.js::serverSetLayoutOverride`
- Op: `src/server/ops/ui.rs::op_clay_ui_set_layout_override`
- Rust: `src/server/ui.rs::PackageUiRegistry::set_layout_override`

## Lookup metadata

Tags: ui, package-ui, layout-overrides, configuration, phase18.4, runtime-backed.
