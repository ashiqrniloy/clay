---
id: agent.knowledgeSetOptions
kind: clay-js-api
js_module: "clay:agent"
js_export: knowledgeSetOptions
js_facade: runtime/js/agent.js::knowledgeSetOptions
backing_rust: src/server/ops/agent.rs::agent_registration_rpc; clay-agent/src/host.ts::knowledgeSetOptions
deno_op: op_clay_agent_knowledge_set_options
deno_op_path: src/server/ops/agent.rs::op_clay_agent_knowledge_set_options
name: knowledgeSetOptions
user_facing_name: Set Agent Knowledge Options
summary: Opt a workspace in (or out) of the opt-in knowledge bases (wiki, graft); the daemon loads or disposes the kernel extensions.
owner: server
phase: Phase 2
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties: []
security: Forwards a configuration intent (workspace root, opt-in flags, and an optional graft deep-build model identity) to the daemon, which owns activation; writes stay inside the workspace `.wiki/` tree, the optional qmd binary is never spawned unless explicitly configured, and a graft deep-build key is read from the credential vault unless the trusted caller passes one inline (it joins the redactor and reaches the child process only in its environment, never argv). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority to package JavaScript.
agent_guidance: Use `agent.knowledgeSetOptions` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. The call is queued while the daemon is down and applies after the daemon initializes, so load entries never block on daemon startup.
lookup_tags: [agent, knowledge, wiki, graft, prism-memory, options, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# knowledgeSetOptions

## Summary

Opt a workspace in (or out) of the opt-in knowledge bases (wiki, graft); the daemon loads or disposes the kernel extensions.

## Description

`knowledgeSetOptions` is the runtime-backed public API for **Set Agent Knowledge Options**. It forwards `{ workspaceRoot, wiki?, graft?, graftMode?, graftCliPath?, graftDeepModel?, qmdPath? }` to the clay-agent daemon's `knowledge.setOptions`. With `wiki: true` the daemon loads `@arnilo/prism-memory/wiki` via `kernel.load`, which registers the `/wiki-init`, `/wiki-refresh`, and `/wiki-lint` commands, the `wiki_search`/`wiki_read_page`/`wiki_record_insight`/`wiki_ingest` tools, and the `wiki-searcher`/`wiki-maintainer` skills (progressive disclosure through `load_skill`). With `wiki: false` the wiki extension is disposed — nothing loads, no residue. `graft: true` loads the `@arnilo/prism-memory/graft` extension: the `graft_ask`/`graft_grep`/`graft_callers`/`graft_skeleton`/`graft_map`/`graft_blast` pull tools, the `/graft`-family commands, and the `graft` skill. Graft CLI resolution fails closed — an absent CLI (no `graftCliPath`, no host package root, no `@nanonets/graft` peer) leaves the option off with tools hidden and the agent unperturbed. `graftDeepModel` (requires `graft: true`) turns on `/graft-build-deep` by giving the graft child the provider/model/base URL it needs; the API key comes from the stored credential for that provider unless `apiKey` is passed inline, and it reaches the child as `GRAFT_API_KEY` in the environment, never on argv. `graft: false` disposes it. Each flag defaults to off; absent flags leave that capability untouched. Disabled is the default for both. While the daemon is unavailable the call queues server-side and applies after the daemon's initialize handshake. Authority: `user-intent-forwarding-to-daemon-rpc`. Runtime path: `server-first-rpc-forwarding`. One-shot configuration; never runs in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API from workspace configuration (`init.js`) to enable or disable the knowledge bases for a workspace. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { knowledgeSetOptions } from "clay:agent";

// Enable the wiki for a workspace (queued while the daemon is down).
const result = await knowledgeSetOptions({
  workspaceRoot: "/home/me/projects/clay",
  wiki: true,
});
```

## Example

```js
// init.js — enable the wiki for the active workspace
await agent.knowledgeSetOptions({
  workspaceRoot: "/home/me/projects/clay",
  wiki: true, // false disables and leaves no residue
});

// Enable the graft context graph with a host-owned CLI path
await agent.knowledgeSetOptions({
  workspaceRoot: "/home/me/projects/clay",
  graft: true,
  graftCliPath: "/usr/local/bin/graft",
});

// Let /graft-build-deep run: graft's own LLM pass gets the model identity,
// the key comes from the stored `anthropic` credential (never from init.js)
await agent.knowledgeSetOptions({
  workspaceRoot: "/home/me/projects/clay",
  graft: true,
  graftDeepModel: { provider: "anthropic", model: "claude-sonnet-4-5" },
});
```

## Options

- `workspaceRoot` (string, required): absolute workspace root the knowledge base binds to.
- `wiki` (boolean, optional): `true` loads the wiki kernel extension; `false` disposes it. Absent = no change (at least one of `wiki`/`graft` is required).
- `graft` (boolean, optional): `true` loads the graft kernel extension; `false` disposes it. Fails closed when the graft CLI does not resolve.
- `graftMode` (string, optional): graft extension mode — `pull` (tools + commands + skill, default), `push` (retrieval pack + first-turn orientation + edit blast radius), or `both`.
- `graftCliPath` (string, optional): explicit path to a `graft` CLI entry (host-owned).
- `graftDeepModel` (object, optional): explicit model for `/graft-build-deep`; requires `graft: true`.
  - `provider` (string, required): graft's own provider id — `openai`, `anthropic`, `litellm`, or `orcarouter`. This is not a Clay provider id.
  - `model` (string, required): model id passed to graft as `GRAFT_MODEL`.
  - `apiKey` (string, optional): inline key for providers with no stored credential. Omitted, the daemon reads the stored credential for `provider` (the same secret the agent picker writes), so configuration files never carry the key. An inline key joins the redactor set.
  - `baseUrl` (string, optional): custom endpoint (`GRAFT_BASE_URL`).
- `qmdPath` (string, optional): explicit path to a `qmd` binary for wiki hybrid search. Host-owned and deny-by-default: without it, only the catalog fallback serves `wiki_search`.

One binding per capability per daemon: enabling for a second workspace disposes the first, and a changed `graftDeepModel` rebinds the graft extension in place (the child environment is resolved once at load). Activation affects new coding sessions; live sessions pick the tools up on their next session rebuild. Knowledge-base content is untrusted input to the agent — it never reaches tool-authoritative state without validation, and graft graph output is labeled agent-aid, not authority. The graft CLI runs in a host-owned child process (bounded time and stdout); package JavaScript never spawns it. The deep-build key is never written to argv or logs: it reaches the child as `GRAFT_API_KEY` in its environment, and the daemon fails closed (`agent.invalid_params`) when neither an inline `apiKey` nor a stored credential for `provider` exists — a half-configured deep build never binds.

## Key bindings

No default key binding is assigned. `agent.knowledgeSetOptions` runs as one-shot workspace configuration.

## Custom properties

None.

## Return and async behavior

Returns a promise resolving to `{ workspaceRoot, wiki?, graft?, graftDeepModel?, queued? }` — capability keys appear when their flag was set; `graft: false` also means the CLI did not resolve, and `graftDeepModel` echoes only the provider/model identity (never the key). `queued: true` means the daemon was down and the options apply after its initialize handshake. Malformed arguments (missing/non-empty `workspaceRoot`, non-boolean flags, no `wiki`/`graft` flag at all, a malformed `graftDeepModel`, a deep model without `graft: true`, or a deep model whose key resolves nowhere) fail closed with typed errors before reaching the daemon; daemon-side activation failures disable the option (fail closed), never half-bind it. An unresolvable graft CLI is a `graft: false` result, not an error.

## Errors

Fails closed with typed errors when the configuration is malformed (`agent.invalid_params`) or the daemon reports an activation failure (`agent.rpc_failed`). Queued application while the daemon is down never fails the load entry.

## Permissions and security

Requires: `agent-host`.

Forwards a configuration intent (workspace root, opt-in flags, and an optional graft deep-build model identity) to the daemon, which owns activation; writes stay inside the workspace `.wiki/` tree, the optional qmd binary is never spawned unless explicitly configured, and a graft deep-build key is read from the credential vault unless the trusted caller passes one inline (it joins the redactor and reaches the child process only in its environment, never argv). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority to package JavaScript.

## Agent guidance

Use `agent.knowledgeSetOptions` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. The call is queued while the daemon is down and applies after the daemon initializes, so load entries never block on daemon startup.

## Backing implementation

## Lookup metadata

- Stable ID: `agent.knowledgeSetOptions`
- User-facing name: Set Agent Knowledge Options
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `knowledgeSetOptions`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, knowledge, wiki, prism-memory, options, js-api]`
