# Language Server Process Service

## Source

- `src/packages/permissions.rs`
- `src/packages/record.rs`
- `src/packages/authorization.rs`
- `src/packages/service.rs`
- `src/server/language_server.rs`
- `src/server/ops/language_server.rs`
- `src/server/ops/mod.rs`
- `runtime/js/language-server.js`
- `src/server/js_runtime.rs`
- `src/perf/budgets.rs`
- `tests/language_server_authority.rs`
- `tests/package_loading.rs`
- Approved decision: `decision-logs/2026-07-14-2023-language-server-package-authority.md`
- Public APIs: [`authorizeLanguageServer`](../../reference/clay-js-api/language-server/authorize-language-server.md), [`startLanguageServerSession`](../../reference/clay-js-api/language-server/start-language-server-session.md)

## Overview

`LanguageServerProcessService` is Clay's one host-owned process boundary for package-declared language servers. It exposes opaque bounded send/read/stop sessions, never raw `tokio::process::Child`, stdio handles, PID controls, shell strings, or runtime-selected executables. Package load and bundled trust do not grant this authority.

This is trusted same-user subprocess authority, not an operating-system sandbox. Workspace roots and cwd constrain Clay's approval, launch, and audit identity; the child can still use the host user's OS permissions to read other files, access the network, or spawn processes.

## Responsibilities

- Parse and validate fixed `clay.contributions.languageServers` descriptors.
- Record explicit, contribution/root/provenance-bound grants before package load.
- Seal authorization before package `loadEntry` executes.
- Revalidate grant identity on every session operation.
- Spawn directly with fixed executable/argv, cleared environment, approved cwd, and piped stdio.
- Bound session count, message size, stderr retention, and read timeout.
- Kill/reap sessions on stop, package withdrawal, revocation, reload, runtime replacement, channel shutdown, or child failure.
- Return typed sanitized errors without leaking document text, environment values, or unbounded child output.

## Authority and Grant Flow

A package requests `language-server` and declares one or more package-prefixed `LanguageServerContributionDescriptor` values containing only `id`, `executable`, fixed `args`, and explicit `inheritEnvironment` names. Descriptor validation caps counts and string sizes, rejects controls and duplicate environment names, and requires a contribution when the permission is requested.

Configuration must grant authority before loading package code:

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/lsp-rust");
```

Grant creation resolves and canonicalizes the executable, validates directory workspace roots, and binds package name/version/source/API prefix, contribution ID/fingerprint, canonical executable, fixed argv/environment declaration, workspace-root IDs, and approver. The authorization gate is open only while evaluating the configuration root. `loadPackage` seals it before package enable/import, so loaded package code cannot self-authorize.

Bundled `NativeTrust` explicitly filters out `language-server`. Enablement re-resolves the executable and verifies the exact current grant; package/source/version/contribution/executable/root drift fails closed.

## Process and Concurrency Model

`LanguageServerProcessService` owns a dedicated `clay-language-server` standard thread with a current-thread Tokio runtime. Callers communicate over a bounded central command channel and receive operation results through oneshot channels. The central router now performs only short table/identity operations: every session has an independent Tokio actor with a bounded queue (`LANGUAGE_SERVER_SESSION_COMMAND_CAPACITY = 8`) and exclusively owns its child/stdin/stdout/stderr. A read blocked on one child therefore cannot hold the router or another session's actor. Full actor ingress rejects immediately as `SessionBusy` rather than blocking the central router.

```text
package facade start/send/read/stop
  -> private deno_core op
  -> PackageService exact-grant revalidation
  -> bounded central command channel
  -> router: exact session identity/table lookup + bounded try_send
  -> session-owned actor queue/task
  -> fixed child process with piped stdin/stdout/stderr
```

Session IDs are allocated atomically before commands enter the router. The router rejects a second live session with the same package, contribution fingerprint, and canonical workspace root, enforcing one session per approved contribution/root. Its table is capped at `LANGUAGE_SERVER_MAX_SESSIONS`; each `SessionHandle` stores exact identity/root metadata plus actor sender, stop signal, and task handle. `SessionProcess` exists only inside the actor. Stop/revoke/shutdown remove table entries immediately, signal actors independently of queue fullness, then kill/reap children asynchronously; service drop stops and reaps all actors before the dedicated runtime exits. `kill_on_drop` remains final protection.

## Launch and I/O

Spawn uses `tokio::process::Command` directly:

- canonical executable and literal argv come from validated contribution/grant metadata;
- `current_dir` is the approved canonical directory root;
- `env_clear` removes ambient inheritance, then only declared environment names are copied;
- stdin/stdout/stderr are piped;
- `kill_on_drop(true)` protects abnormal teardown;
- no shell, interpolation, runtime argv/cwd/env override, inherited terminal, or generic process API exists.

Phase 18.21 replaces the text-only `send`/`read` with exact byte `sendBytes`/`readBytes`. `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES` is raised to 1 MiB (from 256 KiB) to accommodate LSP 3.17 response payloads. The `SessionCommand::Read` response type changed from `String` to `Vec<u8>` to avoid `String::from_utf8_lossy` corrupting multibyte `Content-Length` framing. Two new deno ops (`op_clay_language_server_send_bytes` with `#[buffer]` input, `op_clay_language_server_read_bytes` returning `#[buffer] Vec<u8>`) expose `sendBytes`/`readBytes` through the JS facade as `Uint8Array`. All Content-Length framing, JSON-RPC parsing, and LSP method routing remain in package-owned JavaScript. The original text `send`/`read` ops are preserved for existing callers but are deprecated for LSP use.

