# 145 — Gate Hygiene: Security-Suite Determinism, Toolchain Pinning, and the SC-1 Errata

Source: `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → `## Further Actions`
(recorded 2026-09-21), plus the review of that list done the same day, which
resolved the clippy break, the `clay-desktop` gate gap, and the misleading
bindings-guard message in place and left these four items.

No UI prototype gate applies: this plan changes tests, CI configuration, a
toolchain pin, and documentation — no component, token, typography, layout, or
design-language value. No example-configuration task applies either (no
`init.js`-callable surface changes), and the manual-test-plan task below records
an automated-only surface instead of executing steps.

## Objectives

- C1: the `security` integration suite stops failing for reasons unrelated to
  the code under test. Three tests are known flaky on this host and abort
  `scripts/check.sh full` before the test stage completes:
  `language_server_authority::generic_fake_lsp_server_initialize_and_shutdown_through_process_service`,
  `language_server_authority::generic_fake_lsp_exit_early_surfaces_typed_sanitized_exit`,
  and `agent_session_isolation::two_workspaces_keep_their_agent_writes_in_their_own_root`.
  Attribution work in plan 132 showed a baseline (untouched tree) failure rate
  indistinguishable from the changed tree (1/12 vs 3/13), so the cause is the
  suite's own nondeterminism, not the fanout refactor. Measured again on
  2026-09-21 after the gate review: 1 failure in 4 consecutive security-suite
  runs on an otherwise untouched tree.
- C1b: the gate can also **hang** rather than fail: the
  `manual_smoke_docs::plan118_ui_review_harness_captures_the_shipped_system_and_rejects_removed_states`
  test runs the real UI-review harness, whose portal screenshot step
  (`python3 portal_capture.py`, `scripts/capture-ui-review.sh:1169`) blocked for
  more than four minutes on a live desktop-portal session during the 2026-09-21
  gate review. The step has no timeout, so the test — and therefore
  `cargo test --all-targets` and `scripts/check.sh full` — waits indefinitely.
  The protocol suite that normally finishes in ~50 s took 304 s in that run.
- C2: one recorded toolchain policy — a pinned version or a deliberate floating
  `stable` with the reason — so a new Rust release cannot silently turn the gate
  set red again (Rust 1.98.1 broke two lints that 1.96.1 accepted; both are now
  fixed in place, see task 3).
- C3: the 2026-09-14 SC-1 decision log states the boundary the code actually
  ships: the DTO layer stays a projection, `src/protocol` stays untouched, and a
  contract type that is not TS-derivable may gain the feature-gated `ts-rs`
  derive in its own crate. The log currently claims the opposite in three places
  while plan 119 shipped 71 such derives and plan 132 added 2 more.

## Expected Outcome

- `for i in $(seq 1 20); do cargo test --test security --quiet || break; done`
  completes 20/20 green on this host, including under the parallel load of the
  rest of the workspace suite; the fake-LSP and agent-session assertions still
  fail when the behavior they guard is genuinely broken (verified by one
  deliberate negative check per fixed mechanism).
- `scripts/check.sh full` runs to completion without a retry, and a failure in
  the security suite is a signal rather than a coin flip.
- `cargo test --all-targets` cannot hang on a stuck desktop portal: the UI-review
  harness invocation is bounded, and a timed-out harness is recorded as an
  environment outcome rather than a pass or an unbounded wait.
- The repository states one toolchain policy in a discoverable place, and the
  gate set is green on the version CI installs.
- The 2026-09-14 log carries an amendment (date, approving statement, corrected
  wording, and the evidence that contradicted the original claim), and the
  original decision text is otherwise unchanged.

## Tasks

