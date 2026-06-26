---
date: 2026-06-26 13:38
status: approved
decision_about: "Expand Clay's controlled server-side runtime authority to keep first-party package handlers alive in a persistent runtime and invoke validated JS parse handlers through ParseCoordinator"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Expand Phase 18.7 runtime authority for persistent package handlers and JS-backed parsing

## Decision

Clay will use a persistent, server-owned JavaScript runtime per server/configuration generation to retain resolver-validated first-party package state, mode activation metadata, and token-backed parse handler functions across evaluations. Resolver-validated first-party packages may register JS parse handlers through `clay.parse.serverRegisterParseHandler({ module, exportName, ... })`; Rust stores only validated metadata plus a server-issued token, adapts the handler into `ParseCoordinator`, and invokes it as bounded background parse work.

This authority expansion is constrained to the existing first-party `@clay/*` package load path and curated `clay:*` facades. It grants no new filesystem, network, shell, AI, WASM, raw-op, native-widget, client-side JavaScript, package-manager, or package enable/disable authority.

## Context

Phase 18.6 made `loadPackage("@clay/*")` work by expanding the controlled runtime's module-loading authority to resolver-validated first-party package `loadEntry` modules. The next blocked cleanup was removing the hardcoded Markdown selected-file open path, which previously spawned a fresh runtime, copied `packages/markdown/dist` files, generated an `init.js`, evaluated that temporary runtime, and deleted it for every Markdown open.

The Phase 18.7 primitive review and implementation showed that the generic replacement required a second authority expansion:

1. A long-lived server runtime must retain package modules, mode registries, activation metadata, and handler token registries after startup/configuration evaluation.
2. The runtime must hold executable JS package parser functions, but Rust must not accept function values or user-supplied callbacks across the op boundary.
3. Open-time activation must call generic mode/package/parse primitives on selected-file open, including lazy first-party package loading when no mode is registered yet.
4. Parse work must be scheduled through `ParseCoordinator` as cancellable background work with budgets, timeouts, stale-result rejection, and validation before publication.

This phase completed those primitives and removed the Markdown-specific Rust branch from `src/server/connection.rs`.

## Approval

- Proposed by: both (roadmap/plan required this decision log before closing Phase 18.7; agent implemented the constrained persistent-runtime bridge).
- Approved by user: Yes.
- Approval evidence: The user directed: "Complete the next task Create the Phase 18.7 authority-expansion decision log and update the plan once done". That plan task explicitly required creating this decision log for the Phase 18.7 authority expansion.

## Alternatives Considered

1. **Keep per-evaluation runtimes and per-open Markdown runtime roots.** — Rejected. It preserves bounded behavior but fails the generic mode-activation goal, keeps per-open V8/disk churn, and leaves Markdown as a Rust-side special case.
2. **Accept executable `handler`/`callback`/`onParse` functions directly in the public op payload.** — Rejected. Passing arbitrary executable callbacks from user configuration would violate the Phase 18.7 security boundary. The selected token registry keeps JS function references inside the persistent runtime and sends Rust only metadata/token state.
3. **Embed Markdown parsing directly in Rust.** — Rejected. It would violate primitive-first mode planning, reintroduce a mode-specific Rust parser branch, and make future modes require Rust changes.
4. **Add a Markdown-only persistent runtime or Markdown-specific open branch.** — Rejected. One-off abstractions for Markdown would hide, not remove, the architecture problem. The selected path reuses generic `clay:packages`, `clay:modes`, `clay:parse`, `ParseCoordinator`, and `apply_runtime_outputs` primitives.
5. **Expand runtime/package authority to arbitrary npm or non-`@clay/*` packages now.** — Rejected. Phase 18.7 remains constrained to first-party resolver-validated packages. Non-`@clay/*` package resolution stays deferred to Phase 23.
6. **Block the generic activation path on V8 heap limits or a separate-process JS sandbox.** — Rejected for this phase. The timeout, deny-by-default module loader, permission checks, token bridge, and parse budgets bound the shipped path. V8 heap limits and a separate-process sandbox remain hardening follow-ups, not prerequisites for removing the Markdown branch.

