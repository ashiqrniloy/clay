# Persistent Runtime Hardening

## Source

- `src/server/js_runtime.rs`
- `src/server/runtime_sandbox.rs`
- `src/bin/clay-runtime-sandbox.rs`
- `src/server/ops/packages.rs`
- `src/server/parse_coordinator.rs`
- `src/perf/budgets.rs`
- `docs/reference/primitives/package-security.md`
- `tests/package_loading_docs.rs`
- `tests/runtime_sandbox_harness.rs`

## Overview

Clay currently runs JavaScript in one in-process, server-owned `deno_core` runtime per runtime generation. That runtime may execute resolver-validated first-party `@clay/*` package `loadEntry` modules and token-backed parse handlers. This is enough for first-party package proof points, but it is not enough authority for third-party/non-`@clay/*` package execution.

This page is the gate: first-party runtime hardening can continue in process, but third-party package execution stays disabled until a separate-process sandbox and a new approved authority decision log exist.

## Current Authority Inventory

- Runtime lifecycle: `ClayJsRuntimeService` owns a persistent worker-thread `JsRuntime`; `RuntimeGenerationStore` swaps whole generations for reload.
- Module loading: `ClayModuleLoader` is deny-by-default for arbitrary specifiers and accepts only controlled runtime modules, curated `clay:*` facades, configuration-root relative modules, the vendored `markdown-it` shim, and resolver-validated first-party `loadEntry` URLs recorded in `FirstPartyLoadEntryAllowlist`.
- Package load path: `loadPackage("@clay/*")` calls `op_clay_packages_load_package_by_specifier`, validates metadata through `PackageService::enable`, records one opaque `clay://packages/...` load entry, and confines transitive relative imports to the package root.
- Parse bridge: `clay:parse.serverRegisterParseHandler` stores JS functions inside the persistent runtime behind server-issued tokens; Rust receives metadata and invokes handlers as cancellable `ParseCoordinator` background work.
- Platform API denial: runtime tests and facades deny broad platform APIs such as filesystem reads, network fetch/listeners, shell commands, workers, package-manager execution, raw-op use as a public API, native widget handles, client-side JavaScript, and WASM authority by default.

## Third-Party Authority Gate

Non-`@clay/*` package execution is blocked by default. Before any registry-fetched, user-installed, local-path, tarball, URL, or custom-scope third-party package can execute, Clay needs all of these:

1. A separate-process JavaScript runtime sandbox with supervised start, bounded request/response protocol, cancellation, kill/restart, payload limits, and parent-side validation.
2. A package trust policy covering package identity, integrity/signatures, compatibility, permissions, lifecycle scripts, registry/package-manager behavior, updates, rollback, and diagnostics.
3. Measured startup, package-load, parse, reload, timeout, and heap overhead proving no keypress, paint, layout, scroll, text-event, or edit-ack path waits on JavaScript or sandbox round trips.
4. A new approved decision log that grants the exact third-party execution authority and lists denied authorities and rollback/revisit conditions.
5. Documentation-as-code and tests proving non-`@clay/*` execution remains disabled without that decision-log reference.

No approved decision log means no non-`@clay/*` runtime execution.

## Denied Authorities by Default

Packages and package primitives do not get these authorities unless a later approved decision grants a narrow capability with tests and docs:

- filesystem outside already-open Clay document/workspace data
- network
- shell
- WASM
- AI mutation or tool orchestration
- raw-op / raw `Deno.core.ops` public authority
- native-widget handles, direct Masonry mutation, renderer callbacks, arbitrary GPU draw calls
- client-side JavaScript
- package-manager execution, lifecycle scripts, install/enable/disable mutation from package JS
- remote listeners
- workspace mutation outside declared Clay workspace APIs

## Hardening Roadmap

- V8 heap limits are installed for the current first-party in-process runtime using server-owned compiled `JS_RUNTIME_HEAP_LIMIT_BYTES` and sanitized `clay.runtime.heap_limit` diagnostics.
- Treat heap-limit and timeout termination as worker poisoning: callers receive sanitized diagnostics, the worker exits, and the next controlled evaluation starts a fresh worker instead of reusing stale `globalThis` or half-mutated module state.
- The separate-process sandbox design is documented in [`docs/design/persistent-runtime-sandbox.md`](../../design/persistent-runtime-sandbox.md). Parent owns documents, package metadata, permissions, behavior publication, diagnostics, and restart policy; child returns only inert bounded results.
- A minimal internal harness now exists in `src/server/runtime_sandbox.rs` plus `src/bin/clay-runtime-sandbox.rs`. It uses newline-delimited JSON over child stdio to prove spawn/handshake, controlled fixture evaluation, parent timeout kill, restart by spawning a fresh supervisor, parent-side payload rejection, sanitized child diagnostics, and no filesystem/network/shell globals in the child runtime. It is not wired into production package loading and does not grant third-party authority.
- Keep non-`@clay/*` resolver guards centralized in package load/enable/resolver code, not mode-specific branches. The resolver rejects bare, scoped, URL, path, traversal, and malformed `@clay/*` specifiers before module loading; package-manager/store metadata alone does not grant runtime execution authority.
- Keep hardening controls out of user configuration. Heap and timeout budgets, sandbox kill/restart policy, denied authorities, and third-party execution gates are server-owned security boundaries documented in `docs/reference/clay-js-api/configuration.md`, not hidden `init.js` keys or `clay:configuration` APIs.
- Keep Plan 034 hardening out of the public Clay JS API registry. `clay.runtime.timeout` and `clay.runtime.heap_limit` are diagnostic codes, not facade IDs. `RuntimeSandboxSupervisor`, sandbox protocol frames, payload budgets, and kill/restart controls stay internal `#[doc(hidden)]` harness surfaces and are not exported from `runtime/js/mod.ts`.

