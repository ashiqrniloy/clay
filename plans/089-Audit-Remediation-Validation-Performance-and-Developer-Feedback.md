# Audit Remediation: Validation, Performance, and Developer Feedback

Prerequisites: Plan 086 green. UI-dependent measurements should run after Plans 087–088; workflow/test infrastructure may begin earlier when isolated.

Source review: P1-5, P2 performance, security warning follow-through, and test gaps 3–5 in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`.

## Objectives

- Define one supported quick check and one serial full Linux check without duplicate target trees or concurrent heavyweight runs.
- Make deadlocked async tests fail quickly with useful ownership/cleanup diagnostics.
- Measure editor typing, menu filtering, tab switching, accessibility updates, and completion layout after correctness fixes.
- Add compact malformed/state-machine coverage for IPC, key sequences, menus, multi-window DPI/font-scale, and Wayland behavior.

## Expected Outcome

- Developers/agents use one documented command for quick feedback and one lock-protected command for release validation.
- CI and local automation cannot accidentally overlap full GUI/V8 verification in the same checkout.
- Timeouts identify leaked session/generation/channel state instead of hanging indefinitely.
- Plan 086 focus/accessibility follow-ups have one shared reconciliation boundary with consumer regression coverage; deterministic budgets and compact generated cases cover trust boundaries/state machines; live Linux UI checks cover platform behavior without claiming Windows is blocking.

## Tasks

- [ ] Establish current build/test/storage/performance baseline
  - Acceptance Criteria:
    - Functional: Record current gate commands, suite composition, CI steps, running-process behavior, and 17 GiB target shape; identify which tests/benches compile V8/Masonry/GPU paths.
    - Performance: Measure warm quick/full command durations, target growth, executable count, and peak concurrent Cargo process count.
    - Code Quality: Trace existing docs/CI before adding scripts; preserve four integration-suite roots and normal target reuse.
    - Security: Include `cargo audit` and security suite in full blocking path; quick path must be labeled non-release.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/build-and-test.md`, `performance.md`, `security.md`, `.github/workflows/ci.yml`.
      - Project patterns `maintenance-validation.md`, `protocol-and-performance.md`, `planning-checklist.md`.
    - Options Considered:
      - Add another task runner/dependency: rejected.
      - Use Cargo plus a minimal POSIX/Linux shell entry point and `flock`: chosen if direct documented commands cannot enforce serialization.
    - Chosen Approach:
      - Measure first, then introduce at most one tiny wrapper owning quick/full policy.
    - API Notes and Examples:
      ```bash
      du -sh target target/debug/deps target/debug/incremental
      cargo test --all-targets --no-run
      ```
    - Files to Create/Edit:
      - This plan: baseline evidence.
    - References:
      - `Cargo.toml` explicit suites; `.github/workflows/ci.yml`.
  - Test Cases to Write:
    - Baseline inventory assigns every required gate and suite to exactly one supported workflow.

- [ ] Implement one quick check and one lock-protected serial full check
  - Acceptance Criteria:
    - Functional: `quick` runs the measured smallest representative compile/unit set; `full` acquires one repo-local lock and runs fmt, check, clippy, all-target tests, bench compile, and audit serially. A second full invocation exits clearly or waits by documented policy.
    - Performance: Both reuse repository `target/`; quick materially beats full; wrapper itself adds negligible overhead.
    - Code Quality: One script with `quick|full`, no custom test graph, package manager, daemon, or duplicated command definitions; CI invokes the same full path or a drift-tested equivalent.
    - Security: Full cannot skip audit/security suites; lock path cannot follow hostile symlinks outside repo or expose secrets.
  - Approach:
    - Documentation Reviewed:
      - Cargo CLI current behavior as installed; Linux `flock(1)` host capability; `docs/development/build-and-test.md`.
    - Options Considered:
      - Make/Just/new Rust xtask: rejected as unnecessary dependency/scaffolding.
      - Documentation only: insufficient to prevent overlapping agent runs.
      - Minimal shell wrapper + `flock`: chosen for Linux-required host.
    - Chosen Approach:
      - Put lock under `target/`, use `set -eu`, execute direct Cargo commands, and print exact failed stage.
    - API Notes and Examples:
      ```bash
      scripts/check.sh quick
      scripts/check.sh full
      flock target/.clay-full-check.lock <command>
      ```
    - Files to Create/Edit:
      - `scripts/check.sh`: quick/full owner.
      - `docs/development/build-and-test.md`: supported commands and lock policy.
      - `.github/workflows/ci.yml`: use/drift-check full workflow.
      - `tests/manual_smoke_docs.rs` or focused docs-policy test: command drift.
    - References:
      - Linux primary-host policy in `AGENTS.md`.
  - Test Cases to Write:
    - Quick/full stage order, second full invocation behavior, failed stage propagation, no alternate `CARGO_TARGET_DIR`, CI/doc command parity.

