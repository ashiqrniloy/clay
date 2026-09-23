# Plan 134 — baseline gates, clippy inventories, and R2/R4 records (2026-09-22)

Task: "Baseline gates, clippy inventories, and no-action decision records (R2, R4)"
(plan 134 task 1). **No source edits** were made before the gate run.

- Tree: branch `review/1909`, HEAD `40022f6` ("WIP 133"). Dirty tree: 52 pre-existing
  `git status --porcelain` entries — plan 133's *completed but uncommitted*
  implementation (`src/shell/theme/` split, `src/shell/theme.rs` re-export, wiki
  pages, plan 133 file) — plus this task's untracked artifacts. No plan-134 target
  was modified by plan 133 (verified by path).
- Environment: rustc/cargo **1.98.1**, clippy **0.1.98**, cargo-audit 0.22.2,
  node v26.9.0, npm 12.0.2. `javascriptcoregtk-4.1`, `webkit2gtk-4.1`, `gtk+-3.0`
  all present, so the desktop stages plan 133 task 7 could not run **do** run here;
  `frontend/dist` present. No `rust-toolchain.toml` (toolchain pinning is plan 148).

## Gate result

`scripts/check.sh full` → **exit 0**, ≈253 s (20:08:07 → 20:12:20 +0600),
`full check PASSED` as the last line. Log: `baseline-check-full.log`; per-stage
detail: `baseline-exit-codes.txt`.

| Stage | Result |
| --- | --- |
| audit | exit 0 — 9 allowed RUSTSEC advisory warnings (pre-existing) |
| fmt | exit 0 |
| check | exit 0 (`cargo check --all-targets`) |
| clippy | exit 0 — `cargo clippy --all-targets -- -D warnings`, zero warnings |
| test | exit 0 — **2000 passed, 0 failed, 1 ignored** (root 1944 = lib 1419 + 9 + 62 + 227 + 75 + 152; desktop 55 = 32 + 2 + 1 + 4 + 16; bindings 1 / 32 filtered) |
| bench-compile | exit 0 (1m 22s) |
| desktop-clippy | exit 0 — environment blocker from plan 133 task 7 is gone here |
| desktop-test | exit 0 |
| bindings | exit 0 — `webview bindings up to date` |

## Finding 1 — the source review does not exist (repeat of plan 133's finding)

All of plans 126–134 cite `code-reviews/2026-09-18-comprehensive-implementation-review.md`;
that file is absent from the working tree and from git history (`git log --all` has
only the 2026-06-21 / 07-19 / 08-14 reviews). Plan 134's quoted inventories (82
float_cmp sites with per-file production split, "324" blocking-fs sites) cannot be
reconciled against their source. The measured inventories below **supersede** the
plan-quoted ones; a later task should treat this baseline as the work order.

## Float comparison inventory (D4 re-verified)

Command: `cargo clippy --all-targets --message-format=json -- -W clippy::pedantic`
(clippy 0.1.98), exit 0, 28.6 s. Raw JSON: `baseline-clippy-pedantic.json`;
deduped lists + context: `baseline-clippy-float-cmp.txt`,
`baseline-clippy-float-cmp-rendered.txt`.

**81 unique sites** — 84 raw diagnostics collapse to 81 after dedupe by
(file, line, column) across the lib/test-harness compilations.

| Class | Sites | Files |
| --- | --- | --- |
| A. runtime-production (outside any test module) | **3** | `src/shell/icons.rs` 369, 375, 564 |
| B. test modules inside production files (`#[cfg(test)]`) | 19 | `package_ui.rs` 7, `design_system.rs` 4, `icons.rs` 2 (both `:997`), `editor/theme.rs` 2, `ops/theme.rs` 2, `ops/modes.rs` 2 |
| C. test files (`tests/` paths, `*_tests.rs`) | 59 | `src/shell/theme/tests.rs` 15, `tests/package_ui_conformance.rs` 12, `src/shell/layout/tests.rs` 10, `theme_packages.rs` 4, `runtime_generation_tests.rs` 4, … |

Reconciliation with the plan:

- The plan's production split (theme.rs 16 + package_ui.rs 7 + icons.rs 6 +
  design_system.rs 4 + editor/theme.rs 2 + ops/theme.rs 2 + ops/modes.rs 2 +
  server/mod.rs 4 = 43) does not match this tree:
  - the old `src/shell/theme.rs` 16 are **test-side**: plan 133's split moved them
    to `src/shell/theme/tests.rs` 15 + `theme_snapshot_tests.rs` 1, all under
    `#[cfg(test)]` (they were test assertions in the pre-split file too);
  - the old `src/server/mod.rs` 4 are **test-side**: plan 133's split moved them to
    `src/server/runtime_generation_tests.rs:617,618,619,838`, declared
    `#[cfg(test)] mod` from `src/server/mod.rs:1018`;
  - `icons.rs` is 5 total, not 6 — 3 runtime + 2 in its test module.
- The plan's 82 vs measured 81 unique: one site difference; the plan count is not
  reproducible without its source.
- No `#[allow(clippy::float_cmp)]` exists anywhere in the tree. The default
  `check.sh` clippy stage is **not** pedantic, so none of these sites currently
  fails a gate — the inventory comes only from the explicit pedantic run.
