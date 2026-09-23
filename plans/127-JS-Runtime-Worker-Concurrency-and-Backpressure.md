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

- [x] Preserve and prove the two trust domains under the new scheduling
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
    - `lanes_share_their_domain_op_set` (added: the acceptance criterion "new lanes
      inherit domain extensions exactly (no lane-specific op sets)" needed its own
      proof; the two named tests cover reload and revocation, not the op inventory).
  - Outcome (2026-09-19):
    - Files went beyond the planned "tests.rs extensions only": proving revocation
      per lane required a host-side revocation gate, because the runtime dispatched
      commands for a revoked/disabled package's stale registration into the lane and
      left the refusal to whatever the generated provider module happened to call.
    - Deliverable (Rust-mediated revocation check, one place, every lane/domain):
      `RuntimeCommand::package_identity()` (worker.rs) derives the host-stamped
      package identity from the registration the host minted (never a
      package-supplied string); `ClayJsRuntimeService::ensure_package_enabled`
      (mod.rs) requires that package to be enabled at that exact version; it runs in
      `dispatch_to_domain` (Completion, Parse, DocumentAnalysis,
      LanguageIntelligence, package-context Evaluate) and in
      `evaluate_entry_for_domain` (package loads/activation), i.e. before the lane's
      mailbox sees the command. Refusal is the new typed
      `ClayRuntimeError::Revoked { package, version }` (Display + `runtime.revoked`
      diagnostic); host/configuration commands carry no package identity and are
      unaffected.
    - Placement detail: the gate sits *after* the poisoned-lane recovery block, so a
      lane left poisoned by an earlier command is still replaced and its isolate
      dropped no matter which command arrives next. Moving it ahead of recovery
      broke `third_party_poison_replay_skips_disabled_packages` (the refused
      completion never consumed the poison flag, so no replacement and no generation
      bump); recovery-first restores that contract.
    - `lanes_share_their_domain_op_set`: enumerates the *running* op inventory from
      inside all four live isolates through the real dispatch path (not a claim about
      the wiring code): `PLAN127_LANES trusted_ops=194 third_party_ops=142
      trusted_only=52`. Both trusted lanes and both third-party lanes install
      identical sets; eight named privileged ops (`op_clay_runtime_ping`,
      `op_clay_configuration_get_state`, `op_clay_documents_open_document`,
      `op_clay_packages_load_package`, `op_clay_packages_load_in_package_domain`,
      `op_clay_language_server_authorize`, `op_clay_modes_classify_document`,
      `op_clay_theme_set_theme`) are absent from both third-party lanes; the service
      started exactly `2 × JS_RUNTIME_LANES_PER_DOMAIN` workers, i.e. every lane of
      both domains comes from one construction path (`start_domain_runtime` loops
      `RuntimeLane::ALL`; `init_runtime_extension(domain)` has a single production
      call site in `create_js_runtime`; only the thread name and the lane heap cap
      differ per lane). Lane isolation is scheduling-only — the domain stays the
      trust boundary.
    - `revoked_package_commands_refused_per_lane`: fixture is two third-party
      packages (one inline provider served by the general lane's global registry,
      one module-backed provider served by the latency lane) because a single
      registration call claims every provider a manifest declares and the lane
      follows that call's `moduleSpecifier`. After the production CLI revocation
      sequence (`revoke_package_approval` then `disable`, launch.rs) both lanes
      refuse with `Revoked` naming their own package (`@vendor/lanerevoke@0.1.0`,
      `@vendor/lanerevokelat@0.1.0`) and `workers_started` is unchanged (a refusal is
      host-side, it replaces no lane). Baseline with the gate disabled, captured by
      mutation: both lanes already failed closed, but only *inside* the isolate —
      `Runtime("Error: packages.package_not_enabled: package `@vendor/lanerevoke`
      version `0.1.0` is not enabled")` raised by the generated provider module — so
      the pre-change refusal was untyped, lane-shaped, and dependent on that module
      continuing to call a provenance-checking op.
    - `reload_shares_third_party_lanes_untouched`: after `production_reload`, the
      third-party domain generation is unchanged, `workers_started` equals exactly
      `JS_RUNTIME_LANES_PER_DOMAIN` (only the trusted domain's lanes were rebuilt),
      the survived registration snapshot still carries both providers, and both
      lanes serve from their original isolates while `workers_started` stays at that
      number (no replacement, no replay).
    - Layering kept auditable: `disabled_package_callback_publications_fail_closed`
      now asserts both layers — a command pushed straight to the lane's mailbox
      (bypassing the gate, i.e. already inside the isolate) still fails closed at op
      ingress with `packages.package_not_enabled`, while the same request through
      `invoke_parse_handler` returns the typed `Revoked`. Neither layer lost its
      regression coverage.
    - Enumerated suites from decision log 2026-07-21-0001, all passing on the pooled
      runtime: 5 cross-domain validator tests (`oversize_payload_and_bad_deadline_…`,
      `stale_requester_and_revoked_approval_rejected`,
      `wrong_target_point_and_operation_denied`,
      `enabled_requester_with_expanded_scope_denied`,
      `mismatched_result_provenance_denied`), 21 js_runtime
      denial/module-boundary/poison/replay/reload/revocation tests (including
      `third_party_runtime_cannot_see_trusted_ops_or_admin_modules`,
      `third_party_cannot_import_trusted_package_modules`,
      `third_party_package_cannot_load_other_packages`,
      `cross_domain_load_bridge_rejects_trusted_records`,
      `third_party_termination_replaces_only_third_party_generation`,
      `third_party_poison_replays_approved_graph_and_restores_providers`,
      `third_party_poison_replay_skips_disabled_packages`,
      `package_load_entry_allowlist_revokes_owned_entries`,
      `trusted_reload_preserves_third_party_providers`,
      `unapproved_third_party_package_never_executes_before_adoption`), plus
      `reload_preserves_authority_denials_and_cleans_old_lsp_worker` and the three new
      tests: 29/29 pass.
    - Verification: `cargo test --lib --all` (frozen tree) all seven gate stages exit
      0; serial `--test-threads=1` lib run green (1414 passed / 1 ignored). No JS API
      change and no op-set change (next task stays verify-only).
    - Evidence: `code-reviews/2026-09-18-plan127-baseline/logs/task6-trust-domain-suites.log`,
      `.../task6-serial-lib-suite.log`, `.../task6-gates.log`,
      `.../task6-stage-exit-codes.txt`, `.../run-stages-task6.sh`.

- [x] Execute and update the manual test plan (test-plan/)
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
  - Outcome (2026-09-19):
    - Modules executed/amended: **04** (new E40), **09** (new P56), **11** (new
      Q42). No existing step was deleted or weakened; each new step is
      referenced exactly once in the parity ledger
      (`docs/development/tauri-react-parity-ledger.json`) and
      `cargo test --test protocol documentation_coverage` passes (12/12).
    - Live run shape: isolated mode-700 root, private socket, bundled packages
      only, portal-driven input (`computer-use-linux` `activate_window` /
      `type_text` / `press_key`), AT-SPI probe for the tree and the popup, and
      `CLAY_PERF_PROFILE=1` + `CLAY_PERF_REPORT_DIR` for measured numbers.
    - E16/E18/E19/E39 re-run with a package registered (bundled `@clay/rust`,
      65 KiB `review.rs`): typing `fn live_probe` / `let value = std.` echoed
      immediately (2,798 → 2,827 chars, caret tracked); the `.` autocomplete
      trigger opened `list box Completions at 507,183,250x130` with rows `rust`
      (group), `as` (selected), `fn`, `fn function snippet`, `if`, `in`; the
      editor reported `editable,focused,has-popup`; `Enter` accepted `as`
      (2,827 → 2,829); a second trigger reopened the popup and `Escape` closed
      it with no text change.
    - New live step (4 E40 + 11 Q42): a ≥1 MiB markdown document with the
      bundled `@clay/markdown` package parse handler registered for the mode.
      Typing 19 characters echoed immediately (806 → 825 chars, caret 19) and
      the measured acknowledgement was `server.edit_ack` p50 0.329 ms / p95
      0.520 ms / max 0.520 ms, with parse work continuing in the background
      (`syntax.parse.invocations` 6, `syntax.edit_to_publish` p50 61.6 ms; cold
      first full parse of the 1 MiB debug-build document 63.2 s, recorded as a
      parse-side ceiling, not input latency). No `js_runtime.command.superseded`
      / `evicted` churn, which is the intended shape: real parse commands are
      admitted and not superseded.
    - Completion-under-a-held-lane half: **automated-only, reason recorded**. No
      bundled package registers a JS completion provider (live completion items
      come from the built-in host-side Rust provider), and a third-party package
      cannot be enabled with `parse-document`/`completion-provider` because
      those capability grants are recorded by
      `PackageService::authorize_package`, which has no CLI/desktop/JS surface
      yet (bundled packages get `authorize_bundled_defaults`, language servers
      get `authorizeLanguageServer`). The lane behaviour stays pinned by
      `latency_lane_unblocked_by_busy_general_lane` (4.1–24.9 ms under a 100–500
      ms general-lane hold versus the ~454 ms single-worker baseline) and the
      rest of the plan 127 scheduling suites.
    - New negative check (9 P56) found while reaching for that trigger: a local
      third-party fixture package declaring `parse-document`,
      `completion-provider`, and `mode-registration` installed and adopted
      cleanly (`Adopted @fixture/lane 0.1.0`), then `clay package enable` failed
      closed with `MissingCapabilityGrant { capability: CompletionProvider }`;
      the live app started, opened `demo.lane`, echoed 9 typed characters
      (`v1 dirty`, 38 chars), applied no fixture mode/handler/provider, and
      surfaced only the sanitized
      `packages.load_failed: JavaScript runtime evaluation failed.` diagnostic.
      This is the same host-side fail-closed contract the task-6 revocation gate
      implements, now verified end-to-end from the CLI through the live client.
    - Host ceilings recorded (not Clay defects): `Ctrl+Space` is consumed by this
      host's GNOME input-source switch, so the fixtures also bind `Ctrl+J`; the
      AT-SPI probe still cannot resolve sub-second paint, so no live
      keypress→paint number is claimed (the measured server-side numbers above
      carry the budget claim); `portal-shot.py` needs the Clay window activated
      first or the crop can capture another window.
    - Artifacts: `test-plan/artifacts/127-lane-scheduling/` with `README.md`,
      `run-live.sh` (`fixture|completion|markdown`, perf-report wiring,
      tree-kill teardown), `store-list.py`, `probe.py`, `portal-shot.py`,
      `init-fixture.js`, `init-markdown.js`, both fixture packages, and the
      evidence directories `grant-gate-live/`, `live-completion/`,
      `live-markdown-parse-handler/`.
    - Verification: `cargo test --test protocol` 223/223 pass (includes
      `documentation_coverage` 12/12), `cargo test --lib -- --test-threads=1`
      1,414 passed / 1 ignored, `cargo fmt --check` clean on the frozen tree.
      `scripts/check.sh quick` (parallel lib run) trips the pre-existing baseline
      red `coding_agent_clean_init_one_line_activates_working_defaults` — the same
      parallel-ordering failure recorded in tasks 1, 3, and 6 and unaffected by
      this task (it has no `src/` diff). Evidence:
      `test-plan/artifacts/127-lane-scheduling/gates.log`.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface added or changed (scheduling is internal); verify via phase diff; any new Rust function stays `pub(crate)`.
      - **Amended by execution (2026-09-19):** the diff shows exactly one public
        surface change — an additive, optional `moduleSpecifier?: string` on the
        completion-provider and language-intelligence provider registration
        options (`runtime/js/completion.d.ts`, `runtime/js/language.d.ts`), added
        by task 3 so a latency lane can materialize a handler by module import.
        Absent-by-default, so every existing call shape keeps its previous
        behavior. The amended criterion is: no public surface may be removed,
        renamed, or made stricter for existing callers; additive optional fields
        must be validated, documented, and registered.
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
  - Outcome (2026-09-19): verified, with one corrected acceptance criterion and
    the missing reference-doc/inventory/registry sync completed. Evidence:
    `code-reviews/2026-09-18-plan127-baseline/task8-js-api-surface.md`.
    - **Public surface, by diff:** exactly one change — `moduleSpecifier?: string`
      (optional) on `ServerRegisterCompletionProviderOptions` and
      `ServerRegisterLanguageIntelligenceProviderOptions`; 13 added lines in
      `runtime/js/`, 0 removed. Accepted-argument handling in
      `src/server/ops/completion.rs` (+21) and
      `src/server/ops/language_intelligence.rs` (+23): length-bounded to 512,
      empty treated as absent, validated with
      `PackageLoadEntryAllowlist::is_package_module` against the registering
      package, rejected with
      `completion.invalid_provider`/`language.invalid_provider: moduleSpecifier must
      resolve to a loaded module owned by the package`. The LI op accepts it
      top-level or nested in `provider` (mirroring `exportName`); the completion
      op reads it top-level.
    - **No new public Rust function or type:** the `pub (fn|struct|enum)` scan over
      `git diff b04f46b -- src/` is empty; every plan 127 function is `pub(crate)`,
      `pub(super)`, or private. The only new `pub` items are seven budget/metric
      constants in `src/perf/{budgets,metrics}.rs`, which follow those modules'
      existing `pub const` convention and are read by tests and benches.
    - **No op added, renamed, or removed**, and `runtime/js/*.js` facades are
      unchanged by plan 127; `src/server/ops/mod.rs` gains only the internal
      mailbox sender type for third-party dispatch.
    - **Package load/activation API behavior unchanged:** no diff in
      `runtime/js/packages.js`, `runtime/js/packages.d.ts`, or
      `src/server/ops/packages.rs`; manifest schema and error codes untouched.
      Task 6's guard hardens runtime *dispatch* admission only (typed
      `ClayRuntimeError::Revoked` instead of an isolate-level untyped
      `packages.package_not_enabled`), proven by module 09 P56 live plus
      `revoked_package_commands_refused_per_lane` and
      `disabled_package_callback_publications_fail_closed`.
    - **Docs sync (the work this task had to add):** `module`, `exportName`, and
      `moduleSpecifier` are now documented on the completion provider page
      (frontmatter + Options + Custom properties + Errors + Permissions and
      security), `moduleSpecifier` on the language-intelligence page, matching
      entries in `docs/reference/clay-js-api/api-inventory.toml`, and a
      regenerated `docs/generated/clay-js-api-registry.json`
      (`cargo run --bin update-doc-registry`, diff limited to those two entries,
      still 145 entries).
    - **Pre-existing drift found and fixed:** the completion page claimed JS
      provider execution was not exposed and that `module` is rejected, while
      `runtime/js/completion.js` has accepted a package-owned `module` +
      `exportName` since before plan 127 (baseline `src/server/completion.rs`
      already defines `JsCompletionProviderRegistration`). Page and inventory
      notes now describe the module bridge accurately without weakening the
      denied-authority list the registry guard enforces.
    - **Gates:** `cargo test --test protocol` 223/223 (includes
      `clay_js_api_inventory`, `clay_js_doc_registry`, `clay_js_facade_layout`,
      `documentation_coverage`, `primitives_docs`), `cargo test --test
      presentation` 62/62, `cargo fmt --check` clean.
    - Attribution note: plan 127's baseline tree was committed mid-plan as
      `66648e3` ("WIP"), so `git diff 66648e3` = tasks 5–7 and `git diff
      b04f46b` = plan 126 + tasks 1–4; the `moduleSpecifier` hunks are attributable
      to plan 127 by their `Plan 127 P1` TSDoc and by the plan document being the
      only one that mentions the field (`plans/126-*.md`: 0 mentions).

