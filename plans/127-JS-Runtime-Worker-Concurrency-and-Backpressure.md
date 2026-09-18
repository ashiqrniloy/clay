# 127 — JS Runtime Worker Concurrency and Backpressure

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
P1, P2, P5, U3, A3 (review §2, §4, §5, §10). This plan materially changes package
execution scheduling, so the trust-domain preservation duty applies as a dedicated
task.

## Objectives

- P1: remove head-of-line blocking between latency-sensitive JS lanes
  (completions, language intelligence) and slow lanes (config evaluation, document
  analysis, parse handlers) inside each trust domain.
- P2: bound the worker command queue and supersede stale same-document work
  instead of accumulating it unboundedly while an evaluation runs to its timeout.
- P5: stop the near-heap-limit ratchet (callback doubling the isolate cap
  permanently); restore the configured limit after a recovered evaluation.
- U3: extract the 5 duplicated `RuntimeCommand` match-arm epilogues
  (begin/context/heap-flag/eval/timeout-check) into one helper.
- A3: document the process-wide trust-domain constraint (2 isolates total) in the
  wiki as the standing architecture decision this plan keeps.

## Expected Outcome

- A slow (≤ timeout) document-analysis or parse evaluation no longer delays
  completion/LI requests from any client; demonstrated by a test holding one lane
  busy while a completion round-trips under its existing budget.
- Worker queue depth is bounded; superseded work is dropped/aborted and observable
  in existing runtime metrics; no request loss for distinct documents.
- All Linux gates green; js_runtime suites (`src/server/js_runtime/tests.rs`) pass
  including the timeout/heap-limit recovery tests, updated for pooling.
- Two trust domains (Trusted/ThirdParty) preserved exactly: no V8 objects,
  functions, globals, or module instances cross domains; third-party workers never
  gain trusted ops.

## Tasks

- [ ] Baseline gates and concurrency baselines on the unmodified tree
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full stage set passes untouched; record exit codes.
    - Performance: record a baseline of "completion request latency while a parse handler holds the domain worker for X ms" (temporary probe; removed after recording).
    - Code Quality: no code changes.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §2 (P1/P2), §10 (A3).
    - Options Considered:
      - Skip baseline: scheduling changes are exactly where regressions hide. Fresh baseline chosen.
    - Chosen Approach:
      - One-shot probe using the existing js_runtime test harness (`with_timeout`, controlled-source evals).
    - Files to Create/Edit:
      - None.
    - References:
      - `src/server/js_runtime/tests.rs` (harness patterns), `src/perf/budgets.rs` (`JS_RUNTIME_EVALUATION_TIMEOUT_MS`, `JS_RUNTIME_HEAP_LIMIT_BYTES`).
  - Test Cases to Write:
    - None (evidence-recording task).

