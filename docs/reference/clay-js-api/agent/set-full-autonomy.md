---
id: agent.setFullAutonomy
kind: clay-js-api
js_module: "clay:agent"
js_export: setFullAutonomy
js_facade: runtime/js/agent.js::setFullAutonomy
backing_rust: src/server/agent.rs::AgentHost::run; clay-agent/src/host.ts::sessionSetAutonomy
deno_op: op_clay_agent_set_autonomy
deno_op_path: src/server/ops/agent.rs::op_clay_agent_set_autonomy
name: setFullAutonomy
user_facing_name: Set Agent Full Autonomy
summary: Toggle full autonomy for one live agent session; on by default, host-set only.
owner: server
phase: Phase 1
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties:
  - name: default
    type: boolean
    default: true
    description: The autonomy default itself — sessions are fully autonomous unless a caller explicitly blocks it (decision 2026-09-20-2049, superseding the "off by default" part of decision 2157). Documented as a property, not a settable option; there is no "default autonomy" key in init.js.
security: Autonomy is on by default and only the host side can change it; no agent-facing tool can flip this flag. With autonomy on, gated tool calls — out-of-workspace writes, delete/move, and shell-metacharacter commands — run without an approval prompt in that session, and a fresh session inherits the same default. Callers who want prompts must block autonomy explicitly (`fullAutonomy: false` at creation, or this API with `enabled: false`). A resumed session restores the autonomy recorded with it. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.setFullAutonomy` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Autonomy defaults to on: do not change it on a user's behalf in either direction, and never re-enable it after a user block without an explicit user action.
lookup_tags: [agent, autonomy, approval, permissions, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# setFullAutonomy

## Summary

Toggle full autonomy for one live agent session; on by default, host-set only.

## Description

`setFullAutonomy` is the runtime-backed public API for **Set Agent Full Autonomy**. It flips the session's `fullAutonomy` flag (decisions 2157 and 2026-09-20-2049): when true — the default for a new session — the acceptance policy skips approval prompts for gated tool calls; when false, out-of-workspace writes, shell metacharacters, and other gated operations require an approval decision. The flag is session state: `session.new` records the caller's choice, a resumed session restores that recorded value, and sessions recorded before the field existed fall back to the same default (on). Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Autonomy changes are user-action driven and never run in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when Clay automation or first-party packages need the documented **Set Agent Full Autonomy** behavior, such as an explicit autonomy toggle in agent UI. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { setFullAutonomy } from "clay:agent";

await setFullAutonomy({ sessionId, enabled: false });
```

## Example

```ts
// Re-arm approvals (the opt-out): gated calls need an approval decision.
await setFullAutonomy({ sessionId: "s1", enabled: false });
// And back to the default, streaming straight through.
await setFullAutonomy({ sessionId: "s1", enabled: true });
```

## Options

- `sessionId` (string, required): the live agent session.
- `enabled` (boolean, required): true skips approval prompts for gated calls; false restores them. New sessions start with true unless their creator passed `fullAutonomy: false`.

## Key bindings

No default key binding is assigned. Users may bind a key to `agent.setFullAutonomy` in `~/.clay/init.js`.

## Custom properties

- `default` (`boolean`, default `true`): The autonomy default itself — a session is fully autonomous unless its creator blocks it (decision 2026-09-20-2049). This documents the default; it is not a settable init.js key. There is no "default autonomy" configuration option: autonomy state lives with the session, `session.new`'s `fullAutonomy` parameter seeds it, and only this API (host-side) changes it afterwards.

## Return and async behavior

Returns a promise resolving to `{ sessionId, fullAutonomy }` reflecting the applied state. Disabling autonomy on an unknown or already-dead session still resolves (the fail-closed default already applies); enabling requires a live session.

## Errors

Fails closed with typed errors when the runtime has no attached agent host (`agent.unavailable`), parameters are malformed (`agent.invalid_params`), or the session is unknown while enabling.

## Permissions and security

Requires: `agent-host`.

Autonomy is on by default and only the host side can change it; no agent-facing tool can flip this flag. With autonomy on, gated tool calls — out-of-workspace writes, delete/move, and shell-metacharacter commands — run without an approval prompt in that session, and a fresh session inherits the same default. Callers who want prompts must block autonomy explicitly (`fullAutonomy: false` at creation, or this API with `enabled: false`). A resumed session restores the autonomy recorded with it. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.setFullAutonomy` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Autonomy defaults to on: do not change it on a user's behalf in either direction, and never re-enable it after a user block without an explicit user action.

## Backing implementation

- JS facade: `runtime/js/agent.js::setFullAutonomy`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_set_autonomy` (`op_clay_agent_set_autonomy`)
- Backing Rust/current owner: `src/server/agent.rs::AgentHost::run`; `clay-agent/src/host.ts::sessionSetAutonomy`

## Lookup metadata

- Stable ID: `agent.setFullAutonomy`
- User-facing name: Set Agent Full Autonomy
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `setFullAutonomy`
- Default key bindings: none
- Custom properties: `default` (boolean, default true — documented default, not a settable option)
- Tags: `[agent, autonomy, approval, permissions, js-api]`