- [x] Update or verify the code wiki after implementation
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
  - Outcome (2026-09-19): wiki updated and manually reviewed against the code.
    - `docs/wiki/modules/persistent-runtime-hardening.md` (the page the index
      already linked) gained a new **Worker Lanes, Queue Bounds, and Heap
      Restoration (plan 127)** section: lane topology table (general vs latency
      commands and their isolate budgets), routing rules (`moduleSpecifier`
      decides the lane at registration time; package `Evaluate` is never
      replayed into another lane; trusted latency lane has no cross-domain
      bridge), per-lane poison/replacement, the bounded mailbox
      (`JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY` = 64, stale-first eviction, only
      `Completion`/`LanguageIntelligence` supersedable, guaranteed/delta
      commands always admitted, `js_runtime.command.superseded` /
      `.evicted`), heap-limit restoration (`remove_near_heap_limit_callback`
      because v8 147.4.0 has no `set_heap_limit`, live-heap clamping, no
      ratchet inheritance), a budgets/constants table, and a dedicated
      **Lane Isolation Is Not a Trust Boundary** subsection (domains stay the
      boundary; lanes share a domain's op set; revocation is lane-independent
      and fails closed with `ClayRuntimeError::Revoked`; trusted reload leaves
      third-party lanes untouched).
    - Test paths added to the same page (lane scheduling, lane/domain
      invariants, queue bounds, heap restoration) plus pointers to
      `test-plan/artifacts/127-lane-scheduling/` and
      `code-reviews/2026-09-18-plan127-baseline/task8-js-api-surface.md`.
    - **A3 constraint documented:** two trust domains per process, each with two
      lanes (four persistent workers per generation); lanes are scheduling-only
      and add isolates *inside* a domain, they never change the trust split.
      The `RuntimeLane::General`/`Latency` split is why the old "exactly two
      persistent runtimes" invariant had to be corrected rather than silently
      kept.
    - Stale current-state statements fixed in the pages that carried them, so
      the wiki no longer contradicts the code:
      `docs/wiki/modules/embedded-js-runtime.md` (two `JsRuntime` → two domains
      × two lanes; `DomainRuntime` mailbox/sender description; cross-domain
      bridge target lane; per-lane poison + replay; document-analysis poisoning;
      invariants; worker-start description; heap-limit stop-gap and restore;
      language-intelligence invocation lane),
      `docs/wiki/modules/parse-coordinator.md` (JS handlers run on the general
      lane), `docs/development/tauri-react-primitive-migration.md:256`
      ("exactly two persistent runtimes" → two trust domains, two lanes each).
    - Cross-links added from `docs/wiki/modules/completion-snippet-expansion.md`
      and `docs/wiki/modules/language-intelligence.md`, and the
      `docs/wiki/index.md` row for the hardening page now names the lanes,
      mailbox bounds, supersession, and heap restoration.
    - Verification: `cargo test --test protocol documentation_coverage` 12/12
      (the wiki contract test: every evergreen page linked from the index, every
      intra-wiki link resolves, current-state pages name existing source paths),
      plus a manual read-through of the new section against
      `src/server/js_runtime/{mod,worker,error}.rs`, `src/perf/{budgets,metrics}.rs`,
      and the cited test names.