- [ ] Baseline: reproduce the security-suite flake deterministically and record the gate/toolchain state
  - Acceptance Criteria:
    - Functional: each of the three flaky tests is reproduced with its failure mode and mechanism — (a) the fake-LSP tests (`tests/language_server_authority.rs:665`, `:777`) build their scratch root in `fake_workspace_root` (`:536-547`) from `SystemTime::now().as_nanos()` alone (no pid, no thread id), so two parallel tests can share a root and read each other's framed `Content-Length` response; (b) `tests/agent_session_isolation.rs:595` races a write against session close (`write mock-session-1 ok` followed by a rejection); (c) `tests/agent_protocol.rs` `mock_daemon` (`:44`) uniques its executable path by nanos + pid, which can still collide as `ETXTBSY`; (d) the hang is reproduced separately — `tests/manual_smoke_docs.rs` invokes the UI-review harness, whose portal capture has no timeout, so a stuck portal session blocks the whole test binary; the baseline records the observed wait and which step lacked the timeout. The baseline records a failure count over N ≥ 12 full-suite runs plus the observation that the same tests pass in isolation.
    - Performance: the baseline records the suite's runtime and the wall-clock cost of the planned verification loop, so the fix is not paid for with sleeps (no `sleep`-based synchronization is accepted as a fix).
    - Code Quality: the baseline names the exact shared state per test (temp root, fake-child script path, daemon executable path, session lifecycle ordering) and states the invariant each fix must establish (per-test unique paths; an observable ordering, not a timing assumption); it also counts how many test files hand-roll a temp root, so the fix is scoped as one helper rather than three patches.
    - Security: the baseline confirms the fixes must not weaken what these tests assert — the LSP tests' typed/sanitized exit assertions and the isolation test's cross-root write denial stay exactly as strict.
  - Approach:
    - Documentation Reviewed:
      - `tests/suites/security.rs` (module aggregation), `tests/language_server_authority.rs`, `tests/agent_session_isolation.rs`, `tests/agent_protocol.rs`.
      - `scripts/check.sh` (test stage) and `.github/workflows/ci.yml` (the job that runs it).
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → task 3 evidence (flake attribution and stash-and-loop method).
    - Options Considered:
      - Mark the three tests `#[ignore]` or serialize the suite with `--test-threads=1`: removes the signal, and CI's `scripts/package-smoke.sh` already runs serially while `check.sh` does not.
      - Fix the shared-state collisions (chosen): the failures are path and lifecycle races, which are real defects in the tests.
      - Retry-until-green wrapper: rejected, it hides races.
    - Chosen Approach: reproduce, then fix each mechanism at its source (unique per-test paths, explicit ordering), then prove it with a loop plus a deliberate negative check.
    - API Notes and Examples:
      ```bash
      # reproduction loop used for attribution in plan 132
      for i in $(seq 1 20); do cargo test --test security --quiet || echo "FAIL run $i"; done
      # isolated (always green, which is the tell)
      cargo test --test security -- generic_fake_lsp
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/145-gate-hygiene/baseline.md`: reproduction, mechanisms, run counts, runtime.
    - References:
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → task 3 evidence and `## Further Actions`.
  - Test Cases to Write:
    - Reproduction loop: N full-suite runs with per-run pass/fail recorded, plus per-test isolated runs that pass.