- The 3 runtime sites are alpha-extreme identity checks in icon path emission
  (`p[3] != 0.0 && p[3] != 1.0`, `value == 0.0 || value == 1.0`); plan task 2's
  classification (identity vs tolerance) applies to them. All other sites are
  `assert_eq!`-style test assertions (plan 133's `float_cmp`-as-test-signal pattern).

## Blocking `std::fs` inventory (P3 re-verified)

Method + per-file detail: `baseline-fs-inventory.txt`; counts: `baseline-fs-counts.txt`.

| Measure | Count |
| --- | --- |
| `std::fs::` call lines, all of `src/` | 188 |
| `std::fs::` + bare `fs::` call lines, all of `src/` (84 files) | 1278 |
| same, excluding test-path files | 506 across 35 files |

The review's "324" is unreproducible (Finding 1); methodology unknown. Priority
findings that change the later P3 task:

- **Hot open-document path confirmed**: `canonical_file_state` (1957, 1966, 1994,
  2003) is called directly by async `prepare_open_existing`/`register_loaded_file`;
  `reauthorize_open_file` (2035, 2058) by async `authorize_document_access`/
  `authorize_save`; `canonical_selected_file` (2115, 2120) by async
  `prepare_open_selected` — all `fs::canonicalize` + `fs::metadata`, no offload.
- **Two plan P3 files are already done**: `src/server/agent_documents.rs` and
  `src/server/ops/theme.rs` use `tokio::fs` on all production paths (their only
  `std::fs` lines are `#[cfg(test)]` fixtures). Only the `configuration.rs:517`
  `write_preferences` site the theme op triggers remains.
- **ASYNC-REACHABLE sites per named file**: `configuration.rs` 107, 254, 367,
  517/518, 784, 828; `launcher.rs` 59, 86–93, 164, 221; `ops/packages.rs` 155,
  255/262, 430, 676/682; `workspace/mod.rs` 621/626, 664/669, 1349, 2344.
- **Adjacent gap** (not in the plan's list): async `execute_workspace`
  (`command_execution.rs:287`) calls sync `WorkspaceState::list_directory` →
  `traverse_directory`, a recursive blocking walk on the async runtime; the
  `run_directory_listing` op (`ops/workspace.rs:143`) already uses `spawn_blocking`.
- `FileMetadata::capture` (`workspace/mod.rs:2328`) has zero production callers.

## R2/R4 no-action decision records

Source review §3 is absent (Finding 1); these records use the plan's own one-line
statements and the current code/budgets as the evidence base.

**R2 — broadcast-lane snapshot cloning at desktop scale: no action.**
`StateFanout::publish` clones the value into the current slot
(`src/server/fanout.rs:83–86`) and `StateFanout::current` clones on every read
(`:89–91`); `ActiveRuntimeStateFanout::latest_for` clones the latest
`RuntimeStateSnapshot` before per-client narrowing
(`src/server/runtime_state.rs:88–97`). Publishing already carries an id-only
`Fanout<RuntimeGenerationId>`. At the measured desktop scale — ≤64 active
connections and ≤64 open documents/client (`src/perf/budgets.rs:65,69`), snapshot
document cap 64 (`:249`) — there is no profile evidence that these clones are a
cost. Arc-wrapping the snapshot now would add indirection to every reader for an
unmeasured gain (YAGNI).
Revisit triggers: (a) tab/active-connection/open-document counts above the existing
64 caps, i.e. the scale assumption changes; (b) the existing runtime-diff review
triggers fire — encoded snapshot payload p95 > `RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES`
(768 KiB, `:254`) or install p95 > `RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS`
(16 ms, `:255`); (c) a profile names `StateFanout::current`/`latest_for` clones on a
hot publish/subscribe path. Upgrade path then: `Arc<RuntimeStateSnapshot>` lanes or
generation-keyed diffs.

**R4 — resident-set floor bounded by budgets: no action.**
The process RSS floor is the desktop shell + runtime baseline and is already bounded
by server-owned budgets, not unbounded retention: open-document resident budget
`DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES` = 256 MiB (security budget, open/reload fail
closed; `src/perf/budgets.rs:399–403`), syntax cache `SYNTAX_CACHE_BUDGET_BYTES` =
30 MiB (`:154`), latency-lane JS heap `JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES` =
32 MiB (`:389`); `docs/development/performance.md` carries the same advisory
≤256 MiB envelope for the 16 MiB fixture workflow. No code change is warranted
without evidence of a floor breach.
Revisit triggers: profile evidence (smoke-gui/task-manager `RSS`, perf report) shows
closed-document resident memory above the 256 MiB budget envelope, or a new
long-lived retained cache is added without a byte budget constant.

## Findings that change later tasks

1. **D4 task 2 is much smaller than planned**: 3 runtime sites (`src/shell/icons.rs`)
   plus 19 in-file test-module assertions. The plan's file list is stale — theme.rs
   (plan 133 split) and server/mod.rs (plan 133 split) sites are test-only, and
   `icons.rs` counts differ.
2. **P3 task list trims**: drop `agent_documents.rs` and `ops/theme.rs` (already
   `tokio::fs`); keep `configuration.rs`, `launcher.rs`, `ops/packages.rs`,
   `workspace/mod.rs`; consider the `command_execution.rs:287` listing leg.
3. **Environment**: this host now has the WebKitGTK/JSC dev libs; the desktop gate
   stages pass, so later tasks can use the full `scripts/check.sh full`.
4. **No toolchain pin**: gates are 1.98.1-sensitive (plan 148 owns the pin).
5. Baseline was measured on plan 133's uncommitted work; if that work changes or is
   reverted before plan 134's edits, re-run the gate before attributing failures.