- [ ] Extract the worker command epilogue (U3)
  - Acceptance Criteria:
    - Functional: `run_runtime_worker`'s five arms share one `run_bounded_evaluation` helper wrapping begin/package-context/heap-flag/eval/timeout-or-heap check; behavior identical (poison on Timeout|HeapLimit, response sent before break).
    - Performance: no extra allocation on the command path.
    - Code Quality: clippy clean; the poison decision lives in exactly one place.
    - Security: package provenance stamping unchanged for every command kind.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/worker.rs:230–345` (the five arms).
    - Options Considered:
      - Leave duplication until after pooling: pooling touches the same lines; extract first so the pool lands on the shared helper. (Chosen order: this task before pooling.)
    - Chosen Approach:
      - Closure-parameterized helper: `(setup, evaluate) -> (result, poison)`; arms shrink to setup closures.
    - Files to Create/Edit:
      - `src/server/js_runtime/worker.rs`.
    - References:
      - Review U3.
  - Test Cases to Write:
    - Existing timeout/heap-limit recovery suites stay green (no new suite for a pure extraction).

- [ ] Lane-aware scheduling: separate latency lanes per domain (P1)
  - Acceptance Criteria:
    - Functional: within each domain, completion and language-intelligence commands run on a dedicated worker lane; a lane busy to its timeout does not delay the other lane. Config/parse/analysis keep the general lane. Cross-domain routing (trusted config loading third-party packages) unchanged.
    - Performance: baseline scenario from task 1 improves to "completion latency unaffected by a concurrently busy general lane" (bounded by its own timeout); no measurable regression in single-lane throughput (benchmark suite `benches/protocol_server_baselines.rs` unchanged).
    - Code Quality: lane count and per-lane heap limits are `src/perf/budgets.rs` constants; worker replacement (poison) applies per lane.
    - Security: each lane's isolate gets the same domain extension set and load-entry allowlist as today — trusted-only ops never registered in third-party lanes (asserted by existing denial tests, extended per lane).
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/worker.rs` (worker start/replace), `mod.rs` (`DomainRuntime`, `replace_domain_worker`, `production_reload` sharing the third-party domain).
      - `src/server/document_analysis.rs` (the 4-worker bounded pool precedent to generalize).
      - `.agents/skills/clay-execution/references/packages.md` (trust-domain authority boundaries).
    - Options Considered:
      - Full per-package isolates: RAM cost ×N packages; no evidence of need.
      - One extra lane per domain for latency-sensitive commands: bounded, reuses `DomainRuntime`/generation machinery. (Chosen.)
      - Task-thread pool per domain shared by all command kinds: completes the analysis-pool generalization but complicates per-isolate op-state (module registrations are isolate-global).
    - Chosen Approach:
      - `DomainRuntime` holds `lanes: [Arc<RuntimeWorker>; N]` where N is small (2: latency + general); registrations must be replayed to a new lane's isolate on replacement via the existing `harvest_op_state_evaluation` path (Plan 061 task 12 precedent for third-party reload).
    - API Notes and Examples:
      ```rust
      // src/server/js_runtime/mod.rs
      enum RuntimeLane { Latency, General }
      fn lane_worker(&self, domain: RuntimeDomain, lane: RuntimeLane) -> Arc<RuntimeWorker>;
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime/mod.rs`, `src/server/js_runtime/worker.rs`, `src/perf/budgets.rs`.
    - References:
      - Review P1, A3; decision log `2026-07-21-0001` (two package runtime trust domains).
  - Test Cases to Write:
    - `latency_lane_unblocked_by_busy_general_lane`: controlled busy-loop parse handler; completion completes under budget.
    - `lane_poison_replaces_only_that_lane`: Timeout on one lane leaves the other serving.
    - `third_party_lane_denies_trusted_ops`: per-lane denial test mirroring `cross_domain_load_bridge_rejects_trusted_records`.

