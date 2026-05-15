---
id: clay.keybindings.listKeyBindings
kind: clay-js-api
js_module: "clay:keybindings"
js_export: listKeyBindings
js_facade: runtime/js/keybindings.ts::listKeyBindings
backing_rust: src/protocol/mod.rs::BehaviorManifest::minimal_text_editing
deno_op: op_clay_keybindings_list_keybindings
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_list_keybindings
name: listKeyBindings
user_facing_name: List Key Bindings
summary: List Key Bindings through the planned `clay:keybindings` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: scope
    type: enum
    default: all
    description: Behavior-changing setting `scope` for this API.
security: Returns documented key binding metadata only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.keybindings.listKeyBindings` only for its documented keybindings responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, keybindingmanagement, keybindings]
app_visible: true
help_visible: true
stability: planned
async: false
---

# listKeyBindings

## Summary

List Key Bindings through the planned `clay:keybindings` Clay JavaScript facade.

## Description

`listKeyBindings` is the planned public API for **List Key Bindings**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `configuration-query-api`. Runtime path: `server-side-query`. Listing key bindings is a background/help/configuration query and is not part of ordinary keypress handling.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `List Key Bindings` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { listKeyBindings } from "clay:keybindings";

const bindings = listKeyBindings("editor");
```

## Example

```ts
const bindings = listKeyBindings("editor");
```

## Options

- `scope` (`"all" | "global" | "editor"`): Optional scope filter; defaults to `"all"`.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.keybindings.listKeyBindings` in `~/.config/clay/init.js`.

## Custom properties

- `scope` (`enum`, default `all`): Behavior-changing setting `scope` for this API.

## Return and async behavior

Returns key binding records for help/configuration inspection.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Returns documented key binding metadata only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.keybindings.listKeyBindings` when the user asks for list key bindings through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/keybindings.ts::listKeyBindings`
- Future Deno op: `src/server/ops/keybindings.rs::op_clay_keybindings_list_keybindings` (`op_clay_keybindings_list_keybindings`)
- Backing Rust/current owner: `src/protocol/mod.rs::BehaviorManifest::minimal_text_editing`
- Current implementation audit path: `src/protocol/mod.rs::BehaviorManifest; src/protocol/mod.rs::KeyBindingRule`

## Lookup metadata

- Stable ID: `clay.keybindings.listKeyBindings`
- User-facing name: List Key Bindings
- Kind: `clay-js-api`
- Module/export: `clay:keybindings` / `listKeyBindings`
- Default key bindings: none
- Custom properties: `scope`
- Tags: `js-api`, `keybindingmanagement`, `keybindings`
