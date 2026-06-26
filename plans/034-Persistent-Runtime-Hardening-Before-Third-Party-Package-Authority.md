# Persistent Runtime Hardening Before Third-Party Package Authority

## Objectives

- Harden Clay's persistent server-side JavaScript runtime before any non-`@clay/*` package execution.
- Add memory-failure containment for the current in-process first-party runtime.
- Define the separate-process sandbox gate required before third-party package authority.
- Keep package installation, validation, execution, and user configuration authority separate.

## Expected Outcome

- V8 heap exhaustion is bounded by server-owned budgets and produces sanitized diagnostics instead of process-wide instability where possible.
- Runtime timeout, heap-limit, module-loading, permission, and platform-authority denial tests all run in the all-target gate.
- Third-party package execution remains disabled until a separate approved authority decision and sandbox implementation pass.
- Documentation states which hardening ships for first-party packages and which gates block non-`@clay/*` package execution.

## Tasks

- [x] Review runtime/package threat model and third-party authority gate
  - Acceptance Criteria:
    - Functional: Inventory the current first-party runtime authority, package load path, parse-handler bridge, denied platform APIs, and Phase 23 third-party expansion needs.
    - Performance: Identify hardening work as startup/package-load/parse background work, never keypress, paint, layout, scroll, or edit-ack work.
    - Code Quality: Produce generic runtime/package hardening requirements, not Markdown/package-specific branches.
    - Security: Explicitly state that filesystem, network, shell, WASM, AI, raw-op, native-widget, package-manager, and client-JS authority remain denied by default.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `docs/reference/primitives/package-security.md`
      - `docs/wiki/modules/embedded-js-runtime.md`
      - `docs/wiki/modules/package-loading.md`
      - `decision-logs/2026-06-16-1526-generic-first-party-package-loadentry-module-bridge.md`
      - `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`
      - `roadmap.md` Phase 23 ecosystem hardening.
    - Options Considered:
      - Expand third-party packages with current timeout-only guard. Rejected; Phase 18.7 decision requires renewed authority review.
      - Treat heap limits as enough for third-party code. Rejected; in-process V8 still shares process fate and host memory.
      - Define a staged gate: heap hardening for first-party runtime now, separate-process sandbox before third-party execution. Chosen.
    - Chosen Approach:
      - Write a threat-model/gate document first, naming the minimum hardening bar and decision-log checkpoint before any non-`@clay/*` execution.
    - API Notes and Examples:
      ```text
      @clay/* first-party packages only today.
      non-@clay/* execution requires approved sandbox authority decision.
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/persistent-runtime-hardening.md`: threat model, gates, denied authorities, and hardening roadmap.
      - `docs/wiki/index.md`: link hardening page.
      - `tests/package_loading_docs.rs`: docs-as-code hardening gate test.
    - References:
      - Phase 18.6 and 18.7 authority decision logs; package security reference.
  - Test Cases to Write:
    - `tests/package_loading_docs.rs::persistent_runtime_hardening_gate_doc_covers_threat_model`: Requires the hardening gate doc to mention heap limits, separate-process sandbox, denied authorities, first-party-only scope, hot-path exclusion, and third-party decision-log gate.

