---
id: agent.resumeRun
kind: clay-js-api
js_module: "clay:agent"
js_export: resumeRun
js_facade: runtime/js/agent.js::resumeRun
backing_rust: src/server/agent.rs::AgentHost::run; clay-agent/src/host.ts::runResume
deno_op: op_clay_agent_resume_run
deno_op_path: src/server/ops/agent.rs::op_clay_agent_resume_run
name: resumeRun
user_facing_name: Resume Suspended Agent Run
summary: Deliver the user's approval decision to a suspended durable agent run.
owner: server
phase: Phase 1
visibility: public
permissions: ["agent-host"]
key_bindings: []
custom_properties: []
security: Forwards the user's decision to the daemon, which validates the decision shape and run-state version fail-closed; stale or malformed resumes have no side effects. Surfaces the pending approval to the user when not resuming programmatically. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `agent.resumeRun` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Only resume with a decision the user actually made; automated approval without user intent is forbidden.
lookup_tags: [agent, approval, resume, run, durable, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# resumeRun

## Summary

Deliver the user's approval decision to a suspended durable agent run.

## Description

`resumeRun` is the runtime-backed public API for **Resume Suspended Agent Run**. Coding sessions suspend before gated tool calls (interruptBeforeTool); this API delivers the decision (`approve`/`deny` or a decision batch) together with the expected run-state version. The daemon validates shape and version fail-closed: a stale resume executes nothing. Authority: `user-intent-forwarding`. Runtime path: `server-first-rpc-forwarding`. Resume is a user-decision flow and never runs in editor input, client paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when Clay automation or first-party packages need the documented **Resume Suspended Agent Run** behavior, such as an approval prompt dialog. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { resumeRun } from "clay:agent";

await resumeRun({ sessionId, runId, expectedVersion, decision: "approve" });
```

## Example

```ts
const result = await resumeRun({
  sessionId: "s1",
  runId: "r1",
  expectedVersion: 3,
  decisions: [{ toolCallId: "c1", outcome: "allow_once" }],
});
console.log(result.status);
```

## Options

- `sessionId` (string, required): the session that owns the suspended run.
- `runId` (string, required): the suspended run to resume.
- `expectedVersion` (number, required): optimistic run-state version; mismatch fails closed.
- `decision` (`"approve" | "deny"`, optional): single decision for the pending interruption.
- `decisions` (array, optional): batch of per-tool-call decisions with outcomes (`allow_once`, `allow_for_run`, `deny_once`, `deny_for_run`). Mutually exclusive with `decision`.

## Key bindings

No default key binding is assigned. Users may bind a key to `agent.resumeRun` in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise resolving to `{ sessionId, runId, status }`; a resumed run that suspends again returns `status: "suspended"` with the new `version` and `interruption`. Asynchronous; the daemon continues the run and may take unbounded time.

## Errors

Fails closed with typed errors when the runtime has no attached agent host (`agent.unavailable`), parameters or decision shape are malformed (`agent.invalid_params`), the run or session is unknown, or `expectedVersion` is stale. Malformed resumes never touch checkpoints.

## Permissions and security

Requires: `agent-host`.

Forwards the user's decision to the daemon, which validates the decision shape and run-state version fail-closed; stale or malformed resumes have no side effects. Surfaces the pending approval to the user when not resuming programmatically. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

## Agent guidance

Use `agent.resumeRun` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`. Only resume with a decision the user actually made; automated approval without user intent is forbidden.

## Backing implementation

- JS facade: `runtime/js/agent.js::resumeRun`
- Deno op: `src/server/ops/agent.rs::op_clay_agent_resume_run` (`op_clay_agent_resume_run`)
- Backing Rust/current owner: `src/server/agent.rs::AgentHost::run`; `clay-agent/src/host.ts::runResume`

## Lookup metadata

- Stable ID: `agent.resumeRun`
- User-facing name: Resume Suspended Agent Run
- Kind: `clay-js-api`
- Module/export: `clay:agent` / `resumeRun`
- Default key bindings: none
- Custom properties: none
- Tags: `[agent, approval, resume, run, durable, js-api]`