## Rationale and Evidence

### Persistent runtime lifecycle

`ClayJsRuntimeService` now owns a dedicated runtime worker thread with one long-lived `deno_core::JsRuntime`. Evaluations are serialized through a channel/oneshot boundary. The first evaluation uses the main ES module; later evaluations use side modules so global state, imported packages, mode activation registries, loaded-package cache state, and parse handler token registries survive across evaluations.

`ClayOpState::begin_evaluation` clears per-evaluation output records while preserving long-lived package/module/global state. `ClayOpState::set_runtime_context` updates workspace/document context without rebuilding the isolate. Timeouts still use the Plan 030 runtime guard and surface `clay.runtime.timeout` diagnostics.

### Token-backed JS ParseHandler bridge

`serverRegisterParseHandler` validates package identity, `parse-document` permission, mode, parse unit, timeout, window, guard, and memory budgets. The facade rejects executable `handler`/`callback`/`onParse`/`function` keys before calling the op, and the op rejects them too.

The op returns a server-issued token. The JS facade resolves `module[exportName]`, verifies it is a function, and stores the function in `globalThis.__clayParseHandlers[token]` inside the persistent runtime. Rust stores `JsParseHandlerRegistration` metadata plus the token, never a JavaScript function value. `ClayJsRuntimeService::register_parse_handlers` adapts accepted registrations into `ParseCoordinator::register_handler_meta`.

### Generic open-time activation

`connection::selected_file_open_followup_messages` now classifies selected files through `clay:modes`. If no mode is registered, the runtime scans first-party `@clay/*` package specifiers and calls idempotent `loadPackage` until classification succeeds. The classified mode is activated through generic stored activation metadata, then the connection schedules a bounded parse window through `ParseCoordinator` and emits validated behavior/decorations without Markdown-specific Rust logic.

The removed branch included `is_markdown_path`, `evaluate_markdown_open`, `create_markdown_open_runtime_root`, `unique_markdown_open_runtime_root`, `markdown_open_init_source`, and the `clay-markdown-open-runtime-*` temp root/copy path.

### Safety boundaries

