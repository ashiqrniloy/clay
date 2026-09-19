# Plan 135 — Agent Registration Queue Test Isolation and Gate Reliability

## Objectives

- Make the lib test suite deterministic under parallel execution: no test's
  package-registration declarations may be consumed by another test's agent
  host or daemon. Today
  `server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`
  fails in 3 of 4 default-thread `cargo test --lib` runs on an unmodified tree
  (it passes with `--test-threads=1`), and skipping one sibling test makes five
  more declaration tests fail — the declarations of one test land in another
  test's queue/host.
- Keep production semantics exactly as they are: one `AgentHost` per server,
  registrations queued until the daemon's first `initialize`, applied in
  declaration order, package JS reaching the daemon only through validated ops.
  The isolation seam must be test-only and must not add runtime work.
- Remove the "this process has no agent authority" order dependence from tests
  that cannot own the process (a `OnceLock` authority installed by any earlier
  server test is never uninstalled).
- Restore the clean gate claim: this is the last pre-existing red recorded by
  plan 126 task 1, and it is the only reason the `test` stage cannot pass on an
  untouched tree.

## Expected Outcome

- `cargo test --lib` (default parallel threads) passes 5 consecutive runs on
  Linux, with the declaration tests asserting their own declarations in
  contract order instead of whatever the shared queue happened to hold.
- The process-global package-registration queue has one documented, test-visible
  lifetime: registrations queued while no authority exists, drained into the
  host on `install_global`, applied on the daemon's first `initialize` — and a
  test that holds `JS_RUNTIME_TEST_LOCK` observes only its own declarations.
- `cargo test --test security` and `cargo test --test runtime` stay green (the
  seam must not change daemon behavior).
- Evidence recorded in the task outcome: the before matrix (which runs failed,
  which test drained which queue), the after matrix (5 parallel runs), and the
  exact seam API.
- `docs/wiki/modules/agent-process-manager.md` explains the queue's ownership,
  lifetime, and the test-only isolation seam.

## Tasks