## Compromises Made
- **Latency lane scope is provider-registration-driven, not request-driven.**
  Only completion and language-intelligence providers registered with a
  package-owned `moduleSpecifier` get the latency lane; inline `module`
  providers and every parse/analysis/evaluate command stay on the general lane.
  Chosen because replaying package `Evaluate` side effects into a second isolate
  would duplicate file writes, language-server spawns, and registrations.
  Upgrade path: an explicit per-command lane hint if a future provider cannot
  ship a module specifier.
- **Supersession is stale-first eviction, not back-pressure.** At the 64-slot
  supersedable bound the oldest same-key work is dropped for newer work instead
  of rejecting the new request (`ClayRuntimeError::Superseded`; no `QueueFull`
  variant shipped). Chosen because completion/LI requests are idempotent and
  latency-sensitive; a rejected request would surface as a failed completion
  while the dropped one was already stale. Guaranteed and delta-carrying
  commands are never dropped, so no document version or registration is lost.
- **The mailbox supersedes by linear scan rather than a secondary index map.**
  The backlog is bounded at 64 per lane, so a scan is cheaper than maintaining
  an index; no measured hotspot.
- **The latency lane's heap ceiling is 32 MiB** (`JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES`),
  below the 128 MiB general limit, because latency-lane work is request-scoped
  provider evaluation rather than package load entries or document analysis.
