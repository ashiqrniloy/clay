# Audit Remediation: Validation, Performance, and Developer Feedback

Prerequisites: Plan 086 green. Plan 088 implementation is complete; its explicit P1/P2 follow-ups are owned by tasks below. UI-dependent measurements should run after Plans 087–088; workflow/test infrastructure may begin earlier when isolated.

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
- Plan 088 recovery/loading, safe-targeted visual review, and advisory Criterion warnings each have an executable owner and evidence gate; no P1 visual defect or unexplained benchmark warning is silently carried into Plans 090–091.
- Plan 086 focus/accessibility follow-ups have one shared reconciliation boundary with consumer regression coverage; deterministic budgets and compact generated cases cover trust boundaries/state machines; live Linux UI checks cover platform behavior without claiming Windows is blocking.

### UI planning gate (2026-08-15)

- Ran `npx ui-skills start`; inspected `accessibility` and `testing`; loaded `ibelick/fixing-accessibility` and `pbakaus/audit` (2 skills, within prefer-1/max-3). The selected guidance is web-oriented, so this plan translates accessible names, keyboard/focus, modal containment, state announcements, deterministic audits, responsive checks, and performance triage to Clay's native Masonry/AccessKit/token context.
- Each independently executed UI task below must rerun the preflight before implementation/review; this routing record does not replace that gate.

## Tasks