- Parser execution is background work and must not block client-first typing, edit acknowledgement, Masonry text-event handling, or paint.
- Handler invocation uses the smaller of the service runtime timeout and the handler's registered `timeoutMs`.
- Parse windows are bounded by `maxWindowBytes`, `guardBytes`, `memoryBudgetBytes`, and `SYNTAX_CACHE_BUDGET_BYTES`.
- Published updates are validated against document/version/provenance metadata, stale versions, decoration payload budget, and `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.
- Handler errors/timeouts/invalid updates increment `ParseCoordinatorStats.failed_tasks` and publish no half-updated result.
- The persistent runtime denies platform authorities such as `fetch`, `WebSocket`, `Worker`, `process`, `require`, `Deno.readTextFile`, and `Deno.Command`.
- Only resolver-validated first-party packages can register live parse handlers; install/enable metadata alone is not enough without declared `parse-document` permission.

### Hardening status

The persistent runtime still runs in-process with V8. V8 heap-limit enforcement via `v8::CreateParams::heap_limits` and a separate-process JS sandbox remain deferred hardening items. They are important follow-ups, but Phase 18.7 ships with deterministic timeout enforcement, deny-by-default module loading, constrained facades, package permission checks, bounded parse windows, payload budgets, and failure instrumentation.

## References

- `roadmap.md` Phase 18.7 — states the runtime-lifecycle/authority gate, focus areas, expected outcome, and carried-forward items.
- `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md` — implementation plan and completed task evidence.
- `decision-logs/2026-06-16-1526-generic-first-party-package-loadentry-module-bridge.md` — Phase 18.6 first-party package module-loading authority expansion that Phase 18.7 builds on.
- `decision-logs/2026-06-23-1823-defer-remove-hardcoded-markdown-open-path-to-phase-18-7-persistent-runtime-and-parse-bridge.md` — deferral that identified the persistent-runtime and JS parse bridge gaps.
- `docs/wiki/modules/phase18.7-persistent-runtime-bridge-primitive-review.md` — primitive review and rejected alternatives.
- `docs/wiki/modules/embedded-js-runtime.md` — persistent runtime lifecycle, token-backed parse handler bridge, timeout diagnostics, and platform authority isolation.
- `docs/wiki/modules/parse-coordinator.md` — parse scheduling, validation, budgets, failure instrumentation, and JS-backed handler adapter.
- `docs/reference/packages/creating-packages.md` — package authoring contract for `loadPackage`, `serverRegisterParseHandler({ module, exportName })`, no-client-JS/no-hot-path-JS, budgets, and security.
- `docs/reference/clay-js-api/parse/server-register-parse-handler.md` — public Clay JS parse-handler API contract.
- `src/server/js_runtime.rs` — persistent runtime worker, module loader, facades, parse invocation, timeout enforcement, security tests.
- `src/server/parse_coordinator.rs` — `ParseHandler`, registration, scheduling, validation, stats.
- `src/server/connection.rs` — generic selected-file open-time mode activation and bounded parse scheduling.
- `runtime/js/parse.ts`, `runtime/js/packages.ts`, `runtime/js/modes.ts` — public facade behavior for parse registration, idempotent package loading, and classified mode activation.
- `packages/markdown/dist/load.js`, `packages/markdown/dist/parser.js` — first-party Markdown package registration and parser adapter.
- Verification commands run during Phase 18.7 tasks: `cargo test connection --lib`, `cargo test js_runtime --lib`, `cargo test --test parse_coordinator`, `cargo test --test package_loading_docs`, `cargo test --test clay_js_api_inventory`, `cargo fmt --check`, and `cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline`.

## Consequences

### Positive outcomes

- Opening a Markdown file now routes through generic package/mode/parse primitives on the persistent runtime.
- `src/server/connection.rs` no longer contains Markdown-specific open-time runtime creation, path checks, temp-root creation, or `dist` copying.
- Package parser functions stay inside the persistent server runtime and are invoked through server-issued tokens, not serialized function values.
- A future mode can reuse the same `loadPackage` → `serverRegisterModePattern` → `serverRegisterParseHandler` → classified open activation → `ParseCoordinator` path without Rust mode branches.
- Parse failures are instrumented and non-publishing; typing/rendering do not wait for JS parse work.

### Risks and follow-up work

- **In-process V8 remains a hardening risk.** V8 heap limits and a separate-process JS sandbox are deferred. Revisit before expanding beyond first-party packages or exposing broader package ecosystems.
- **Hot reload is not implemented.** Repeated `loadPackage` calls are idempotent per persistent runtime lifetime. Phase 19 must define runtime replacement, handler invalidation, and behavior update semantics.
- **Non-`@clay/*` packages remain unsupported.** External package registries, user-installed package roots, and third-party trust decisions remain Phase 23 work and require a new authority review.
- **Persistent runtime state must stay generation-scoped.** Configuration reload/hot reload must avoid stale handlers, duplicate registrations, or mixed package versions.

### Conditions for revisiting this decision

- Clay needs to run third-party/non-`@clay/*` packages, package-manager code, or registry-fetched modules.
- Clay needs hot reload that replaces package code while documents remain open.
- Security review requires separate-process isolation or heap limits before any persistent runtime invocation.
- Parse work appears on typing/rendering/edit-ack hot paths or violates Phase 14/15 performance budgets.
- A future package requires authorities outside the current curated `clay:*` facade and first-party load-entry allowlist.
