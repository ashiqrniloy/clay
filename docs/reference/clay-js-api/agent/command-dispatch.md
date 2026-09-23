---
id: agent.commandDispatch
kind: clay-js-api
js_module: "clay:agent"
js_export: commandDispatch
js_facade: runtime/js/agent.js::commandDispatch
backing_rust: src/server/ops/agent.rs::agent_rpc; clay-agent/src/host.ts::commandDispatch
deno_op: op_clay_agent_command_dispatch
deno_op_path: src/server/ops/agent.rs::op_clay_agent_command_dispatch
name: commandDispatch
user_facing_name: Dispatch Agent Command
summary: Dispatch a registered daemon command (slash surface) against daemon session state; live execution only.
owner: server
phase: Phase 2
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties:
  - name: name
    type: string
    default: required
    description: the registered daemon command name (for example `"/compact"`).
  - name: sessionId
    type: string
    default: optional
    description: the session to act on. Session-scoped handlers (`compact`, `newSession`, `checkout`, `forkSession`, `cloneSession`, `tree`) require it and fail closed without it.
  - name: args
    type: object
    default: optional
    description: handler arguments, e.g. `{ "entryId": "..." }` for `/branch`, `/fork`, `/clone`, or `{ "sessionId": "..." }` for the open-session commands.
security: Dispatches a registered daemon command (slash surface) against daemon session state; handlers call daemon session RPCs only and cannot elevate acceptance policy or spawn processes. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.commandDispatch` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Dispatch is live-only: an unavailable daemon is a typed failure, never a deferred execution.
lookup_tags: [agent, command, dispatch, slash-commands, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# commandDispatch

## Summary

Dispatch a registered daemon command (slash surface) against daemon session state; live execution only.

## Description

`commandDispatch` is the runtime-backed public API for **Dispatch Agent Command**. It forwards a command invocation — name, optional session id, and optional JSON arguments — to the clay-agent daemon's `command.dispatch`, which executes the registered command's host-built-in handler against the session. Unlike registration, dispatch is live-only: commands act on daemon session state, so an unavailable daemon is a typed failure rather than a queued execution. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Runs only on user-initiated command execution, never in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when a surface (for example the composer's `/` completion, or the tree UI) executes a registered slash command such as `/compact`, `/branch`, or `/tree`. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { commandDispatch } from "clay:agent";

const result = await commandDispatch({ name: "/compact", sessionId });
```

## Example

```ts
// Slash token arguments ride as a JSON object.
const tree = await commandDispatch({ name: "/tree", sessionId });
const forked = await commandDispatch({
  name: "/fork",
  sessionId,
  args: { entryId },
});
```

## Options

- `name` (string, required): the registered daemon command name (for example `"/compact"`).
- `sessionId` (string, optional): the session to act on. Session-scoped handlers (`compact`, `newSession`, `checkout`, `forkSession`, `cloneSession`, `tree`) require it and fail closed without it.
- `args` (object, optional): handler arguments, e.g. `{ "entryId": "..." }` for `/branch`, `/fork`, `/clone`, or `{ "sessionId": "..." }` for the open-session commands.

## Key bindings

No default key binding is assigned. `agent.commandDispatch` runs on user-initiated command execution.

## Custom properties

None.

## Return and async behavior

Returns a promise resolving to `{ name, value }` where `value` is the handler's daemon RPC result (for example the compaction summary, the new session id, the checkout leaf id, or the branch tree summary). Session-scoped handlers without a session, unknown command names, and malformed arguments fail closed with typed errors; nothing is queued.

## Errors

Fails closed with typed errors when the invocation is malformed (`agent.invalid_params`), the command is unknown, the handler requires an active session, or the daemon is unavailable (`agent.rpc_failed`).

## Permissions and security

Requires: `agent-host`.

Dispatches a registered daemon command (slash surface) against daemon session state; handlers call daemon session RPCs only and cannot elevate acceptance policy or spawn processes. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.commandDispatch` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Dispatch is live-only: an unavailable daemon is a typed failure, never a deferred execution.

## Backing implementation

- JS facade: `runtime/js/agent.js::commandDispatch`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_command_dispatch` (`op_clay_agent_command_dispatch`)
- Backing Rust/current owner: `src/server/ops/agent.rs::agent_rpc`; `clay-agent/src/host.ts::commandDispatch`

## Lookup metadata

- Stable ID: `agent.commandDispatch`
- User-facing name: Dispatch Agent Command
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `commandDispatch`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, command, dispatch, slash-commands, js-api]`
