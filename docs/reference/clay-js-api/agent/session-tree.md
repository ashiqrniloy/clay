---
id: agent.sessionTree
kind: clay-js-api
js_module: "clay:agent"
js_export: sessionTree
js_facade: runtime/js/agent.js::sessionTree
backing_rust: src/server/agent.rs::AgentHost::run; clay-agent/src/host.ts::sessionCheckout
deno_op: op_clay_agent_session_tree
deno_op_path: src/server/ops/agent.rs::op_clay_agent_session_tree
name: sessionTree
user_facing_name: Navigate Agent Session Tree
summary: Checkout, fork, clone, or checkpoint an agent session's conversation tree with document restore.
owner: server
phase: Phase 1
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties: []
security: Checkout first restores server-recorded document checkpoints (buffer-only, lease-respecting) before moving the conversation leaf; abandoned branches are kept, never deleted. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.sessionTree` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Checkout rewrites the live branch; confirm with the user before discarding their current position.
lookup_tags: [agent, tree, checkout, fork, clone, checkpoint, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# sessionTree

## Summary

Checkout, fork, clone, or checkpoint an agent session's conversation tree with document restore.

## Description

`sessionTree` is the runtime-backed public API for **Navigate Agent Session Tree**. It dispatches one of four tree operations for a live session: `checkout` (move the leaf to an existing entry and restore its document checkpoint first), `fork` (new branch view on the same session), `clone` (copy the branch to a new session id), or `checkpoint` (record document versions at an entry). Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Tree navigation is a user-driven control and never runs in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when Clay automation or first-party packages need the documented **Navigate Agent Session Tree** behavior, such as a `/tree` or `/fork` command. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { sessionTree } from "clay:agent";

await sessionTree({ sessionId, method: "checkout", entryId });
```

## Example

```ts
const marked = await sessionTree({ sessionId: "s1", method: "checkpoint", entryId: leafId });
const restored = await sessionTree({ sessionId: "s1", method: "checkout", entryId: leafId });
console.log(restored.leafId);
```

## Options

- `sessionId` (string, required): the live agent session.
- `method` (`"checkout" | "fork" | "clone" | "checkpoint"`, required): the tree operation.
- `entryId` (string, required): the target tree entry.

## Key bindings

No default key binding is assigned. Users may bind a key to `agent.sessionTree` in `~/.clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise resolving to an operation-specific object (`{ sessionId, leafId? }` at minimum). Checkout fails closed if the server cannot restore document versions (for example a lease held elsewhere); the leaf stays put.

## Errors

Fails closed with typed errors when the runtime has no attached agent host (`agent.unavailable`), parameters are malformed (`agent.invalid_params`), the method is unknown, or document restore cannot proceed.

## Permissions and security

Requires: `agent-host`.

Checkout first restores server-recorded document checkpoints (buffer-only, lease-respecting) before moving the conversation leaf; abandoned branches are kept, never deleted. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.sessionTree` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Checkout rewrites the live branch; confirm with the user before discarding their current position.

## Backing implementation

- JS facade: `runtime/js/agent.js::sessionTree`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_session_tree` (`op_clay_agent_session_tree`)
- Backing Rust/current owner: `src/server/agent.rs::AgentHost::run`; `clay-agent/src/host.ts::sessionCheckout`

## Lookup metadata

- Stable ID: `agent.sessionTree`
- User-facing name: Navigate Agent Session Tree
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `sessionTree`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, tree, checkout, fork, clone, checkpoint, js-api]`