- [x] Close Plan 088 welcome recovery/status and loading-fixture follow-ups
  - Acceptance Criteria:
    - Functional: `ui-review-recovery` exposes matching disconnected/recovery state in the status chrome and WelcomeWidget accessibility label; `ui-review-loading` exposes its published loading SDUI tree (including the loading panel/status) instead of falling back to the welcome shell; default/error states remain intact.
    - Performance: Status propagation is event-driven through the existing pane state and the capture harness waits on bounded semantic readiness; no per-frame polling, full-document reload, or unbounded fixture retry is added.
    - Code Quality: Repair the shared status/WelcomeState or fixture-observability seam; do not patch only the captured string, add fixture-specific production branches, or weaken unresolved-state detection.
    - Security: Keep private mode-700 HOME/XDG/socket/workspace roots, sanitized accessibility labels, fixture-only documents, and no absolute authorized paths in evidence.
  - Approach:
    - Documentation Reviewed:
      - Plan 088 Task 8/12 evidence and `code-reviews/screenshots/2026-08-14-plan088-modernization/review-log.md`.
      - `src/masonry_pane_document.rs` status flow, `src/masonry_welcome.rs` `WelcomeState`, `src/main.rs` connection-event routing, and `scripts/capture-ui-review.sh` fixture/wait logic.
      - Project patterns `ui-visual-review.md`, `ui-modernization.md`, `maintenance-validation.md`, and `planning-checklist.md`.
    - Options Considered:
      - Change only recovery screenshot expectations: rejected; stale state would remain.
      - Add a loading-only production branch: rejected; it hides the real startup/SDUI observability seam.
      - Trace shared connection/status propagation and fixture readiness, then add focused regression checks and recapture: chosen.
    - UI Routing Evidence:
      - Planning preflight: `npx ui-skills start`; inspected `accessibility` and `testing`; loaded `ibelick/fixing-accessibility` and `pbakaus/audit` (2 skills, within prefer-1/max-3). Their web guidance is translated to native Masonry/AccessKit semantics; rerun the preflight before execution.
    - Chosen Approach:
      - Verify `set_status → refresh_welcome_state → WelcomeWidget` and the `publishTree → AT-SPI readiness` path, write failing structural checks first, then capture recovery/loading/default/error with `get_app_state` and bounded private fixtures.
    - API Notes and Examples:
      ```text
      set_status → refresh_welcome_state → WelcomeWidget accessibility
      ui-review-loading: publishTree(...) → wait for the `Loading workspace` SDUI semantic marker → capture
      ```
    - Files to Create/Edit:
      - `src/masonry_pane_document.rs`, `src/masonry_welcome.rs`, and `src/main.rs` only if the shared event seam requires correction.
      - `src/driver/restore.rs` for the confirmed-tab restore gate; `src/masonry_sdui_region.rs` and `src/masonry_editor.rs` for nested kind replacement, bounded sidebar layout, and retained-tree accessibility sync.
      - `scripts/capture-ui-review.sh` and `tests/fixtures/configuration/ui-review-loading/init.js` for truthful readiness/fixture evidence.
      - Owning unit/integration tests and `code-reviews/screenshots/2026-08-14-plan089-platform-validation/` review artifacts.
    - References:
      - Plan 088 Further Actions and `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Disconnected WelcomeWidget label matches `Disconnected` status and recovery diagnostic.
    - Loading fixture exposes its intended SDUI nodes and does not pass on a welcome-only tree.
    - Default/error/recovery/loading capture matrix records PASS or explicit UNRESOLVED with reason; no absolute path/secret appears in screenshot or AT-SPI dump.

  - Evidence (2026-08-16):
    - UI preflight rerun: `npx ui-skills start`; systems/accessibility routing inspected; loaded `ibelick/fixing-accessibility` and `pbakaus/harden` (2 skills, within prefer-1/max-3). Web guidance was applied to native Masonry/AccessKit and bounded fixture readiness.
    - Root cause was not a missing watcher reload: the post-handshake `touch` delivered RuntimeStateSnapshot generation 2, but restore completion treated the empty registry replay as confirmed, so `loading.txt` reopen was skipped. The harness also started the server from the repository root, causing a separate bootstrap/document-ID mismatch.
    - Fixed shared restore gating to wait for server `TabId`, started isolated server from fixture workspace, opened `loading.txt`, delayed the watcher touch until after handshake, and made loading readiness require the SDUI `Loading workspace` semantic marker. Nested SDUI kind changes now rebuild retained children; the sidebar viewport supplies bounded width/fill constraints so the published panel paints and remains accessible.
    - Added `runtime_loading_tree_reaches_accessibility_after_document_open` and `restore_completion_waits_for_registry_tab_id`; existing `disconnected_welcome_accessibility_tracks_status_update` passes.
    - Live artifacts under `code-reviews/screenshots/2026-08-14-plan089-platform-validation/`: `default`, `error`, `loading`, and `recovery` all `review.status PASS`. Loading `runtime-tree.txt` records the delivered `Loading review` / `Loading workspace…` RuntimeStateSnapshot; the cropped screenshot shows the label in the Clay SDUI slot, while AT-SPI exposes the Server-driven UI region and a distinct `Fixture document` (not the welcome shell). Recovery AT-SPI names now agree: `Connection lost` / `Connection: Disconnected` in WelcomeWidget and status chrome.
    - Verification: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, focused loading/restore/accessibility tests, and `cargo test --all-targets --quiet` (1559 passed, 2 ignored in lib; all bin/integration suites and bench harnesses passed).

- [x] Establish current build/test/storage/performance baseline
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
      # Candidate non-release quick check measured for the next wrapper task.
      cargo fmt --check && cargo test --lib --quiet

      # Current serial full-gate candidate, including benchmark-target compilation.
      cargo audit && cargo fmt --check && cargo check --all-targets \
        && cargo clippy --all-targets -- -D warnings \
        && cargo test --all-targets --quiet \
        && cargo bench --no-run
      ```
    - Files to Create/Edit:
      - This plan: baseline evidence.
    - References:
      - `Cargo.toml` explicit suites; `.github/workflows/ci.yml`.
  - Test Cases to Write:
    - Baseline inventory: `cargo metadata --no-deps --format-version 1`, the four `tests/suites/*.rs` roots, and `integration_suite_inventory_assigns_every_source_once` assign every current test/bench target exactly once; CI/docs command parity is recorded for the next wrapper task.

  - Evidence (2026-08-16):
    - Environment: Linux `7.1.5-46.stable` / `x86_64`, rustc/Cargo `1.96.1`, 16 logical CPUs, 61.4 GiB RAM, commit `a03cec9` with the existing working tree changes, 471 GiB free on the target filesystem. No `CARGO_TARGET_DIR` override or competing Cargo run was present at start.
    - Existing CI/docs inventory: `.github/workflows/ci.yml` runs `cargo audit`, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, then reports `target/debug/deps`, `target/debug/incremental`, total `target`, and executable count. It has no lock or quick wrapper and does not run `cargo bench --no-run`; the measured full candidate adds that benchmark compile step.
    - Target topology: `Cargo.toml` has `autotests = false`, four explicit integration roots (`security`, `runtime`, `editor`, `protocol`), four binaries, and six Criterion benches. The roots include 37 source modules (8/10/9/10). A passing `cargo test --all-targets --quiet` run reported lib 1559 passed/2 ignored, bin `clay` 64 passed, utility bins 0, and integration suites `editor` 164, `protocol` 160, `runtime` 198, `security` 130 passed/1 ignored. The same command executed all six harness=false Criterion targets successfully; `cargo bench --no-run` then built 11 optimized bench-profile executables (package lib/main/3 bins + 6 benches). `cargo metadata --no-deps --format-version 1` and the standalone `cargo test --test protocol integration_suite_inventory_assigns_every_source_once -- --exact --test-threads=1` both confirmed the target inventory and one-owner suite assignment.
    - V8/Masonry/GPU compile map: every suite and bench links the `clay` library, whose dependency graph includes `deno_core 0.400.0`, `masonry 0.4.0`, `masonry_winit 0.4.0`, `vello 0.6.0`, and `wgpu 26.0.1`; therefore all-target compilation pays those dependency compile/link costs even when tests are headless. The `clay` bin compiles the Masonry/winit window path; `clay-runtime-sandbox` compiles the standalone V8 path. Headless tests do not claim GPU rendering; live GUI/AT-SPI tests remain environment-gated/ignored as applicable.
    - Warm command timings and process peaks (temporary `/tmp/plan089-baseline` sampler):

      | Candidate | Result | Duration | Target delta | Peak Cargo / rustc |
      | --- | --- | ---: | ---: | ---: |
      | `cargo fmt --check && cargo test --lib --quiet` run 1 | PASS, 1559/2 ignored | 19.777 s | -50,797,858 B | 1 / 1 |
      | same quick candidate run 2 | PASS, 1559/2 ignored | 12.536 s | 0 B | 1 / 0 |
      | full candidate run 1 | FAIL: one existing watcher test timed out after 1558 lib tests passed | 43.381 s | -31,994,270 B | 2 / 16 |
      | full candidate run 2 | PASS; all gates and bench compile | 103.945 s | +109,507 B | 2 / 15 |

      The failed watcher case, `server::runtime_generation_tests::configuration_watcher_preserves_generation_on_failure_and_recovers`, passed in an isolated exact rerun in 0.14 s; retain this as flaky baseline evidence for the later bounded-timeout task, not as a release pass.
    - Post-run storage shape: `target/` 41,644,426,544 B (38.78 GiB), `target/debug` 33.09 GiB, `debug/deps` 17.15 GiB, `debug/incremental` 15.52 GiB across 244 incremental directories, `debug/build` 0.34 GiB, `target/release` 5.23 GiB (`release/deps` 5.00 GiB), `target/pi-docs` 0.46 GiB, and `target/criterion` 0.01 GiB. Executable files: 162 in `debug/deps`, 4 in `debug`, 93 in `release/deps`, 4 in `release`, 567 under `target` including retained historical artifacts. This is 2.28× the comprehensive-review snapshot of 17 GiB; the dominant shape is accumulated debug dependency/incremental state, not a second target tree.
    - Security baseline: `cargo audit` 0 vulnerabilities with the three documented allowed unmaintained warnings (`bincode`, `paste`, `ttf-parser`); security suite passed 130 with 1 ignored. Baseline is non-release for the quick candidate; only the serial full candidate includes audit and all suites.
    - Conclusion: the next task should own one Linux `quick|full` wrapper, a repo-local lock for full runs, CI/doc parity, and a cheap storage report. No production/script change was made in this baseline task.

