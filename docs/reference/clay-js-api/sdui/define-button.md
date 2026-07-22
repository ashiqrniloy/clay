---
id: clay.sdui.defineButton
kind: clay-js-api
js_module: "clay:sdui"
js_export: defineButton
js_facade: runtime/js/sdui.js::defineButton
backing_rust: src/protocol/sdui.rs::SduiNodeKind::Button
deno_op: op_clay_sdui_define_node
deno_op_path: src/server/ops/sdui.rs::op_clay_sdui_define_node
name: defineButton
user_facing_name: Define Button
summary: Create a runtime-backed inert SDUI button with a server-routed action intent through the `clay:sdui` Clay JavaScript facade.
owner: server
phase: Phase 12
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: label
    type: string
    default: "required"
    description: Button label rendered by the native client.
  - name: action
    type: SduiActionIntent
    default: "required"
    description: Server-routed command intent; it is metadata, not executable code.
  - name: id
    type: string|number
    default: "optional"
    description: Optional stable SDUI node identifier used for reconciliation.
security: Creates inert declarative UI metadata only; Phase 12/13 SDUI helper metadata does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.sdui.defineButton` only to describe server-driven native UI nodes; do not invent raw Rust, protocol, or `Deno.core.ops` access and do not use it for external effects.
lookup_tags: [sdui, server-driven-ui, js-api, phase12, phase13, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# defineButton

## Summary

Create a runtime-backed inert SDUI button with a server-routed action intent through the `clay:sdui` Clay JavaScript facade.

## Description

`defineButton` reserves the public Clay JS helper name for building Phase 12 server-driven UI schema nodes. The current Rust implementation publishes static SDUI trees from `src/server/sdui.rs`; JavaScript runtime wiring is available in Phase 13 through Clay-owned server ops.

The helper describes native UI metadata only. It does not run code on the client, mutate documents, open files, contact the network, or bypass server validation.

## When to use

Use this API when future configuration, extensions, or automation need to construct a button with a server-routed action intent for a server-authored native UI tree. Prefer this facade over raw protocol DTOs or Rust symbols.

## JavaScript usage

```ts
import { defineButton } from "clay:sdui";

const node = defineButton({ label: "Refresh", action: { commandId: "workspace.refresh" } });
```

## Example

```ts
const node = defineButton({ label: "Refresh", action: { commandId: "workspace.refresh" } });
```

## Options

- `id` (`string | number`, optional): Stable node ID. If omitted, a future builder may assign one before publication.
- `label` (`string`): Button label rendered by the native client.
- `action` (`SduiActionIntent`): Server-routed command intent; it is metadata, not executable code.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.sdui.defineButton` in `~/.config/clay/init.js` only if a future command surface explicitly supports invoking SDUI helper APIs; normal use is from startup/reload configuration modules before `publishTree`.

## Custom properties

- `label` (`string`, default `required`): Button label rendered by the native client.
- `action` (`SduiActionIntent`, default `required`): Server-routed command intent; it is metadata, not executable code.

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

Use `clay.sdui.defineButton` when the user asks for runtime-backed Clay SDUI schema helpers. Avoid raw `SduiNodeKind` construction in user-facing docs, arbitrary command IDs, filesystem effects, network effects, shell commands, extension loading, AI mutation, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/sdui.js::defineButton`
- Deno op: `src/server/ops/sdui.rs::op_clay_sdui_define_node` (`op_clay_sdui_define_node`)
- Backing Rust/current owner: `src/protocol/sdui.rs::SduiNodeKind::Button`
- Current implementation audit path: `src/protocol/sdui.rs`; `src/server/sdui.rs`; `src/masonry_sdui.rs`

## Lookup metadata

- Stable ID: `clay.sdui.defineButton`
- User-facing name: Define Button
- Kind: `clay-js-api`
- Module/export: `clay:sdui` / `defineButton`
- Default key bindings: none
- Custom properties: `label`, `action`
- Tags: `sdui`, `server-driven-ui`, `js-api`, `phase12`, `phase13`, `runtime-backed`