## Hot-Path Policy

Hardening work happens during runtime startup, configuration evaluation, package load/enable, parse scheduling, reload, or sandbox supervision. It must not run in keypress, paint, layout, scroll, edit acknowledgement, or text-event handlers. Clients continue to consume validated inert behavior manifests, SDUI, decorations, and protocol updates.

## Repository Gates

Plan 034 hardening is verified by focused security tests plus repository-wide gates:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo test --test performance_protocol`
- `cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline`

`performance_protocol` is the deterministic hard gate for protocol/edit hot paths. Criterion protocol baselines are advisory local comparisons; regressions there require triage only when they are sustained and tied to the changed production hot path.

## Tests

- `tests/package_loading_docs.rs::persistent_runtime_hardening_gate_doc_covers_threat_model`: requires this page to mention heap limits, separate-process sandbox, first-party-only scope, denied authorities, hot-path exclusions, and the third-party decision-log gate.
- `src/server/js_runtime.rs::tests::js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic`: verifies heap growth terminates with `clay.runtime.heap_limit` and no successful evaluation count.
- `src/server/js_runtime.rs::tests::js_runtime_timeout_recovery_uses_fresh_worker`: verifies a timeout-poisoned worker is replaced before the next controlled evaluation.
- `src/server/js_runtime.rs::tests::js_runtime_heap_limit_recovery_uses_fresh_worker`: verifies a heap-poisoned worker is replaced before the next controlled evaluation.
- `tests/clay_js_api_inventory.rs::plan_034_runtime_hardening_does_not_add_hidden_configuration_knobs`: verifies heap/timeout budgets, sandbox supervision, denied authorities, and third-party execution gates are not hidden `clay:configuration` APIs.
- `tests/clay_js_api_inventory.rs::plan_034_runtime_hardening_adds_no_public_clay_js_api_surface`: verifies runtime diagnostics and sandbox harness internals are not public Clay JS API IDs, docs-index entries, generated registry entries, or runtime JS facade exports.
- `tests/clay_js_api_inventory.rs`: verifies heap/sandbox/third-party hardening knobs are not hidden `clay:configuration` APIs.
- `tests/package_loading_docs.rs::persistent_runtime_sandbox_design_pins_process_boundary`: verifies the sandbox design records process boundary, bounded protocol, restart policy, parent-side validation, hot-path exclusion, denied authorities, and decision-log gate language.
- `tests/runtime_sandbox_harness.rs`: verifies child start/evaluate, startup/evaluation elapsed measurements remain inside the request timeout, timeout kill and fresh restart, oversized output rejection by the parent budget, and absence of filesystem/network/shell globals in the child runtime. `SandboxEvaluation::elapsed` records evaluation overhead for follow-up comparisons against the in-process runtime; no CI threshold is enforced yet.
- `src/server/js_runtime.rs::tests::op_clay_packages_load_package_by_specifier_rejects_non_first_party_specifier`: verifies `left-pad`, `@scope/pkg`, URL, path, traversal, malformed `@clay/*`, and registry-style specifiers fail before module loading.
- `tests/package_loading.rs::third_party_install_metadata_does_not_imply_runtime_execution_authority`: verifies package-manager/store metadata does not enable or execute third-party package JavaScript.
- `tests/rust_visibility_api_mapping.rs::server_public_items_have_api_inventory_entries_or_are_allowlisted`: allowlists the `#[doc(hidden)]` sandbox harness types as non-JS server infrastructure so they cannot accidentally become public Clay JS APIs without inventory/docs.
- Future hardening tests should cover production sandbox routing and sanitized diagnostics once the harness graduates beyond test-only use.

Run focused docs gate:

```text
cargo test --test package_loading_docs persistent_runtime_hardening_gate_doc_covers_threat_model
cargo test --test package_loading_docs persistent_runtime_sandbox_design_pins_process_boundary
cargo test --test runtime_sandbox_harness
cargo test --test rust_visibility_api_mapping server_public_items_have_api_inventory_entries_or_are_allowlisted
cargo test op_clay_packages_load_package_by_specifier_rejects_non_first_party_specifier --lib
cargo test --test package_loading third_party_install_metadata_does_not_imply_runtime_execution_authority
cargo test js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic --lib
cargo test js_runtime_timeout_recovery_uses_fresh_worker --lib
cargo test js_runtime_heap_limit_recovery_uses_fresh_worker --lib
cargo test --test clay_js_api_inventory phase18_7_persistent_runtime_does_not_add_hidden_configuration_knobs
cargo test --test clay_js_api_inventory plan_034_runtime_hardening_does_not_add_hidden_configuration_knobs
cargo test --test clay_js_api_inventory plan_034_runtime_hardening_adds_no_public_clay_js_api_surface
```

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Package Loading](package-loading.md)
- [Parse Coordinator](parse-coordinator.md)
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md)
- `docs/reference/primitives/package-security.md`
- `decision-logs/2026-06-16-1526-generic-first-party-package-loadentry-module-bridge.md`
- `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`
- `roadmap.md` Phase 23
