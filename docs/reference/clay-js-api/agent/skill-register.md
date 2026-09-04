---
id: agent.skillRegister
kind: clay-js-api
js_module: "clay:agent"
js_export: skillRegister
js_facade: runtime/js/agent.js::skillRegister
backing_rust: src/server/ops/agent.rs::agent_rpc; clay-agent/src/host.ts::skillRegister
deno_op: op_clay_agent_skill_register
deno_op_path: src/server/ops/agent.rs::op_clay_agent_skill_register
name: skillRegister
user_facing_name: Register Agent Skill
summary: Register an agent skill (progressive-disclosure instructions) on the daemon's validated registry; one-shot during package load.
owner: server
phase: Phase 2
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties: []
security: Forwards an inert skill declaration (name, description, instructions, tool names) to the daemon's validated skill registry; re-registering a name replaces the definition. Skills surface only through the daemon's load_skill progressive-disclosure tool and never execute anything by being registered. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.skillRegister` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Register skills before the profile that references them; keep instructions as documentation, never as an execution surface.
lookup_tags: [agent, skill, registration, progressive-disclosure, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# skillRegister

## Summary

Register an agent skill (progressive-disclosure instructions) on the daemon's validated registry; one-shot during package load.

## Description

`skillRegister` is the runtime-backed public API for **Register Agent Skill**. It forwards an inert skill declaration — name, description, full instructions, and optional tool names — to the clay-agent daemon's `skill.register` registry. Skills follow progressive disclosure: sessions with active skills see only names and descriptions; the full instructions load through the daemon's `load_skill` tool when the agent requests them. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Registration runs once during package load and never in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when a first-party agent package contributes a skill to its profiles, such as the Coding Agent's plan-file convention skill. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { skillRegister } from "clay:agent";

await skillRegister({
  name: "coding-agent.createPlan",
  description: "Create and maintain numbered plan documents.",
  instructions: "Plan files live under plans/ ...",
});
```

## Example

```ts
// The Coding Agent package registers its plan-file skill before its profile.
await skillRegister({
  name: "coding-agent.createPlan",
  description: "Create and maintain numbered plan documents.",
  instructions: "Plan files live under plans/ ...",
  toolNames: ["read", "write"],
});
```

## Options

- `name` (string, required): the skill name; profiles reference it by name. Duplicate names fail closed.
- `description` (string, optional): short description shown in the skill catalog.
- `instructions` (string, optional): full instructions loaded on demand through `load_skill`.
- `toolNames` (string[], optional): tools the skill documents; validated against the tool registry.

## Key bindings

No default key binding is assigned. `agent.skillRegister` is a load-time registration call, not a user command.

## Custom properties

None.

## Return and async behavior

Returns a promise resolving to `{ name, registered: true }` when the daemon accepted the declaration. When the daemon is not yet running, the call resolves immediately with `{ queued: true }`: the declaration is held server-side and applied right after the daemon's next initialize handshake — a load entry never spawns the daemon or waits on its boot. In runtimes with no agent host at all, the declaration queues process-global and applies when a host installs. Re-registering an existing skill name replaces the previous definition (kernel default duplicate policy).

## Errors

Fails closed with typed errors when the declaration is malformed (`agent.invalid_params`) or the pending-registration queue is full.

## Permissions and security

Requires: `agent-host`.

Forwards an inert skill declaration (name, description, instructions, tool names) to the daemon's validated skill registry; re-registering a name replaces the definition. Skills surface only through the daemon's `load_skill` progressive-disclosure tool and never execute anything by being registered. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.skillRegister` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Register skills before the profile that references them; keep instructions as documentation, never as an execution surface.

## Backing implementation

- JS facade: `runtime/js/agent.js::skillRegister`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_skill_register` (`op_clay_agent_skill_register`)
- Backing Rust/current owner: `src/server/ops/agent.rs::agent_rpc`; `clay-agent/src/host.ts::skillRegister`

## Lookup metadata

- Stable ID: `agent.skillRegister`
- User-facing name: Register Agent Skill
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `skillRegister`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, skill, registration, progressive-disclosure, js-api]`