- [ ] Baseline: pin the flake and its mechanism on the current tree
  - Acceptance Criteria:
    - Functional: reproduce at least three failures in five default-thread
      `cargo test --lib` runs and record the failing test set; identify the
      drain site that consumes another test's declarations (which host, which
      op path) rather than only the symptom.
    - Performance: no measurements required; record the wall-clock of the
      parallel lib suite so the fix can be shown not to slow it down.
    - Code Quality: no code changes in this task — diagnostic instrumentation is
      reverted before the task closes, and the tree is `git diff`-clean.
    - Security: none introduced (diagnostics must not print credentials, vault
      contents, or session data).
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent.rs` — `AGENT_HOST_AUTHORITY` (`OnceLock`),
        `PENDING_PACKAGE_REGISTRATIONS`, `install_global`,
        `rpc_or_queue`, `ensure_running` (the drain), `take_pending_registrations`.
      - `src/server/ops/agent.rs` — `agent_registration_rpc` (host=live vs
        host=absent paths) and the `[agent-reg]` log line.
      - `src/server/js_runtime/tests.rs` — `drain_all_pending_registrations`
        and the declaration tests.
      - `plans/126-Document-Access-Path-Hardening.md` task 1 outcome (the
        recorded red and its "parallel-only" characterization).
    - Options Considered:
      - Repeated parallel runs plus temporary instrumentation at the drain
        site: cheapest way to name the mechanism. (Chosen.)
      - Reading the code only: it explains the possibility, not which test
        actually drains which queue.
      - Bisecting with `--skip`: useful as a second experiment (it shows the
        breadth of the coupling) but not sufficient to name the drain.
    - Chosen Approach:
      - Run `cargo test --lib` five times, record pass/fail per run; then add a
        temporary `eprintln!` in `ensure_running` (count + data dir + methods)
        and in the test's drain helper, capture the parallel output, and revert.
    - API Notes and Examples:
      ```text
      cargo test --lib                        # default threads, 5 runs
      cargo test --lib -- --skip agent_facade_fails_closed_without_an_attached_host
      cargo test --lib -- --test-threads=1 coding_agent_clean_init_one_line
      ```
    - Files to Create/Edit:
      - None (evidence-recording task; instrumentation reverted).
    - References:
      - `src/server/agent.rs:230-350` (queue + authority), `src/server/agent.rs`
        `ensure_running` drain, `src/server/ops/agent.rs:165-190`.
      - Plan 126 task 1 outcome; `code-reviews/2026-09-18-plan126-baseline/`.
  - Test Cases to Write:
    - None (evidence task). Record the run matrix, the drained method list, and
      the op path that triggered the drain in the task outcome.

- [ ] Make the package-registration path deterministic under test
  - Acceptance Criteria:
    - Functional: every declaration test observes its own declarations in
      contract order while the lib suite runs in parallel; the coding-agent
      one-line-load test, the double-load idempotency test, the malformed
      registration test, the launcher/agent-pane test, and
      `example_configuration_loads_cleanly_and_applies_effects` all pass in the
      same parallel run that previously failed.
    - Performance: no new work on any production path (the seam is `cfg(test)`;
      the `test` stage must not slow down measurably — record the before/after
      wall clock from task 1).
    - Code Quality: production semantics unchanged (single authority,
      queue-until-initialize, declaration order preserved); no new public API;
      the seam is small, documented, and names why it exists; no test asserts on
      another test's state.
    - Security: package JS still reaches the daemon only through validated ops;
      the seam grants no new authority and is absent from non-test builds.
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent.rs` — `install_global` ("first install wins"), the
        host pending queue, `ensure_running`'s post-initialize drain.
      - `src/server/ops/agent.rs` — the two registration paths.
      - `src/server/js_runtime/tests.rs` — the drain helper and the tests that
        depend on it; `JS_RUNTIME_TEST_LOCK` usage across `src/server/**`.
      - `.agents/skills/clay-execution/references/testing.md` (if present) for
        the suite's existing isolation conventions.
    - Options Considered:
      - `#[cfg(test)]` suspend/restore of the process-global authority: a test
        holding `JS_RUNTIME_TEST_LOCK` always takes the hostless path, so its
        declarations land in the process-global queue it drains. Small, keeps
        the production path untouched, and fixes the root cause (shared state).
        (Chosen.)
      - `#[cfg(test)]` record of applied registrations + assert the union of
        applied and queued declarations: also removes the flake, but the two
        lists cannot preserve a single contract order, so the order assertions
        stay racy.
      - Scope the pending queue per runtime generation: closest to "correct" in
        production terms, but it changes a production data structure for a
        test-only problem and needs a real ownership decision.
      - Serialize the affected tests with `JS_RUNTIME_TEST_LOCK` only: does not
        help — the authority installed by an earlier test outlives the lock.
      - Delete or weaken the order assertions: rejected; they are the contract
        the package's one-line load must keep.
    - Chosen Approach:
      - Replace the `OnceLock`-only authority with a cell that keeps
        first-install-wins production semantics and gains a `#[cfg(test)]`
        suspend/restore used only while `JS_RUNTIME_TEST_LOCK` is held; audit
        every test that writes registrations or drives an agent op so it either
        holds that lock or uses the seam, so no unlocked writer pollutes the
        process-global queue.
    - API Notes and Examples:
      ```rust
      // cfg(test) seam: registrations queue process-globally while suspended,
      // exactly as they do before any server exists in the process.
      #[cfg(test)]
      pub(crate) fn with_global_suspended<R>(f: impl FnOnce() -> R) -> R;
      ```
      ```text
      cargo test --lib                                     # 5 consecutive runs
      cargo test --lib -- --test-threads=1 coding_agent_clean_init_one_line
      cargo test --test security -- --test-threads=1
      ```
    - Files to Create/Edit:
      - `src/server/agent.rs`: authority cell + `cfg(test)` suspend/restore,
        keeping `install_global`'s first-install-wins and queue-handoff
        semantics identical for non-test builds.
      - `src/server/js_runtime/tests.rs`: use the seam in the declaration tests;
        add a test for the seam's restore behavior.
      - `src/server/**/tests.rs` (tentative): add `JS_RUNTIME_TEST_LOCK` to any
        test that writes registrations or drives an agent op without it.
    - References:
      - `src/server/agent.rs` authority/queue/drain sites; `src/server/mod.rs`
        (`install_global` call site); `VENT.md` (probe-target friction that
        motivated keeping the seam test-only).
  - Test Cases to Write:
    - Parallel lib suite (5 runs): the flake is gone and the previously failing
      set passes.
    - `coding_agent_clean_init_one_line_activates_working_defaults`: unchanged
      assertions, deterministic outcome.
    - New: the suspend seam restores the previous authority (a registration
      after the seam still reaches the installed host).
    - `cargo test --test security -- --test-threads=1`: daemon behavior
      unchanged.