- [x] Implement one quick check and one lock-protected serial full check
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

  - Evidence (2026-08-16):
    - `scripts/check.sh` (new, executable) is the one supported entry point: `quick` runs `cargo fmt --check` then `cargo test --lib --quiet` (labeled non-release), and `full` acquires the repo-local lock at `target/.clay-full-check.lock` via `flock 9` and runs `cargo audit`, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets --quiet`, `cargo bench --no-run` serially in that order with `set -eu`; each stage prints its command and the EXIT trap reports `FAILED at stage: <name>` on failure. A concurrent `full` invocation waits on the lock (documented wait policy, never two gates on one checkout); `full` refuses to run when `target/` is a symlink so the lock stays inside the repo; both modes reuse the repository `target/` and never set `CARGO_TARGET_DIR`.
    - Behavior verified live: `quick` PASSED in 20.9 s (1559 passed/2 ignored). A first `full` run correctly waited ~45 s behind a held lock (log showed only the waiting line until the holder released), then failed at stage `test` on the pre-existing flaky `configuration_watcher_preserves_generation_on_failure_and_recovers` watcher timeout (the same flake recorded in the baseline evidence; its fix is owned by the next bounded-timeout task) with a clear `FAILED at stage: test` report and exit 101. A clean rerun then PASSED end to end (2 m 24.6 s, exit 0, all gates plus bench-profile compile).
    - `.github/workflows/ci.yml` now invokes `scripts/check.sh full` as a single step (replacing the five inline gate steps), keeping the rust-toolchain/cargo-audit setup and the always-on build-storage report step, so CI runs the identical serial gate as local `full`.
    - `docs/development/build-and-test.md` gained a `## Supported check wrapper` section (commands, lock path, wait policy, symlink refusal, CI parity) and the Required gates note now points at the wrapper.
    - New deterministic gate test `manual_smoke_docs::check_script_pins_quick_and_full_gates_and_ci_parity` in `tests/manual_smoke_docs.rs` (protocol suite) pins the quick/full markers, the serial full-branch stage order, the absence of `CARGO_TARGET_DIR`, and the CI `scripts/check.sh full` invocation; it passed via `cargo test --test protocol manual_smoke_docs::check_script_pins_quick_and_full_gates_and_ci_parity -- --exact --test-threads=1` and inside the full gate.
    - Verification: the clean `scripts/check.sh full` run passed all gates including `cargo clippy --all-targets -- -D warnings`; `git diff --check` clean; script `bash -n` clean.

- [x] Add bounded async-test helpers and actionable timeout diagnostics
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

  - Evidence (2026-08-16):
    - New `wait_until` helper in `server::runtime_generation_tests` (src/server/mod.rs): polls a condition every 10 ms under a bounded deadline; on timeout the panic reports the scenario name, the live generation id, and the diagnostic-code snapshot (`generation_id=… diagnostics=[…]`) with a pointer to pending session/runtime-replacement cleanup, never a bare `Elapsed`. It is test-only (no production work) and is used by all four configuration-watcher reload waits (previously four near-identical 2 s poll loops with bare `.expect` messages), with the bound raised to 5 s matching the Plan 086 whole-workflow precedent.
    - New self-test `wait_until_panics_with_scenario_and_server_state_on_timeout`: a deliberately pending condition (50 ms bound) panics with the scenario name and server state, pinning the diagnostic message; normal scenarios complete far below the deadline (watcher module runs in ~0.6 s).
    - The new diagnostics exposed the real root cause of the recurring `configuration_watcher_preserves_generation_on_failure_and_recovers` flake (2–3 of every 3 full-lib runs failed under parallel load even at 5 s): the panic showed `generation_id=2 diagnostics=["runtime.syntax_error"]` — the recovery reload never ran. Root cause: `watch_configuration_root_with_intervals` adopted the post-reload scan as its baseline, so a change that landed while a reload was in flight (the recovery write after the failed-reload diagnostic) was absorbed and never re-detected; the watcher then waited forever with a stale baseline. Fixed in src/server/config_watch.rs by keeping the pre-reload stable snapshot as the baseline (reloads never write watched files; `preferences.json` is written only by the settings command handlers before they reload, so a post-reload difference is a genuine external change).
    - New deterministic regression test `watcher_reloads_for_a_change_that_lands_during_a_reload` (src/server/config_watch.rs): a reload callback that writes the watched file mid-reload must trigger a second reload. Validated both ways: it FAILS with the old baseline-adoption code (`left: 1, right: 2`) and PASSES with the fix.
    - Stability: the previously flaky test passed 6/6 full parallel `cargo test --lib` runs after the fix (and 1/1 inside the all-targets gate), versus ~50% failure before; the helper self-test and all 39 `runtime_generation_tests` pass. `docs/development/build-and-test.md` bounded-tests section documents the helper and its timeout report.
    - Verification: `scripts/check.sh full` PASSED end to end (audit, fmt, check, clippy `-D warnings`, all-targets tests, bench compile); `git diff --check` clean. `src/server/connection.rs` needed no change — its timeout site is production code with an intentional timeout-fallback, and the Plan 086 bounded menu/runtime tests already carry actionable 5 s messages.

