---
id: ui.serverRequestLayoutIntent
kind: clay-js-api
js_module: "clay:ui"
js_export: serverRequestLayoutIntent
js_facade: runtime/js/ui.js::serverRequestLayoutIntent
backing_rust: src/server/ui.rs::PackageUiRegistry::request_layout_intent
deno_op: op_clay_ui_request_layout_intent
deno_op_path: src/server/ops/ui.rs::op_clay_ui_request_layout_intent
name: serverRequestLayoutIntent
user_facing_name: Request Layout Intent
summary: Submit an inert versioned layout intent requesting a pane split through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 20.3
visibility: public
permissions: ["package-configuration"]
key_bindings: []
custom_properties:
  - name: id
    type: string
    default: package-prefixed
    description: Package-prefixed unique intent ID (e.g. `markdown.splitPreview`).
  - name: targetPane
    type: string
    default: required
    description: Target pane identifier (e.g. `active`).
  - name: orientation
    type: enum
    default: required
    description: One of `horizontal` or `vertical`.
  - name: ratio
    type: number
    default: required
    description: Split ratio between 0.05 and 0.95.
  - name: position
    type: enum
    default: second
    description: One of `first` or `second`; controls which side the new pane occupies.
hot_path_policy: Evaluated during package load/configuration work only; client hot paths read already-validated inert state.
security: does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, hidden layout keys, direct client widgets, native widget handles, raw CSS, or renderer callbacks. Packages cannot mutate native layout directly; intents are advisory and composed at Clay's discretion.
agent_guidance: Use only documented typed intent records; never expose hidden keys, raw ops, callbacks, native handles, raw CSS, or client-side JavaScript.
lookup_tags: [ui, package-ui, layout-intent, split, configuration, clay-js-api, phase20.3, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRequestLayoutIntent

## Summary

`serverRequestLayoutIntent` submits an inert versioned layout intent requesting a pane split.

## Description

The runtime validates package-prefixed IDs, orientation (`horizontal`/`vertical`), ratio bounds (0.05–0.95), position (`first`/`second`), payload size, and prohibited authority. Accepted intents are stored in the `PackageUiRegistry` and composed into `WorkingAreaLayoutUpdate` at Clay's discretion. Packages cannot mutate native layout directly.

## When to use

Use this from package load or command handling to request a pane split (e.g. a Markdown preview pane beside the editor).

## JavaScript usage

```ts
import { serverRequestLayoutIntent } from "clay:ui";
```

## Example

```ts
serverRequestLayoutIntent({
  id: "markdown.splitPreview",
  targetPane: "active",
  orientation: "horizontal",
  ratio: 0.5,
  position: "second",
});
```

## Options

- `id`: package-prefixed unique intent ID.
- `targetPane`: target pane identifier.
- `orientation`: `horizontal` or `vertical`.
- `ratio`: split ratio (0.05–0.95).
- `position`: `first` or `second` (default: `second`).

## Key bindings

No key bindings are registered by this API.

## Custom properties

| Name | Type | Default | Description |
| --- | --- | --- | --- |
| `id` | string | package-prefixed | Package-prefixed unique intent ID. |
| `targetPane` | string | required | Target pane identifier. |
| `orientation` | enum | required | `horizontal` or `vertical`. |
| `ratio` | number | required | Split ratio (0.05–0.95). |
| `position` | enum | `second` | `first` or `second`. |

## Return and async behavior

Synchronous. Returns a JSON object with `registered: true`, the intent `id`, `targetPane`, `orientation`, `ratio`, `position`, `source`, and `estimatedPayloadBytes`. Throws a typed error on validation failure.

## Errors

Throws `ui.layout_intent_failed` for invalid package prefix on `id`, duplicate `id`, invalid `orientation`, `ratio` outside 0.05–0.95, invalid `position`, oversized payload, or prohibited authority fields.

## Permissions and security

Requires: `package-configuration`. Server-side validation is required before any layout intent is accepted. This API does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct client widgets, native widget handles, raw CSS, renderer callbacks, or hidden layout-key authority. Packages cannot mutate native layout directly; intents are advisory and composed at Clay's discretion.

## Agent guidance

Use documented typed intent records only. Do not add renderer-specific or package-specific Rust branches.

## Backing implementation

- Facade: `runtime/js/ui.js::serverRequestLayoutIntent`
- Op: `src/server/ops/ui.rs::op_clay_ui_request_layout_intent`
- Rust: `src/server/ui.rs::PackageUiRegistry::request_layout_intent`

## Lookup metadata

Tags: ui, package-ui, layout-intent, split, configuration, phase20.3, runtime-backed.
