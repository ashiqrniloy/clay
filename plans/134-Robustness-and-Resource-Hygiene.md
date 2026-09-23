# 134 — Robustness and Resource Hygiene: Float Comparisons, Lock Poisoning, Async FS, Snapshot Clones

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
D4, D5, P3, R3 (review §1, §2, §3). Small, independently valuable robustness
fixes. Also records the two explicit no-action ceilings (R2, R4) so they are
decisions, not oversights.

## Objectives

- D4: eliminate strict `f32/f64 ==` comparisons in production shell/theme/layout
  code (82 clippy sites; production: `src/shell/theme.rs` 16, `package_ui.rs` 7,
  `icons.rs` 6, `design_system.rs` 4, `editor/theme.rs` 2, `ops/theme.rs` 2,
  `ops/modes.rs` 2, `server/mod.rs` 4) — identity round-trips become explicit
  integer/tolerance comparisons.
- D5: adopt a poison policy for the ~90 `.expect("…mutex poisoned")` sites on
  server-critical mutexes: recover via `PoisonError::into_inner` (precedent:
  `src/server/locks.rs:226`) for state mutexes where the invariant is
  self-healing, keep `expect` only where poison implies irrecoverable
  corruption; no behavior change on the healthy path.
- P3: move hot-path blocking `std::fs` calls in async contexts to `tokio::fs` /
  `spawn_blocking` — priority: `src/server/workspace/mod.rs`
  `canonicalize`/`metadata` on the open-document path; `configuration.rs`,
  `agent_documents.rs`, `launcher.rs`, `ops/theme.rs`, `ops/packages.rs`
  read/write sites. (Not all 324 sites — hot paths only; the rest recorded as a
  known ceiling.)
- R3: swap the completion-provider snapshot only when changed
  (`src/server/js_runtime/mod.rs:704` clone-on-every-evaluation).
- R2/R4: record explicit no-action decisions (broadcast-lane snapshot cloning at
  desktop scale; resident-set floor bounded by budgets).

## Expected Outcome

- Clippy pedantic `float_cmp` count in production files: zero (tests may compare
  with explicit tolerance helpers).
- A poisoned state mutex no longer kills the server process on paths where
  recovery is safe; a policy comment names the rule; the unhealthy path is
  covered by a test that poisons a mutex and observes continued service.
- Open-document path performs no blocking `std::fs` on the async runtime
  (spot-check via `tokio::console` or code review evidence); remaining cold-path
  `std::fs` sites inventoried as a known ceiling with a `ponytail:`-style comment
  naming the upgrade path.
- All Linux gates green.

## Tasks