- [x] Close Plan 086 focus and accessibility follow-ups before platform validation
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
      - `src/masonry_shell.rs` and `src/driver/reconcile.rs`: focus transfer/clear before pane/tab detachment and survivor focus after the active tab is laid out.
      - `Cargo.toml`, `Cargo.lock`, and `vendor/masonry_core/`: exact Masonry 0.4.0 patch for subtree-removal focus invalidation and synthetic Window action ingress; remove when upstream carries the guard.
      - Existing consumer/accessibility tests in the owning module and `tests/live_atspi_smoke.rs` only for live regression coverage.
      - `docs/development/accessibility.md`: document the focus invariant and regression command.
    - References:
      - `code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log` and `focus-frame-crash.log`.
  - Test Cases to Write:
    - Focused dirty-pane removal, dirty-close rejection, clean focused-pane removal, survivor focus restoration, stale focused-ID consumer update, top-level Frame focus event, editor Entry focus control, and repeated tab/pane removal without panic.

  - Evidence (2026-08-16):
    - UI preflight rerun: `npx ui-skills start`; accessibility routing loaded `ibelick/fixing-accessibility` (1 skill, within prefer-1/max-3). Its focus, dialog, keyboard, and semantic-state guidance was applied to native Masonry/AccessKit rather than web widgets.
    - Root cause was the shared Masonry removal/event seam: `MutateCtx::remove_child` repaired only `focus_anchor`, leaving `focused_widget`/`next_focused_widget` naming a detached pane; `RenderRoot::handle_access_event` also converted the synthetic top-level Window node directly into a non-existent `WidgetId`.
    - Vendored exact Masonry 0.4.0 source under `vendor/masonry_core/` and patched the shared core: subtree removal invalidates focused/next/fallback IDs, then the normal focus rewrite rebuilds the path and clears ancestor flags; `MutateCtx` exposes bounded focus transfer/clear methods, and Window-node actions return before widget dispatch. Clay shell tree changes transfer focus to a surviving same-tab target or clear it before detach; active-tab removal clears while the survivor is stashed, and `driver/reconcile.rs` focuses the survivor after layout.
    - Regression coverage: `dirty_pane_close_rejection_and_discarded_removal_keep_focus_consumer_safe`, `pane_close_removes_routing_target_and_reconciles_host`, `remove_tab_uninstalls_hosts_and_switches_to_first_remaining`, and `top_level_frame_focus_action_does_not_dispatch_to_a_widget`; each uses the real `accesskit_consumer::Tree` where a tree update is expected. Valid editor focus remains the control path.
    - The existing live smoke remains environment-gated by `CLAY_LIVE_A11Y_SMOKE=1`; the synthetic-frame ingress regression is deterministic in the headless shell test because blind live AT-SPI action calls can block on this host's window/event bridge.
    - Verification: focused lib tests, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`, `git diff --check`, and the serial `scripts/check.sh full` gate passed (1563 lib, 64 bin, 164 editor, 161 protocol, 198 runtime, 130 security + 1 ignored, all bench harnesses; audit 0 vulnerabilities / 3 documented unmaintained warnings).

- [x] Measure and guard editor, menu, tab, completion, and accessibility costs
  - Acceptance Criteria:
    - Functional: Bench/metrics cover typing/local paint proxy, command/filter updates, tab switch, completion projection/selection, and accessibility tree update after stable IDs.
    - Performance: Record before/after distributions and re-run the Plan 088 flagged local Criterion groups; deterministic work/payload/allocation bounds remain hard gates, and wall-clock values stay advisory until promotion criteria exist.
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
      - Add only missing benchmark groups/counters and deterministic ceilings; reproduce and classify any regression before edits, keeping machine-variance warnings separate from correctness failures.
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
    - Repeated fixed-input runs reproduce or dismiss each Plan 088 Criterion warning without changing blocking budgets.

  - Evidence (2026-08-16):
    - UI/performance preflight rerun: `npx ui-skills start`; inspected `accessibility`, `performance`, and `testing`; loaded `pbakaus/audit` (1 skill, within prefer-1/max-3). Its web-only audit guidance was translated to native Masonry/AccessKit: deterministic tree/update bounds, stable semantic IDs, no hot-path authority, and advisory-vs-blocking measurement separation.
    - Reused existing editor/protocol/runtime coverage: `editor_render_adjacent` remains the typing/local-paint proxy; `client_edit_queue_pressure` and `server_document_acknowledgements` cover edit/ack costs; `runtime_configuration_baselines` and `sdui_application_baselines` cover runtime/SDUI costs. No production metrics or public API were needed because existing `PerfRecorder` hooks already cover local edits/layout and the new surfaces are pure/native benchmark paths.
    - Added `completion_selection_work` and `command_centre_open_projection_work` to `src/perf/baselines.rs`; `window_baselines` now measures Command Centre open projection (16/60/256), completion selected-row projection (1/8/60/256), and retained accessibility label updates (2/4/8/16 tabs) with setup outside the timed closure via `AccessibilityTreeBench`. The existing completion filter group is the shared Command Centre/completion fuzzy matcher.
    - Added deterministic guards `accessibility_updates_reuse_stable_virtual_ids_without_allocator_churn` and `retained_accessibility_update_fixture_stays_bounded`; existing no-reserialization, no-IPC/JS/filesystem hot-path, payload, responsive-layout, completion-bound, and `accesskit_consumer` tests remain blocking. Virtual IDs continue to derive from retained owner/client slots; no `WidgetId::next()` churn was introduced.
    - Fixed-input before/after measurements used the exact Plan 088 command (`--sample-size 10 --warm-up-time 1 --measurement-time 2`) on the same Linux host. Before/after medians are recorded in `docs/development/performance.md`: pane 72/373/718 ns → 94/481/1.803 µs; tab 87/380/783 ns → 180/864/1.898 µs; responsive 2.13–2.35 µs → 4.43–5.47 µs; completion open 1.62/8.42/55.83/234.64 µs → 2.82/17.03/110.26/484.64 µs; filter 8.04/45.96/255.61 µs → 15.97/100.49/533.28 µs; layout 550/545/554 ns → 967/1.084/1.118 µs. New medians: Command Centre open 22.74/84.33/270.86 µs, completion selection 2.93/14.80/97.85/448.24 µs, and accessibility update 70.61/121.98/208.08/410.33 µs for the documented inputs.
    - The after-run Criterion output reported broad local shifts in existing groups; these remain advisory and are explicitly handed to the next task, `Triage Plan 088 advisory Criterion regressions before policy promotion`, rather than being hidden by budget changes or benchmark rewrites. No deterministic budget regressed.
    - Focused verification passed: `cargo test --test editor editor_performance_invariants -- --test-threads=1` (35), `cargo test --test protocol performance_budgets -- --test-threads=1` (21), editor/protocol/runtime benchmark commands, `cargo clippy --all-targets -- -D warnings`, formatting, and `git diff --check`.
    - Final verification: `scripts/check.sh full` passed (audit: 0 vulnerabilities / 3 documented unmaintained warnings; lib 1563 passed/2 ignored; bin 64; editor 166; protocol 162; runtime 198; security 130 passed/1 ignored; all benchmark harnesses and `cargo bench --no-run` passed). `docs/wiki/index.md` and `docs/wiki/modules/performance-fixtures.md` document the new guards and command paths; no Clay JS API, package authority, filesystem, network, shell, or client-JavaScript capability was added.

- [x] Triage Plan 088 advisory Criterion regressions before policy promotion
  - Acceptance Criteria:
    - Functional: Re-run the exact Plan 088 `window_baselines` command and compare pane-paint, tab-switch, responsive-layout, centered-overlay, completion, and filter groups with the recorded baseline; each warning is classified as machine variance, benchmark instability, or reproducible implementation regression.
    - Performance: Do not promote advisory wall-clock numbers to cross-machine CI policy or raise budgets solely to hide a warning; reproducible regressions get a bounded fix task or an explicit deferred ceiling with evidence.
    - Code Quality: Keep benchmark inputs, sample/warm-up/measurement settings, and comparison method fixed; change benchmark code only when it measures the intended pure-geometry path and the change is independently verified.
    - Security: Benchmark fixtures remain inert, bounded, and isolated from package JavaScript, filesystem/network/process authority, user documents, and secrets.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md`, `benches/window_baselines.rs`, `src/perf/baselines.rs`, and `src/perf/budgets.rs`.
      - Plan 088 Task 7/12 evidence and the recorded advisory Criterion warnings.
      - Project patterns `protocol-and-performance.md`, `maintenance-validation.md`, and `planning-checklist.md`.
    - Options Considered:
      - Raise budgets or delete Criterion warnings: rejected; that converts noise into policy without evidence.
      - Treat one local run as a source regression: rejected; Criterion noise and host variance need repeated runs.
      - Repeat fixed runs, compare distributions, then fix/defer only reproducible regressions: chosen.
    - Chosen Approach:
      - Run the same bounded benchmark command at least three times on an otherwise idle host, record medians/ranges and Criterion direction, compare against Plan 088 evidence, and preserve advisory status until a stable cross-machine promotion rule exists.
    - API Notes and Examples:
      ```bash
      cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
      ```
    - Files to Create/Edit:
      - `docs/development/performance.md`: current comparison and classification.
      - `plans/089-Audit-Remediation-Validation-Performance-and-Developer-Feedback.md`: evidence/deferred ceiling.
      - `benches/window_baselines.rs` or `src/perf/baselines.rs` only for a demonstrated measurement defect; no budget change without independent evidence.
    - References:
      - `plans/088-Audit-Remediation-Clay-UI-Modernization.md` Task 7/12 evidence and `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md` P2 performance findings.
  - Test Cases to Write:
    - Three fixed-input runs produce a classification table; deterministic performance-budget tests remain green; a synthetic/reproducible regression is not marked machine variance without a recorded rationale.

  - Evidence (2026-08-16):
    - UI/performance preflight rerun: `npx ui-skills start`; inspected `performance` and `testing`; loaded `pbakaus/audit` (1 skill, within prefer-1/max-3). Its web-only audit guidance was translated to native Masonry/AccessKit benchmark triage: fixed inputs, bounded inert fixtures, deterministic hard gates, and advisory timing separation. `clay-ui` references were checked; no UI implementation or catalog change was needed.
    - Ran the exact fixed-input command three times sequentially on the same Linux host with no competing Cargo/rustc process: `cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2`. Inputs and Criterion settings stayed unchanged; CPU pinning/cross-machine stability was not claimed. Raw logs: `/tmp/plan089-task7/window-run-{1,2,3}.txt`.
    - Classification is recorded in `docs/development/performance.md`: pane/tab, responsive layout, completion open/filter/selection, Command Centre, retained accessibility updates, and completion layout are machine-variance warnings; centered overlay is benchmark instability because its ~0.25–0.55 ps values are below useful timer resolution. Run medians moved in both directions and varied materially between adjacent runs; no reproducible implementation regression was found.
    - Criterion saved-target directions were contradictory with unchanged code: Run 1 reported 34 improvements / 1 regression / 1 unchanged group; Run 2 reported 2 / 33 / 1; Run 3 reported 14 / 8 / 14. This is evidence against promoting a single-run warning to a source regression. No benchmark inputs/code, deterministic budget, or policy threshold was changed.
    - Focused verification passed: `cargo fmt --all -- --check`, `cargo test --test protocol performance_budgets -- --test-threads=1` (21), and `git diff --check`. The prior Plan 089 full gate remains green (1563 lib, 64 bin, editor 166, protocol 162, runtime 198, security 130 + 1 ignored, all bench harnesses); this task changed documentation/plan evidence only.

