---
id: agent.profileRegister
kind: clay-js-api
js_module: "clay:agent"
js_export: profileRegister
js_facade: runtime/js/agent.js::profileRegister
backing_rust: src/server/ops/agent.rs::agent_rpc; clay-agent/src/host.ts::profileRegister
deno_op: op_clay_agent_profile_register
deno_op_path: src/server/ops/agent.rs::op_clay_agent_profile_register
name: profileRegister
user_facing_name: Register Agent Profile
summary: Register an agent profile (name, prompt layer, tools, skills) on the daemon's validated registry; one-shot during package load.
owner: server
phase: Phase 2
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties: []
security: Forwards an inert profile declaration to the daemon's validated registry; re-registering a profile name replaces the definition, and unknown tool or skill ids fail closed at session start, before any provider turn. Tool execution authority stays governed by the acceptance policy, not by registration. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.profileRegister` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Register referenced skills first (`agent.skillRegister`); a profile that names an unregistered skill fails closed before any provider turn.
lookup_tags: [agent, profile, registration, tools, skills, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# profileRegister

## Summary

Register an agent profile (name, prompt layer, tools, skills) on the daemon's validated registry; one-shot during package load.

## Description

`profileRegister` is the runtime-backed public API for **Register Agent Profile**. It forwards an inert profile declaration — name, description, instructions (the coding system prompt layer), tool names, and skill names — to the clay-agent daemon's `agentProfile.register` registry. Sessions select a profile by name at `session.new` time; the daemon resolves the declaration's tools and skills fail-closed at session start, before any provider turn. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Registration runs once during package load and never in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when a first-party agent package defines a replaceable agent surface, such as the Coding Agent's coding profile. Do not use it to re-register core profiles, and do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { profileRegister } from "clay:agent";

await profileRegister({
  name: "coding",
  description: "Coding agent",
  instructions: "Coding system prompt layer.",
  tools: ["shell", "read", "write", "edit", "repo_list", "repo_search", "glob", "delete", "move", "ask_user_decision"],
  skills: ["coding-agent.createPlan"],
});
```

## Example

```ts
// Register the skill before the profile that references it.
await skillRegister({ name: "coding-agent.createPlan", description: "Plan files" });
await profileRegister({
  name: "coding",
  description: "Coding agent",
  instructions: "Coding system prompt layer.",
  tools: ["read", "edit"],
  skills: ["coding-agent.createPlan"],
});
```

## Options

- `name` (string, required): the profile name; sessions select it by name. Duplicate names fail closed.
- `description` (string, optional): short profile description shown in pickers.
- `instructions` (string, optional): the profile's system prompt layer.
- `tools` (string[], optional): tool names resolved against the daemon tool registry; unknown names fail closed at session start.
- `skills` (string[], optional): skill names resolved against the daemon skill registry; register skills before the profile that references them.

## Key bindings

No default key binding is assigned. `agent.profileRegister` is a load-time registration call, not a user command.

## Custom properties

None.

## Return and async behavior

Returns a promise resolving to `{ name, registered: true, ... }` when the daemon accepted the declaration. When the daemon is not yet running, the call resolves immediately with `{ queued: true }`: the declaration is held server-side and applied right after the daemon's next initialize handshake, before any later session command — a load entry never spawns the daemon or waits on its boot. In runtimes with no agent host at all, the declaration queues process-global and applies when a host installs. Re-registering an existing profile name replaces the previous definition (kernel default duplicate policy); sessions resolve the profile at start, so the latest definition wins.

## Errors

Fails closed with typed errors when the declaration is malformed (`agent.invalid_params`) or the pending-registration queue is full. Unknown skill/tool ids fail closed at session start, before any provider turn.

## Permissions and security

Requires: `agent-host`.

Forwards an inert profile declaration to the daemon's validated registry; re-registering a profile name replaces the definition, and unknown tool or skill ids fail closed at session start, before any provider turn. Tool execution authority stays governed by the acceptance policy, not by registration. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.profileRegister` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Register referenced skills first (`agent.skillRegister`); a profile that names an unregistered skill fails closed before any provider turn.

## Backing implementation

- JS facade: `runtime/js/agent.js::profileRegister`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_profile_register` (`op_clay_agent_profile_register`)
- Backing Rust/current owner: `src/server/ops/agent.rs::agent_rpc`; `clay-agent/src/host.ts::profileRegister`

## Lookup metadata

- Stable ID: `agent.profileRegister`
- User-facing name: Register Agent Profile
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `profileRegister`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, profile, registration, tools, skills, js-api]`