- [ ] Add bounded async-test helpers and actionable timeout diagnostics
  - Acceptance Criteria:
    - Functional: Menu/runtime/configuration/channel tests use one small helper or direct `tokio::time::timeout` pattern that reports scenario, pending session/generation/channel counts, and cleanup state on timeout.
    - Performance: Test-only diagnostics add no production/runtime work; default deadlines keep full suite bounded without making normal CI flaky.
    - Code Quality: Reuse one helper only where at least two tests need identical behavior; do not wrap every test or hide assertion locations.
    - Security: Timeout cleanup terminates spawned runtimes/processes/sockets and never preserves revoked grants or ambient config roots.
  - Approach:
    - Documentation Reviewed:
      - Existing timeout patterns in `src/server/connection.rs`, `src/server/js_runtime.rs`, `tests/parse_coordinator.rs`.
      - `rust-async-patterns` guidance: cancellation, JoinSet/task cleanup, bounded channels, no lock across await.
    - Options Considered:
      - Global external test timeout only: poor diagnostics and cleanup.
      - Per-await ad hoc timeouts everywhere: noisy.
      - Small scenario-level helper plus explicit lifecycle assertions: chosen.
    - Chosen Approach:
      - Apply to known menu/runtime replacement families first; expand only with evidence.
    - API Notes and Examples:
      ```rust
      run_bounded_test("runtime replacement", Duration::from_secs(5), async { /* ... */ }).await;
      ```
    - Files to Create/Edit:
      - Existing test support module or `src/test_support.rs` under `#[cfg(test)]` (tentative).
      - `src/server/connection.rs`, `src/server/mod.rs`, affected integration tests.
    - References:
      - Plan 086 bounded P0 regression tests.
  - Test Cases to Write:
    - Deliberately pending future produces useful diagnostics and cleanup; normal scenarios remain below deadline.

- [ ] Close Plan 086 focus and accessibility follow-ups before platform validation
  - Acceptance Criteria:
    - Functional: Dirty active-pane close, including a focused dirty pane, reconciles focus to a surviving valid widget or clears it before the next AccessKit update; `accesskit_consumer` never receives a stale focused ID. Top-level Frame/window focus events cannot dispatch to a nonexistent Masonry widget, while the working editor-Entry focus path remains valid.
    - Performance: Focus repair is event/reconciliation-driven and bounded by changed focus/child state; no per-frame tree scan, synchronous IPC, JavaScript, or document work is added.
    - Code Quality: Fix the shared focus-removal/event-ingress seam rather than adding caller-specific guards; keep consumer validation and live smoke checks separate from production focus ownership.
    - Security: Removed, stashed, and inactive widgets cannot remain reachable or focused; valid keyboard/accessibility focus is not hidden by a broad event suppression rule.
  - Approach:
    - Documentation Reviewed:
      - Plan 086 Task 8/11 evidence and `docs/development/accessibility.md`.
      - `src/masonry_shell.rs`, `src/masonry_editor.rs`, `src/masonry_pane_document.rs`, app event routing in `src/main.rs`, and local `accesskit_consumer` 0.31.0 tree/focus behavior.
      - Project patterns `authority-boundaries.md`, `protocol-and-performance.md`, and `ui-visual-review.md`.
    - Options Considered:
      - Suppress every focus event after pane removal: rejected; it masks valid focus and would hide future regressions.
      - Patch only the dirty-close caller or only the consumer test: rejected; sibling removal and top-level focus paths would remain unsafe.
      - Repair shared focus reconciliation and validate both removal and ingress paths: chosen.
    - Chosen Approach:
      - Establish one valid-focus reconciliation point before tree updates/events, explicitly handle top-level window focus without a widget target, and retain the existing editor-entry focus behavior as a control case.
    - API Notes and Examples:
      ```text
      remove focused pane → reconcile focus → publish TreeUpdate
      frame Focus without widget target → ignore safely or route to a valid window owner
      ```
    - Files to Create/Edit:
      - `src/masonry_shell.rs`, `src/masonry_editor.rs`, `src/masonry_pane_document.rs`, or `src/main.rs`: exact owner determined by the event trace; keep the fix at the shared seam.
      - Existing consumer/accessibility tests in the owning module and `tests/live_atspi_smoke.rs` only for live regression coverage.
      - `docs/development/accessibility.md`: document the focus invariant and regression command.
    - References:
      - `code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log` and `focus-frame-crash.log`.
  - Test Cases to Write:
    - Focused dirty-pane removal, dirty-close rejection, clean focused-pane removal, survivor focus restoration, stale focused-ID consumer update, top-level Frame focus event, editor Entry focus control, and repeated tab/pane removal without panic.