- [ ] Bound the command queue and supersede stale work (P2)
  - Acceptance Criteria:
    - Functional: worker input channel is bounded (`budgets` constant); on full queue, a newer command for the same (lane, document, kind) replaces the older undelivered command (matching `CompletionCoordinator`'s abort-on-supersede semantics); distinct-document commands never dropped while queue space exists.
    - Performance: flood test (many rapid edits/completions for one document while worker busy) holds memory flat vs unbounded growth today.
    - Code Quality: dropped/superseded counters exported through the existing metrics recorder (`global_recorder`).
    - Security: supersession keys on host-stamped identity (document id + command kind), never package-controlled fields.
  - Approach:
    - Documentation Reviewed:
      - `src/server/completion.rs:1030–1110` (abort/supersede precedent), `worker.rs:135` (channel creation), `src/perf/metrics.rs`.
    - Options Considered:
      - Backpressure via blocking send: stalls the editor event loop — worse than drop for stale completions.
      - Bounded + supersede: consistent with the coordinator's existing policy. (Chosen.)
    - Chosen Approach:
      - Replace `mpsc::channel()` with a bounded channel plus a small supersede index at the dispatch boundary (sender side, under `DomainRuntime`'s existing mutex).
    - Files to Create/Edit:
      - `src/server/js_runtime/worker.rs`, `src/server/js_runtime/mod.rs`, `src/perf/budgets.rs`.
    - References:
      - Review P2.
  - Test Cases to Write:
    - `queue_bounded_under_flood`: N rapid same-document completions while busy → bounded queued commands, newest wins, older responses dropped not delivered stale.
    - `distinct_documents_never_superseded`.

- [ ] Fix the heap-limit ratchet (P5)
  - Acceptance Criteria:
    - Functional: after an evaluation that tripped the near-heap callback but completed without `HeapLimit` error, the isolate's limit is restored to the configured `heap_limit_bytes`; a subsequent evaluation sees the original cap.
    - Performance: none beyond correctness of the cap.
    - Code Quality: unit test with a lowered synthetic heap limit exercising near-miss recovery.
    - Security: heap ceiling stays enforced — no permanent doubling of package memory authority.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/worker.rs:371–392` (`add_near_heap_limit_callback`), deno_core/v8 `remove_near_heap_limit_callback`/`set_heap_limit` API (verify against the vendored `deno_core` version via `cargo tree -i deno_core` + rustdoc before writing).
    - Options Considered:
      - Keep ratchet (worker replacement masks it): leaves recovered workers with 2× authority indefinitely.
      - Restore configured limit after each evaluation. (Chosen.)
    - Chosen Approach:
      - After `evaluate_*` returns without `HeapLimit`, reset the limit on the isolate handle.
    - Files to Create/Edit:
      - `src/server/js_runtime/worker.rs`.
    - References:
      - Review P5.
  - Test Cases to Write:
    - `near_heap_limit_recovers_with_original_cap`: synthetic small heap; first eval brushes the limit and finishes; second eval's effective limit equals configuration.

- [ ] Preserve and prove the two trust domains under the new scheduling
  - Acceptance Criteria:
    - Functional: all existing cross-domain denial/reload/revocation suites pass; new lanes inherit domain extensions exactly (no lane-specific op sets).
    - Performance: none.
    - Code Quality: a single `start_domain_runtime` path constructs every lane (no per-lane bespoke wiring).
    - Security: cross-domain communication remains typed, bounded, inert, Rust-mediated with generation/payload/timeout/provenance/revocation checks; trusted classification still compiled-inventory-based.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (Package Runtime Trust-Domain Task).
      - Decision log `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
    - Options Considered:
      - Fold into lane task: a standalone proof task keeps the security invariant auditable per plan. (Chosen: standalone.)
    - Chosen Approach:
      - Enumerate the required denial/reload/revocation test names from the decision log; run/extend against the pooled runtime.
    - Files to Create/Edit:
      - `src/server/js_runtime/tests.rs` (extensions only).
    - References:
      - Decision log 2026-07-21-0001.
  - Test Cases to Write:
    - `reload_shares_third_party_lanes_untouched` (pooled analog of the Plan 061 reload test).
    - `revoked_package_commands_refused_per_lane`.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: completion/typing modules re-run on a real Linux build with a third-party package registered; record pass/fail; add a step "completions remain responsive while a package parse handler runs" if a reproducible manual trigger exists (else record as automated-only with reason).
    - Performance: steps record perceived typing latency with packages active.
    - Code Quality: `test-plan/index.md` updated if steps are added.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`, `.agents/skills/create-plan/references/clay.md`.
    - Chosen Approach:
      - Automated suites carry the scheduling proofs; manual pass verifies no user-visible typing regressions.
    - Files to Create/Edit:
      - `test-plan/04-core-editing.md` (tentative — exact module per index), `test-plan/index.md`.
    - References:
      - `scripts/editor-performance-smoke.sh`.
  - Test Cases to Write:
    - Manual steps as described.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface added or changed (scheduling is internal); verify via phase diff; any new Rust function stays `pub(crate)`.
    - Performance: none.
    - Code Quality: registry/doc-guard suites pass unchanged.
    - Security: package load/activation API behavior identical.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach:
      - Verify-only task; record "no JS API change" in evidence.
    - Files to Create/Edit:
      - None expected.
    - References:
      - Decision log 2026-05-08-1509.
  - Test Cases to Write:
    - None.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the persistent-runtime wiki page(s) document lane scheduling, bounded queues, supersession, heap-limit restoration, and the process-wide two-domain constraint (A3) with invariants and test paths.
    - Performance: wiki notes lane/queue/heap budgets and their constants.
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: documents that lane isolation is scheduling-only — not a trust boundary; the domain remains the boundary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`; `docs/wiki/modules/persistent-runtime-*.md` (existing pages).
    - Chosen Approach:
      - Update once after tests pass.
    - Files to Create/Edit:
      - `docs/wiki/modules/persistent-runtime-hardening.md` (tentative — locate via index), `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