- [x] Baseline gates, clippy inventories, and no-action decision records (R2, R4)
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full set passes untouched; record exit codes.
    - Performance: record `float_cmp` and blocking-fs inventories (file:line lists from the review, re-verified).
    - Code Quality: R2/R4 recorded as explicit decisions in the task evidence (desktop-scale ceiling; bounded floor) with revisit triggers (tab count > N, profile evidence).
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §1 (D4), §2 (P3), §3 (R1–R4).
    - Options Considered:
      - Act on R2 now (Arc snapshots): no profile evidence of need; YAGNI. Recorded as ceiling.
    - Chosen Approach:
      - Evidence-recording task.
    - Files to Create/Edit:
      - None.
    - References:
      - Review §3.
  - Test Cases to Write:
    - None (evidence-recording task).
  - Evidence (2026-09-22):
    - Tree: branch `review/1909`, HEAD `40022f6` ("WIP 133"), dirty — plan 133's
      completed-but-uncommitted split (`src/shell/theme/`, `src/shell/theme.rs`
      re-export, wiki, plan 133) is present; no plan-134 target touched before the
      run. Environment: rustc/cargo/clippy 1.98.1 (clippy 0.1.98), cargo-audit
      0.22.2, node v26.9.0, npm 12.0.2; WebKitGTK/JSC dev libs present, so the
      desktop stages plan 133 task 7 could not run now pass here.
    - Gates: `scripts/check.sh full` **exit 0** in ≈253 s, `full check PASSED` last
      line — audit (9 allowed RUSTSEC warnings), fmt, `check --all-targets`,
      clippy `-D warnings` (zero), test **2000 passed / 0 failed / 1 ignored**
      (root 1944 + desktop 55 + bindings 1), `bench --no-run`, `desktop-clippy`,
      `desktop-test`, `check-bindings.sh` (`webview bindings up to date`). Log
      `test-plan/artifacts/134-robustness/baseline-check-full.log`, exit codes
      `…/baseline-exit-codes.txt`.
    - Source review missing (repeat of plan 133 finding 1): plans 126–134 cite
      `code-reviews/2026-09-18-comprehensive-implementation-review.md`; absent from
      tree and git history, so plan-quoted inventories are unreconcilable and the
      measured lists below are the work order. Details `…/baseline.md`.
    - Float inventory (`cargo clippy --all-targets --message-format=json -- -W
      clippy::pedantic`, 28.6 s, exit 0): **81 unique sites** — **3 runtime
      production** (`src/shell/icons.rs:369,375,564`, alpha 0.0/1.0 identity
      checks), 19 in `#[cfg(test)]` modules inside production files
      (`package_ui.rs` 7, `design_system.rs` 4, `icons.rs` 2, `editor/theme.rs` 2,
      `ops/theme.rs` 2, `ops/modes.rs` 2), 59 in test files (`src/shell/theme/tests.rs`
      15, `tests/package_ui_conformance.rs` 12, `src/shell/layout/tests.rs` 10, …).
      The plan's per-file production split (43) is stale: plan 133 moved the old
      `theme.rs` 16 (`src/shell/theme/tests.rs` + `theme_snapshot_tests.rs`) and
      `server/mod.rs` 4 (`runtime_generation_tests.rs`) sites, all test-only; no
      `#[allow(clippy::float_cmp)]` exists anywhere. Lists
      `…/baseline-clippy-float-cmp.txt`, rendered `…-rendered.txt`, raw
      `…/baseline-clippy-pedantic.json`.
    - Blocking-fs inventory: 188 `std::fs::` lines in `src/`; 1278 `std::fs::`+bare
      `fs::` lines in 84 files; 506 outside test paths in 35 files — the review's
      "324" is unreproducible. Confirmed hot open-document sites (
      `canonical_file_state` 1957/1966/1994/2003, `reauthorize_open_file`
      2035/2058, `canonical_selected_file` 2115/2120, called directly by async
      prepare/authorize fns); **`agent_documents.rs` and `ops/theme.rs` already use
      `tokio::fs`** (P3 list stale for those two); ASYNC-REACHABLE sites in
      `configuration.rs` 107, 254, 367, 517/518, 784, 828; `launcher.rs` 59, 86–93,
      164, 221; `ops/packages.rs` 155, 255/262, 430, 676/682. Adjacent gap for P3:
      async `execute_workspace` (`command_execution.rs:287`) calls sync
      `list_directory` → recursive `traverse_directory` on the runtime.
      `FileMetadata::capture` (`workspace/mod.rs:2328`) has no production caller.
      Detail `…/baseline-fs-inventory.txt`, counts `…/baseline-fs-counts.txt`.
    - R2 recorded (no action): `StateFanout::publish`/`current` clone the snapshot
      (`src/server/fanout.rs:83–91`) and `latest_for` clones before per-client
      narrowing (`src/server/runtime_state.rs:88–97`); publish already sends an
      id-only lane. Desktop scale is capped at 64 connections / 64 documents per
      client / 64 snapshot documents (`src/perf/budgets.rs:65,69,249`) with no
      profile evidence of clone cost — YAGNI, Arc-sharing deferred. Revisit when
      those 64 caps rise, when the existing diff-review triggers fire (snapshot
      payload p95 > 768 KiB or install p95 > 16 ms; `budgets.rs:254–255`), or when
      a profile names the clone paths.
    - R4 recorded (no action): resident floor bounded by server budgets —
      `DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES` 256 MiB (security, fail-closed;
      `budgets.rs:399–403`), `SYNTAX_CACHE_BUDGET_BYTES` 30 MiB (`:154`), JS
      latency-lane heap 32 MiB (`:389`), plus the `docs/development/performance.md`
      advisory ≤256 MiB envelope. Revisit on profile evidence above that envelope
      with documents closed, or when a new retained cache lands without a byte
      budget.
    - No files created/edited by the task; evidence lives under
      `test-plan/artifacts/134-robustness/`. Findings that change later tasks:
      D4 shrinks to 3 runtime sites + 19 in-file test assertions; P3 drops
      `agent_documents.rs`/`ops/theme.rs` and should consider
      `command_execution.rs:287`; desktop gates now runnable on this host.