- **Heap-limit restoration is unconditional after every non-poisoned
  evaluation**, using `remove_near_heap_limit_callback(configured)` because v8
  147.4.0 exposes no `set_heap_limit` binding, and V8 clamps the restored limit
  to the live heap — so the effective cap can sit above the configured value
  while garbage is uncollected. Chosen over exposing a raw `set_heap_limit`
  path; the ratchet value is never inherited, which is the property under test.
- **The live manual step for "completions while a package parse handler runs"
  is half-live.** The typing half ran live; the provider-lane half is
  automated-only because no bundled package registers a JS completion provider
  and third-party capability grants have no user-facing surface yet (module 09
  P56). The behavior is pinned by `latency_lane_unblocked_by_busy_general_lane`
  and the fixture pair in `test-plan/artifacts/127-lane-scheduling/` is ready for
  the day a grant surface exists.
- **Plan 127 added one public JS field** (optional `moduleSpecifier` on the two
  provider registration option types) although the plan text claimed scheduling
  was internal-only; recorded, validated, documented, and registered rather than
  hidden (module 8 evidence).
- **`Ctrl+Space` is bound to completion but consumed by GNOME input-source
  switching on this host**, so the manual fixtures also bind `Ctrl+J`; no Clay
  behavior was weakened for the host.