- [ ] Measure and guard editor, menu, tab, completion, and accessibility costs
  - Acceptance Criteria:
    - Functional: Bench/metrics cover typing/local paint proxy, command/filter updates, tab switch, completion projection/selection, and accessibility tree update after stable IDs.
    - Performance: Record before/after distributions; deterministic work/payload/allocation bounds remain hard gates, wall-clock values stay advisory until promotion criteria exist.
    - Code Quality: Extend existing `window_baselines`, editor/protocol/runtime benchmark groups; no parallel benchmark framework.
    - Security: Fixtures are bounded/inert and execute no package JS/filesystem/network/process work in measured hot paths.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md`; `benches/window_baselines.rs`, `runtime_sdui_baselines.rs`, `protocol_server_baselines.rs`.
      - Project pattern `protocol-and-performance.md`.
    - Options Considered:
      - Optimize based on source intuition: rejected.
      - Measure corrected implementation first: chosen.
    - Chosen Approach:
      - Add only missing benchmark groups/counters and deterministic ceilings; profile any regression before edits.
    - API Notes and Examples:
      ```bash
      cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
      cargo bench --bench runtime_sdui_baselines --no-run
      ```
    - Files to Create/Edit:
      - Existing benchmark files and `src/perf/{budgets,baselines,metrics}.rs` only as needed.
      - `tests/editor_performance_invariants.rs`, `tests/performance_budgets.rs` for deterministic guards.
      - `docs/development/performance.md` evidence.
    - References:
      - Plans 086–088 corrected UI paths.
  - Test Cases to Write:
    - No global virtual-ID allocation churn; list/layout work bounded; no document reserialization on tab switch; no IPC/JS in local typing/paint.

- [ ] Add compact malformed-codec and key/menu state-machine generated coverage
  - Acceptance Criteria:
    - Functional: Deterministic generated cases cover framed archive mutations, length prefixes, key-sequence prefix/mismatch/timeout transitions, and menu open/query/select/activate/cancel/reload ordering.
    - Performance: Fixed seeds/case caps complete quickly in normal suite; no fuzz daemon/corpus explosion.
    - Code Quality: Use stdlib deterministic generator/fuzz-lite helpers already present in menu tests; add no property-test dependency until coverage complexity proves need.
    - Security: Invalid archives/state transitions fail closed; stale generation/provenance/selection never activates authority.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/codec.rs` tests; `src/server/menu_sessions.rs` fuzz-lite comment/tests; keybinding and transient-menu state owners.
    - Options Considered:
      - Add proptest/libFuzzer immediately: rejected; no existing dependency and bounded state spaces are small.
      - Deterministic table/generated loops: chosen.
    - Chosen Approach:
      - Keep generators local and seed/case counts explicit; graduate to a fuzzer only after a discovered gap justifies it.
    - API Notes and Examples:
      ```rust
      for seed in 0..256 { apply_deterministic_actions(seed, &mut model); assert_invariants(); }
      ```
    - Files to Create/Edit:
      - `src/protocol/codec.rs` tests.
      - `src/client/behavior.rs` or keybinding test owner.
      - `src/server/menu_sessions.rs`, `src/server/control_center.rs` tests.
    - References:
      - Audit test gaps 3–4; ponytail no-new-dependency rule.
  - Test Cases to Write:
    - Truncation/bit mutation/length mismatch/oversize; every key/menu transition order; stale generation; repeated cancel/close; selection bounds.