- [ ] Make the security integration suite deterministic and bound the UI-review harness
  - Acceptance Criteria:
    - Functional: every scratch path is unique per process *and* thread (pid + thread id + a monotonic counter), the fake-LSP child scripts are written under that unique root rather than a fixed `fake-lsp-{profile}` name, the mock daemon's executable path cannot collide, and the agent-session test observes the close/write ordering it asserts instead of assuming it; the three known-flaky tests then pass 20/20 consecutive full-suite runs. The UI-review harness step in `tests/manual_smoke_docs.rs` gets a hard bound (a `timeout` wrapper around the harness invocation, sized from the script's own `--timeout` default plus startup), so a stuck desktop portal produces the harness's documented unresolved/prereq outcome or a bounded test failure instead of an unbounded wait; a timed-out run is recorded as an environment outcome, never as a pass.
    - Performance: the suite's runtime stays within noise of the baseline (no added sleeps, no serialization), the fix does not increase per-test process spawns, and the protocol suite's worst case is bounded by the new harness timeout instead of the portal's stall (304 s observed on 2026-09-21).
    - Code Quality: each change carries a comment naming the race it removes; shared helpers (unique-root creation) are used by all affected tests instead of three ad-hoc schemes; `cargo clippy --all-targets -- -D warnings` stays clean.
    - Security: the sanitization and isolation assertions are unchanged in strictness; one deliberate negative check per fixed mechanism (e.g. force two tests to the same root, or break the close ordering) confirms the test still fails when the guarded behavior is wrong.
  - Approach:
    - Documentation Reviewed:
      - `std::thread::current().id()`, `std::process::id()`, and `std::sync::atomic::AtomicU64` for collision-free naming without a new dependency.
      - `tests/language_server_authority.rs` `fake_lsp_shell_child`/`fake_lsp_spawn`; `tests/agent_session_isolation.rs` session lifecycle; `tests/agent_protocol.rs` `mock_daemon`.
      - Every other test that builds a temp root by hand: `tests/agent_session_isolation.rs:145`, `tests/agent_settings_listing.rs:26`, `tests/editor_performance.rs:619`, `tests/example_config_control_center_chord.rs:32` — the helper replaces all of them, so the next collision cannot appear in a file this plan did not touch.
    - Options Considered:
      - One shared `unique_root(label)` helper over `temp_dir()` using pid + thread id + a process-wide `AtomicU64` counter (chosen): stdlib only, no dependency change, one place to reason about.
      - `tempfile::TempDir`: collision-free and self-cleaning, but `tempfile` is a dependency of the `clay-desktop` package (`src-tauri/Cargo.toml:43`) and these integration tests belong to the root `clay` package, so it would mean adding a dev-dependency to the root manifest for a path-naming fix.
      - Serialize the suite: rejected in the baseline.
    - Chosen Approach: one shared unique-root helper, used by every test that spawns a fake child, plus an explicit ordering barrier in the agent-session test.
    - API Notes and Examples:
      ```rust
      // tests/common/unique_root.rs (or the existing shared test-support module)
      pub fn unique_root(label: &str) -> PathBuf {
          static NEXT: AtomicU64 = AtomicU64::new(0);
          let seq = NEXT.fetch_add(1, Ordering::Relaxed);
          let root = std::env::temp_dir().join(format!(
              "clay-{label}-{}-{:?}-{seq}",
              std::process::id(),
              std::thread::current().id()
          ));
          std::fs::create_dir_all(&root).expect("temp root");
          root
      }
      ```
    - Files to Create/Edit:
      - `tests/` shared test-support module: `unique_root` helper.
      - `tests/language_server_authority.rs`: unique per-test roots and script paths.
      - `tests/agent_session_isolation.rs`: deterministic close/write ordering.
      - `tests/agent_protocol.rs`: collision-free daemon executable path.
      - `tests/agent_settings_listing.rs`, `tests/editor_performance.rs`, `tests/example_config_control_center_chord.rs`: switch to the helper.
      - `tests/manual_smoke_docs.rs`: bounded UI-review harness invocation.
      - `scripts/capture-ui-review.sh`: timeout on the portal capture step (`:1169`) so a stuck portal cannot block the harness itself.
    - References:
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → task 3 evidence (failure modes and line numbers).
  - Test Cases to Write:
    - 20 consecutive `cargo test --test security` runs: all green.
    - Deliberate negative check per mechanism: the test still fails when the guarded behavior is broken.
    - Bounded-harness check: a harness invocation with an unreachable portal completes within the bound and reports the environment outcome instead of hanging.