## Further Actions

All five further actions were dispositioned on 2026-09-20 (see
`plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md`
for the ones that became work).

- **Third-party capability grant surface — moved to plan 136** (tasks: review
  the grant/lane primitives, implement `packages.authorize` end to end, add the
  host CLI verb + `inspect` output, prove the grant → provider → latency-lane
  path with a fixture, then the standard JS API / configuration /
  example-config / launch-test / manual-test / wiki duties). The
  in-app GUI grant surface stays out of plan 136's scope because it is app-UI
  work that needs the prototype → explicit approval → implementation loop; it
  is recorded in plan 136's Further Actions. Rationale unchanged: without a
  grant, the latency lane cannot be exercised by any non-bundled package.
- **Completion provider reference page rewrite — completed 2026-09-20.** The
  page no longer frames the API as "Phase 18.19 inert metadata"; it leads with
  the registration contract that actually ships (manifest
  `clay.contributions.completionProviders` is the metadata source; the call's
  options are `module`, `moduleSpecifier`, `exportName`), documents the
  ignored metadata keys and the seven rejected authority fields, records the
  Phase 27 provenance of the manifest contract, and its `custom_properties`,
  `security`, and `agent_guidance` frontmatter now match the op. The same
  falsehood class was fixed in `docs/reference/packages/creating-packages.md`
  (Phase 18.11/18.18 sections claimed package authors cannot ship a handler and
  that `module` is rejected). `api-inventory.toml` and the generated registry
  were resynced; `cargo test --test protocol` and `cargo fmt --check` green.
