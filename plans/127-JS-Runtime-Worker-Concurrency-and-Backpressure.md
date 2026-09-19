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

- [x] Baseline gates and concurrency baselines on the unmodified tree
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
  - Outcome (2026-09-19):
    - Evidence: `code-reviews/2026-09-18-plan127-baseline/` (README with
      tables, per-stage logs, probe source).
    - Tree: HEAD `b04f46b` + Plan 126's completed uncommitted working-tree
      changes (plan-127-untouched); `git diff | sha256sum`
      `4d6cd1e…4d54c2d` verified identical before the gate run and after probe
      removal.
    - Gates: audit/fmt/check/clippy/bench-compile/bindings exit 0; `test`
      exit 101 for the same pre-existing parallel-ordering red as the Plan 126
      baseline
      (`server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`,
      `tests.rs:10797`; `--no-fail-fast` per target: lib 1400/1 failed, main+bin
      ok, presentation 62, protocol 223, runtime 75, security 152 all green —
      Plan 126's intermittent daemon-session-bind red did not reproduce).
    - Baseline (temporary probe, removed; debug build): completion issued 50 ms
      into a same-domain parse handler busy for X ms waited `~X − 50 ms` —
      54.4–56.1 ms at X=100, 204.2–206.6 ms at X=250, 454.0–455.7 ms at X=500 —
      versus 2.6–4.3 ms idle median (head-of-line blocking confirmed, no lane
      separation). A 20-request same-document flood during the busy window was
      accepted 20/20 and drained in arrival order with max latency
      `~X − 50 ms + 20 × per-eval` (unbounded queue, no supersede);
      `workers_started` stayed 2 (one worker per trust domain, no replacement)
      across every scenario.

- [x] Extract the worker command epilogue (U3)
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
  - Outcome (2026-09-19):
    - Deliverable: `src/server/js_runtime/worker.rs` only (+120/−88). New
      `run_bounded_evaluation<T>(op_state, heap_limit_hit, package_context,
      evaluate, response) -> bool` owns `begin_evaluation` → optional
      package-context/activation → heap-flag reset → `evaluate` → the single
      `Timeout|HeapLimit` poison decision → response send; all five arms route
      through it and `break` on its `true`.
    - The two arms that stamp a distinct runtime context (`Evaluate`,
      `DocumentAnalysis`) call `set_runtime_context` immediately before the
      helper, preserving the old order (context → begin_evaluation →
      provenance → heap flag → eval). The trusted-only config reject and the
      `prepare_runtime_entry` failure keep sending their typed error and
      continuing without entering the helper, as before; `main_module_loaded`
      is still set after the Evaluate eval attempt, and `analysis_timeout` is
      pure and computed from `event` only.
    - Performance: no allocation added — `Option<PackageContext>` is moved into
      the helper and the closures capture by reference/move; no new `String`,
      `Vec`, or channel on the command path.
    - Gates: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy
      --all-targets -- -D warnings` all exit 0; serial lib `1401 passed; 0
      failed; 1 ignored` (the parallel-ordering red is serial-green as
      before); `cargo test --all-targets --no-fail-fast --quiet` exit 101 with
      only that same pre-existing red — presentation 62, protocol 223,
      runtime 75, security 152 all green.
    - Evidence: `code-reviews/2026-09-18-plan127-baseline/logs/task2-extraction-gates.log`,
      `.../task2-test-all-targets-no-fail-fast.log`.

- [x] Lane-aware scheduling: separate latency lanes per domain (P1)
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
      - `DomainRuntime` holds `lanes: [LaneRuntime; JS_RUNTIME_LANES_PER_DOMAIN]`
        (2: general + latency), each lane with its own isolate, command thread,
        and poison flag; replacement and poison apply per lane.
      - CORRECTION (implementation): the plan expected registration replay via
        `harvest_op_state_evaluation`; that path carries inert metadata only,
        not the JS closures the token registry holds, so replay would have
        required re-running package/config evaluation (duplicated init side
        effects). Implemented instead: registrations may carry a
        host-validated `moduleSpecifier` (document-analyzer precedent) and the
        serving lane materializes the handler by importing that module — no
        replay, and a replaced latency lane self-heals on the next request.
        Inline `module: {...}` registrations stay on the general lane until the
        package adopts `moduleSpecifier`; upgrade path if ever needed is
        replay-warming the latency lane at registration time.
    - API Notes and Examples:
      ```rust
      // src/server/js_runtime/mod.rs
      enum RuntimeLane { General, Latency }
      impl RuntimeLane { fn for_provider(module_specifier: Option<&str>) -> Self }
      fn domain_lane_worker(&self, domain: RuntimeDomain, lane: RuntimeLane) -> Arc<RuntimeWorker>;
      // src/perf/budgets.rs
      const JS_RUNTIME_LANES_PER_DOMAIN: usize = 2;
      const JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES: usize = 32 * 1024 * 1024;
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime/mod.rs`, `src/server/js_runtime/worker.rs`, `src/perf/budgets.rs`.
    - References:
      - Review P1, A3; decision log `2026-07-21-0001` (two package runtime trust domains).
  - Test Cases to Write:
    - `latency_lane_unblocked_by_busy_general_lane`: controlled busy-loop parse handler; completion completes under budget.
    - `lane_poison_replaces_only_that_lane`: Timeout on one lane leaves the other serving.
    - `third_party_lane_denies_trusted_ops`: per-lane denial test mirroring `cross_domain_load_bridge_rejects_trusted_records`.
  - Outcome (2026-09-19):
    - Deliverable: `RuntimeLane::{General, Latency}` per trust domain
      (`DomainRuntime.lanes` array sized by `JS_RUNTIME_LANES_PER_DOMAIN`),
      each lane an isolate + command thread + poison flag; per-lane
      poison/replacement (`replace_domain_worker(domain, lane)`, domain
      generation bumps only for the general lane); lane threads named
      `clay-js-runtime[-third-party][-latency]`.
    - Routing: `Evaluate`/`Parse`/`DocumentAnalysis` → general;
      `Completion`/`LanguageIntelligence` → latency iff the registration
      carries `moduleSpecifier`, else general (inline closure path unchanged).
      `Completion`/`LI` regs gained `module_specifier: Option<String>`,
      validated in both ops by
      `load_entry_allowlist().is_package_module(spec, package)` (same rule as
      analyzers); `evaluate_js_*_provider` imports the module when present,
      otherwise uses the token registry as before. Facades needed no JS change
      (options pass through); `.d.ts` documents `moduleSpecifier`.
    - Cross-domain: trusted general → third-party general (package loads) keeps
      Plan 061 task 12 semantics; the third-party latency lane receives
      active-mode replication only and the trusted latency lane gets no bridge,
      so latency-lane provider modules cannot drive package loading.
    - Security: every lane carries its domain's extension set (third-party
      lanes still lack trusted-only ops); `shutdown_generation_resources`,
      `shutdown_trusted_generation_resources`, and session counts now cover
      every lane so a provider module cannot strand a language-server child.
    - Performance: `PLAN127_LANE` — completion while a parse handler holds the
      general lane for 500 ms: busy completion 4.1 ms / idle median 1.8 ms on a
      quiet host (24.9 ms / 7.0 ms under concurrent full-gate load), versus
      454–456 ms in the task-1 single-worker baseline at X=500; the assertion
      threshold is 250 ms. Inline-provider throughput is untouched (same
      general-lane path as before); `benches/protocol_server_baselines.rs`
      covers protocol codec/doc-ack paths only (no JS runtime), and its
      compile gate is green.
    - Cost: lanes are created eagerly, so a service now starts 4 isolates
      (2 domains × 2 lanes) instead of 2; ceilings stay budgeted (general =
      configured `JS_RUNTIME_HEAP_LIMIT_BYTES`, latency ≤
      `JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES` = 32 MiB, and never above a
      synthetic small configured ceiling). Test expectations updated to the
      lane baseline (`workers_started` 2→4 in the three isolation tests).
    - Tests added: the three plan cases plus
      `language_intelligence_module_specifier_serves_from_latency_lane`,
      `completion_and_language_providers_reject_unowned_module_specifier`, and
      `lane_heap_limits_stay_within_the_configured_budget`.
    - Gates: audit/fmt/check/clippy/bench-compile/bindings exit 0; the `test`
      stage exits 101 for the suite's pre-existing parallel-ordering/timing
      sensitivity — final gate run: 1406 passed / 1 failed
      (`third_party_poison_replays_approved_graph_and_restores_providers`, a
      100 ms-budget fixture test), and two immediate parallel re-runs tripped
      only the baseline red (`coding_agent_clean_init_one_line_activates_working_defaults`).
      Every version is green under `--test-threads=1` (1407 passed / 1 ignored),
      which is the reference run for this change.
    - Evidence: `code-reviews/2026-09-18-plan127-baseline/logs/task3-gates.log`,
      `.../task3-stage-exit-codes.txt`, `.../run-stages-task3.sh`,
      `.../task3-lane-evidence.log`.

- [x] Bound the command queue and supersede stale work (P2)
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
    - `queue_evicts_oldest_at_capacity` (added): past
      `JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY`, the oldest undelivered
      supersedable command is dropped for the newest.
  - Outcome (2026-09-19):
    - Deliverable: per-lane `CommandMailbox` (`worker.rs`) replaces the
      unbounded `mpsc::channel()`: a `Mutex<VecDeque<RuntimeCommand>>` +
      `Condvar` the command thread waits on, bounded by
      `JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY = 64` undelivered supersedable
      commands. `RuntimeCommandSender` (cloneable handle, used by dispatch and
      by the op-state bridges) keeps `send` semantics; `Drop for RuntimeWorker`
      now closes the mailbox instead of sending a `RuntimeCommand::Shutdown`
      sentinel (variant removed) — close drains the bounded queue, then the
      thread exits.
    - Supersede rule: a newer `Completion`/`LanguageIntelligence` command for
      the same work key replaces the older undelivered one; at capacity the
      oldest supersedable command is evicted (stale-first). Dropped commands
      answer the new `ClayRuntimeError::Superseded` (diagnostic
      `runtime.superseded`) and never deliver a stale result. Keys are
      host-stamped only: command kind + `client_id` + `document_id` +
      the runtime's registration token, so one package cannot claim or evict
      another package's queued work, and keys are lane-scoped.
    - Non-supersedable kinds are deliberately exempt from both
      capacity and supersede: `Evaluate`, `Parse`, `DocumentAnalysis`
      (deltas/side effects a newer sibling does not re-encode; one pending job
      per document already enforced by their coordinators) and
      `UpdateActiveEditorMode` (host state). They are also never dropped.
    - CORRECTION/refinement vs the Approach above: (a) the queue lives in the
      worker, not under `DomainRuntime`'s worker mutex — that mutex also guards
      the worker swap and cannot be held while the command thread waits;
      (b) no side "supersede index" map: the work key is found by scanning a
      queue bounded by `capacity`, so there is no second structure to keep
      consistent; (c) capacity overflow evicts the oldest supersedable command
      instead of rejecting the newest request — no rejection path, no new
      failure mode for callers, and newest work is always admitted.
    - Metrics: `js_runtime.command.superseded` and `js_runtime.command.evicted`
      recorded through `global_recorder()`; the same counts are exposed per lane
      (`ClayJsRuntimeService::lane_queue_stats`) for tests. Documented in
      `docs/development/performance.md` (Plan 127 section).
    - Performance (`PLAN127_QUEUE_*`, serial `--nocapture`):
      - flood, 40 same-document completions during a 400 ms lane hold:
        `peak_pending=1 capacity=64 superseded=38 evicted=0 served=2` — the
        queue never grew with the burst and 38 stale provider invocations never
        ran (task-1 baseline accepted and executed all 20/20 requests with no
        cap).
      - distinct documents, 8 simultaneous: `peak_pending=7 superseded=0
        evicted=0` — all 8 served, queue space never taken from a distinct
        document.
      - capacity, 72 distinct documents: `peak_pending=64 evicted=7 served=65`
        — bounded by the budget, newest request survives.
      - P1 regression check unchanged: completion while the general lane is
        held 500 ms → 2.8 ms busy / 1.7 ms idle median.
    - Tests added: the three cases above (`queue_bounded_under_flood`,
      `distinct_documents_never_superseded`,
      `queue_evicts_oldest_at_capacity`). Mutation check: forcing
      `supersede_key()` to `None` makes the flood and capacity tests fail, so
      they are not vacuous.
    - Gates (frozen tree): audit/fmt/check/clippy/test/bench-compile/bindings
      all exit 0 — `cargo test --all-targets` green in that run; an earlier run
      of the same tree tripped only the suite's pre-existing parallel-ordering
      red (`coding_agent_clean_init_one_line_activates_working_defaults`,
      unchanged from the task-1/task-3 baselines). Serial `--test-threads=1` lib
      run fully green (1410 passed / 1 ignored).
    - Evidence: `code-reviews/2026-09-18-plan127-baseline/logs/task4-gates.log`,
      `.../task4-stage-exit-codes.txt`, `.../task4-queue-evidence.log`,
      `.../run-stages-task4.sh`.

- [x] Fix the heap-limit ratchet (P5)
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
  - Outcome (2026-09-19):
    - Deliverable: `worker.rs` grew `install_heap_limit_callback` (the
      near-heap supervision) and `restore_heap_limit` (cancel a pending
      termination request, drop the raised limit, re-arm supervision).
      `run_bounded_evaluation` now takes the isolate and the lane's
      `heap_limit_bytes` and owns both the poison decision and the restore, so
      all five command kinds — Evaluate, Parse, Completion, DocumentAnalysis,
      LanguageIntelligence — recover through one path; `create_js_runtime`
      installs the same callback it re-arms with.
    - API correction vs the Approach above: the vendored stack is
      `deno_core 0.400.0` / `v8 147.4.0` (`cargo tree -i v8`), and v8 exposes
      **no** `set_heap_limit` binding — only
      `JsRuntime::remove_near_heap_limit_callback(heap_limit)`
      (`deno_core-0.400.0/runtime/jsruntime.rs:1980`) wrapping v8's
      `RemoveNearHeapLimitCallback` (`v8-147.4.0/src/isolate.rs:1613`, header
      doc: "Remove the given callback and restore the heap limit to the given
      limit … if the current heap size is greater than the given limit, then the
      heap limit is restored to the minimal limit that is possible for the
      current heap size"). The restore is therefore remove-and-re-arm, not a
      limit setter. v8's own `AutomaticallyRestoreInitialHeapLimit` would be
      cheaper (one call) but is not exposed by the Rust binding and only
      restores on a later heap-shrink threshold, so it cannot guarantee the cap
      for the next evaluation.
    - Also fixed in the same path: a termination request that raced the
      completion of an evaluation (near-heap callback or watchdog firing after
      the evaluation's own check) used to be carried into the next evaluation,
      which then died with a confusing terminated-execution error. The restore
      clears it; the evaluation that armed it already finished.
    - Scope: the restore runs only for evaluations that did not fail with
      `Timeout`/`HeapLimit`; a poisoned worker is discarded, so restoring its
      isolate would be busywork.
    - Test: `near_heap_limit_recovers_with_original_cap` (lib, serial-safe; own
      current-thread tokio runtime, `create_js_runtime` with an 8 MiB configured
      max). A synthetic near-heap callback raises the limit instead of
      terminating, evaluation 1 (400k retained objects, then released)
      completes without poisoning, and evaluation 2 probes the limit that is
      actually in force: a probe callback reports the `current_limit` v8 hands
      it, flags the hit, and terminates. `PLAN127_HEAP configured=8388608
      ratchet=536870912 ratchet_hits=[(2097152, 2097152)]
      used_after_recovery=26796024 recovered_cap=33864540` — the next evaluation
      runs against ~32 MiB (v8's minimal limit for the ~26 MiB live heap), not
      the 512 MiB ratchet, and is poisoned with `HeapLimit`. Note the configured
      8 MiB max-heap becomes an effective ~2 MiB old-generation cap in v8
      (`ConfigureDefaultsFromHeapSize`), which is why the assertions are
      expressed against the observed initial limit and the ratchet rather than
      the raw budget; v8's clamp to the live heap is why the second assertion is
      `recovered_cap <= 2 * used`, not `== heap_limit_bytes`. The security claim
      holds: authority tracks the live heap instead of multiplying, and a
      package that keeps a large live set fails its evaluation and is replaced.
    - Mutation check: removing the `restore_heap_limit` call leaves the 512 MiB
      ratchet latched, the probe never fires, and the test fails
      (`recovered_cap=0`); with the call it passes.
    - Performance: no regression on the scheduling work of tasks 3–4 — queue
      flood/capacity numbers are identical
      (`peak_pending=1 superseded=38 evicted=0` / `peak_pending=64 evicted=7`)
      and the latency-lane probe stays within run-to-run noise (idle
      3.1–3.5 ms vs busy 3.6–4.8 ms across three runs, versus a 500 ms lane
      hold). The added per-command cost is two v8 callback calls.
    - Docs: `docs/reference/clay-js-api/configuration.md` heap-limit bullet now
      states that the callback's raised limit is a stop-gap for one evaluation
      and is restored before the next command.
    - Gates (frozen tree): audit/fmt/check/clippy/test/bench-compile/bindings all
      exit 0; serial `--test-threads=1` lib run fully green (1411 passed /
      1 ignored), including the existing heap-termination and heap-recovery
      tests.
    - Evidence: `code-reviews/2026-09-18-plan127-baseline/logs/task5-gates.log`,
      `.../task5-stage-exit-codes.txt`, `.../task5-heap-ratchet-evidence.log`,
      `.../task5-lane-and-queue-evidence.log`, `.../run-stages-task5.sh`.

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
