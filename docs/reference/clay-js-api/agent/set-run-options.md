---
id: agent.setRunOptions
kind: clay-js-api
js_module: "clay:agent"
js_export: setRunOptions
js_facade: runtime/js/agent.js::setRunOptions
backing_rust: src/server/ops/agent.rs::agent_registration_rpc; clay-agent/src/host.ts::runSetOptions
deno_op: op_clay_agent_run_set_options
deno_op_path: src/server/ops/agent.rs::op_clay_agent_run_set_options
name: setRunOptions
user_facing_name: Set Agent Run Options
summary: Set coding-run policy caps and the default compaction strategy for the clay-agent daemon.
owner: server
phase: Phase 2
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties:
  - name: maxInputTokens
    type: number
    default: null
    description: Cumulative billed input-token cap. Positive safe integer, or null to disable (default).
  - name: maxOutputTokens
    type: number
    default: null
    description: Cumulative billed output-token cap. Positive safe integer, or null to disable (default).
  - name: maxTurns
    type: number
    default: null
    description: Turn cap. Positive safe integer, or null to disable (default).
  - name: maxToolRounds
    type: number
    default: null
    description: Tool-round cap. Positive safe integer, or null to disable (default).
  - name: maxToolCalls
    type: number
    default: null
    description: Tool-call cap. Positive safe integer, or null to disable (default).
  - name: maxWallTimeMs
    type: number
    default: null
    description: Wall-clock cap in milliseconds. Positive safe integer, or null to disable (default).
  - name: compactAfterTokens
    type: number
    default: 800000
    description: Auto-compact ceiling in tokens for the next created coding sessions with a declared context window. Fires at or above this estimate in addition to the attention compiler's compactRatio gate. Distinct from agent.compact compactAfterTokens (OM threshold, decision 2158 default 80000).
  - name: compaction
    type: string
    default: llm
    description: Default compaction when agent.compact or /compact omit strategy. One of default, llm, om.