- [ ] Add real Linux multi-window, DPI, font-scale, and Wayland validation
  - Acceptance Criteria:
    - Functional: Environment-gated smoke opens representative multi-window/tab/pane states on Wayland, verifies logical/physical bounds, scale change, user font-scale layout, focus, and accessibility; fixed fixtures support manual screenshots. The review/manual workflow provides safe Clay window target/raise/focus controls or reports the exact unavailable-backend prerequisite, never relying on blind portal input.
    - Performance: Smoke has bounded startup/cleanup and adds no production per-frame work.
    - Code Quality: Extend existing GUI/review fixtures; no second event loop or platform abstraction.
    - Security: Isolated sockets/config/documents, no ambient user paths, no remote listener; accessibility names sanitized.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`, `windows.md`, Plan 087 review harness.
      - Masonry/winit 0.4 exact local source for window/scale events.
    - Options Considered:
      - Treat Linux-host Windows cross-compile as blocking: rejected by project policy.
      - Linux Wayland live smoke plus documented non-blocking Windows preservation: chosen.
      - Blind portal/coordinate input as a substitute for window targeting: rejected; it cannot prove which Clay window received the action.
    - Chosen Approach:
      - Add deterministic structural event tests, environment-gated real desktop smoke, and a safe window-targeting prerequisite for manual dialog/focus flows.
    - API Notes and Examples:
      ```bash
      CLAY_LIVE_WINDOW_SMOKE=1 cargo test --lib live_multi_window_scale_smoke -- --ignored --exact --test-threads=1
      ```
    - Files to Create/Edit:
      - Existing smoke/window test owner in `src/main.rs` or `tests/window_management_protocol.rs` (tentative).
      - `docs/development/launch-and-gui-smoke.md`, `docs/development/windows.md` if behavior notes change.
      - `scripts/capture-ui-review.sh` or the existing review-tooling owner (tentative): safe target/raise/focus orchestration, never blind portal input.
      - Plan 087 fixture/capture files.
    - References:
      - Audit test gap 5; Linux platform policy.
  - Test Cases to Write:
    - 1x/2x scale, UI typography extremes, two windows, multi-pane, focus transfer, close cleanup, Wayland launch, accessibility bounds, safe window raise/target/focus, native dialog selection, observer/restart/local-fallback keyboard flows, and full quit/relaunch persistence when the host targeting prerequisite is available.

- [ ] Report and control build artifact size
  - Acceptance Criteria:
    - Functional: Full workflow reports total/deps/incremental sizes and executable count; docs state cleanup thresholds/workflow without automatically deleting useful incremental state.
    - Performance: No duplicate target tree; artifact reporting itself is cheap and always runs after CI full check.
    - Code Quality: Reuse `du`/`find`; no storage service or bespoke analyzer.
    - Security: Report paths/sizes only, not artifact contents or environment secrets.
  - Approach:
    - Documentation Reviewed:
      - Existing CI “Report build storage” and `docs/development/build-and-test.md` measurements.
    - Options Considered:
      - Hard fail on absolute target size immediately: rejected; machine/profile dependent.
      - Report trend and document cleanup: chosen.
    - Chosen Approach:
      - Keep advisory measurement until stable CI history supports a threshold.
    - API Notes and Examples:
      ```bash
      du -sh target/debug/deps target/debug/incremental target
      ```
    - Files to Create/Edit:
      - `scripts/check.sh`, `.github/workflows/ci.yml`, `docs/development/build-and-test.md`.
    - References:
      - Audit P1-5.
  - Test Cases to Write:
    - Report executes on success/failure and missing subdirectories without masking original gate status.

- [ ] Perform visual screenshot and accessibility review of validation fixtures
  - Acceptance Criteria:
    - Functional: Capture multi-window, 1x/2x representative scale, typography extremes, menu/completion/tab states; accessibility tree bounds/names/focus match visible state.
    - Performance: No visible relayout loop or interaction stall during scale/font changes.
    - Code Quality: Evidence and blockers recorded under one artifact path.
    - Security: Fixture screenshots contain no real user data/paths.
  - Approach:
    - Documentation Reviewed:
      - `ui-visual-review.md`, Plan 087 harness, computer-use-linux workflow.
    - Options Considered:
      - Mark live smoke passed from process exit only: rejected.
      - Screenshot and semantic review: chosen.
    - Chosen Approach:
      - Use `get_app_state` before/after keyboard and scale/font-state changes; retain evidence.
    - API Notes and Examples:
      ```text
      get_app_state → exercise window/focus state → get_app_state → screenshot
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-14-plan089-platform-validation/*.png`.
      - This plan: findings.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Visual/semantic bounds and focus checks for every live fixture.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Test/benchmark/smoke helpers remain internal/CLI developer surfaces; inventory changed public Rust functions and expose none to JS unless they are real user capabilities.
    - Performance: No instrumentation JS API enters production hot paths.
    - Code Quality: Existing API registry remains complete; internal functions are private/`pub(crate)`.
    - Security: No test hook, arbitrary fixture, archive byte, window handle, or process-control API reaches packages/configuration.
  - Approach:
    - Documentation Reviewed:
      - Project API boundary/naming/schema/doc-registry patterns.
    - Options Considered:
      - Public debug APIs: rejected.
      - Internal tooling/CLI only: chosen.
    - Chosen Approach:
      - Run visibility and doc-registry gates, recording no-new-API result.
    - API Notes and Examples:
      ```bash
      cargo test --test security rust_visibility_api_mapping::
      cargo test --test protocol clay_js_doc_registry::
      ```
    - Files to Create/Edit:
      - Public API docs/registry only if inventory finds a genuine new capability (not planned).
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
  - Test Cases to Write:
    - Test-only/CLI helpers cannot be imported through `clay:*` or package ops.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Update/execute modules 01, 07, 10, 11, 13, and 14 for quick/full workflow, timeout diagnostics, performance feel, multi-window/scale/Wayland fixtures, and the Plan 086 native-dialog, observer/restart, local-fallback, and quit/relaunch cases once safe window targeting is available; retain explicit blocked status otherwise.
    - Performance: Record advisory measurements and perceived behavior without promoting unstable wall-clock gates.
    - Code Quality: Manual steps point to one supported workflow and exact artifact paths.
    - Security: Include isolated config/socket and no-debug-surface negative checks; reject blind keyboard/coordinate workarounds that could operate on an ambient window.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` and listed modules.
    - Options Considered:
      - Pure internal change, no manual work: rejected because live window/platform validation is user-visible.
      - Focused module updates: chosen.
    - Chosen Approach:
      - Add minimal numbered steps and update coverage matrix only where needed.
    - API Notes and Examples:
      ```bash
      scripts/check.sh quick
      scripts/check.sh full
      ```
    - Files to Create/Edit:
      - Relevant test-plan modules and `test-plan/index.md`.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
  - Test Cases to Write:
    - Manual quick/full, live multi-window/scale, timeout diagnostic, and performance scenarios.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki documents supported workflows, suite ownership, timeout diagnostics, benchmark/budget policy, generated-case strategy, and live platform validation; index links pages.
    - Performance: Explain advisory vs hard gates and target reuse.
    - Code Quality: Include exact commands/source/test paths and extension guidance.
    - Security: Explain audit/security inclusion and test-hook confinement.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Wiki per task: rejected.
      - One final update after validation: chosen.
    - Chosen Approach:
      - Update build/performance fixture pages and index once.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/performance-fixtures.md
      docs/wiki/index.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/performance-fixtures.md`, and testing/workflow page if needed.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
  - Test Cases to Write:
    - Manual wiki index/link review and docs-policy tests.

## Compromises Made

- No new property-testing/fuzzing dependency. Fixed-seed generated coverage is sufficient for current bounded state spaces; add a dedicated fuzzer only after it finds value unavailable from compact deterministic cases.

## Further Actions

- Consider hard artifact/wall-clock gates only after stable CI history establishes low-noise thresholds.
