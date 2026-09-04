---
id: agent.searchSessions
kind: clay-js-api
js_module: "clay:agent"
js_export: searchSessions
js_facade: runtime/js/agent.js::searchSessions
backing_rust: src/server/agent.rs::AgentHost::run; clay-agent/src/host.ts::sessionSearch
deno_op: op_clay_agent_search_sessions
deno_op_path: src/server/ops/agent.rs::op_clay_agent_search_sessions
name: searchSessions
user_facing_name: Search Agent Sessions
summary: Search past agent sessions within the session's workspace; hits are metadata, never implicit context.
owner: server
phase: Phase 1
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties: []
security: Workspace scoping is server-owned (the daemon filters by the session's stamped workspaceRoot); snippets are redacted daemon-side. Results are transcript metadata for explicit user-facing flows and are never auto-injected into agent context. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.searchSessions` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Never treat search hits as injectable context; content becomes context only when the user explicitly opens or attaches a result.
lookup_tags: [agent, search, sessions, workspace, fts, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# searchSessions

## Summary

Search past agent sessions within the session's workspace; hits are metadata, never implicit context.

## Description

`searchSessions` is the runtime-backed public API for **Search Agent Sessions**. It queries the daemon's full-text session index, always scoped to the workspace stamped on the referenced session (decision 2201). Cross-workspace content is invisible by construction. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Search is a background/help/programmatic query and never runs in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when Clay automation or first-party packages need the documented **Search Agent Sessions** behavior, such as a session-picker search box. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { searchSessions } from "clay:agent";

const { hits } = await searchSessions({ sessionId, query: "flake" });
```

## Example

```ts
const page = await searchSessions({ sessionId: "s1", query: "flake", limit: 20 });
for (const hit of page.hits) {
  console.log(hit.sessionId, hit.leafId, hit.snippet);
}
```

## Options

- `sessionId` (string, required): the live session whose workspace scopes the search.
- `query` (string, optional): full-text query. Omitted queries list recent sessions.
- `limit` (number, optional): maximum hits (server caps the page size).

## Key bindings

No default key binding is assigned. Users may bind a key to `agent.searchSessions` in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise resolving to `{ hits, nextCursor? }`. Each hit carries `sessionId`, `leafId`, `updatedAt`, `label`, `summary`, redacted `snippet`, and `metadata`. Raw tool output is not indexed.

## Errors

Fails closed with typed errors when the runtime has no attached agent host (`agent.unavailable`), parameters are malformed (`agent.invalid_params`), or the referenced session is unknown.

## Permissions and security

Requires: `agent-host`.

Workspace scoping is server-owned (the daemon filters by the session's stamped workspaceRoot); snippets are redacted daemon-side. Results are transcript metadata for explicit user-facing flows and are never auto-injected into agent context. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.searchSessions` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Never treat search hits as injectable context; content becomes context only when the user explicitly opens or attaches a result.

## Backing implementation

- JS facade: `runtime/js/agent.js::searchSessions`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_search_sessions` (`op_clay_agent_search_sessions`)
- Backing Rust/current owner: `src/server/agent.rs::AgentHost::run`; `clay-agent/src/host.ts::sessionSearch`

## Lookup metadata

- Stable ID: `agent.searchSessions`
- User-facing name: Search Agent Sessions
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `searchSessions`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, search, sessions, workspace, fts, js-api]`
