---
id: language-server.startLanguageServerSession
kind: clay-js-api
js_module: "clay:language-server"
js_export: startLanguageServerSession
js_facade: runtime/js/language-server.js::startLanguageServerSession
backing_rust: src/server/language_server.rs::LanguageServerProcessService
deno_op: op_clay_language_server_start_session
deno_op_path: src/server/ops/language_server.rs::op_clay_language_server_start_session
name: startLanguageServerSession
user_facing_name: Start Language Server Session
summary: Spawn an authorized language-server child process for one contribution + workspace root and return an opaque bounded session handle.
owner: server
phase: Phase 18.20
visibility: public
permissions: ["language-server"]
key_bindings: []
custom_properties:
  - name: package
    type: string
    default: required
    description: Package name that owns the contribution and holds an active grant.
  - name: contribution
    type: string
    default: required
    description: Contribution id matching an active authorizeLanguageServer grant.
  - name: workspaceRootId
    type: number|string
    default: required
    description: One approved directory workspace-root id from the grant's workspaceRootIds.
hot_path_policy: Async background-thread I/O only; spawn and message read/write run on a dedicated clay-language-server thread with its own tokio runtime. Never blocks the Deno worker thread, typing, layout, or paint hot paths.
security: Requires prior authorizeLanguageServer grant; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript; revalidates grant identity on every operation; spawn uses fixed contribution executable/argv/env_clear+declared inherits; session exposes only bounded send/read/stop; kill_on_drop safety net; trusted subprocess authority not an OS sandbox; LSP framing deferred to Phase 18.21.
agent_guidance: Use only with an existing grant. Never expose raw process handles, stdio, or shell execution through the session object.
lookup_tags: [language-server, session, process, phase18.20, runtime-backed, deny-by-default]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# startLanguageServerSession

## Summary

`startLanguageServerSession` spawns an authorized language-server child for one fixed contribution descriptor + approved workspace root. Returns an opaque `LanguageServerSession` with bounded `send`, `read`, and `stop` methods.

## Description

Requires a prior `authorizeLanguageServer` grant with matching `package`, `contribution`, and `workspaceRootIds`. On every operation the grant is revalidated against `PackageService` to catch revocation. Spawn parameters come from the fixed validated `LanguageServerContributionDescriptor`: canonical executable, literal argv, declared inheritance environment names, and approved cwd root. `env_clear` removes all ambient environment; only explicitly listed inherited env var names survive.

The session exposes only bounded UTF-8 message I/O under `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES` (write) and `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES` / `LANGUAGE_SERVER_READ_TIMEOUT_MS` (read). Stderr is accumulated up to `LANGUAGE_SERVER_STDERR_BUDGET_BYTES` and surfaced in sanitized error diagnostics. `LANGUAGE_SERVER_MAX_SESSIONS` caps concurrent sessions per runtime generation.

Child lifetime: `kill_on_drop` reaps the process when the session is dropped. Revocation, package update, contribution change, or workspace root removal terminates all associated sessions. The background thread (`clay-language-server`) owns all child I/O; the Deno worker thread communicates via bounded mpsc channels and oneshot responses.

LSP `Content-Length` framing, initialize/capabilities negotiation, document sync, and `$/cancelRequest` are Phase 18.21 package adapters layered on the opaque session — Clay core stays analyzer-neutral and LSP-free.

## When to use

Use from package load entry after `authorizeLanguageServer` has been granted. Call only with matching package, contribution, and an approved workspace root id. Never expose the session object to other packages or to user configuration.

## JavaScript usage

```ts
import { startLanguageServerSession } from "clay:language-server";
```

## Example

```ts
const session = await startLanguageServerSession({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootId: 1,
});

await session.send(jsonRpcInitMessage);
const response = await session.read(65536, 5000);
await session.stop();
```

## Options

- `package`: package name that owns the contribution and holds an active grant.
- `contribution`: contribution id matching an active `authorizeLanguageServer` grant.
- `workspaceRootId`: one approved directory workspace-root id from the grant.

## Key bindings

No key bindings are registered by this API.

## Custom properties

- `package`
- `contribution`
- `workspaceRootId`

## Return and async behavior

Returns a `LanguageServerSession` object with `sessionId` (number), `send(message: string): Promise<void>`, `read(maxBytes: number, timeoutMs: number): Promise<string>`, and `stop(): Promise<void>`. Always awaited (`async: true`).

## Errors

- `language_server.unauthorized` — no grant, mismatched contribution, or revoked grant.
- `language_server.executable_not_found` — canonical executable no longer resolves.
- `language_server.session_already_running` — same package contribution already has a live session for this workspace root.
- `language_server.too_many_sessions` — `LANGUAGE_SERVER_MAX_SESSIONS` exceeded.
- `language_server.spawn_failed` — child process could not start.
- `language_server.payload_too_large` — message exceeds `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`.
- `language_server.timeout` — read timed out.
- `language_server.child_exited` — child process exited unexpectedly.
- `language_server.unknown_session` — session id not recognized (reaped/revoked).

## Permissions and security

Requires: `language-server`. Only available after an `authorizeLanguageServer` grant. server-side validation re-checks grant identity on every operation. does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript. Spawn uses fixed validated contribution metadata (no shell strings, no runtime-chosen executable/argv/cwd/env). Session operations are bounded by compiled server budgets. Child is trusted same-user subprocess authority, not an OS sandbox. See `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Agent guidance

Use only with an existing grant and documented contribution. Never expose raw process handles, stdio, or shell execution through the session object. LSP framing is package-side adapter work, not core API.

## Backing implementation

- Facade: `runtime/js/language-server.js::startLanguageServerSession`
- Op: `src/server/ops/language_server.rs::op_clay_language_server_start_session`
- Rust: `src/server/language_server.rs::LanguageServerProcessService`

## Lookup metadata

Tags: language-server, session, process, phase18.20, runtime-backed, deny-by-default.