- [ ] Decide and apply the toolchain policy
  - Acceptance Criteria:
    - Functional: the repository states one policy — either a `rust-toolchain.toml` pinning a version (with the CI workflow reading it) or an explicit version in `.github/workflows/ci.yml` plus a documented "float `stable` on purpose" statement — and the policy is recorded in a decision log with the user's approval before the file lands.
    - Performance: no runtime effect; the task records the gate-set runtime on the pinned version.
    - Code Quality: one source of truth for the version (no duplication between a toolchain file and CI), documented where a contributor finds it (`docs/development/**` or the CI workflow comment), and the two 1.98.1 lint fixes already in place are noted as the reason the pin matters.
    - Security: the pinned toolchain is the one that runs `cargo audit`; the task records that pinning does not change the audit's advisory database source or the dependency set.
  - Approach:
    - Documentation Reviewed:
      - `dtolnay/rust-toolchain` action behaviour with and without `rust-toolchain.toml`; `rustup` toolchain-override precedence.
      - `.github/workflows/ci.yml`; `scripts/check.sh`; `scripts/package-smoke.sh`.
      - The Rust 1.98.1 breakage evidence (`clippy::result_large_err` at `src/server/connection/documents.rs`, `clippy::useless_borrows_in_formatting` at `src-tauri/src/commands.rs`) — both fixed on 2026-09-21.
    - Options Considered:
      - `rust-toolchain.toml` pin (chosen if reproducibility wins): one file, local and CI agree; cost is that contributors must have that toolchain installed.
      - Explicit version in CI only: keeps local flexibility, but local and CI can diverge again.
      - Keep floating `stable`: cheapest, and the gate catches breaks on the day they appear — acceptable only if the user prefers it, recorded explicitly.
    - Chosen Approach: decided in the decision log with the user; applied in the same task; the gate set is re-run afterwards.
    - API Notes and Examples:
      ```toml
      # rust-toolchain.toml (if the pin wins)
      [toolchain]
      channel = "1.96.1"
      components = ["rustfmt", "clippy"]
      ```
    - Files to Create/Edit:
      - `rust-toolchain.toml` or `.github/workflows/ci.yml`: the chosen policy.
      - `docs/development/**`: the documented policy and re-run instruction.
      - `decision-logs/**`: the approved decision.
    - References:
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → baseline evidence (1.96.1 green, 1.98.1 red before the fixes).
  - Test Cases to Write:
    - Gate re-run on the chosen version: `scripts/check.sh full` green, with the version recorded in the evidence.

- [ ] Correct the 2026-09-14 SC-1 decision log's core-crate derive wording
  - Acceptance Criteria:
    - Functional: the log no longer claims core crates carry no TS-codegen derives; it states the shipped boundary — the DTO layer is a projection and the only definition of the webview contract, `src/protocol` (rkyv) stays untouched, and a contract type the DTO layer references may carry the feature-gated `ts_rs::TS` derive in its own crate (plan 119: 71 derives; plan 132: `ActiveTypography`, `DesignSystemProvenance`) with `ts-bindings` off in shipped builds.
    - Performance: no runtime effect; the amendment notes the derives are codegen-only.
    - Code Quality: the amendment is dated, quotes the approving statement, cites the evidence (the derive sites and `scripts/check-bindings.sh`), and leaves the rest of the original decision text intact; if the user prefers a superseding log instead, that log is created and linked from the original.
    - Security: the log's trust-boundary statements stay unchanged — no new field crosses the bridge, narrowing is preserved, and the derive adds no runtime surface.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`: "keep existing logs immutable unless correcting factual errors or the user asks for an update"; explicit approval before writing.
      - `decision-logs/2026-09-14-1602-ts-rs-generated-webview-contract-from-dto-layer.md` lines 23, 104, 143-144 (the three claims); `src/protocol/mod.rs`, `src/shell/design_system.rs`, `src/protocol/decorations.rs` (the shipped derives).
    - Options Considered:
      - Amend in place with an errata section (chosen): the claim is factually wrong and the skill permits correcting factual errors.
      - Supersede with a new log: heavier, and the original decision itself (DTO layer as the source of truth) is still correct.
      - Leave it and note the drift in plan 132 only: rejected — the log is the project's memory for this boundary and other plans cite it.
    - Chosen Approach: ask for explicit approval, then amend with an errata section naming the corrected wording and its evidence.
    - API Notes and Examples:
      ```bash
      grep -rn "ts_rs::TS" src/ | wc -l   # shipped derive count backing the errata
      ```
    - Files to Create/Edit:
      - `decision-logs/2026-09-14-1602-ts-rs-generated-webview-contract-from-dto-layer.md`: errata section + corrected claims.
    - References:
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → task 4 evidence (the two added derives) and `## Further Actions`.
  - Test Cases to Write:
    - Manual review: the three corrected claims match the shipped code, and the amendment records its approval.