security: Forwards a configuration intent (policy caps and a named compaction strategy) to the daemon. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority to package JavaScript.
agent_guidance: Use `agent.setRunOptions` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. The call is queued while the daemon is down and applies after the daemon initializes, so init.js never blocks on daemon startup.
lookup_tags: [agent, run, limits, tokens, compaction, options, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# setRunOptions

## Summary

Set coding-run policy caps and the default compaction strategy for the clay-agent daemon.

## Description

`setRunOptions` is the runtime-backed public API for **Set Agent Run Options**. It forwards a partial update to the clay-agent daemon's `run.setOptions`. Absent fields keep the current daemon values. `null` on a Prism policy axis disables that cap (Prism 0.7.0). Defaults when nothing has been set: all policy axes (`maxInputTokens`, `maxOutputTokens`, `maxTurns`, `maxToolRounds`, `maxToolCalls`, `maxWallTimeMs`) `null` (unbounded), `compactAfterTokens` 800000, `compaction` `llm`. The only hard caps left are Prism's per-frame request/response bytes (64 MiB). Token fields are cumulative billed `usage.inputTokens` / `outputTokens` for the run, not the model context window. `maxProviderAttempts` is omitted so Prism lifts it to at least `maxTurns`. `maxTotalTokens` is derived (`null` if either token axis is `null`, else the sum) and is not independently configurable. `compactAfterTokens` is the absolute ceiling of the automatic compaction gate: the daemon arms auto-compaction on coding sessions whose resolved model declares a context window, and the gate fires when the assembled input estimate reaches this ceiling, the attention compiler's compactRatio (0.9 of the resolved input cap), or after two consecutive `truncated` attention turns. Automatic compaction runs once per prompt, before provider turns, with Prism's local deterministic strategy (no provider call, so an outage cannot fail a run at assembly); explicit `session.compact` / `/compact` still use `compaction`. It is not the OM `agent.compact` threshold (decision 2158, default 80000). `compaction` is the strategy used when `agent.compact` or `/compact` omit `strategy`. While the daemon is unavailable the call queues server-side and applies after the daemon's initialize handshake. Authority: `user-intent-forwarding-to-daemon-rpc`. Runtime path: `server-first-rpc-forwarding`. One-shot configuration; never runs in editor input, client paint/layout, or ordinary edit acknowledgement hot paths. Trusted-only (`clay:agent`): call from `~/.clay/init.js` or first-party/trusted configuration modules, not third-party package JS.

New sessions pick up caps at `createAgent`. Live sessions keep the limits they were created with until rebuilt (model switch / resume recreate).

## When to use

Use this API from workspace configuration (`init.js` or a loaded first-party/third-party config module) to tighten or disable coding-run policy caps or to change the default compact strategy. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { setRunOptions } from "clay:agent";

const result = await setRunOptions({
  maxInputTokens: null,
  maxOutputTokens: null,
  maxTurns: 64,
  maxWallTimeMs: 1_800_000,
  compaction: "llm",
});
```

## Example

```js
// init.js — after loadPackage("@clay/coding-agent")
import { setRunOptions } from "clay:agent";

await setRunOptions({
  maxInputTokens: 5_000_000,
  maxTurns: 128,
  compaction: "llm",
});
```

## Options

- `maxInputTokens` (number or null, optional): positive safe integer, or `null` to disable. Default `null`.
- `maxOutputTokens` (number or null, optional): positive safe integer, or `null` to disable. Default `null`.
- `maxTurns` (number or null, optional): positive safe integer, or `null` to disable. Default `null`.
- `maxToolRounds` (number or null, optional): positive safe integer, or `null` to disable. Default `null`.
- `maxToolCalls` (number or null, optional): positive safe integer, or `null` to disable. Default `null`.
- `maxWallTimeMs` (number or null, optional): positive safe integer milliseconds, or `null` to disable. Default `null`.
- `compactAfterTokens` (number, optional): positive safe integer. Default 800000. Absolute ceiling of the automatic compaction gate for coding sessions with a declared context window (the gate also fires at the attention compiler's compactRatio and after two truncated turns).
- `compaction` (string, optional): `default`, `llm`, or `om`. Default `llm`.

At least one field is required. `maxTotalTokens` is derived as above and is not independently configurable.

## Key bindings

No default key binding is assigned. `agent.setRunOptions` runs as one-shot workspace configuration.

## Custom properties

- `maxInputTokens` (`number`, default `null`): Cumulative billed input-token cap. Positive safe integer, or null to disable.
- `maxOutputTokens` (`number`, default `null`): Cumulative billed output-token cap. Positive safe integer, or null to disable.
- `maxTurns` (`number`, default `64`): Fork-bomb turn fence. Positive safe integer, or null to disable.
- `maxToolRounds` (`number`, default `64`): Fork-bomb tool-round fence. Positive safe integer, or null to disable.
- `maxToolCalls` (`number`, default `256`): Fork-bomb tool-call fence. Positive safe integer, or null to disable.
- `maxWallTimeMs` (`number`, default `1800000`): Wall-clock fence in milliseconds. Positive safe integer, or null to disable.
- `compactAfterTokens` (`number`, default `800000`): Auto-compact ceiling in tokens. Fires at or above this estimate for the next created coding sessions with a declared context window, in addition to the attention compiler's compactRatio (0.9) gate and the two-truncated-turns signal. Distinct from `agent.compact` `compactAfterTokens` (OM threshold, decision 2158 default 80000).
- `compaction` (`string`, default `llm`): Default compaction when `agent.compact` or `/compact` omit strategy. One of `default`, `llm`, `om`.

## Return and async behavior

Returns a promise resolving to `{ maxInputTokens, maxOutputTokens, maxTurns, maxToolRounds, maxToolCalls, maxWallTimeMs, compactAfterTokens, compaction, queued? }` — the effective daemon values after the patch. `queued: true` means the daemon was down and the options apply after its initialize handshake. Malformed arguments fail closed with typed errors before reaching the daemon.

## Errors

Fails closed with typed errors when the configuration is malformed (`agent.invalid_params`) or the daemon reports a failure (`agent.rpc_failed`). Queued application while the daemon is down never fails the load entry.

## Permissions and security

Requires: `agent-host`.

Forwards a configuration intent (policy caps and a named compaction strategy) to the daemon. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority to package JavaScript.

## Agent guidance

Use `agent.setRunOptions` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. The call is queued while the daemon is down and applies after the daemon initializes, so init.js never blocks on daemon startup.

## Backing implementation

## Lookup metadata

- Stable ID: `agent.setRunOptions`
- User-facing name: Set Agent Run Options
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `setRunOptions`
- Default key bindings: none
- Custom properties: `maxInputTokens`, `maxOutputTokens`, `maxTurns`, `maxToolRounds`, `maxToolCalls`, `maxWallTimeMs`, `compactAfterTokens`, `compaction`
- Tags: `[agent, run, limits, tokens, compaction, options, js-api]`
