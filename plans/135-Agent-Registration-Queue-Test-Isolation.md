# Plan 135 — Agent Registration Queue Test Isolation and Gate Reliability

> **Superseded (2026-09-20) by plan 130 task A1.** A1 deleted the
> process-global `AGENT_HOST_AUTHORITY` and `PENDING_PACKAGE_REGISTRATIONS` and
> injects the `AgentHostHandle` into each runtime lane's `ClayOpState`;
> declarations now queue on the lane that made them. The root cause this plan
> wanted to isolate under test is gone rather than isolated: the baseline-red
> `coding_agent_clean_init_one_line_activates_working_defaults` passes in the
> full parallel lib suite, `two_servers_in_one_process_own_independent_agent_hosts`
> pins per-server independence, and no test can drain another test's queue
> (evidence: `plans/130-Agent-Host-Decomposition-and-Ownership-Cleanup.md`, task
> A1). Do not build a test-only isolation seam on removed state — the tasks below
> are moot and kept only for their failure-matrix history.
>
> Task 1 was still executed (2026-09-23) as the post-A1 baseline a plan of this
> shape should start from: the registration-queue red is recorded as
> unreproducible (0/10 parallel lib runs, formerly red test green in all ten),
> the deleted mechanism is pinned by removal commit and by its per-lane
> replacements, and the one flake the matrix did hit — an npm fixture ETXTBSY
> spawn race, 1/10 runs — is recorded in Further Actions. Tasks 2-4 stay moot.

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