- [x] Add V8 heap-limit guard to the persistent runtime
  - Acceptance Criteria:
    - Functional: `ClayJsRuntimeService` creates `JsRuntime` with server-owned V8 heap limits and terminates execution when the near-heap callback fires.
    - Performance: Heap checks add no work to client hot paths; runtime startup/evaluation overhead is measured or smoke-tested.
    - Code Quality: Heap budgets are compiled server constants, not `init.js` configuration, and the implementation uses `deno_core::RuntimeOptions::create_params` with `v8::Isolate::create_params().heap_limits(...)`.
    - Security: Heap exhaustion produces a sanitized diagnostic and does not expose source text, absolute paths, tokens, or package internals.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/denoland/deno_core`: `JsRuntime::new` uses `RuntimeOptions`; `run_event_loop` is embedder-driven.
      - Local crate docs: `deno_core v0.400.0` exposes `RuntimeOptions { create_params: Option<v8::CreateParams> }`.
      - Local crate test: `deno_core::runtime::tests::misc::test_heap_limits` uses `v8::Isolate::create_params().heap_limits(0, 5 * 1024 * 1024)` plus `add_near_heap_limit_callback` and `terminate_execution()`.
      - `docs/reference/clay-js-api/configuration.md`: security budgets are not Clay JS APIs.
    - Options Considered:
      - Rely on timeout only. Rejected; memory growth can crash before timeout is useful.
      - Make heap limit user-configurable. Rejected; security boundary must not be raised from `init.js`.
      - Add compiled heap limit and near-heap termination. Chosen.
    - Chosen Approach:
      - Add budget constants, wire `create_params`, register a near-heap callback that terminates execution, map termination to a stable sanitized diagnostic such as `clay.runtime.heap_limit`, and keep the existing timeout diagnostic separate.
    - API Notes and Examples:
      ```rust
      let create_params = v8::Isolate::create_params().heap_limits(0, JS_RUNTIME_HEAP_LIMIT_BYTES);
      JsRuntime::new(RuntimeOptions { create_params: Some(create_params), ..Default::default() });
      runtime.add_near_heap_limit_callback(|current, _initial| {
          isolate_handle.terminate_execution();
          current
      });
      ```
    - Files to Create/Edit:
      - `src/perf/budgets.rs`: Added non-configurable `JS_RUNTIME_HEAP_LIMIT_BYTES`.
      - `src/server/js_runtime.rs`: Added V8 create params, near-heap callback, `clay.runtime.heap_limit` diagnostic mapping, worker stop after heap failure, and heap-growth test.
      - `docs/reference/clay-js-api/configuration.md`: Documented heap guard as non-configurable.
      - `docs/wiki/modules/embedded-js-runtime.md`: Documented heap-limit implementation and diagnostic.
      - `docs/wiki/modules/persistent-runtime-hardening.md`: Updated hardening status and focused tests.
      - `tests/clay_js_api_inventory.rs`: Asserted no hidden heap-limit/sandbox/third-party configuration APIs.
    - References:
      - `deno_core v0.400.0` local rustdoc/source; Plan 030 runtime hardening notes.
  - Test Cases to Write:
    - `cargo test js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic --lib`: JS allocation loop hits heap guard and returns `clay.runtime.heap_limit`.
    - `cargo test js_runtime_short_timeout_does_not_break_fast_evaluation --lib`: Fast runtime evaluation still succeeds with heap guard installed.
    - `cargo test js_runtime --lib`: Runtime boundary regression coverage still passes.
    - `cargo test --test clay_js_api_inventory phase18_7_persistent_runtime_does_not_add_hidden_configuration_knobs`: Heap/sandbox/third-party budgets cannot be changed through `clay:configuration` or `init.js`.
    - `cargo test --test clay_js_api_inventory plan_030_security_budgets_are_intentionally_non_configurable`: Compiled security budgets remain non-configurable.
    - `cargo fmt --check`: Formatting gate passed.

- [x] Verify runtime recovery after heap-limit and timeout termination
  - Acceptance Criteria:
    - Functional: After timeout or heap-limit termination, the runtime generation is either safely reused or replaced by a fresh generation with stale handlers invalidated.
    - Performance: Recovery work stays off client hot paths and does not block local typing/rendering.
    - Code Quality: Recovery semantics are explicit and shared with Phase 19 generation replacement instead of ad hoc reset logic.
    - Security: A malicious package cannot leave half-registered handlers, half-published behavior manifests, or unsanitized diagnostics after termination.
  - Approach:
    - Documentation Reviewed:
      - `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md`
      - `docs/wiki/modules/parse-task-lifecycle.md`
      - `src/server/js_runtime.rs` timeout handling.
    - Options Considered:
      - Always reuse isolate after termination. Rejected unless tests prove state safety.
      - Always replace runtime generation after heap-limit termination. Chosen default for memory guard failure; safer and aligns with Phase 19 generation semantics.
      - Keep timeout reuse as today if existing tests prove it remains safe. Acceptable for timeout-only path.
    - Chosen Approach:
      - Treat heap-limit termination as generation-poisoning: publish diagnostic, cancel parse tasks for that generation, and require a fresh runtime generation before more package JS executes. Keep or adjust timeout reuse based on existing watchdog tests.
    - API Notes and Examples:
      ```text
      heap-limit -> mark generation failed -> cancel parse work -> reload/restart generation -> keep prior validated client state until replacement succeeds
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime.rs`: Added restartable runtime worker ownership and focused timeout/heap recovery tests. Terminated workers exit; the next controlled evaluation lazily starts a fresh worker.
      - `src/server/parse_coordinator.rs`: No code change; Phase 19 generation-scoped handler replacement/stale-result checks already cover parse recovery. JS parse-handler tokens are not replayed into a fresh worker.
      - `docs/wiki/modules/embedded-js-runtime.md`: Documented timeout/heap worker poisoning and recovery semantics.
      - `docs/wiki/modules/persistent-runtime-hardening.md`: Updated recovery status and verification commands.
    - References:
      - Phase 19 runtime generation plan; parse lifecycle wiki.
  - Test Cases to Write:
    - `cargo test js_runtime_timeout_recovery_uses_fresh_worker --lib`: Timeout failure stops the worker; subsequent safe evaluation runs on a fresh worker with no stale global state.
    - `cargo test js_runtime_heap_limit_recovery_uses_fresh_worker --lib`: Heap-limit failure stops the worker; subsequent safe evaluation runs on a fresh worker with no stale global state.
    - `cargo test js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic --lib`: Heap-limit diagnostic remains distinct and sanitized.
    - `cargo test js_runtime_infinite_loop_is_terminated_with_timeout --lib`: Timeout diagnostic remains distinct and sanitized.
    - `cargo test js_runtime --lib`: Runtime boundary regression coverage, including behavior/decorations and parse-handler bridge, still passes.
    - `cargo test --test parse_coordinator`: Generation-scoped parse cancellation/stale-result coverage still passes.
    - `cargo test --test package_loading_docs persistent_runtime_hardening_gate_doc_covers_threat_model`: Hardening wiki gate remains covered.
    - `cargo fmt --check`: Formatting gate passed.

- [x] Design the separate-process JavaScript runtime sandbox gate
  - Acceptance Criteria:
    - Functional: A design document defines the process boundary, supervisor lifecycle, message protocol, allowed requests, cancellation, restart, diagnostics, and migration path from in-process first-party runtime.
    - Performance: Design includes measurable startup, package-load, parse, and reload overhead targets and states that typing/rendering do not depend on sandbox round trips.
    - Code Quality: The process protocol carries inert requests/results and does not expose Rust internals, raw ops, V8 handles, or package JS functions across process boundaries.
    - Security: The sandbox design denies filesystem, network, shell, WASM, AI, package-manager, native-widget, client-JS, and raw-op authority unless a later approved decision grants a narrow capability.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-security.md`
      - `docs/wiki/modules/embedded-js-runtime.md`
      - `docs/wiki/modules/package-loading.md`
      - `roadmap.md` Phase 23.
    - Options Considered:
      - Same-process hardening only. Rejected for third-party package execution.
      - One sandbox per package. Strong isolation but likely high startup/memory cost; keep as option for untrusted packages.
      - One sandbox per runtime generation. Simpler and matches current generation model; chosen initial design target.
    - Chosen Approach:
      - Design a supervised child process that receives validated module/package/parse requests over a bounded protocol and returns inert outputs. The parent owns documents, permissions, package metadata, behavior publication, diagnostics, and restart policy.
    - API Notes and Examples:
      ```text
      parent: RuntimeRequest::Parse { generation, token, windows, budgets }
      child:  RuntimeResponse::ParseUpdate { inert_json }
      parent validates before publishing
      ```
    - Files to Create/Edit:
      - `docs/design/persistent-runtime-sandbox.md`: Added sandbox design and migration gate covering process boundary, supervisor lifecycle, bounded protocol, allowed requests, cancellation/restart, diagnostics, denied authorities, performance targets, and migration path.
      - `docs/wiki/modules/persistent-runtime-hardening.md`: Linked and summarized sandbox gate.
      - `tests/package_loading_docs.rs`: Added docs-as-code guard for process boundary, bounded protocol, restart policy, parent-side validation, denied authorities, hot-path exclusion, and decision-log gate.
      - `plans/034-Persistent-Runtime-Hardening-Before-Third-Party-Package-Authority.md`: Marked design task complete and recorded verification.
    - References:
      - Authority boundaries and package distribution patterns.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs persistent_runtime_sandbox_design_pins_process_boundary`: Design-level guard requires denied-authority list, bounded protocol, restart policy, parent-side validation, hot-path exclusion, and decision-log gate language.
    - `cargo test --test package_loading_docs persistent_runtime_hardening_gate_doc_covers_threat_model`: Hardening wiki links the sandbox design gate.
    - `cargo test --test package_loading_docs`: Full package docs-as-code suite passes.
    - `cargo fmt --check`: Formatting gate passed.

- [x] Implement a minimal sandbox supervisor behind an internal feature gate or test harness
  - Acceptance Criteria:
    - Functional: A parent process can spawn a Clay runtime child, evaluate a controlled first-party package request, receive inert results, cancel/kill the child on timeout, and restart it.
    - Performance: Supervisor overhead is measured against in-process runtime startup/evaluation and documented; no typing/rendering hot-path dependency is introduced.
    - Code Quality: The supervisor is internal and reusable; it does not become a public Clay JS API by accident.
    - Security: The child process receives no broad workspace/file/package-manager authority and all child output is revalidated by the parent before publication.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime.rs`
      - `src/protocol/codec.rs` for bounded message patterns.
      - `docs/wiki/modules/maintenance-validation.md`
    - Options Considered:
      - Implement full sandbox immediately. Rejected; too broad before protocol shape is tested.
      - Internal harness first. Chosen; proves process lifecycle and kill/restart semantics without third-party authority.
    - Chosen Approach:
      - Add the smallest child-process harness needed for tests: spawn, handshake, evaluate bounded fixture, timeout kill, restart. Keep existing in-process runtime as production default until the sandbox decision gate is approved.
    - API Notes and Examples:
      ```bash
      cargo test --test runtime_sandbox_harness
      ```
    - Files to Create/Edit:
      - `src/server/runtime_sandbox.rs`: Added internal supervisor and request/response handling for child spawn/handshake, controlled evaluation, parent timeout kill, payload-budget rejection, and sanitized protocol errors.
      - `src/bin/clay-runtime-sandbox.rs`: Added minimal child entrypoint using a no-extension `deno_core::JsRuntime` and newline-delimited JSON over stdio.
      - `tests/runtime_sandbox_harness.rs`: Added lifecycle tests for harmless first-party fixture evaluation, timeout kill plus fresh restart, oversized output rejection, and denied filesystem/network/shell globals.
      - `Cargo.toml`: Enabled Tokio `process` feature for async child supervision.
      - `docs/wiki/modules/persistent-runtime-hardening.md`: Documented harness status, source paths, tests, and overhead recording.
      - `docs/design/persistent-runtime-sandbox.md`: Recorded minimal harness status and test coverage.
    - References:
      - Sandbox design doc from prior task.
  - Test Cases to Write:
    - `cargo test --test runtime_sandbox_harness`: Child starts and evaluates a harmless first-party fixture; infinite loop is killed by parent timeout and a fresh child restarts; oversized child output is rejected by the parent payload budget; child runtime exposes no filesystem/network/shell globals.
    - `cargo test --test package_loading_docs persistent_runtime_sandbox_design_pins_process_boundary`: Sandbox design guard still passes after implementation notes.
    - `cargo test --test clay_js_api_inventory phase18_7_persistent_runtime_does_not_add_hidden_configuration_knobs`: Harness adds no hidden Clay JS configuration API.
    - `cargo fmt --check`: Formatting gate passed.

