---
id: agent.compact
kind: clay-js-api
js_module: "clay:agent"
js_export: compact
js_facade: runtime/js/agent.js::compact
backing_rust: src/server/agent.rs::AgentHost::run; clay-agent/src/host.ts::sessionCompact
deno_op: op_clay_agent_compact_session
deno_op_path: src/server/ops/agent.rs::op_clay_agent_compact_session
name: compact
user_facing_name: Compact Agent Session
summary: Request a mid-session compaction of the agent conversation history.
owner: server
phase: Phase 1
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties:
  - name: compactAfterTokens
    type: number
    default: 80000
    description: Observational-memory auto-compaction threshold override in tokens (decision 2158 default 80000; Prism package default 81000). Only meaningful for OM-attached sessions; also governs post-run auto-compaction.
security: Forwards user intent to the clay-agent daemon through a validated op; the daemon applies the secret redactor to the compaction run. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.compact` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Compaction runs mid-session and fails closed while another run is active.
lookup_tags: [agent, compaction, session, history, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# compact

## Summary

Request a mid-session compaction of the agent conversation history.

## Description

`compact` is the runtime-backed public API for **Compact Agent Session**. It forwards a compaction request for one live agent session to the clay-agent daemon, which resolves the named strategy (or `agent.setRunOptions` `compaction`, default `llm`) and appends a compaction entry to the persisted session. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Compaction is a background/programmatic control and never runs in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when Clay automation or first-party packages need the documented **Compact Agent Session** behavior, such as a `/compact` command implementation. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { compact } from "clay:agent";

await compact({ sessionId, strategy: "default" });
// OM-attached sessions can override the auto-compaction threshold (2158):
await compact({ sessionId, compactAfterTokens: 80_000 });
```

## Example

```ts
const result = await compact({ sessionId: "s1", strategy: "llm" });
console.log(result.entryId, result.strategy);
```

## Options

- `sessionId` (string, required): the live agent session to compact.
- `strategy` (string, optional): `default`, `llm`, or `om`. Defaults to `agent.setRunOptions` `compaction` (default `llm`).
- `compactAfterTokens` (number, optional): observational-memory auto-compaction threshold override in tokens. Positive integer; default 80000 (decision 2158). Persisted for the session and also applied by post-run auto-compaction; ignored for non-OM sessions.

## Key bindings

No default key binding is assigned. Users may bind a key to `agent.compact` in `~/.clay/init.js`.

## Custom properties

- `compactAfterTokens` (`number`, default `80000`): Observational-memory auto-compaction threshold override in tokens (decision 2158 default 80000; Prism package default 81000). Only meaningful for OM-attached sessions; also governs post-run auto-compaction.

## Return and async behavior

Returns a promise resolving to `{ sessionId, summary?, entryId?, strategy }`. The call is asynchronous; the daemon may take seconds because compaction can invoke the summarizer model. If a run is active in the session the request fails closed rather than queueing.

## Errors

Fails closed with typed errors when the runtime has no attached agent host (`agent.unavailable`), parameters are malformed (`agent.invalid_params`), the session is unknown, or a run is active. Daemon-side failures surface as `agent.rpc_failed` with a redacted message.

## Permissions and security

Requires: `agent-host`.

Forwards user intent to the clay-agent daemon through a validated op; the daemon applies the secret redactor to the compaction run. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.compact` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Compaction runs mid-session and fails closed while another run is active.

## Backing implementation

- JS facade: `runtime/js/agent.js::compact`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_compact_session` (`op_clay_agent_compact_session`)
- Backing Rust/current owner: `src/server/agent.rs::AgentHost::run`; `clay-agent/src/host.ts::sessionCompact`

## Lookup metadata

- Stable ID: `agent.compact`
- User-facing name: Compact Agent Session
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `compact`
- Default key bindings: none
- Custom properties: `compactAfterTokens` (number, default 80000)
- Tags: `[agent, compaction, session, history, js-api]`
