---
id: agent.commandRegister
kind: clay-js-api
js_module: "clay:agent"
js_export: commandRegister
js_facade: runtime/js/agent.js::commandRegister
backing_rust: src/server/ops/agent.rs::agent_registration_rpc; clay-agent/src/host.ts::commandRegister
deno_op: op_clay_agent_command_register
deno_op_path: src/server/ops/agent.rs::op_clay_agent_command_register
name: commandRegister
user_facing_name: Register Agent Command
summary: Register an agent command declaration (slash-surface data) on the daemon's validated command registry; one-shot during package load.
owner: server
phase: Phase 2
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties: []
security: Forwards an inert command declaration (name, optional handler name and description) to the daemon's validated command registry; handlers are host-built-in only and can never inject drivers or execute client code. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.commandRegister` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Commands are data; the daemon's built-in handlers execute them under existing session authority.
lookup_tags: [agent, command, registration, slash-commands, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# commandRegister

## Summary

Register an agent command declaration (slash-surface data) on the daemon's validated command registry; one-shot during package load.

## Description

`commandRegister` is the runtime-backed public API for **Register Agent Command**. It forwards an inert command declaration — a name, an optional host-built-in handler name, and a description — to the clay-agent daemon's `command.register` registry. Commands are pure data on the wire: the daemon's own handlers execute them under existing session authority. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Registration runs once during package load and never in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when a first-party agent package contributes slash commands for its sessions, such as the Coding Agent's `/compact`, `/new`, `/branch`, `/tree`, `/fork`, `/clone`, `/n`, and open-session commands. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { commandRegister } from "clay:agent";

await commandRegister({
  name: "/compact",
  handler: "compact",
  description: "Compact the current session manually.",
});
```

## Example

```ts
// The Coding Agent package registers its slash commands after its profile.
await commandRegister({ name: "/compact", handler: "compact", description: "Compact this session." });
await commandRegister({ name: "/new", handler: "newSession", description: "Start a fresh session with the same profile." });
```

## Options

- `name` (string, required): the daemon command name. Slash names (`"/compact"`) are invocable from prompt text; other names are dispatch-only. Duplicate names fail closed.
- `handler` (string, optional): a host-built-in handler — `compact`, `newSession`, `checkout`, `forkSession`, `cloneSession`, `tree`, `openSession`, `openSessionAsFork`, `startRun`, `startWorkflow`, or `steer`. Omitted handlers are data-only commands (dispatch rejects them).
- `description` (string, optional): short description shown in command discovery.

## Key bindings

No default key binding is assigned. `agent.commandRegister` is a load-time registration call, not a user command.

## Custom properties

None.

## Return and async behavior

Returns a promise resolving to `{ name, registered: true }` when the daemon accepted the declaration. When the daemon is not yet running, the call resolves immediately with `{ queued: true }`: the declaration is held server-side and applied right after the daemon's next initialize handshake — a load entry never spawns the daemon or waits on its boot. In runtimes with no agent host at all, the declaration queues process-global and applies when a host installs. Re-registering an existing command name fails closed (duplicate command).

## Errors

Fails closed with typed errors when the declaration is malformed (`agent.invalid_params`) or the pending-registration queue is full.

## Permissions and security

Requires: `agent-host`.

Forwards an inert command declaration (name, optional handler name and description) to the daemon's validated command registry; handlers are host-built-in only and can never inject drivers or execute client code. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.commandRegister` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Commands are data; the daemon's built-in handlers execute them under existing session authority.

## Backing implementation

- JS facade: `runtime/js/agent.js::commandRegister`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_command_register` (`op_clay_agent_command_register`)
- Backing Rust/current owner: `src/server/ops/agent.rs::agent_registration_rpc`; `clay-agent/src/host.ts::commandRegister`

## Lookup metadata

- Stable ID: `agent.commandRegister`
- User-facing name: Register Agent Command
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `commandRegister`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, command, registration, slash-commands, js-api]`
