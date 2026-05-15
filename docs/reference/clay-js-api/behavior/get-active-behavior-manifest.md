---
id: clay.behavior.getActiveBehaviorManifest
kind: clay-js-api
js_module: "clay:behavior"
js_export: getActiveBehaviorManifest
js_facade: runtime/js/behavior.ts::getActiveBehaviorManifest
backing_rust: src/client/behavior.rs::ClientBehaviorState::active_manifest
deno_op: op_clay_behavior_get_active_manifest
deno_op_path: src/server/ops/behavior.rs::op_clay_behavior_get_active_manifest
name: getActiveBehaviorManifest
user_facing_name: Get Active Behavior Manifest
summary: Get Active Behavior Manifest through the planned `clay:behavior` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Exposes behavior metadata, not executable JavaScript; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.behavior.getActiveBehaviorManifest` only for its documented behavior responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [behavior, behaviormanifestrouting, js-api]
app_visible: true
help_visible: true
stability: planned
async: true
---

# getActiveBehaviorManifest

## Summary

Get Active Behavior Manifest through the planned `clay:behavior` Clay JavaScript facade.

## Description

`getActiveBehaviorManifest` is the planned public API for **Get Active Behavior Manifest**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `server-owned-behavior-query`. Runtime path: `background-query`. Queries manifest metadata for help/agents/configuration; hot-path routing uses the already-installed inert manifest locally.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Get Active Behavior Manifest` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { getActiveBehaviorManifest } from "clay:behavior";

const manifest = await getActiveBehaviorManifest("current");
```

## Example

```ts
const manifest = await getActiveBehaviorManifest("current");
```

## Options

- `documentId` (`string`): Optional document/editor surface whose active manifest should be inspected.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.behavior.getActiveBehaviorManifest` in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for inert behavior manifest metadata.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Exposes behavior metadata, not executable JavaScript; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.behavior.getActiveBehaviorManifest` when the user asks for get active behavior manifest through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/behavior.ts::getActiveBehaviorManifest`
- Future Deno op: `src/server/ops/behavior.rs::op_clay_behavior_get_active_manifest` (`op_clay_behavior_get_active_manifest`)
- Backing Rust/current owner: `src/client/behavior.rs::ClientBehaviorState::active_manifest`
- Current implementation audit path: `src/behavior/manifest.rs::validate_manifest; src/client/behavior.rs::ClientBehaviorState`

## Lookup metadata

- Stable ID: `clay.behavior.getActiveBehaviorManifest`
- User-facing name: Get Active Behavior Manifest
- Kind: `clay-js-api`
- Module/export: `clay:behavior` / `getActiveBehaviorManifest`
- Default key bindings: none
- Custom properties: none
- Tags: `behavior`, `behaviormanifestrouting`, `js-api`
