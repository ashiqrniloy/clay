---
id: clay.keybindings.unbindKey
kind: clay-js-api
js_module: "clay:keybindings"
js_export: unbindKey
js_facade: runtime/js/keybindings.ts::unbindKey
backing_rust: src/protocol/mod.rs::KeyBindingRule
deno_op: op_clay_keybindings_unbind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_unbind_key
name: unbindKey
user_facing_name: Unbind Key
summary: Unbind Key through the planned `clay:keybindings` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: key
    type: string
    default: none
    description: Behavior-changing setting `key` for this API.
  - name: scope
    type: enum
    default: editor
    description: Behavior-changing setting `scope` for this API.
  - name: when
    type: string
    default: none
    description: Behavior-changing setting `when` for this API.
security: Changes documented key routing only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.keybindings.unbindKey` only for its documented keybindings responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, keybindingmanagement, keybindings]
app_visible: true
help_visible: true
stability: planned
async: false
---

# unbindKey

## Summary

Unbind Key through the planned `clay:keybindings` Clay JavaScript facade.

## Description

`unbindKey` is the planned public Phase 8 configuration API for **Unbind Key**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `configuration-api`. Runtime path: `server-side-configuration-to-behavior-manifest`. Unbinding affects future manifest routing only and must not execute JavaScript in keypress handlers. The planned runtime validates key chords, scopes, and `when` conditions before publishing manifest changes.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Unbind Key` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { unbindKey } from "clay:keybindings";

unbindKey("Ctrl+I", { scope: "editor" });
```

## Example

```ts
unbindKey("Ctrl+I", { scope: "editor" });
```

## Options

- `key` (`string`): Key chord to remove.
- `scope` (`"global" | "editor"`): Binding scope; defaults to `"editor"`.
- `when` (`string`): Optional condition expression identifying a specific binding; conditions are metadata for server-owned manifest routing, not executable client JavaScript.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.keybindings.unbindKey` in `~/.config/clay/init.js`.

## Custom properties

- `key` (`string`, default `none`): Behavior-changing setting `key` for this API.
- `scope` (`enum`, default `editor`): Behavior-changing setting `scope` for this API.
- `when` (`string`, default `none`): Behavior-changing setting `when` for this API.

## Return and async behavior

Returns nothing after removing the planned key binding.

Current facade/runtime status is `planned`; this page defines the Phase 8 configuration contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes documented key routing only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.keybindings.unbindKey` when the user asks for unbind key through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/keybindings.ts::unbindKey`
- Future Deno op: `src/server/ops/keybindings.rs::op_clay_keybindings_unbind_key` (`op_clay_keybindings_unbind_key`)
- Backing Rust/current owner: `src/protocol/mod.rs::KeyBindingRule`
- Current implementation audit path: `src/protocol/mod.rs::KeyBindingRule`

## Lookup metadata

- Stable ID: `clay.keybindings.unbindKey`
- User-facing name: Unbind Key
- Kind: `clay-js-api`
- Module/export: `clay:keybindings` / `unbindKey`
- Default key bindings: none
- Custom properties: `key`, `scope`, `when`
- Tags: `js-api`, `keybindingmanagement`, `keybindings`
