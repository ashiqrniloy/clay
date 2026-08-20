# Persistent Runtime Hardening

## Source

- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
- `src/server/js_runtime/mod.rs`
- `src/server/runtime_sandbox.rs`
- `src/bin/clay-runtime-sandbox.rs`
- `src/server/ops/packages.rs`
- `src/server/parse_coordinator.rs`
- `src/perf/budgets.rs`
- `docs/reference/primitives/package-security.md`
- `tests/runtime_sandbox_harness.rs`

## Overview

Clay runs JavaScript in a server-owned `deno_core` runtime per runtime generation. The current implementation only resolves bundled `@clay/*` package `loadEntry` modules, but the approved target is a unified package authority model where npm, GitHub/git, tarball, local path, and Clay-shipped packages can receive the same user-approved capabilities.

Hardening is still required. It protects runtime generations, documents, diagnostics, and hot paths; it is not a reason to make user-installed packages permanently second-class.

## Current Authority Inventory

- Runtime lifecycle: `ClayJsRuntimeService` owns a persistent worker-thread `JsRuntime`; `RuntimeGenerationStore` swaps whole generations for reload.
- Module loading: `ClayModuleLoader` accepts controlled runtime modules, curated `clay:*` facades, configuration-root relative modules, the vendored `markdown-it` shim, and resolver-recorded package `loadEntry` URLs in `PackageLoadEntryAllowlist`.
- Package load path: `loadPackage(specifier)` validates bundled and installed user-authorized packages through `PackageService::enable`, records one opaque `clay://packages/...` load entry, and confines transitive relative imports to the package root.
- Parse bridge: `clay:parse.serverRegisterParseHandler` stores JS functions inside the persistent runtime behind server-issued tokens; Rust receives metadata and invokes handlers as cancellable `ParseCoordinator` background work.
- Platform APIs: broad host APIs such as filesystem, network, shell, workers, package-manager execution, raw-op use, native widget handles, client-side JavaScript, and WASM need explicit Clay APIs and user-approved capabilities before packages use them.

## Unified Package Authority Gate

For source-aware user-installed package execution, Clay requires:

1. Source-aware package resolution for npm, GitHub/git, tarball, and local path specs.
2. User authorization records for requested capabilities and runtime profile.
3. Package graph support for `dependsOn`, `extends`, `disables`, and `replaces`.
4. Conflict resolution that supports explicit user/package override, extend, disable, and replace semantics.
5. Measured startup, package-load, parse, reload, timeout, and heap overhead proving no keypress, paint, layout, scroll, text-event, or edit-ack path waits on JavaScript or sandbox round trips.
6. Documentation-as-code and tests proving the unified model stays documented.

## Powerful Capabilities

Packages do not get powerful host access accidentally. Clay must expose explicit APIs, prompts/config, diagnostics, and revocation for capabilities such as:

- filesystem scopes
- network access
- shell commands
- WASM
- AI mutation or tool orchestration
- raw-op / raw `Deno.core.ops` public authority
- native-widget handles, direct Masonry mutation, renderer callbacks, GPU draw paths
- client-side JavaScript/runtime
- package-manager execution or lifecycle-script opt-ins
- package install/enable/disable mutation through `package-control`
- remote listeners
- workspace mutation through declared Clay workspace APIs

These capabilities are grantable to any package source after user authorization.

## Hardening Roadmap

- V8 heap limits are installed for the current in-process runtime using server-owned compiled `JS_RUNTIME_HEAP_LIMIT_BYTES` and sanitized `runtime.heap_limit` diagnostics.
- Treat heap-limit and timeout termination as worker poisoning: callers receive sanitized diagnostics, the worker exits, and the next controlled evaluation starts a fresh worker.
- The separate-process sandbox design is documented in [`docs/design/persistent-runtime-sandbox.md`](../../design/persistent-runtime-sandbox.md). It remains an optional runtime profile / hardening primitive, not a mandatory third-party-only boundary.
- A minimal internal harness exists in `src/server/runtime_sandbox.rs` plus `src/bin/clay-runtime-sandbox.rs`. It proves spawn/handshake, controlled fixture evaluation, parent timeout kill, fresh restart, parent-side payload rejection, sanitized child diagnostics, and no filesystem/network/shell globals in the child runtime. Its evidence-only newline framing is read with `AsyncBufRead::fill_buf`: the parent retains at most `maxPayloadBytes + 1`, accepts a newline only after at most `maxPayloadBytes`, and kills then awaits the child on overflow, EOF before a delimiter, I/O failure, or malformed JSON. It never calls unbounded `read_line`.
- Keep package guards centralized in package load/enable/resolver code, not mode-specific branches.
- Heap and timeout budgets, sandbox kill/restart policy, and source-aware package authorization are server-owned controls, not hidden `init.js` keys.

## Hot-Path Policy

Hardening work happens during runtime startup, configuration evaluation, package load/enable, parse scheduling, reload, package graph changes, or sandbox supervision. It must not run in keypress, paint, layout, scroll, edit acknowledgement, or text-event handlers. Clients continue to consume validated behavior manifests, SDUI, decorations, and protocol updates.

## Repository Gates

Plan 034 hardening is verified by focused security tests plus repository-wide gates:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo test --test protocol performance_protocol::`
- `cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline`

## Tests

- Package reference documentation uses generic manifest/API/security validators in `tests/package_loading_docs.rs`; executable package/runtime tests remain authoritative for behavior.
- `src/server/js_runtime/mod.rs::tests::js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic`: verifies heap growth terminates with `runtime.heap_limit`.
- `src/server/js_runtime/mod.rs::tests::js_runtime_timeout_recovery_uses_fresh_worker`: verifies a timeout-poisoned worker is replaced.
- `src/server/js_runtime/mod.rs::tests::js_runtime_heap_limit_recovery_uses_fresh_worker`: verifies a heap-poisoned worker is replaced.
- `tests/runtime_sandbox_harness.rs`: verifies child start/evaluate, timeout kill and fresh restart, valid oversized JSON rejection, absence of filesystem/network/shell globals, and hostile newline-terminated/unterminated streams. Linux fixture scripts emit exactly `max + 1` bytes (one then sleeps without a delimiter); both fail immediately at the bound and `/proc/<pid>` confirms the supervisor reaped each child.

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Package Loading](package-loading.md)
- [Parse Coordinator](parse-coordinator.md)
- [Unified Package Runtime Authority](third-party-runtime-authority.md)
- `docs/design/persistent-runtime-sandbox.md`
- `docs/reference/primitives/package-security.md`
