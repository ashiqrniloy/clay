---
id: clay.behavior.listBehaviorRoutes
kind: clay-js-api
js_module: "clay:behavior"
js_export: listBehaviorRoutes
js_facade: runtime/js/behavior.ts::listBehaviorRoutes
backing_rust: src/client/behavior.rs::ClientBehaviorState::route_key
deno_op: op_clay_behavior_list_routes
deno_op_path: src/server/ops/behavior.rs::op_clay_behavior_list_routes
name: listBehaviorRoutes
user_facing_name: List Behavior Routes
summary: List Behavior Routes through the planned `clay:behavior` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Returns inert routing metadata only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.behavior.listBehaviorRoutes` only for its documented behavior responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [behavior, behaviormanifestrouting, js-api]
app_visible: true
help_visible: true
stability: planned
async: true
---

# listBehaviorRoutes

## Summary

List Behavior Routes through the planned `clay:behavior` Clay JavaScript facade.

## Description

`listBehaviorRoutes` is the planned public API for **List Behavior Routes**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `server-owned-behavior-query`. Runtime path: `background-query`. Route inspection is a background query; actual route decisions for keypresses remain local manifest evaluation.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `List Behavior Routes` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { listBehaviorRoutes } from "clay:behavior";

const routes = await listBehaviorRoutes("current");
```

## Example

```ts
const routes = await listBehaviorRoutes("current");
```

## Options

- `documentId` (`string`): Optional document/editor surface whose routes should be listed.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.behavior.listBehaviorRoutes` in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for inert behavior route metadata.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Returns inert routing metadata only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.behavior.listBehaviorRoutes` when the user asks for list behavior routes through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/behavior.ts::listBehaviorRoutes`
- Future Deno op: `src/server/ops/behavior.rs::op_clay_behavior_list_routes` (`op_clay_behavior_list_routes`)
- Backing Rust/current owner: `src/client/behavior.rs::ClientBehaviorState::route_key`
- Current implementation audit path: `src/protocol/mod.rs::RoutingPolicy; src/client/behavior.rs::ClientBehaviorState::route_key`

## Lookup metadata

- Stable ID: `clay.behavior.listBehaviorRoutes`
- User-facing name: List Behavior Routes
- Kind: `clay-js-api`
- Module/export: `clay:behavior` / `listBehaviorRoutes`
- Default key bindings: none
- Custom properties: none
- Tags: `behavior`, `behaviormanifestrouting`, `js-api`