- [x] Add compact malformed-codec and key/menu state-machine generated coverage
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
      - `src/protocol/codec.rs`: compact framed archive mutation generator.
      - `src/editor/surface.rs`: generated pending-chord state-machine coverage, including timeout.
      - `src/server/menu_sessions.rs`: generated menu lifecycle/authority coverage.
      - `docs/wiki/modules/protocol-codec.md`, `docs/wiki/modules/sequence-keybindings.md`, and `docs/wiki/modules/transient-menu-round-trip.md`: synchronized test/coverage guidance.
    - References:
      - Audit test gaps 3–4; `protocol-and-performance.md`; `planning-checklist.md`; ponytail no-new-dependency rule.
  - Test Cases to Write:
    - Truncation/bit mutation/length mismatch/oversize; every key/menu transition order; stale generation; repeated cancel/close; selection bounds.

  - Evidence (2026-08-16):
    - UI preflight rerun before key/menu test review: `npx ui-skills start`; inspected `interaction`; loaded `jakubkrehel/better-accessibility` (1 skill, within prefer-1/max-3). Its keyboard/focus/menu guidance was translated to Clay's native key-routing and server-owned menu state; this task changed tests only, so no visual UI state changed.
    - Added `protocol::codec::tests::compact_generated_frame_mutations_fail_closed_without_panicking`: 64 fixed seeds × client/server frames with deterministic truncation, mismatched length prefixes, payload bit mutations, and oversize declarations. Truncation/length/oversize cases assert fail-closed errors; mutated archives are wrapped in `catch_unwind` so no malformed local-IPC frame can panic the test boundary. Existing rich archive sweeps remain unchanged.
    - Added `editor::surface::tests::editor_generated_chord_sequences_preserve_prefix_mismatch_and_timeout_transitions`: 128 bounded cases cover completed two- and three-stroke chords, mismatch re-evaluation without swallowing text, and stale timeout re-evaluation. Every case ends with no pending chord and at most one intended inserted character.
    - Added `server::menu_sessions::tests::generated_menu_intent_ordering_preserves_lifecycle_and_authority`: 64 seeds × 18 deterministic intents cover open/query/select/activate/cancel/reload, repeated stale operations, selection extremes (`i64::MIN`/`MAX`), high-bit session IDs, one-active-session replacement, stale-generation rejection, catalogue/provenance checks, and cancel cleanup.
    - No production behavior, public API, dependency, fuzz daemon, or unbounded corpus was added. Verification passed: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --quiet` (1566 passed/2 ignored), serial `scripts/check.sh full` (all suites, audit 0 vulnerabilities / 3 documented unmaintained warnings, all benches and bench compilation passed), `cargo test --test protocol primitives_docs -- --test-threads=1` (27), and `git diff --check`. Relevant wiki pages were synchronized after the green code gate; the master wiki index already links all three pages.

- [x] Add real Linux multi-window, DPI, font-scale, and Wayland validation
  - Acceptance Criteria:
    - Functional: Environment-gated smoke opens representative multi-window/tab/pane states on Wayland, verifies logical/physical bounds, scale change, user font-scale layout, focus, and accessibility; fixed fixtures support manual screenshots. It also provides a safe path to reach Plan 088 completion, Command Centre, settings, file-browser, multi-tab/multi-pane, narrow/wide, and DPI states, or reports the exact unavailable-backend prerequisite. The review/manual workflow provides safe Clay window target/raise/focus controls or a no-focus semantic harness path, never relying on blind portal input.
    - Performance: Smoke has bounded startup/cleanup and adds no production per-frame work.
    - Code Quality: Extend existing GUI/review fixtures; no second event loop or platform abstraction; keep semantic AT-SPI actions and target selection explicit when compositor focus is unavailable.
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
      CLAY_LIVE_WINDOW_SMOKE=1 cargo test --test security live_atspi_smoke::live_multi_window_scale_smoke -- --ignored --exact --test-threads=1
      ```
    - Files to Create/Edit:
      - Existing smoke/window test owner in `src/main.rs` or `tests/window_management_protocol.rs` (tentative).
      - `docs/development/launch-and-gui-smoke.md`, `docs/development/windows.md` if behavior notes change.
      - `scripts/capture-ui-review.sh` or the existing review-tooling owner (tentative): safe target/raise/focus orchestration, never blind portal input.
      - Plan 087 fixture/capture files.
    - References:
      - Audit test gap 5; Linux platform policy.
  - Test Cases to Write:
    - 1x/2x scale, UI typography extremes, two windows, multi-pane, focus transfer, close cleanup, Wayland launch, accessibility bounds, safe window raise/target/focus or no-focus semantic action routing, Plan 088 interactive states, native dialog selection, observer/restart/local-fallback keyboard flows, and full quit/relaunch persistence when the host targeting prerequisite is available.

  - Evidence (2026-08-17):
    - UI preflight rerun before implementation/review: `npx ui-skills start`; loaded `pbakaus/adapt` plus `ibelick/fixing-accessibility` (2 skills within prefer-1/max-3). Their responsive/adaptation and focus/accessibility guidance was translated to Masonry logical bounds, typed typography, AT-SPI semantic observation, and safe compositor targeting; no web/CSS breakpoint or native widget was added.
    - Added deterministic `masonry_shell::tests::rescale_event_recomputes_logical_bounds_from_physical_size`: Masonry `Rescale(2.0)` plus physical 1800×1200 resize produces a 900×600 logical shell. Added `live_multi_window_scale_smoke` in `tests/live_atspi_smoke.rs`, gated by `CLAY_LIVE_WINDOW_SMOKE=1`, which launches one isolated server and two real clients on Wayland, applies complete user typography (monospace 20, proportional 21, UI 24), disambiguates AT-SPI frame paths by application PID, and checks two distinct frames, positive bounded physical extents, and two large bounded status bars. The live command passed on this host: `CLAY_LIVE_WINDOW_SMOKE=1 cargo test --test security live_atspi_smoke::live_multi_window_scale_smoke -- --ignored --exact --test-threads=1`.
    - Added `tests/fixtures/configuration/ui-review-large-typography/init.js` and wired `ui-review-large-typography` into `scripts/capture-ui-review.sh`, launch documentation, smoke-doc tests, and the indexed UI-review wiki page for later screenshots. The fixture uses only the public `clay:theme` typography API and no ambient files; a live capture returned `review.status=PASS`.
    - Safe targeting path is documented: run `computer-use-linux doctor`, then `computer-use-linux setup-window-targeting` and log out/in when GNOME requests shell reload; this host's setup installed the extension but still reports `org.freedesktop.DBus.Error.ServiceUnknown` for the GNOME Shell window-control API (`can_query_windows=false`, `can_focus_windows=false`). Targeted completion/Command Centre/settings/file-browser/multi-tab/multi-pane/narrow/wide/native-dialog input therefore remains `UNRESOLVED`; no blind portal coordinates or unscoped chords were used. Existing deterministic focus/event-ingress tests remain the no-focus semantic coverage.
    - Verification: serial `scripts/check.sh full` passed (audit 0 vulnerabilities / 3 documented unmaintained warnings; lib 1567 passed/2 ignored, bin 64, editor 166, protocol 163, runtime 198, security 130 passed/2 ignored, all six bench harnesses and bench compilation); `git diff --check` clean.