- [x] Baseline: pin the flake and its mechanism on the current tree
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
  - Baseline Outcome (2026-09-23) — recorded on the post-A1 tree; the criterion
    above ("three failures in five runs", "the drain site that consumes another
    test's declarations") names a mechanism that no longer exists, so it is
    recorded as unattainable rather than met:
    - Superseded before start: plan 130 A1 (commit `36eabd2`, 2026-09-21)
      deleted the process-global authority. Pre-A1 code, kept for the record:
      `static AGENT_HOST_AUTHORITY: OnceLock<AgentHostHandle>`
      (`36eabd2^:src/server/agent.rs:293`, first-install-wins `install_global`
      `:353`, `take_pending_package_registrations` `:340`), the host-queue drain
      inside `ensure_running` (`:1155-1165`, the site that consumed another
      test's declarations), and the `host=live` branch of
      `agent_registration_rpc`. Current-tree negative guard:
      `tests/agent_protocol.rs:514
      agent_authority_is_server_state_not_a_process_global` asserts those
      symbols are absent from `src/server/agent.rs`.
    - What replaced it (pinned): declarations queue on the lane that made them —
      `ClayOpState::agent_host: Mutex<Option<AgentHostHandle>>`
      (`src/server/ops/mod.rs:294`) +
      `pending_agent_registrations: Mutex<Vec<PackageRegistration>>` (`:297`),
      `queue_agent_registration` (`:644`, `PENDING_REGISTRATION_CAP` 64),
      `take_pending_agent_registrations` (`:665`), hand-off on attach
      `set_agent_host` (`:611`), fail-closed read `agent_host()` (`:631`); op
      path `src/server/ops/agent.rs:191-221` (`host=absent` → lane queue,
      `host=live` → the lane's own host); the host's own queue still drains
      after the first initialize (`src/server/agent.rs:1122-1134`). The test
      helper reads its own service
      (`src/server/js_runtime/tests/mod.rs:556` → `test_op_state()` →
      `src/server/js_runtime/mod.rs:1573`), so no test can drain another's
      queue.
    - Run matrix (`cargo test --lib`, default threads, 10 consecutive runs):
      9 × rc=0, 1425 passed / 0 failed / 1 ignored each; run 5 rc=101 with one
      failure, and it is not a registration test:
      `packages::manager::tests::npm_backend_list_parses_npm_json_shape`
      (`src/packages/manager.rs:1163`, panic `:1185`) —
      `BackendError { kind: ProcessSpawnFailed, message: "failed to spawn
      \`/tmp/clay-manager-resolver-499761-fake-npm-list/npm\`: Text file busy
      (os error 26)" }`. Wall clock 13.22-14.24 s of suite time (13-14 s per
      invocation, i.e. no build cost in the figure); no prior figure exists to
      compare against, so this is the before-number for any later change.
    - Registration flake: 0/10 reproductions.
      `server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`
      is `... ok` in all ten logs, including the failing run. The formerly
      coupled set is 10/10 green as well: `agent_facade_fails_closed_without_an_attached_host`,
      `coding_agent_double_load_is_idempotent_within_one_generation`,
      `coding_agent_malformed_registration_fails_closed_without_queue_corruption`,
      `coding_agent_and_launcher::launcher_claims_the_empty_tab_landing_and_the_agent_keeps_its_pane`,
      `example_configuration_loads_cleanly_and_applies_effects`,
      `agent_host_wiring_reaches_every_lane_and_stays_per_service`,
      `registrations_queued_hostless_hand_over_to_a_late_attached_host`,
      `lane_registration_queue_fails_closed_at_capacity`.
    - Experiments from the Approach (the old failure-amplifiers):
      `-- --skip agent_facade_fails_closed_without_an_attached_host` → ok,
      1424 passed / 0 failed / 1 filtered out (14.24 s) — skipping that sibling
      used to make five more declaration tests fail, now it changes nothing;
      `--test-threads=1 coding_agent_clean_init_one_line` → ok (0.02 s);
      `--test-threads=1 agent_lane_host_wiring` → 8 passed / 0 failed (0.10 s).
    - Op path the formerly red test actually takes (targeted `--nocapture`
      run): exactly 11 `[agent-reg]` lines, all `host=absent -> queued` —
      `agentProfile.register 'coding'` plus the ten `command.register` entries
      (`/compact /new /n /branch /tree /fork /clone /open-session
      /open-session-as-fork /discard`). Its declarations never reach a host, so
      nothing in the process can redirect or drain them; that is the drained
      method list in its post-A1 form.
    - Code Quality: no code changes and no instrumentation. Instrumentation at
      the old drain site is impossible (state deleted) and unnecessary (the
      negative guard test plus per-service queue pin it). No `src/` or `tests/`
      file was modified today (`find src tests -name '*.rs' -newermt 2026-09-23`
      empty); the only new path is the untracked evidence directory
      `test-plan/artifacts/135-registration-queue-baseline/`. `git diff
      --check` reports one pre-existing WIP trailing-whitespace warning in
      `docs/wiki/modules/icon-pack-runtime.md` (mtime 2026-09-22, untouched
      here).
    - Security: no diagnostics were added, so nothing new can print
      credentials, vault contents, or session data; the one failure message
      names a `/tmp` fixture path only.
    - Evidence files: `test-plan/artifacts/135-registration-queue-baseline/` —
      `runs.txt` (per-run rc, result line, failing set), `run1.log`-`run10.log`,
      `experiments.txt`, `hostless-path-nocapture.txt`.
    - Finding for Further Actions (a different flake, not this plan's subject):
      the remaining non-determinism in `cargo test --lib` is the npm fixture
      spawn race above (1/10 runs) — a pre-existing repo-wide class (plans
      041/078/084/088/098/134) with an in-repo fix precedent in `fake_git`
      (`src/server/git.rs:1285-1287`: 20 ms settle after writing the script,
      comment naming exactly this ETXTBSY race). `fake_bin`
      (`src/packages/manager.rs:1006-1026`) has no settle step.

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

- Task 1's functional criterion is recorded as unattainable, not met: it asks to
  reproduce three failures in five runs of a mechanism plan 130 A1 had already
  deleted (2026-09-21, one week before this plan was started). The task was
  executed in its post-A1 form instead — ten parallel runs, 0/10 reproductions
  of the registration red with the formerly red test green 10/10, pinned
  mechanism, wall clock, and the failing set the matrix did produce — so the
  checkbox carries evidence rather than a false positive.
- The registration-queue flake is closed by removal, not by the test-only seam
  this plan designed: tasks 2-4 (authority suspend/restore, seam test, facade
  test suspension) are moot and unexecuted. No `cfg(test)` isolation seam exists
  in the tree; the queue is production per-lane state.
- The one parallel-suite failure found (npm fixture ETXTBSY, 1/10 runs) is left
  unfixed: it belongs to `packages::manager`'s test fixture, not to the
  registration queue, and fixing it here would widen a diagnostic task into a
  second subsystem. It is recorded in Further Actions with the in-repo
  precedent.
- Task 6's wiki duty and task 7's JS-API inventory duty are unaffected by this
  task (no code change, no new surface); the wiki already documents the per-lane
  queue and ownership from plan 130 A1.

## Further Actions

- **High — `cargo test --lib` can still fail ~1 in 10 parallel runs, for an
  unrelated reason.** `packages::manager::tests::npm_backend_list_parses_npm_json_shape`
  (`src/packages/manager.rs:1163`) failed once in ten default-thread runs (run
  5, `test-plan/artifacts/135-registration-queue-baseline/run5.log`) with
  `ProcessSpawnFailed … Text file busy (os error 26)`. Mechanism: `fake_bin`
  (`:1006-1026`) writes the fake `npm` script and the test execs it
  immediately, the same ETXTBSY race `fake_git` already handles with a 20 ms
  settle (`src/server/git.rs:1285-1287`). Cheapest fix: mirror that settle
  (or write-then-rename) in `fake_bin`, then re-run the 10× matrix. Rationale:
  this is now the only thing keeping the lib suite from being deterministic, so
  the "`test` stage passes on an untouched tree" claim is 9/10 rather than
  certain. Not covered by plan 148, which owns the security-suite fake-LSP and
  agent-session flakes plus the UI-review harness hang.
- **Low — close the remaining checkboxes** (tasks 2, 3, 4 moot; 5-7 unaffected)
  so a plan scanner does not read them as pending work; the supersession note
  and this outcome already say why.
