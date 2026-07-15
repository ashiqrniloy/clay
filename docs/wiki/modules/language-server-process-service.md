# Language Server Process Service

## Source

- `src/packages/permissions.rs`
- `src/packages/record.rs`
- `src/packages/authorization.rs`
- `src/packages/service.rs`
- `src/server/language_server.rs`
- `src/server/ops/language_server.rs`
- `src/server/ops/mod.rs`
- `runtime/js/language-server.ts`
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

`LanguageServerProcessService` owns a dedicated `clay-language-server` standard thread with a current-thread Tokio runtime. Deno worker commands are sequential, so this separate router prevents a long language-server read from blocking unrelated JS evaluation. Callers communicate over a bounded Tokio command channel and receive operation results through oneshot channels.

```text
package facade start/send/read/stop
  -> private deno_core op
  -> PackageService exact-grant revalidation
  -> LanguageServerProcessService command channel
  -> dedicated Tokio router/session table
  -> fixed child process with piped stdin/stdout/stderr
```

Session IDs are allocated atomically before commands enter the router. The serialized router rejects a second live session with the same package, contribution fingerprint, and canonical workspace root, enforcing one session per approved contribution/root. The router table is capped at `LANGUAGE_SERVER_MAX_SESSIONS`. Each `Session` stores package/contribution/fingerprint identity, child/stdin/stdout, bounded stderr, and approved root metadata. Service drop closes the command channel; router shutdown kills and waits for all children, with `kill_on_drop` as final protection.

## Launch and I/O

Spawn uses `tokio::process::Command` directly:

- canonical executable and literal argv come from validated contribution/grant metadata;
- `current_dir` is the approved canonical directory root;
- `env_clear` removes ambient inheritance, then only declared environment names are copied;
- stdin/stdout/stderr are piped;
- `kill_on_drop(true)` protects abnormal teardown;
- no shell, interpolation, runtime argv/cwd/env override, inherited terminal, or generic process API exists.

`send` accepts UTF-8 message data up to `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES` (256 KiB). `read` caps requested/output bytes to the same limit and applies `LANGUAGE_SERVER_READ_TIMEOUT_MS` (30 seconds). Stderr is asynchronously accumulated only up to `LANGUAGE_SERVER_STDERR_BUDGET_BYTES` (64 KiB), sanitized, and surfaced only on typed child failures. LSP `Content-Length` framing is intentionally not implemented here.

## Errors and Cleanup

`LanguageServerError` distinguishes unauthorized/mismatched sessions, unknown sessions, too many sessions, payload overflow, spawn/I/O failure, timeout, child exit, and invalid roots. Facade ops translate failures to stable Clay error codes and do not expose raw process handles or unrestricted stderr.

Every operation rechecks the current grant using package name, contribution ID, and descriptor fingerprint. Revocation therefore fails the next operation even before asynchronous package cleanup completes. Package withdrawal calls `revoke_for_package`, which kills and reaps every owned session. Runtime service replacement/drop closes the route and performs the same cleanup.

## Primitive Coverage

- **Primitive/category:** `LanguageServerSession`.
- **Owner:** `src/server/language_server.rs::LanguageServerProcessService`.
- **Facade/ops:** `runtime/js/language-server.ts`; private authorize/start/send/read/stop ops in `src/server/ops/language_server.rs`.
- **Permission:** deny-by-default `language-server`, plus exact pre-load `LanguageServerGrant` for contribution and roots.
- **Budgets:** 16 sessions, 256 KiB message, 64 KiB stderr, 30-second read timeout.
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

## Phase 18.21 Publish Handoff

Bridge packages can now:

1. obtain explicit user grant before package load;
2. lazily start one approved contribution/root session;
3. implement bounded LSP framing and JSON-RPC over `send`/`read`;
4. map responses through [Language Intelligence](language-intelligence.md), decoration, diagnostic, and completion primitives;
5. stop sessions during bridge shutdown and rely on host cleanup for revocation/reload/failure.

Bridge release notes must repeat trusted-subprocess containment language and must not claim workspace/filesystem/network sandboxing.

## Tests

- `tests/language_server_authority.rs`: shell/external executable rejection, pre-spawn payload limit, bad cwd spawn error, timeout with stoppable session, sanitized child exit, duplicate contribution/root rejection, session cap, and package-withdrawal reaping.
- `tests/package_loading.rs`: descriptor validation, no bundled auto-grant, exact grant enablement, and revocation failure.
- `src/server/js_runtime.rs`: grant-before-load, unknown-root rejection, and loaded-package self-grant denial.
- `tests/editor_performance_invariants.rs`: process service and `tokio::process::Command` absent from editor/client hot paths.
- `tests/package_loading_docs.rs`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`: configuration/API/security documentation and registry freshness.

```bash
cargo test --test language_server_authority
cargo test --test package_loading
cargo test --test editor_performance_invariants
```

## Related

- [Language Intelligence](language-intelligence.md)
- [Phase 18.20 Primitive Review](phase18.20-language-intelligence-primitive-review.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Third-Party Runtime Authority](third-party-runtime-authority.md)
- [Package Loading](package-loading.md)
- [Package Security Reference](../../reference/primitives/package-security.md)
- [LSP 3.17 Bridge Contract](../../reference/primitives/language-intelligence.md)