- [x] Report and control build artifact size
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

  - Evidence (2026-08-17):
    - Extended `scripts/check.sh` with an advisory `report` mode using only `du`, `find`, and standard shell logic. It reports total `target/`, `target/debug/deps`, `target/debug/incremental`, and executable count; missing directories and symlinked paths are reported safely, no artifact contents are printed, and no cleanup is performed.
    - `.github/workflows/ci.yml` now invokes `scripts/check.sh report` in an `if: always()` storage step after the lock-protected full gate, so a failed/partial build still produces storage evidence without replacing the original gate result. `docs/development/build-and-test.md` documents the command, 50 GiB total / 20 GiB incremental advisory review thresholds, duplicate-target warning, and narrowest cleanup workflow (`cargo clean --profile debugging` before full `cargo clean`).
    - Added `manual_smoke_docs::check_script_reports_artifacts_without_deleting_or_masking_failures`: executes copied wrapper instances with missing and present target subdirectories, checks successful status/markers, and pins CI/docs/script parity. The existing quick/full stage-order test remains green; `bash -n scripts/check.sh` passes.
    - Post-full local report: `target` 50,344,408,122 B (`47G`), `target/debug/deps` 21,838,472,339 B (`21G`), `target/debug/incremental` 20,967,389,290 B (`20G`), and 181 executable files in `target/debug/deps`. Values remain below the exact 50 GiB/20 GiB advisory thresholds; no cleanup was triggered.
    - Verification: serial `scripts/check.sh full` passed (audit 0 vulnerabilities / 3 documented unmaintained warnings; lib 1567 passed/2 ignored, bin 64, editor 166, protocol 164, runtime 198, security 130 passed/2 ignored, all six bench harnesses and bench compilation); focused report/stage-order tests and `git diff --check` passed.