- **Wiki contract test — completed 2026-09-20.**
  `tests/documentation_coverage.rs::plan127_wiki_pages_describe_lane_scheduling_and_the_trust_domain_constraint`
  pins the lane/queue/heap/A3 markers on the persistent-runtime pages, the
  cross-links from the completion, language-intelligence, and parse-coordinator
  pages, and fails if any evergreen wiki page restates the pre-plan-127
  topology (one worker per domain, "exactly two persistent `JsRuntime`").
- **Completion page `module`/`exportName` frontmatter parity — addressed where
  it existed, guarded in plan 136.** The audit behind the rewrite found the
  page's option list was the *inflated* side of the drift (17 documented
  options; 3 real ones), and that `triggers` was documented although the only
  code that read it (`trigger_characters`) is dead — a finding now recorded in
  plan 131's dead-code task. The general guard — a non-`never` option key
  declared in `runtime/js/*.d.ts` must appear in the inventory and the page's
  Options section — is plan 136's "Harden the Clay JS option-surface drift
  guard" task, which also has to clean `runtime/js/completion.d.ts` first: the
  typings still declare eleven options the op never reads, so TypeScript
  autocomplete leads authors into the same silent-ignore trap the page had. A
  faithful guard needs the option-type parsing that the
  frontmatter↔inventory↔registry equality does not do.
- **Lane occupancy measurement — moved to plan 136** ("Measure provider lane
  occupancy and record the lane/tuning decision"): per-lane command counters in
  the perf summary, a fixture measurement with the general lane busy, and a
  recorded keep-or-tune decision for `JS_RUNTIME_LANES_PER_DOMAIN` and the
  32 MiB latency ceiling. It needs plan 136's grant surface to have real
  third-party latency-lane work to measure.