## Errors and Cleanup

`LanguageServerError` distinguishes unauthorized/mismatched sessions, unknown sessions, too many sessions, payload overflow, spawn/I/O failure, timeout, child exit, and invalid roots. Facade ops translate failures to stable Clay error codes and do not expose raw process handles or unrestricted stderr.

Every operation first rechecks the current grant in the op layer, then the central router rechecks package name, contribution ID, and descriptor fingerprint immediately before actor ingress. Revocation therefore fails the next operation even before asynchronous package cleanup completes. Package withdrawal calls `revoke_for_package`, which removes and signals every owned actor; runtime-generation commit calls `shutdown_all` through `ClayJsRuntimeService::shutdown_generation_resources`. Actor stop signals interrupt an in-flight read/write and are not queued behind ordinary actor commands.

## Primitive Coverage

- **Primitive/category:** `LanguageServerSession`.
- **Owner:** `src/server/language_server.rs::LanguageServerProcessService`.
- **Facade/ops:** `runtime/js/language-server.js`; private authorize/start/send/read/stop ops in `src/server/ops/language_server.rs`.
- **Permission:** deny-by-default `language-server`, plus exact pre-load `LanguageServerGrant` for contribution and roots.
- **Budgets:** 16 sessions, 8 queued commands per session, 1 MiB message, 64 KiB retained stderr, 30-second read timeout. Stderr keeps draining/discarding to EOF after the retained prefix fills and records a bounded truncation flag, preventing child pipe backpressure.
- **Hot-path policy:** async dedicated server thread; absent from Masonry/client edit/paint/layout paths.
- **Reuse rule:** every Phase 18.21 LSP bridge uses this session service and keeps framing/server policy package-side; no per-language process launcher belongs in core.

## Invariants and Constraints

- `loadPackage` never authorizes or launches a server.
- First-party/bundled trust never auto-grants `language-server`.
- Runtime code selects only a previously approved contribution and root.
- Every operation revalidates exact grant identity.
- One package contribution has at most one live session per canonical workspace root.
- No generic shell/process/filesystem/network Clay API is granted.
- `current_dir` and root binding are not OS confinement.
- No process/session work enters typing, paint, layout, scroll, or local text application.
- LSP framing, initialization, synchronization, cancellation protocol, and position/URI conversion remain package-owned.

## Phase 18.21 Handoff (Complete)

All four first-party LSP bridge packages use this service. Bridge packages:

1. obtain explicit user grant before package load (`authorizeLanguageServer`);
2. lazily start one approved contribution/root session;
3. implement bounded LSP framing and JSON-RPC over `sendBytes`/`readBytes` (exact `Uint8Array` transport);
4. map responses through [Language Intelligence](language-intelligence.md), [Decoration Transport](decoration-transport.md), [Range Diagnostics](range-diagnostics.md), and [Completion Snippet Expansion](completion-snippet-expansion.md) primitives;
5. stop sessions during bridge shutdown and rely on host cleanup for revocation/reload/failure.

Bridge release notes must repeat trusted-subprocess containment language and must not claim workspace/filesystem/network sandboxing.

## Tests

- `tests/language_server_authority.rs`: shell/external executable rejection, pre-spawn byte budget, bad cwd spawn error, timeout with stoppable session, sanitized child exit, duplicate contribution/root rejection, session cap, package-withdrawal reaping, lossless split-UTF-8 round-trip, fragmented LSP frame reassembly, oversize byte write/read rejection, cross-session head-of-line isolation, and bounded actor-ingress rejection. 16 tests total.
- `src/server/language_server.rs`: `capped_stderr_retains_prefix_and_drains_remainder_to_eof` uses a small duplex pipe and an over-cap payload to prove retained bytes/truncation stay bounded while the writer still reaches normal EOF.
- `tests/package_loading.rs`: descriptor validation, no bundled auto-grant, exact grant enablement, and revocation failure.
- `src/server/js_runtime.rs`: grant-before-load, unknown-root rejection, and loaded-package self-grant denial.
- `tests/editor_performance_invariants.rs`: process service and `tokio::process::Command` absent from editor/client hot paths.
- `tests/package_loading_docs.rs`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`: configuration/API/security documentation and registry freshness.

```bash
cargo test --test security language_server_authority::
cargo test --test security package_loading::
cargo test --test editor editor_performance_invariants::
```

## Related

- [First-Party LSP Bridge Packages](first-party-lsp-bridge-packages.md)
- [Language Intelligence](language-intelligence.md)
- [Phase 18.20 Primitive Review](phase18.20-language-intelligence-primitive-review.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Third-Party Runtime Authority](third-party-runtime-authority.md)
- [Package Loading](package-loading.md)
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md) — Phase 19 `shutdown_all` kills and reaps all previous-generation language-server sessions after atomic commit; `shutdown_generation_resources` delegates through `ClayJsRuntimeService`.
- [Package Security Reference](../../reference/primitives/package-security.md)
- [LSP 3.17 Bridge Contract](../../reference/primitives/language-intelligence.md)