- [x] Replace strict float comparisons in production shell/theme/layout code (D4)
  - Acceptance Criteria:
    - Functional: all production `float_cmp` sites resolved — true identity checks (round-trip serialization of the same value) restructured to compare the serialized form or use exact `==` on the pre-serialization values with a comment; tolerance checks (contrast ratios, layout ratios) use named epsilon constants; no user-visible behavior change (theme/layout suites + conformance fixtures green).
    - Performance: no added allocation in hot compare paths (epsilon compare, not stringifying).
    - Code Quality: zero clippy `float_cmp` warnings in production files after `#[allow]` removal wherever possible; epsilons defined once per domain (e.g. `CONTRAST_EPSILON`).
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - Clippy inventory (`/tmp`-style evidence from the review: theme.rs 16, package_ui.rs 7, icons.rs 6, design_system.rs 4, editor/theme.rs 2, ops/theme.rs 2, ops/modes.rs 2, server/mod.rs 4); `src/shell/theme.rs` contrast math; `DESIGN.md` §13 (accessibility invariants — contrast thresholds must keep their exact floors).
    - Options Considered:
      - Blanket `#[allow(float_cmp)]`: hides the real bugs the lint exists for.
      - Per-site classification (identity vs tolerance). (Chosen.)
    - Chosen Approach:
      - Classify each site; identity → compare source values pre-boundary or bit-pattern (`to_bits`) with comment; tolerance → epsilon constant.
    - Files to Create/Edit:
      - `src/shell/theme.rs`, `src/shell/package_ui.rs`, `src/shell/icons.rs`, `src/shell/design_system.rs`, `src/editor/theme.rs`, `src/server/ops/theme.rs`, `src/server/ops/modes.rs`, `src/server/mod.rs`.
    - References:
      - Review D4; `DESIGN.md` §13.
  - Test Cases to Write:
    - Existing theme/layout/conformance suites green (behavior net); add one regression test per classified tolerance compare only where a golden value existed.
  - Evidence (2026-09-22):
    - Result: `clippy::float_cmp` **81 unique sites → 0** under
      `cargo clippy --all-targets --message-format=json -- -W clippy::pedantic`
      (exit 0); the only lint code removed versus the task-1 baseline set is
      `clippy::float_cmp`, and no `#[allow]` exists or was added anywhere.
      Per-site classification and conversion table:
      `test-plan/artifacts/134-robustness/task2-float-comparisons.md`; raw
      post-run JSON `…/task2-clippy-pedantic-final.json`.
    - Runtime production (3 sites, identity): `src/shell/icons.rs` 369/375/564
      (SVG arc large-arc/sweep flags) now share private `is_arc_flag`
      (`:283`) — `value.abs().to_bits() == 0.0f32.to_bits() || …== 1.0f32.to_bits()`;
      acceptance set preserved exactly (`0.0`/`-0.0`/`1.0` yes, `0.5`,
      `0.999_999_9`, next-float-after-1.0, `2.0`, NaN no). Regression test added:
      `arc_flag_identity_accepts_only_exact_zero_and_one`. Tolerance was *not*
      used for flags (it would validate near-flags).
    - Test-side (78 sites): golden-value assertions converted to
      `(actual - expected).abs() < f32::EPSILON`/`f64::EPSILON` (assert_ne pair →
      `>= EPSILON`), the house pattern at `src/shell/layout/mod.rs:698` and
      `src/editor/theme.rs:1130`; `src/shell/layout/tests.rs` reuses one hoisted
      file-level `EPSILON`; `tests/performance_budgets.rs` uses a `to_bits()`
      constant identity. Files: `editor/theme.rs`, `server/ops/{modes,theme}.rs`,
      `shell/{design_system,package_ui}.rs`, `shell/{layout,theme}/tests.rs`,
      `shell/theme/theme_snapshot_tests.rs`, `server/{connection/tests/mod.rs,
      connection/tests/protocol_and_bootstrap.rs,runtime_generation_tests.rs,
      js_runtime/tests/*}`, `tests/{package_ui_conformance,theme_packages,
      example_config_control_center_chord,performance_budgets}.rs`.
    - Five further strict comparisons that clippy 0.1.98's `float_cmp` does not
      flag (inside `assert!`/closures) converted for D4 consistency:
      `src/shell/layout/tests.rs:859` and `tests/package_ui_conformance.rs`
      1749/2463/2560/2737.
    - Deviation from plan wording: no named domain constants
      (`CONTRAST_EPSILON`) were warranted — no flagged site is a
      contrast-/layout-ratio tolerance comparison (contrast already compares
      `>=` floors); all are exact golden/identity comparisons, so the std
      per-type epsilons are used as the single definition per type domain. No
      behavior change: tolerance is effectively exact (machine epsilon).
    - Gates: `scripts/check.sh full` **exit 0**, ≈366 s — audit/fmt/check/clippy
      (`-D warnings`, zero)/test/**2001 passed, 0 failed, 1 ignored** (1945 root
      = baseline 1944 + the new icons test; 55 desktop; 1 bindings)/bench-compile/
      desktop-clippy/desktop-test/bindings. Log `…/task2-check-full.log`, exit
      codes `…/task2-exit-codes.txt`. Targeted pre-gate runs also green
      (icons 12, shell::theme 29, layout 84, package_ui 12, ops 25, editor::theme 18,
      design_system 6, runtime_generation 42, `--test runtime` 75).
    - Performance: compare paths are bit tests / epsilon arithmetic — no
      allocation, no stringification; flags still compared as parsed.
    - Security: none (no permission/authority path touched; the arc-flag change
      tightens rather than loosens validation).

- [x] Adopt and implement the mutex poison policy (D5)
  - Acceptance Criteria:
    - Functional: a documented rule (wiki + module docs): state mutexes whose invariants are re-derivable recover via `PoisonError::into_inner`; mutexes guarding irreversible consistency (e.g. pending oneshot maps) keep `expect` with a comment naming the corruption; the ~90 sites are triaged and updated accordingly; a poisoned critical section no longer kills the server on the recovered class (test poisons one and observes continued request service).
    - Performance: healthy path identical (recovery helper compiles to the same lock call).
    - Code Quality: one `lock_or_recover` helper (precedent `src/server/locks.rs:226` — note plan 131 deletes that file; move the helper to a live module first) used across sites; no duplicated poison prose.
    - Security: poison recovery never bypasses a permission check — recovered state re-validated on next use where the guard protects authority (per-site review recorded).
  - Approach:
    - Documentation Reviewed:
      - Review D5; `src/server/locks.rs:226` (`unwrap_or_else(PoisonError::into_inner)` precedent); `src/server/js_runtime/mod.rs` / `src/server/completion.rs` expect sites.
    - Options Considered:
      - Keep expect-everywhere: one panicked critical section = whole-server death with every editor connection — the review's finding.
      - Recover-all blindly: some guards protect half-completed irreversible operations; classification required. (Chosen: triage.)
    - Chosen Approach:
      - Helper + triage table in task evidence; mechanical migration.
    - Files to Create/Edit:
      - New small `src/server/lock_util.rs` (tentative) or an existing util module; the triaged sites.
    - References:
      - Review D5; plan 131 (locks.rs deletion ordering — coordinate: helper extracted before deletion).
  - Test Cases to Write:
    - `poisoned_state_mutex_recovers_service`: inject a panicking task holding a recovered-class lock; subsequent request succeeds.
    - `irrecoverable_guard_still_fails loudly`: kept-`expect` sites documented (no test — expect is the behavior).
  - Evidence (2026-09-22):
    - Baseline re-measured (review's "~90" stale): **357
      `expect("…poisoned")` sites**. Triage: **330 recoverable production
      sites → `lock_or_recover()`**; **1 irrecoverable production lock kept**;
      **25 test-only kept** (20 in test files, 5 `#[cfg(test)]` hooks/harness);
      1 condvar waiting on a recovered mailbox recovered inline. Full table
      `test-plan/artifacts/134-robustness/task3-poison-policy.md`.
    - Helper: `src/lock_util.rs`, one `LockOrRecover::lock_or_recover` trait for
      `std::sync::Mutex` (healthy path is exactly `Mutex::lock`; only the poison
      arm differs). Module docs state the rule (self-healing state recovers;
      irreversible hand-off keeps `expect`; test-only locks keep `expect`). No
      per-site poison prose duplicated; no behavior change on the healthy path,
      and no allocation added.
    - Migrated files (330 calls): `server/ops/mod.rs` 133,
      `document_analysis.rs` 31, `parse_coordinator.rs` 26,
      `language_intelligence.rs` 21, `completion.rs` 21,
      `js_runtime/mod.rs` 15, `client/mod.rs` 15, `syntax/mod.rs` 12,
      `ops/packages.rs` 10, `runtime_reload.rs` 6, `configuration.rs` 6,
      `js_runtime/worker.rs` 5, `connection/mod.rs` 5, `perf/metrics.rs` 5,
      `ops/theme.rs` 4, `ops/language_server.rs` 4, `workspace/mod.rs` 3,
      `server/mod.rs` 3, `js_runtime/source.rs` 3, `ops/document_analysis.rs` 1,
      `fanout.rs` 1.
    - **Irrecoverable kept**: embedded tree-sitter `Parser`
      (`src/server/syntax/mod.rs`), with a comment naming the corruption (FFI
      state interrupted mid-parse must not be reused). Test-only kept:
      `workspace/mod.rs` `#[cfg(test)]` hooks, `parse_coordinator.rs` test gate,
      and the `js_runtime/tests` + `syntax/tests.rs` suites — tests fail loud.
    - Condvar arm: `js_runtime/worker.rs` mailbox `ready.wait(state)` recovers
      inline via `PoisonError::into_inner`; the loop re-checks queue/closed, so
      no torn hand-off is served.
    - Security (per-site review): recovery never bypasses authority.
      `PackageLoadEntryAllowlist` only inserts validated paths and a miss
      denies (policy comment added); `ClayOpState::set_current_package` is
      attribution re-resolved through the host-enabled set (policy comment
      added); `PackageService` enable/authorize is unchanged and checked before
      mutation; client capability slots hold server-issued tokens re-validated
      server-side. Mailbox byte accounting may lose/double-count one in-flight
      event on a torn write (entry-bounded queues, re-clamped on next enqueue)
      — recorded as a ceiling, not an authority bypass.
    - Tests: `poisoned_state_mutex_recovers_service` in `src/lock_util.rs`
      (helper), `src/server/fanout.rs` (poison a live state lane, then
      publish/read + broadcast), and `src/server/tests.rs` (real `IpcServer`:
      poison `live_clients`, then the connection-arrival `sweep_expired_tabs`
      service path runs; set stays poisoned, service still works). Kept-`expect`
      class documented; no test (expect is the behavior).
    - Gates: `scripts/check.sh full` **exit 0**, ≈404 s — audit/fmt/check/clippy
      (`-D warnings`, zero)/test/**2004 passed, 0 failed, 1 ignored** (root 1948
      = task-2's 1945 + the 3 poison tests; 55 desktop; 1 bindings)/bench-compile/
      desktop-clippy/desktop-test/bindings. Log `…/task3-check-full.log`, exit
      codes `…/task3-exit-codes.txt`. One pre-existing flake hit on the first
      attempt (`npm_backend_list_parses_npm_json_shape`, ETXTBSY spawn of a fake
      npm script; passes isolated and on re-run; file untouched) — kept log
      `…/task3-check-full-flake-npm-list.log`, flake triage is plan 148.
    - Wiki record of the policy is the plan's dedicated final task (task 8);
      module docs and inline authority comments landed here.

- [x] Move hot-path blocking fs off the async runtime (P3)
  - Acceptance Criteria:
    - Functional: the open-document path's `canonicalize`/`metadata` (`src/server/workspace/mod.rs`) and the touched read/write sites (`configuration.rs`, `agent_documents.rs`, `launcher.rs`, `ops/theme.rs`, `ops/packages.rs`) use `tokio::fs`/`spawn_blocking` with identical semantics (same errors mapped through existing `WorkspaceError` variants); cold-path `std::fs` sites left as-is carry a ceiling comment.
    - Performance: open-document latency on a slow/cold filesystem no longer blocks other connections' I/O (verifiable with a one-shot strace/tokio-console probe or reasoned evidence: no `std::fs` on the awaited path).
    - Code Quality: error mapping unchanged (`RootUnavailable`/`FileUnavailable` variants preserved); clippy/fmt green.
    - Security: permission checks (`PermissionsExt`, canonical-path validation) unchanged and still precede any read.
  - Approach:
    - Documentation Reviewed:
      - `src/server/workspace/mod.rs:523–530, 621–626, 664–669, 763` (hot canonicalize/metadata); `tokio::fs` API for the vendored tokio (`Cargo.toml`: tokio `fs` feature already enabled); atomic-write precedent `src/server/agent_documents.rs` (plan 119 P2-2).
    - Options Considered:
      - Migrate all 324 sites: large diff, cold paths don't block the reactor meaningfully — ceiling comment instead.
      - Hot paths only. (Chosen.)
    - Chosen Approach:
      - `tokio::fs::canonicalize`/`metadata` equivalents where they exist; `spawn_blocking` for the rest; keep std fs in `Drop`/sync contexts (documented).
    - Files to Create/Edit:
      - `src/server/workspace/mod.rs`, `src/server/configuration.rs`, `src/server/agent_documents.rs`, `src/server/launcher.rs`, `src/server/ops/theme.rs`, `src/server/ops/packages.rs`.
    - References:
      - Review P3.
  - Test Cases to Write:
    - Existing workspace/open suites green (error-mapping net); add one test asserting `RootUnavailable` still surfaces for an unavailable root through the async path.
  - Evidence (2026-09-22):
    - Baseline inventory re-verified in task 1; per-site change list and
      remaining cold-path ceiling:
      `test-plan/artifacts/134-robustness/task4-async-fs.md`.
    - Open-document path now `tokio::fs`: `canonical_file_state`,
      `reauthorize_open_file`, `canonical_selected_file`,
      `contained_existing_path`, `contained_new_file_path`, `matches_current`
      are async; callers `prepare_open_existing`/`prepare_open_selected`/
      `register_loaded_file`/`prepare_save` (async now)/`prepare_reload`/
      `atomic_write_chunks` and the agent document ops await them.
      `discover_root_for_path` and `add_explicit_user_grant` are async on
      `tokio::fs`; `add_root` was split into a sync startup wrapper plus async
      `add_root_async` + pure `add_root_state`. `atomic_write_chunks`'s
      read-only mode check, original-permissions capture, and
      `set_permissions` are `tokio::fs` too.
    - Remaining reactor blockers moved to `spawn_blocking`:
      `persist_settings_change` (whole root-resolution + preferences
      read/write, including `effective_configuration_root`),
      `runtime_reload::persisted_appearance` (called from the now-async
      `enumerate_ui_choices`; the package-service guard is dropped before the
      await), and `command_execution`'s `OPEN_DIRECTORY` walk
      (`prepare_directory_listing` under the guard → `traverse_directory` on
      the blocking pool; previously a recursive `std::fs` walk on the reactor).
    - `src/server/launcher.rs` store/callers converted to async `tokio::fs`
      (`read_store`, `write_store`, `record_recent_workspace`,
      `remove_recent_workspace`, `launcher_entries`, `list_agents`,
      `count_skills`); callers in `connection/tabs.rs`,
      `connection/workspace.rs`, `server/mod.rs` await; tests are
      `#[tokio::test]`.
    - Stale P3 list entries confirmed and skipped: `agent_documents.rs` and
      `ops/theme.rs` already use `tokio::fs` on all production paths.
    - Cold-path ceilings recorded as comments: `configuration.rs` (JS worker
      thread / startup; reactor callers spawn_blocking),
      `ops/packages.rs` (JS worker thread), `workspace/mod.rs` (startup root
      registration, blocking-pool traversal helpers, dead
      `FileMetadata::capture`, `cfg(test)` hooks), `launcher.rs`
      (`resolve_agent_type` single stat), `server/mod.rs` startup watcher.
    - Error mapping unchanged: every site keeps its existing
      `map_err` into `WorkspaceError::{RootUnavailable, FileUnavailable}`,
      `UserBrowseError::Unavailable`, `ConfigurationError::{Root, ReadModule}`,
      and the same `settings.*` messages. Security unchanged: canonical
      containment and `validate_regular_file_metadata` still precede reads;
      the atomic-write read-only/permission checks are preserved
      (now async); no workspace lock or `std::sync` guard is held across an
      await in the new code.
    - Tests: new
      `open_existing_file_reports_file_unavailable_when_root_disappears`
      (vanished root → `FileUnavailable` through the async open path);
      `explicit_user_grant_rejects_missing_path` now exercises the async root
      path and still asserts `RootUnavailable`. Existing workspace/launcher/
      connection/configuration/ops suites green.
    - Gates: `scripts/check.sh full` **exit 0** ≈1218 s — audit/fmt/check/clippy
      (`-D warnings`, zero)/test/**2005 passed, 0 failed, 1 ignored** (root 1949
      = task-3's 1948 + the new workspace test; 55 desktop; 1 bindings)/
      bench-compile/desktop-clippy/desktop-test/bindings. Logs
      `…/task4-check-full.log` + two kept flake logs
      (`…-flake-security.log`, `…-flake-lsp.log`), exit codes
      `…/task4-exit-codes.txt`. Two test-side races surfaced under suite load
      and were fixed with bounded waits (daemon log flush in
      `agent_session_isolation`, partial-frame read in
      `language_server_authority`); deeper flake determinism stays with
      plan 148. One guard update: `rust_visibility_api_mapping` now expects
      `pub async fn` launcher entries.
    - Performance: no `std::fs` remains on the awaited open/save/reload path
      (grep-verified; type-only `fs::Metadata` references and the dead
      `capture` helper remain); open-document latency on a slow filesystem no
      longer blocks the reactor's worker threads.
    - Wiki rule (hot paths async, cold paths ceiling) is the plan's final
      task 8; module docs landed here.

- [x] Snapshot swap only on change (R3)
  - Acceptance Criteria:
    - Functional: `completion_providers` snapshot replaces the stored Vec only when the evaluation's provider list differs (or by generation id); downstream readers see identical values.
    - Performance: repeated no-change evaluations (e.g. theme-only reloads) no longer clone the provider Vec — asserted by an allocation-count or Arc-swap test.
    - Code Quality: comparison chosen is cheap (length + prefix/id equality or generation stamp) — documented.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/mod.rs:698–708` (the clone site), `CompletionProviderMeta` shape (id/priority/provenance fields for the equality key).
    - Options Considered:
      - Always clone (status quo): per-evaluation allocation for identical data.
      - Equality-gated swap. (Chosen.)
    - Chosen Approach:
      - `if &*stored != &evaluation.completion_providers { *stored = …clone() }` or generation-keyed overwrite.
    - Files to Create/Edit:
      - `src/server/js_runtime/mod.rs`.
    - References:
      - Review R3.
  - Test Cases to Write:
    - `provider_snapshot_not_replaced_when_unchanged`: hook or `Arc`-identity assertion across two identical evaluations.
  - Evidence (2026-09-22):
    - The clone site was `src/server/js_runtime/mod.rs` in
      `evaluate_entry_for_domain`'s success branch:
      `*self.completion_providers.lock_or_recover() =
      evaluation.completion_providers.clone();` — executed on every successful
      evaluation, including theme-only reloads.
    - Change: extracted
      `replace_snapshot_only_on_change(&Mutex<Vec<T>>, &[T]) -> bool`
      (`src/server/js_runtime/mod.rs`); it gates the clone on structural
      `PartialEq` of the slices (`CompletionProviderMeta` derives
      `PartialEq, Eq`) and only then does `providers.to_vec()` + assignment.
      Structural equality was chosen over a generation stamp because a stamp
      would miss same-generation list edits; the comparison is cheap and
      allocation-free (bounded list of scalars/short strings). Returns whether
      it swapped, giving the test a deterministic signal without an allocation
      counter or global hook. Call site unchanged otherwise (evaluation
      counters still bump).
    - Test: `provider_snapshot_not_replaced_when_unchanged`
      (`src/server/js_runtime/tests/facades_and_modes.rs`) — stores
      `[core.a, core.b]`, re-stores the identical list and asserts no
      replacement + identical values, then stores `[core.a, core.c]` and
      asserts replacement + new values. The end-to-end fixture test
      `config_fixture_workflows::language_packages_config_fixture_loads_and_registers_all_contributions`
      still proves downstream readers see the same values as the evaluation.
      Detail: `test-plan/artifacts/134-robustness/task5-snapshot-swap.md`.
    - Performance: no-change evaluations no longer allocate/clone the provider
      Vec; changed evaluations behave exactly as before.
    - Gates: `scripts/check.sh full` **exit 0**, ≈277 s — audit/fmt/check/clippy
      (`-D warnings`, zero)/test/**2006 passed, 0 failed, 1 ignored** (root 1950
      = task-4's 1949 + the new test; 55 desktop; 1 bindings)/bench-compile/
      desktop-clippy/desktop-test/bindings. Log `…/task5-check-full.log`, exit
      codes `…/task5-exit-codes.txt`; pedantic `float_cmp` regression check
      still 0 (`…/task5-clippy-pedantic-float-cmp.json`).
    - Security: none (inert provider metadata only; no authority surface).

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: theme/appearance and files/workspace modules re-run on a real Linux build (float-compare and fs changes touch theme resolution and open paths); record pass/fail; no new steps expected — record the reason explicitly if none are added.
    - Performance: theme-apply and open-folder steps re-run for perceived regression check.
    - Code Quality: `test-plan/index.md` updated only if steps change.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`.
    - Chosen Approach:
      - Regression pass on the two modules.
    - Files to Create/Edit:
      - None expected.
    - References:
      - None beyond index.
  - Test Cases to Write:
    - None.
  - Evidence (2026-09-22):
    - Regression-only pass on a fresh `cargo build --bin clay` +
      `cargo build -p clay-desktop --bin clay-desktop`; modules **15**
      (theme/appearance) and **03** (files/workspace) re-run. No step added,
      deleted, or weakened; `test-plan/index.md` gained the dated execution
      record only. Full detail: `test-plan/artifacts/134-robustness/manual-pass.md`.
    - Module 15 PASS live (`scripts/capture-ui-review.sh`, isolated mode-700
      root + private socket, portal screenshot + AT-SPI, viewport 1906×1099):
      `ui-review-design-system` and `ui-review-design-system-light` PASS
      (`Design system review` landmark, `Primary action` button, `Document
      editor`; `configuration_failed_lines=0`, distinct screenshots);
      `ui-review-default --theme @clay/theme-gruvbox-material-light
      --appearance light` PASS with `metadata.txt` recording the seeded
      theme/appearance — the live path through the reactor-side persisted
      appearance read made async in task 4.
    - Module 03 PASS live where drivable: `ui-review-workspace` PASS
      (`review.md` in the file browser, `Editor review.md` landmark, Document
      editor, 0 configuration failures); isolated live session (plan-132
      harness `run-live.sh start editing`) opened the workspace root from
      `layout.json`, wrote `launcher.json` with that root (async
      `record_recent_workspace`), accepted `wtype` typing (editor chars 64→85)
      and `Ctrl+S` wrote the file to disk (content verified, server log 0
      errors/diagnostics).
    - F7-class reload recorded **UNRESOLVED, not a false pass**: this build
      exposes `documents.reload` only as the Clay JS API, and the Control
      Center enumerates only `Reload Configuration and Packages`, so no live
      keyboard reload surface exists; reload semantics remain covered by the
      automated reload suites in the task-5 gate. Deeper live reload needs a
      command surface, not a plan-134 change.
    - Harness note recorded: the first capture on the focused compositor tag
      tiled the window to a 949×521 viewport (below the 900×600 floor) and
      correctly recorded `UNRESOLVED`; all recorded captures ran on an empty
      tag and measured 1906×1099.
    - Artifacts: `test-plan/artifacts/134-robustness/live/{design-system-dark,
      design-system-light,theme-seeded-light,workspace-open}/` — each with
      `screenshot.png`, `accessibility.txt`, `a11y-tree.txt`, `metadata.txt`,
      `server.diagnostics.txt`, `review.status=PASS` — plus
      `…/manual-pass.md` and the `test-plan/index.md` execution record.
    - Performance: theme-apply and folder-open steps perceived unchanged
      (single visible swap in both design-system themes; workspace listing and
      document open with no stall); no regression observed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed; verify via diff; record "no JS API change".
    - Performance: none.
    - Code Quality: registry/doc-guard suites pass.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach:
      - Verify-only.
    - Files to Create/Edit:
      - None expected.
    - References:
      - Decision log 2026-05-08-1509.
  - Test Cases to Write:
    - None.
  - Evidence (2026-09-22): **no JS API change** — verify-only, no source
    edited. Raw record `test-plan/artifacts/134-robustness/task7-js-api-verification.txt`.
    - Artifacts byte-identical vs baseline `40022f6`: `runtime/js/`,
      `docs/reference/clay-js-api/` (145 pages), `docs/generated/`, `docs/index.md`,
      `frontend/src/bridge/`, `src-tauri/bindings/` — `git diff --stat` on those
      paths is empty; `runtime/js` alone has 0 diff lines (33 facade modules
      unchanged).
    - Op set unchanged: 103 `op_clay_*` identifiers before and after, sorted-set
      `diff` empty; no `#[op]`/`op_clay_*` line added or removed anywhere in the
      diff (the only matches are prose in
      `docs/wiki/modules/editor-theme-registry.md`, an unrelated plan-133 path
      retarget).
    - The `src/server/ops/` diff is authority-internal only:
      `ops/workspace.rs` gained two `.await`s because
      `add_explicit_user_grant`/`discover_root_for_path` became async (task 4),
      same op names/args/JSON/errors; `ops/mod.rs` and siblings are task 3's
      `.lock_or_recover()` poison-policy swaps plus plan-133 reorganization.
    - Registry freshness: `cargo run --bin update-doc-registry` rewrote
      `docs/generated/clay-js-api-registry.json` and left it byte-identical
      (`git status --porcelain` on the registry/doc/facade paths empty).
    - Guards: `cargo test --test protocol clay_js_api` 15 (inventory:
      `public_inventory_docs_index_and_generated_matrix_match_exactly`,
      `every_public_api_contract_matches_generated_markdown_metadata`,
      `documentation_validation_is_read_only`, naming contract) and
      `… clay_js_doc_registry` 55, part of `cargo test --test protocol` **227
      passed / 0 failed**; the task-5 gate (`scripts/check.sh full`, exit 0,
      2006 passed / 0 failed / 1 ignored) covers the same guards plus
      presentation 62, security 152, runtime 75 and the desktop/bindings stages.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki records the poison policy (which class recovers, which fails loudly), the async-fs rule (hot paths async, cold paths ceiling), and the float-comparison policy (identity vs tolerance conventions).
    - Performance: notes R2/R4 ceilings with revisit triggers.
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: documents that poison recovery never bypasses authority checks.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach:
      - Update once after gates pass.
    - Files to Create/Edit:
      - Affected wiki pages, `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.
  - Evidence (2026-09-22) — wiki updated once after the gate passed; all four
    pages are already linked from `docs/wiki/index.md`, whose one-liners now
    name the new sections:
    - **Poison policy** — new `## Lock Poison Policy (plan 134 D5)` section in
      `docs/wiki/modules/persistent-runtime-hardening.md`:
      the two classes with populations (330 self-healing production sites via
      `src/lock_util.rs::LockOrRecover::lock_or_recover`; 1 irrecoverable
      production lock — the embedded tree-sitter `Parser`; 25 test-only), the
      inline condvar recovery, the mailbox byte-accounting ceiling, the
      security statement (recovery never bypasses authority: allowlist miss
      denies, `set_current_package` re-resolved through the host-enabled set,
      `PackageService` checks before mutation, server-issued capability tokens
      re-validated), and the three tests.
    - **Async-fs rule** — new `### Async Filesystem Rule (plan 134 P3)`
      subsection in
      `docs/wiki/modules/server-file-workspace.md`:
      the `tokio::fs` async functions and their awaited callers, the
      `spawn_blocking` moves (`persist_settings_change`,
      `persisted_appearance`, `OPEN_DIRECTORY` traversal), the cold-path
      `std::fs` ceiling list, and the statement that error mapping, canonical
      containment, and the atomic-write checks are unchanged.
    - **Float-comparison policy** — new `## Plan 134 hygiene policies` section in
      `docs/wiki/modules/maintenance-validation.md`:
      identity (pre-boundary source values or `to_bits()`, example
      `src/shell/icons.rs::is_arc_flag`) vs tolerance (`f32::EPSILON`/
      `f64::EPSILON`, one per type domain, house precedent
      `src/shell/layout/mod.rs`) vs floors (contrast `>=` comparisons),
      81 sites → 0 with no `#[allow(clippy::float_cmp)]`. The same section
      summarizes the poison and async-fs policies and points at the two
      detailed pages.
    - **R2/R4 ceilings with revisit triggers** — R2 (snapshot clones deferred)
      in the Server State Fanout Lanes invariants (revisit when the 64 caps
      rise, on the p95 triggers, or when a profile names the clone paths) and
      R4 (resident floor bounded by the compiled budgets) in the Server File
      Workspace invariants (revisit on profile evidence above the 256 MiB
      envelope with documents closed, or when a retained cache lands without a
      byte budget).
    - Guards: `cargo test --test protocol primitives_docs` **35 passed / 0
      failed** (`wiki_index_links_every_wiki_page` and the plan-088/125/129
      content pins), `cargo test --test protocol` **227 passed**, `cargo test
      --test presentation` **62 passed**. No source file changed. Raw record:
      `test-plan/artifacts/134-robustness/task8-wiki-verification.txt`.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Compromises Made

- **R2/R4 recorded, not implemented (task 1):** snapshot cloning stays
  (`StateFanout::publish`/`current`, `latest_for`) and the resident floor stays
  budget-bounded without a new ceiling test. Both are recorded as ceilings with
  revisit triggers on
  `docs/wiki/modules/server-state-fanout.md` and
  `docs/wiki/modules/server-file-workspace.md`;
  no profile evidence of need exists at the 64-cap desktop scale.
- **D4 uses std per-type epsilons, not named domain constants (task 2):** the
  plan suggested e.g. `CONTRAST_EPSILON`; triage showed no flagged site is a
  contrast-/layout-ratio tolerance comparison (contrast already compares `>=`
  floors), so `f32::EPSILON`/`f64::EPSILON` are the single definition per type
  domain — the house precedent in `src/shell/layout/mod.rs` and
  `src/editor/theme.rs`. Tolerance is effectively exact (machine epsilon), and
  the arc-flag identity sites use `to_bits()` rather than any epsilon.
- **D5 leaves 26 sites on loud `expect` (task 3):** one irrecoverable
  production lock (embedded tree-sitter `Parser` — FFI state interrupted
  mid-parse must not be reused) plus 25 test-only locks where failing loudly is
  the point. Documented in the policy table.
- **D5 mailbox byte accounting can be off by one in-flight event (task 3):** a
  torn write on the `js_runtime/worker.rs` mailbox can lose or double-count one
  event; queues are entry-bounded and re-clamped on the next enqueue. Recorded
  as a ceiling, not an authority bypass (no permission path consults the
  counter).
- **P3 keeps cold-path `std::fs` (task 4):** startup root registration, the
  blocking-pool traversal helpers, the dead `FileMetadata::capture`,
  `#[cfg(test)]` hooks, JS-worker-thread sites in `configuration.rs` and
  `ops/packages.rs`, `launcher.rs`'s single stat, and the startup watcher keep
  `std::fs` with ceiling comments; the dead helper was left in place rather
  than deleted (deletion belongs to a hygiene pass, not this plan).
- **Two pre-existing test flakes were worked around, not root-caused (tasks 3
  and 4):** a fake-npm `ETXTBSY` spawn and two suite-load races (daemon log
  flush, partial-frame read) were fixed with bounded waits or re-run; deeper
  flake determinism stays with plan 148.
- **Manual F7-class document reload has no live surface (task 6):** this build
  exposes `documents.reload` only as the Clay JS API, so the external-edit
  reload step is recorded UNRESOLVED and covered by automated suites instead of
  a false live pass.

## Further Actions

- **Git stale-polling surface has no production caller** (delegated from plan 131
  task 5; priority: low): `GIT_STATUS_POLL_INTERVAL`,
  `GitStatusCache::refresh_stale_workspace`, and
  `GitDiscoveryService::discover_workspace_statuses` are `#[cfg(test)]` since plan
  131, while the live paths are `list_cached` + explicit `refresh_root`
  (`git.serverListGitStatuses`, `git.serverRefreshGitStatus`). Resolve it in this
  plan's hygiene pass: delete the polling surface with its tests (the wiki already
  records reality), or schedule a real poll interval — do not keep a test-only
  poll that the documentation had described as live.