- [ ] Keep non-`@clay/*` package execution blocked until authority approval
  - Acceptance Criteria:
    - Functional: Third-party/non-`@clay/*` specifiers still fail before execution, even if package-manager installation exists.
    - Performance: Rejection happens during install/enable/load validation, not during edit hot paths.
    - Code Quality: Guards are centralized in package resolver/service code and tests, not scattered string checks in mode-specific paths.
    - Security: No registry-fetched module, lifecycle script, package-manager output, or user package root can execute through the server runtime without a new approved decision log.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-16-1526-generic-first-party-package-loadentry-module-bridge.md`
      - `docs/reference/primitives/package-loading.md`
      - `src/server/ops/packages.rs`
      - `tests/package_loading_docs.rs`
    - Options Considered:
      - Allow third-party loading into sandbox as soon as harness works. Rejected; still needs package trust/signature/permission policy.
      - Keep existing first-party-only resolver and add regression tests. Chosen.
    - Chosen Approach:
      - Strengthen docs/tests that non-`@clay/*` execution remains disabled until sandbox + authority decision + Phase 23 ecosystem policy are complete.
    - API Notes and Examples:
      ```javascript
      await loadPackage('@clay/markdown'); // allowed today
      await loadPackage('some-third-party'); // rejected until approved authority expansion
      ```
    - Files to Create/Edit:
      - `src/server/ops/packages.rs`: resolver guard changes only if tests expose gaps.
      - `tests/package_loading_docs.rs`: third-party execution gate tests.
      - `docs/reference/primitives/package-loading.md`: clarify blocked third-party execution gate.
    - References:
      - Phase 18.6 authority decision log; Phase 23 roadmap.
  - Test Cases to Write:
    - `loadPackage('left-pad')`, `loadPackage('@scope/pkg')`, URL, path, and traversal specifiers are rejected before module loading.
    - `pnpm add` metadata does not imply execution authority.

- [ ] Create the third-party runtime authority decision log before enabling execution
  - Acceptance Criteria:
    - Functional: Before any third-party runtime execution ships, an approved decision log records the exact new authority, sandbox model, denied authorities, permissions, package trust policy, and rollback/revisit conditions.
    - Performance: Decision evidence includes measured sandbox/heap overhead and confirms no hot-path dependency.
    - Code Quality: The decision distinguishes install, enable, load, runtime execution, package-manager execution, and client behavior delivery.
    - Security: Approval is explicit; no code path enables non-`@clay/*` execution before the log is approved.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - Phase 18.6 and 18.7 authority decision logs.
    - Options Considered:
      - Write approval now. Rejected; this plan is not implementation approval for third-party execution.
      - Require a future explicit authority log after hardening evidence exists. Chosen.
    - Chosen Approach:
      - Add a hard gate task that cannot be marked complete until the user explicitly approves the third-party authority expansion after reviewing heap/sandbox evidence.
    - API Notes and Examples:
      ```text
      No approved decision log -> non-@clay/* runtime execution remains disabled.
      ```
    - Files to Create/Edit:
      - `decision-logs/<date>-third-party-package-runtime-authority.md`: only after explicit approval.
      - `.agents/skills/project-patterns/references/package-distribution.md`: update only if the approved decision changes durable package guidance.
    - References:
      - `create-decision-log` skill; prior authority logs.
  - Test Cases to Write:
    - A docs/inventory test fails if third-party execution is enabled without a matching approved decision-log reference.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Runtime heap limits, sandbox kill timeouts, and denied-authority gates are server-owned security budgets, not undocumented `init.js` keys; any user-visible diagnostic/control is documented as a Clay JS API or explicitly internal.
    - Performance: Configuration review adds no runtime hot-path work.
    - Code Quality: No hidden JSON/TOML/ad hoc configuration key is added.
    - Security: User configuration cannot raise heap limits, disable sandboxing, enable third-party execution, or grant filesystem/network/shell/package-manager authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay Configuration Task.
      - `docs/reference/clay-js-api/configuration.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
    - Options Considered:
      - Expose heap/sandbox budgets as user options. Rejected; security boundaries.
      - Keep them compiled constants and diagnostics. Chosen.
    - Chosen Approach:
      - Add docs/tests proving these hardening knobs are non-configurable unless a later approved decision says otherwise.
    - API Notes and Examples:
      ```text
      clay.runtime.heap_limit is a diagnostic, not a user configuration API.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`
      - `tests/clay_js_api_inventory.rs`
    - References:
      - Plan 030 security budget policy.
  - Test Cases to Write:
    - Inventory test rejects `clay.configuration.setRuntimeHeapLimit`, `setSandboxDisabled`, `enableThirdPartyPackages`, or similar hidden APIs.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Public diagnostics or developer commands introduced by hardening have Markdown docs, inventory entries, generated registry coverage, user-facing names, key binding metadata, custom properties, examples, errors, permissions, backing Rust paths, ops/facades, and lookup tags; internal helpers remain private or `pub(crate)`.
    - Performance: API docs state hardening work is server-first/background and not a typing/rendering hot path.
    - Code Quality: Raw `Deno.core.ops` names and sandbox protocol messages are not public user APIs.
    - Security: API docs preserve denied-authority language and sanitized diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `docs/index.md`
    - Options Considered:
      - Public API for sandbox controls. Rejected unless a user-facing feature is intentionally shipped.
      - Internal-only hardening plus documented diagnostics. Likely chosen.
    - Chosen Approach:
      - Inventory all new Rust visibility and document only real public programmatic behavior.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test clay_js_api_inventory
      cargo test --test clay_js_doc_registry
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**` as needed.
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `docs/generated/clay-js-api-registry.json`
      - `tests/clay_js_api_inventory.rs`
      - `tests/clay_js_doc_registry.rs`
    - References:
      - Clay JS API boundary and naming patterns.
  - Test Cases to Write:
    - Registry freshness and API inventory tests pass, or internal-only assertion proves no public API was added.

- [ ] Verify hardening with full security, performance, and repository gates
  - Acceptance Criteria:
    - Functional: Heap-limit, timeout, sandbox kill/restart, third-party rejection, denied platform authority, stale parse rejection, and sanitized diagnostic tests pass.
    - Performance: Startup/evaluation overhead is measured and protocol/edit hot-path tests still pass.
    - Code Quality: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` pass.
    - Security: No test skip, broad lint allow, or config knob weakens runtime/package authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/maintenance-validation.md`
      - `docs/development/performance.md`
    - Options Considered:
      - Focused tests only. Rejected; authority hardening needs all-target regression coverage.
      - Full repository gate. Chosen.
    - Chosen Approach:
      - Run focused hardening tests first, then full all-target validation and performance checks.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo test --test performance_protocol
      cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline
      ```
    - Files to Create/Edit:
      - `tests/persistent_runtime_hardening.rs`
      - `tests/runtime_sandbox_harness.rs`
      - Existing security/docs tests as needed.
    - References:
      - Maintenance validation wiki; performance docs.
  - Test Cases to Write:
    - Heap exhaustion bounded and diagnosed.
    - Sandbox child kill/restart works.
    - Third-party execution remains blocked.
    - Platform authorities remain unavailable.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