- [ ] Verify Clay JS APIs and configuration surfaces are unchanged (automated-only plan)
  - Acceptance Criteria:
    - Functional: the plan adds no public programmatic surface and changes no user-visible behavior, so the task records "no JS API change, no configuration change" with the diff as evidence (test files, CI configuration, toolchain pin, decision log only).
    - Performance: no runtime surface is touched.
    - Code Quality: the existing JS API and configuration documentation gates stay green (`cargo test --test protocol`), proving nothing silently required a doc update.
    - Security: no authority, permission, or validation surface changes.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`, `references/config.md`: what would trigger a task if a surface had changed.
    - Options Considered:
      - Skip the task: the create-plan requirement applies to every plan document.
      - Verify-only with recorded evidence (chosen).
    - Chosen Approach: run the documentation gates and record the empty diff.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol --quiet
      git diff --stat -- src/ src-tauri/src/ frontend/src/
      ```
    - Files to Create/Edit: none.
    - References:
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → task 6 (JS APIs) and task 7 (wiki) for the recorded no-change pattern.
  - Test Cases to Write:
    - Documentation gate run plus a diff check that no runtime source file changed.

- [ ] Record the manual test plan as not applicable (automated-only surface)
  - Acceptance Criteria:
    - Functional: the task records explicitly that this plan changes no user-visible behavior, so no `test-plan/` step is executed or added; the reason (test-only and CI/toolchain changes) is written into `test-plan/index.md`'s plan-145 record so the omission is visible rather than silent.
    - Performance: the record notes that the security suite's runtime and the gate-set runtime are the relevant budgets, both captured in the baseline.
    - Code Quality: no existing step is weakened or deleted; the index record links the plan and the flake evidence.
    - Security: the record notes that the fixed tests guard sanitization and workspace isolation, and that their assertions were not relaxed.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (How to use this plan, Conventions, execution-record style); `.agents/skills/create-plan/references/clay.md` (manual-test-plan duty and its explicit "cannot be tested manually" branch).
    - Options Considered:
      - Execute unrelated steps to have something to record: rejected, it would produce noise.
      - Record the explicit not-applicable reason (chosen).
    - Chosen Approach: add one execution-record entry stating the plan's surface is automated-only.
    - API Notes and Examples: none.
    - Files to Create/Edit:
      - `test-plan/index.md`: plan-145 record.
    - References:
      - `plans/144-Client-Lane-Gaps-Caret-Wrap-and-Pane-Focus.md`: the plan that carries the manual pass for the user-visible work from the same Further-Actions list.
  - Test Cases to Write:
    - Manual review of the index entry.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the wiki documents the deterministic test-harness conventions (unique per-test roots, explicit ordering, why sleeps are not synchronization) and the recorded toolchain policy, or is verified unchanged with the reason; the flake history is recorded where a future plan will find it.
    - Performance: wiki updates add no runtime work; any page mentioning the gate set reflects the new `clay-desktop` stages and their cost.
    - Code Quality: pages link from `docs/wiki/index.md` and `cargo test --test protocol -- documentation` stays green.
    - Security: pages document that the isolation/sanitization assertions were preserved and that pinning does not change the audit path.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki workflow, quality bar, archive policy.
    - Options Considered:
      - Update the testing/gates page and the wiki index (chosen).
      - Verify unchanged: insufficient, the harness conventions and gate stages changed.
    - Chosen Approach: one pass after the fixes land.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol -- documentation
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/*.md` (testing/gate pages).
    - References:
      - `docs/wiki/modules/server-state-fanout.md`: the lane page that links the flake note from plan 132.
  - Test Cases to Write:
    - Manual wiki review plus the documentation coverage test run.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
