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
summary: Toggle full autonomy for one live agent session; default false, host-set only.
owner: server
phase: Phase 1
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties:
  - name: default
    type: boolean
    default: false
    description: The autonomy default itself — full autonomy is off unless explicitly enabled by a user action (decision 2157). Documented as a property, not a settable option; there is no "default autonomy" key in init.js.
security: Autonomy defaults to false and only the host side can change it; no agent-facing tool can flip this flag. Enabling it skips approval prompts for gated tool calls in that session. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.setFullAutonomy` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Never enable full autonomy on a user's behalf without an explicit user action; the default is approval-gated.
lookup_tags: [agent, autonomy, approval, permissions, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# setFullAutonomy

## Summary

Toggle full autonomy for one live agent session; default false, host-set only.

## Description

`setFullAutonomy` is the runtime-backed public API for **Set Agent Full Autonomy**. It flips the session's `fullAutonomy` flag (decision 2157): when true, the acceptance policy skips approval prompts for gated tool calls; when false (the default), out-of-workspace writes, shell metacharacters, and other gated operations require an approval decision. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Autonomy changes are user-action driven and never run in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when Clay automation or first-party packages need the documented **Set Agent Full Autonomy** behavior, such as an explicit autonomy toggle in agent UI. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { setFullAutonomy } from "clay:agent";

await setFullAutonomy({ sessionId, enabled: false });
```

## Example

```ts
// Re-enable approvals after a trusted batch run.
await setFullAutonomy({ sessionId: "s1", enabled: false });
```

## Options

- `sessionId` (string, required): the live agent session.
- `enabled` (boolean, required): true skips approval prompts for gated calls; false restores them. Default for new sessions is false.

## Key bindings

No default key binding is assigned. Users may bind a key to `agent.setFullAutonomy` in `~/.clay/init.js`.

## Custom properties

- `default` (`boolean`, default `false`): The autonomy default itself — full autonomy is off unless explicitly enabled by a user action (decision 2157). This documents the default; it is not a settable init.js key. There is no "default autonomy" configuration option: autonomy state lives with the session and only `agent.setFullAutonomy` changes it, host-side.

## Return and async behavior

Returns a promise resolving to `{ sessionId, fullAutonomy }` reflecting the applied state. Disabling autonomy on an unknown or already-dead session still resolves (the fail-closed default already applies); enabling requires a live session.

## Errors

Fails closed with typed errors when the runtime has no attached agent host (`agent.unavailable`), parameters are malformed (`agent.invalid_params`), or the session is unknown while enabling.

## Permissions and security

Requires: `agent-host`.

Autonomy defaults to false and only the host side can change it; no agent-facing tool can flip this flag. Enabling it skips approval prompts for gated tool calls in that session. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.setFullAutonomy` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Never enable full autonomy on a user's behalf without an explicit user action; the default is approval-gated.

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
- Custom properties: `default` (boolean, default false — documented default, not a settable option)
- Tags: `[agent, autonomy, approval, permissions, js-api]`
