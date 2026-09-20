# Persistent Runtime Hardening

## Source

- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
- `decision-logs/2026-07-21-0001-*.md` (two package runtime trust domains)
- `src/server/js_runtime/mod.rs`
- `src/server/js_runtime/worker.rs`
- `src/server/js_runtime/error.rs`
- `src/server/ops/packages.rs`
- `src/server/parse_coordinator.rs`
- `src/perf/budgets.rs`
- `src/perf/metrics.rs`
- `docs/development/performance.md`
- `docs/reference/primitives/package-security.md`

## Overview

Clay runs JavaScript in a server-owned `deno_core` runtime per runtime generation and per trust domain. The current implementation only resolves bundled `@clay/*` package `loadEntry` modules, but the approved target is a unified package authority model where npm, GitHub/git, tarball, local path, and Clay-shipped packages can receive the same user-approved capabilities.

Hardening is still required. It protects runtime generations, documents, diagnostics, and hot paths; it is not a reason to make user-installed packages permanently second-class.

Since plan 127 each trust domain runs **two worker lanes** (a general lane and a latency lane) with a bounded command mailbox per lane, so one slow evaluation cannot delay latency-sensitive requests. Lanes are a scheduling device only: they never change what a package is allowed to do (see [Lane Isolation Is Not a Trust Boundary](#lane-isolation-is-not-a-trust-boundary)).

## Current Authority Inventory

- Runtime lifecycle: `ClayJsRuntimeService` owns persistent worker-thread `JsRuntime` isolates. Each trust domain (`RuntimeDomain::Trusted` / `RuntimeDomain::ThirdParty`) holds one `DomainRuntime` with one `LaneRuntime` per `RuntimeLane` (`General`, `Latency`), so a generation has `JS_RUNTIME_LANES_PER_DOMAIN` (2) workers per domain; `RuntimeGenerationStore` swaps whole generations for reload and poisoned lanes are replaced individually.
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

- V8 heap limits are installed for the current in-process runtime using server-owned compiled `JS_RUNTIME_HEAP_LIMIT_BYTES` and sanitized `runtime.heap_limit` diagnostics. Latency lanes use the smaller `JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES` ceiling.
- Treat heap-limit and timeout termination as worker poisoning: callers receive sanitized diagnostics, the worker exits, and the next controlled evaluation starts a fresh worker.
- The separate-process sandbox design is documented in [`docs/design/persistent-runtime-sandbox.md`](../../design/persistent-runtime-sandbox.md). It remains an optional runtime profile / hardening primitive, not a mandatory third-party-only boundary.
- The plan-034 minimal internal harness (`src/server/runtime_sandbox.rs`, `src/bin/clay-runtime-sandbox.rs`, `tests/runtime_sandbox_harness.rs`) was deleted in plan 119 (2026-09-14): it had no production caller (the persistent JS runtime owns in-process heap-limit and timeout termination), and reviving it would have shipped an unbounded process-isolation path. The design above remains the migration gate for any future separate-process profile.
- Keep package guards centralized in package load/enable/resolver code, not mode-specific branches.
- Heap and timeout budgets, sandbox kill/restart policy, and source-aware package authorization are server-owned controls, not hidden `init.js` keys.

## Worker Lanes, Queue Bounds, and Heap Restoration (plan 127)

Plan 127 split each domain into two lanes and bounded the work each lane accepts. Read this together with `docs/development/performance.md`, which carries the same budgets for the perf-report reader.

### Lane topology

| Lane | Commands | Backing isolate |
|---|---|---|
| `General` | `Evaluate` (configuration/`loadPackage`), `Parse`, `DocumentAnalysis`, `UpdateActiveEditorMode`, completion/language-intelligence providers registered with an inline `module` object | domain isolate, `JS_RUNTIME_HEAP_LIMIT_BYTES` (128 MiB) |
| `Latency` | `Completion` and `LanguageIntelligence` requests for providers registered with a package-owned `moduleSpecifier` | domain isolate, `min(JS_RUNTIME_HEAP_LIMIT_BYTES, JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES)` (32 MiB) |

- Routing is decided per provider registration, not per request: a registration carrying `moduleSpecifier` (validated against `PackageLoadEntryAllowlist::is_package_module` for the registering package) is materialized by import inside the latency lane; everything else stays on the general lane.
- Package `Evaluate` commands are never replayed into another lane, so a package's load-entry side effects (file writes, language-server spawns, registrations) happen exactly once in the domain's general lane.
- `UpdateActiveEditorMode` is replicated to every third-party lane; the cross-domain load bridge stays on the third-party general lane only, and the trusted latency lane has no cross-domain bridge.
- Poisoning and replacement are per lane: a `Timeout`/`HeapLimit` in one lane replaces only that lane's worker, and resource cleanup (language-server session counts, shutdown) iterates all lanes because some dispatch paths bypass normal lane routing.

### Bounded mailbox and supersession

- Each lane has one `CommandMailbox`. The supersedable backlog is bounded by `JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY` (64); commands with a `(kind, client_id, document_id, provider token)` key are replaced in place by a newer request for the same unit of work, and at capacity the **oldest** supersedable command is evicted (stale-first) rather than rejecting the new request.
- Only idempotent, request-scoped commands are supersedable (`Completion`, `LanguageIntelligence`). Guaranteed commands (`Evaluate`, `UpdateActiveEditorMode`) and delta-carrying commands (`Parse`, `DocumentAnalysis`) are always admitted, because dropping them would lose state or a document version. `Parse` is additionally gate-limited to one pending job per document by the parse coordinator.
- Superseded work never reaches the isolate: callers receive `ClayRuntimeError::Superseded` and treat it as no result. Drops are counted as `js_runtime.command.superseded` (replaced by newer work) and `js_runtime.command.evicted` (dropped at capacity) in the perf recorder.
- Worker shutdown closes the lane's mailbox and lets the queue drain; there is no `Shutdown` command variant.

### Heap-limit restoration

- A near-heap-limit callback raises the isolate limit as a temporary stop-gap so a recoverable evaluation can finish. The lane epilogue then unconditionally restores the configured limit and cancels any pending termination request, so a near-heap event or watchdog firing at the end of one evaluation cannot kill the next one and cannot ratchet the cap upwards permanently.
- Restoration goes through `deno_core`'s `remove_near_heap_limit_callback(limit)` because v8 147.4.0 exposes no `set_heap_limit` binding. V8 clamps the restored limit to the isolate's live heap, so the effective cap tracks retained memory (it can sit above the configured value while garbage is uncollected) — the ratchet value itself is never inherited.

### Budgets and constants

| Constant | Value | Meaning |
|---|---|---|
| `JS_RUNTIME_LANES_PER_DOMAIN` | 2 | General + latency worker per domain. |
| `JS_RUNTIME_HEAP_LIMIT_BYTES` | 128 MiB | Per-isolate heap ceiling. |
| `JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES` | 32 MiB | Ceiling for latency lanes (min with the configured limit). |
| `JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY` | 64 | Supersedable commands pending per lane. |
| `JS_RUNTIME_EVALUATION_TIMEOUT_MS` | 5000 | Per-evaluation timeout; exceeding it poisons that lane. |

### Lane Isolation Is Not a Trust Boundary

Lanes exist to keep latency-sensitive work off slow work inside one trust domain. They are **not** a sandbox and **not** a capability boundary:

- The trust boundary remains the domain. `RuntimeDomain` is derived from manifest-fingerprint verification against the bundled root; only that split decides which ops a runtime installs.
- Lanes within a domain install an identical op set, and third-party lanes never gain trusted-only ops (`lanes_share_their_domain_op_set`, `third_party_lane_denies_trusted_ops`).
- No V8 object, function, global, module instance, or registration crosses a domain boundary; lanes only add isolates *inside* a domain.
- Command admission is checked against the package's enabled name and exact version (`ensure_package_enabled`) before any lane is touched, so revocation and disable are lane-independent and fail closed with `ClayRuntimeError::Revoked` (`revoked_package_commands_refused_per_lane`).
- A trusted reload rebuilds trusted lanes and leaves third-party lanes (identity and workers) untouched (`reload_shares_third_party_lanes_untouched`).

## Hot-Path Policy

Hardening work happens during runtime startup, configuration evaluation, package load/enable, parse scheduling, reload, package graph changes, or sandbox supervision. It must not run in keypress, paint, layout, scroll, edit acknowledgement, or text-event handlers. Clients continue to consume validated behavior manifests, SDUI, decorations, and protocol updates. Lane selection is decided at registration time, so no lane lookup, queue mutation, or isolate handoff happens on an input hot path; latent lanes are only drained by their own worker.

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
- Lane scheduling: `latency_lane_unblocked_by_busy_general_lane` (completion round-trips under budget while a parse handler holds the general lane), `lane_poison_replaces_only_that_lane`, `language_intelligence_module_specifier_serves_from_latency_lane`, `lane_heap_limits_stay_within_the_configured_budget`.
- Lane/domain invariants: `lanes_share_their_domain_op_set`, `third_party_lane_denies_trusted_ops`, `revoked_package_commands_refused_per_lane`, `reload_shares_third_party_lanes_untouched`.
- Queue bounds: `queue_bounded_under_flood` (same-document supersession), `distinct_documents_never_superseded`, `queue_evicts_oldest_at_capacity`.
- Heap restoration: `src/server/js_runtime/tests.rs::near_heap_limit_recovers_with_original_cap` (the restored cap tracks the live heap instead of inheriting the near-heap ratchet value).

Manual evidence for the plan-127 scheduling work (isolated live run, measured
`server.edit_ack` p50/p95, grant-gate negative check) is under
`test-plan/artifacts/127-lane-scheduling/`, with the diff-based JS-API surface
verification in `code-reviews/2026-09-18-plan127-baseline/task8-js-api-surface.md`.

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md)
- [Package Loading](package-loading.md)
- [Parse Coordinator](parse-coordinator.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md)
- [Language Intelligence](language-intelligence.md)
- [Unified Package Runtime Authority](third-party-runtime-authority.md)
- `docs/development/performance.md`
- `docs/design/persistent-runtime-sandbox.md`
- `docs/reference/primitives/package-security.md`