- [x] Perform visual screenshot and accessibility review of validation fixtures
  - Acceptance Criteria:
    - Functional: Capture multi-window, 1x/2x representative scale, typography extremes, menu/completion/Command Centre/tab/pane states, settings/file-browser/package-panel states, and Plan 088 empty/error/loading/recovery states; accessibility tree bounds/names/focus/modal containment/announcements match visible state.
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
    - Visual/semantic bounds and focus checks for every live fixture, including renderer-level clipping for completion/Command Centre children and the recovered WelcomeWidget/loading states.

  - Evidence (2026-08-17):
    - Per-task UI preflight reran `npx ui-skills start` for `visual` and `accessibility`, selecting `vercel-labs/web-design-guidelines` and `ibelick/fixing-accessibility` (2 skills, within the prefer-1/max-3 rule). The guidance was applied to Clay's native Masonry/AccessKit surfaces rather than copied as web/CSS implementation.
    - Window targeting became available after the GNOME Shell extension reload. `computer-use-linux doctor`, targeted `get_app_state`, `list_windows`, semantic actions, targeted keyboard input, and AT-SPI dumps all worked with safe window identity checks.
    - Evidence is under `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/`: dark default, delivered loading SDUI, runtime error, recovery, large typography, interactive completion, multi-window, and responsive narrow/wide states have PASS artifacts. Completion exposes bounded `Menu`/`ListItem`/selected `MenuItem` semantics and no visible rows outside the capped surface; loading visibly renders `Loading review`/`Loading workspace…`; multi-window uses two real Clay processes with PID-separated AT-SPI evidence; narrow welcome copy is shortened and remains within bounds.
    - Recovery review found a renderer-only stale state: `WelcomeState` updated the parent accessibility/status path while Masonry retained the WelcomeWidget child scene. `PaneDocumentView::request_welcome_render` plus the event-driven call at `EditorWidget::sync_region` now marks the visible child for `request_render()`; no polling, IPC, or per-frame work was added. The disconnected regression test drives this path, and the final targeted recovery capture shows `Connection lost` / `Connection: Disconnected` consistently in visual and AT-SPI evidence.
    - Command Centre remains `UNRESOLVED`: targeted `Ctrl+Alt+P` and `Ctrl+X`/`Ctrl+P` did not render its dialog, and an alternate targeted key reached a stale-behavior recovery error rather than a modal. Settings remains unrendered because `settings.open`/`settings.close` acknowledge without making the hidden panel visible. File-browser review was safely aborted when native `Pick Files` exposed ambient user locations; no chooser screenshot was retained. Package panels expose only the empty landmark in these fixtures. These are recorded as blockers, not passes, in `visual-review/review-log.md`.
    - Full serial verification after the repaint fix: `scripts/check.sh full` passed (audit 0 vulnerabilities with 3 documented allowed unmaintained warnings; fmt/check/clippy clean; lib 1568 passed/2 ignored, bin 64, editor 166, protocol 164, runtime 198, security 130 passed/2 ignored; all six bench harnesses and `cargo bench --no-run` passed). Focused `masonry_editor::tests::disconnected_welcome_accessibility_tracks_status_update`, `cargo build --bin clay`, and `git diff --check` passed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
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

  - Evidence (2026-08-17):
    - Inventory result: no new public programmatic capability. Plan 089 touched none of `runtime/js`, `src/server/ops`, `src/packages/manifest.rs`, `docs/reference/clay-js-api`, `docs/generated`, or the API inventory; every changed Rust function (driver restore gate, `config_watch` reload baseline, SDUI kind-changed reconcile, welcome status/render helpers, shell focus repair, benchmark/baselines helpers) is `pub(crate)`/private/test-scoped or an established non-server widget method. No new `deno_core` op, `clay:*` facade, JS config API, or command ID was added.
    - Existing public theme surface (`theme.setTheme`/`setAppearance`/`setTypography` as `clay:theme` flat exports with stable IDs and complete metadata) remains documented and discoverable; the `settings.*` inert intents and `keybindings.bindKey` configuration APIs are unchanged.
    - Gates run (all green): `cargo test --test security rust_visibility_api_mapping` (12 passed, including `phase24_4` doc(hidden) pub allowlist of exactly 4 and `phase22_6` window-model/benchmark-helper pins: `pub(crate)`, never wrapped in an op, never in a JS facade); `cargo test --test protocol clay_js_doc_registry` (41 passed, including `generated_registry_is_current`, `set_typography_api_doc_has_required_configuration_metadata`, `set_typography_api_is_linked_and_generated_registry_is_current`, and the closed-inventory tests asserting internal ids never appear in the registry); `cargo test --test protocol clay_js_facade_layout` (5 passed, pinning the 22 facade modules/exports); `cargo test --test editor no_conformance_helper_exposed_as_op_or_facade` (1 passed, scanning every `op_clay_*` name and facade for conformance/test-helper intent).
    - Security fact: no test hook, arbitrary fixture handle, archive-byte surface, window handle, or process-control API reaches packages/configuration authority; benchmark fixtures (`AccessibilityTreeBench`, `command_centre_session_for_benchmark`) stay `#[doc(hidden)]` in `src/perf` and inert (no package JS/filesystem/network/process work in measured paths). No instrumentation JS API enters production hot paths because none was added.
    - Registry/doc state: `generated_registry_is_current` passed, so `cargo run --bin update-doc-registry` was not needed (docs unchanged in this task).
    - UI preflight: not applicable to this task — it is API/registry surface verification only, with no UI review, implementation, or UI file edits (the plan's UI planning gate requires reruns for independently executed UI tasks).
    - Full verification: serial `scripts/check.sh full` PASSED (audit 0 vulnerabilities / 3 documented allowed unmaintained warnings; fmt/check/clippy clean; lib 1568 passed/2 ignored, bin 64, editor 166, protocol 164, runtime 198, security 130 passed/2 ignored; all six bench harnesses and `cargo bench --no-run` passed); `git diff --check` clean.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Update/execute modules 01, 04, 07, 09, 10, 11, 13, and 14 for quick/full workflow, timeout diagnostics, recovery/loading fixture observability, completion/Command Centre/settings/file-browser/package-panel semantics, performance feel, multi-window/scale/Wayland fixtures, and the Plan 086 native-dialog, observer/restart, local-fallback, and quit/relaunch cases once safe window targeting is available; retain explicit blocked status otherwise.
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
    - Manual quick/full, live recovery/loading, completion/Command Centre/settings/file-browser, multi-window/scale, timeout diagnostic, and performance scenarios; every host-blocked interaction records the exact unavailable backend and artifact.

  - Evidence (2026-08-17):
    - All eight affected modules (01, 04, 07, 09, 10, 11, 13, 14) plus `test-plan/index.md` updated (11 files, 343 insertions/16 deletions): Plan 089 validation steps and Linux execution records added to each module; no existing step was deleted or weakened.
    - Plan 089 steps added: L20–L22 (multi-window/scale/large-typography fixture), T18–T19 (rescale/multi-window), S41–S42 (multi-window/responsive captures).
    - Plan 088 follow-ups resolved: L18/T75 recovery now PASS (Plan 089 `request_welcome_render` fix); L19 loading now PASS (delivered-RuntimeStateSnapshot evidence with restore-gate + kind-changed reconcile fixes); E22/K74 completion now PASS live (Plan 089 visual review captured the bounded popup with P1-087-UI-1 containment visually verified).
    - Plan 089 additions documented: focus repair fix (module 13 S40), generated state-machine tests (module 10 K77), Criterion triage classification (module 11 Q15), responsive narrow/wide captures (module 13 S36–S42).
    - Command Centre (K75) remains UNRESOLVED because `Ctrl+Alt+P` is consumed by GNOME before reaching Clay; structural tests pass.
    - Settings/package panels (module 09 P16–P21) remain NOT RUN visually because `settings.open` does not persist or make the panel visible.
    - `test-plan/index.md` coverage matrix gained a Plan 089 row; Plan 089 task 9 Linux execution record section added with per-module PASS/UNRESOLVED/PASS-structural evidence.
    - Full verification: serial `scripts/check.sh full` PASSED (audit 0 vulnerabilities / 3 documented allowed unmaintained warnings; fmt/check/clippy clean; lib 1568 passed/2 ignored, bin 64, editor 166, protocol 164, runtime 198, security 130 passed/2 ignored; all six bench harnesses and `cargo bench --no-run` passed); `git diff --check` clean.

- [x] Update or verify the code wiki after implementation
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

  - Evidence (2026-08-17):
    - `docs/wiki/modules/maintenance-validation.md` gained a Plan 089 section documenting `scripts/check.sh` (quick/full/report gates with serial lock), `wait_until` bounded async-test helper (10 ms poll, 5 s deadline, configuration_watcher race fix), build artifact reporting (advisory target/ size breakdowns), Criterion triage (three fixed-input runs classified all groups as machine variance except centered_overlay as benchmark instability), generated state-machine coverage (deterministic stdlib-generated tests for codec/chord/menu with local Lcg), and live platform validation (CLAY_LIVE_WINDOW_SMOKE multi-window smoke + headless rescale test).
    - Plan 089 sections already present in `performance-fixtures.md` (cost guards), `masonry-shell.md` (focus repair), `masonry-sdui-region.md` (kind-changed reconcile), `pane-document-views.md` (welcome status), `protocol-codec.md` (generated frame mutations), `sequence-keybindings.md` (generated chord sequences), `transient-menu-round-trip.md` (generated menu intent ordering), and `ui-review-harness.md` (Wayland multi-window smoke).
    - `docs/wiki/index.md` already links all Plan 089 implementation pages in the Plan 088 UI modernization map section (theme registry, typography registry, shell, pane-document-views, SDUI/package-UI, workspace-file-browser, performance-fixtures, ui-review-harness).
    - `cargo test --test protocol primitives_docs` passed (27 tests), confirming wiki index and implementation page alignment.
    - Full verification: serial `scripts/check.sh full` PASSED (audit 0 vulnerabilities / 3 documented allowed unmaintained warnings; fmt/check/clippy clean; lib 1568 passed/2 ignored, bin 64, editor 166, protocol 164, runtime 198, security 130 passed/2 ignored; all six bench harnesses and `cargo bench --no-run` passed); `git diff --check` clean.

## Compromises Made

- No new property-testing/fuzzing dependency. Fixed-seed generated coverage is sufficient for current bounded state spaces; add a dedicated fuzzer only after it finds value unavailable from compact deterministic cases.

## Further Actions

- Do not mark Plan 089 complete until its three Plan 088 follow-up owners have evidence: Welcome/loading state closure, safe-targeted visual review, and Criterion regression classification.
- Keep Plan 090 blocked until Plan 089's follow-up tasks and full Linux baseline are green; keep Plan 091 blocked while any P1 visual/accessibility defect remains unresolved.
- Consider hard artifact/wall-clock gates only after stable CI history establishes low-noise thresholds.