- [ ] Remove order-dependent "no authority" assertions from tests that cannot own the process
  - Acceptance Criteria:
    - Functional: `server::js_runtime::tests::agent_facade_fails_closed_without_an_attached_host`
      asserts its denial deterministically whether or not another test already
      installed an authority, instead of passing because an unrelated daemon
      rejected an unknown session.
    - Performance: none (test-only).
    - Code Quality: the test keeps its authority-boundary meaning (the facade
      grants nothing without a Clay-owned authority) and states its dependency
      explicitly; no assertion is deleted without a replacement that covers the
      same boundary.
    - Security: the denial path stays covered — an authority-less runtime must
      not reach the daemon.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/tests.rs` — the facade test, `clay:agent` op
        wrappers in `src/server/ops/agent.rs`, and the `AgentHostHandle::global`
        fail-closed contract in `src/server/agent.rs`.
      - `docs/wiki/modules/embedded-js-runtime.md` and
        `docs/wiki/modules/agent-process-manager.md` — the documented authority
        boundary.
    - Options Considered:
      - Use the task-2 suspend seam so the test truly runs without an authority.
        (Chosen.)
      - Assert a typed denial in both states (authority present but no session):
        weaker — it stops proving the authority-less path.
      - Delete the test: rejected; it guards a real fail-closed boundary.
    - Chosen Approach:
      - Run the facade test with the authority suspended while holding
        `JS_RUNTIME_TEST_LOCK`, and keep the typed-denial assertion.
    - API Notes and Examples:
      ```text
      cargo test --lib -- agent_facade_fails_closed_without_an_attached_host
      cargo test --lib                       # full parallel suite
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime/tests.rs`: the facade test (and any sibling that
        asserts the absence of an authority).
    - References:
      - `src/server/js_runtime/tests.rs` facade test; plan 135 task 2 seam.
  - Test Cases to Write:
    - The facade test alone, in the full parallel suite, and with the
      declaration tests skipped — same result each way.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: record explicitly that this change is internal test
      isolation with no user-visible behavior, and therefore has no manual
      steps — instead of silently dropping the task.
    - Performance: none.
    - Code Quality: `test-plan/index.md` is updated only if the module map or
      coverage matrix changes (it should not).
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` — module map and coverage matrix.
      - `.agents/skills/create-plan/references/clay.md` — manual test plan duty
        (explicit not-applicable recording).
    - Options Considered:
      - Add manual steps for a test-only change: no observable surface to check.
      - Record the not-applicable decision with the reason. (Chosen.)
    - Chosen Approach:
      - Record the decision in the task outcome; no `test-plan/` edit expected.
    - Files to Create/Edit:
      - None expected.
    - References:
      - `test-plan/index.md`.
  - Test Cases to Write:
    - None (no user-visible behavior).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: verify the plan adds no public Clay JS API and no public Rust
      function — every new item is `cfg(test)` or `pub(crate)`, and the
      generated registry is unchanged.
    - Performance: none.
    - Code Quality: if a seam has to exist in non-test builds, it stays private
      and is documented in the wiki; no `deno_core` op is added.
    - Security: no new JS-reachable authority over the agent daemon.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`; `api-inventory.toml`;
        `docs/generated/clay-js-api-registry.json`.
    - Options Considered:
      - Expose the suspend seam to JS: rejected — test-only concern, no user
        value, new authority surface.
      - Verify "no new public surface" with the existing coverage tests.
        (Chosen.)
    - Chosen Approach:
      - Inventory the diff's public items and run the registry/doc coverage
        tests.
    - API Notes and Examples:
      ```text
      cargo test --test protocol primitives_docs::
      cargo run --bin update-doc-registry   # expect no diff
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `docs/reference/clay-js-api/`, `api-inventory.toml`.
  - Test Cases to Write:
    - Doc-registry/coverage tests stay green with no diff in the generated
      registry.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the wiki is updated after all implementation tasks pass, or
      explicitly verified unchanged for non-code work.
    - Performance: wiki updates add no runtime work and record any
      performance-relevant detail the plan changed (none expected beyond the
      removed flakiness).
    - Code Quality: pages explain what the registration queue does, how it
      works, its invariants/tradeoffs, source/test paths, and the test-only
      isolation seam, linked from the master wiki index.
    - Security: the authority boundary (package JS → validated ops → Clay-owned
      daemon) is documented without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md` — wiki
        workflow, quality bar, archive policy.
      - `docs/wiki/modules/agent-process-manager.md` (queue lifetime and
        ownership today: only the lazy-host spawn is documented),
        `docs/wiki/modules/embedded-js-runtime.md` (op boundary),
        `docs/wiki/index.md` (navigation).
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass. (Chosen.)
    - Files to Create/Edit:
      - `docs/wiki/modules/agent-process-manager.md`: registration queue
        ownership/lifetime, `install_global` handoff, the `cfg(test)` seam, and
        the tests that pin them.
      - `docs/wiki/index.md`: only if the page's one-line description needs the
        registration-queue clause.
    - References:
      - `docs/wiki/modules/agent-process-manager.md`,
        `docs/wiki/modules/embedded-js-runtime.md`.
  - Test Cases to Write:
    - Manual wiki review: the master index links the updated page, and the page
      explains the queue and the isolation seam.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
