---
id: sdui.defineLabel
kind: clay-js-api
js_module: "clay:sdui"
js_export: defineLabel
js_facade: runtime/js/sdui.js::defineLabel
backing_rust: src/protocol/sdui.rs::SduiNodeKind::Label
deno_op: op_clay_sdui_define_node
deno_op_path: src/server/ops/sdui.rs::op_clay_sdui_define_node
name: defineLabel
user_facing_name: Define Label
summary: Create a runtime-backed inert SDUI text label through the `clay:sdui` Clay JavaScript facade.
owner: server
phase: Phase 12
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: text
    type: string
    default: "required"
    description: Inert label text rendered by the native client.
  - name: id
    type: string|number
    default: "optional"
    description: Optional stable SDUI node identifier used for reconciliation.
security: Creates inert declarative UI metadata only; Phase 12/13 SDUI helper metadata does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `sdui.defineLabel` only to describe server-driven native UI nodes; do not invent raw Rust, protocol, or `Deno.core.ops` access and do not use it for external effects.
lookup_tags: [sdui, server-driven-ui, js-api, phase12, phase13, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# defineLabel

## Summary

Create a runtime-backed inert SDUI text label through the `clay:sdui` Clay JavaScript facade.

## Description

`defineLabel` reserves the public Clay JS helper name for building Phase 12 server-driven UI schema nodes. The current Rust implementation publishes static SDUI trees from `src/server/sdui.rs`; JavaScript runtime wiring is available in Phase 13 through Clay-owned server ops.

The helper describes native UI metadata only. It does not run code on the client, mutate documents, open files, contact the network, or bypass server validation.

## When to use

Use this API when future configuration, extensions, or automation need to construct a text label for a server-authored native UI tree. Prefer this facade over raw protocol DTOs or Rust symbols.

## JavaScript usage

```ts
import { defineLabel } from "clay:sdui";

const node = defineLabel({ text: "Ready" });
```

## Example

```ts
const node = defineLabel({ text: "Ready" });
```

## Options

- `id` (`string | number`, optional): Stable node ID. If omitted, a future builder may assign one before publication.
- `text` (`string`): Inert label text rendered by the native client.

## Key bindings

No default key binding is assigned. Users may bind a key to `sdui.defineLabel` in `~/.config/clay/init.js` only if a future command surface explicitly supports invoking SDUI helper APIs; normal use is from startup/reload configuration modules before `publishTree`.

## Custom properties

- `text` (`string`, default `required`): Inert label text rendered by the native client.

- `id` (`string | number`, default `optional`): Optional stable SDUI node identifier used for reconciliation.

## Return and async behavior

This runtime-backed facade is synchronous (`async: false`) and returns an inert SDUI node definition in the server-side runtime. Phase 13 evaluates this facade in the server-side runtime and validates the resulting inert node before publication.

## Errors

The runtime fails when options are malformed, node IDs are unstable or duplicated, child references are invalid, editor bindings reference unknown documents, or action intents name undocumented commands.

## Permissions and security

No additional permission is required to describe inert UI metadata. Publishing or acting on a UI tree remains server validated.

Creates inert declarative UI metadata only; Phase 12/13 SDUI helper metadata does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `sdui.defineLabel` when the user asks for runtime-backed Clay SDUI schema helpers. Avoid raw `SduiNodeKind` construction in user-facing docs, arbitrary command IDs, filesystem effects, network effects, shell commands, extension loading, AI mutation, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/sdui.js::defineLabel`
- Deno op: `src/server/ops/sdui.rs::op_clay_sdui_define_node` (`op_clay_sdui_define_node`)
- Backing Rust/current owner: `src/protocol/sdui.rs::SduiNodeKind::Label`
- Current implementation audit path: `src/protocol/sdui.rs`; `src/server/sdui.rs`; `frontend/src/sdui/renderer.tsx (React SDUI host renders validated sdui trees; no native state)`

## Lookup metadata

- Stable ID: `sdui.defineLabel`
- User-facing name: Define Label
- Kind: `clay-js-api`
- Module/export: `clay:sdui` / `defineLabel`
- Default key bindings: none
- Custom properties: `text`
- Tags: `sdui`, `server-driven-ui`, `js-api`, `phase12`, `phase13`, `runtime-backed`
