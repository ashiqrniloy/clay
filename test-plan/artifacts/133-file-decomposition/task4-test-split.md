# Plan 133 task 4 evidence — giant inline test modules split into suite trees

Task: "Extract the giant inline test files (`js_runtime/tests.rs`, `connection/tests.rs`,
`client/tests.rs`, `workspace/tests.rs`)" — move-only, zero lost tests.

## Result

Each `src/**/tests.rs` became a thin `src/**/tests/mod.rs` (module doc + all imports +
shared helpers + the suite map) plus one file per contiguous thematic run of tests.

| Module | Before | After (root) | Suites | Suite lines | Tests |
| --- | --- | --- | --- | --- | --- |
| `src/server/js_runtime/tests` | 12,663 lines | 839 | 24 | 11,880 | 241 |
| `src/server/connection/tests` | 8,946 lines | 1,061 | 14 | 7,923 | 87 |
| `src/client/tests` | 3,541 lines | 219 | 8 | 3,347 | 48 |
| `src/server/workspace/tests` | 2,605 lines | 51 | 9 | 2,577 | 80 |

Test counts are identical before/after (241 + 87 + 48 + 80 = 456); the four owner
`mod.rs` files keep their `#[cfg(test)] mod tests;` declarations unchanged (Rust
resolves `tests.rs` → `tests/mod.rs`). Largest suite is 911 lines
(`js_runtime/tests/lanes_and_queues.rs`); per-suite map with line counts:
`task4-suite-map.txt`.

## Method

`task4-split-test-files.py` (kept for provenance) drives the move:

- A per-file *ladder* names each suite by the first test fn of its contiguous run
  (`('runtime_third_party', 'js_runtime_evaluates_controlled_module')`, …); the
  script slices items between consecutive ladder starts, so the grouping is
  reviewable and stable, and every test lands in exactly one suite.
- All imports and shared helpers stay in `tests/mod.rs`; each suite starts with
  `use super::*;` (child modules see the root's private items and its `use`
  bindings), so no helper or test had to change reachability.
- Only two classes of textual edits inside moved items:
  - explicit `super::` chains gain one level (`super::X` → `super::super::X`,
    maximal chains extended once): 6 lines in js_runtime, 52 in connection,
    12 in client, 0 in workspace;
  - `include_str!`/`include_bytes!` paths gain one level (11 in js_runtime, e.g.
    `../../packages/modes.rs` → `../../../packages/modes.rs`; 1 in connection,
    `mod.rs` → `../mod.rs`).
  `cargo fmt` then reflowed previously wrapped lines (no token changes).
- `src/client/tests/mod.rs` declares the Windows end-to-end suite as
  `#[cfg(windows)] mod windows_named_pipe;` — the same gating its tests already
  carried, now at module level so non-Windows builds do not warn about the
  suite's unused `use super::*;`.

## Verification (`task4-verify.py`, exit 0)

For every one of the four modules: every original top-level item (imports,
helpers, tests) reappears **exactly once** across `tests/mod.rs` + suites as the
same code (whitespace-insensitive, trailing-comma moves from rustfmt re-wrapping
tolerated) after the documented bump; root files hold 0 tests; all 456 test
bodies are placed; all 66 root helper items are referenced from at least one
suite (no stranded helpers). Output is the per-suite counts in `task4-suite-map.txt`.

## Guard/doc updates forced by the move

- `tests/performance_budgets.rs::is_sibling_test_file`: now also treats any file
  under a `tests/` directory (and `<module>_tests.rs`) as test-only; without it
  the folding-range production check scanned the new suites and false-positived
  on their fixtures.
- `tests/package_ui_conformance.rs` `RETIREMENT_GUARDS`: the chat-retirement
  exemption path moved to `src/server/js_runtime/tests/coding_agent_and_launcher.rs`
  (the only new file naming `@clay/chat`); the guard asserts the path exists.
- `docs/wiki/flows/document-chunked-loading.md`: the guard-enforced page now
  points at `js_runtime/tests/document_and_git_facades.rs` and
  `connection/tests/language_intelligence.rs` for the two named tests.
- **Deferred to plan task 9 (wiki update)**: ~20 other wiki pages still name the
  old `src/**/tests.rs` paths (~80 references) plus test-name-based `cargo test`
  filters (`client::tests::real_server_tab`); a named test resolves to its suite
  by grepping `src/**/tests/<suite>.rs`, and `task4-suite-map.txt` lists them.

## Gates

- `scripts/check.sh full` **exit 0**, 244 s, `full check PASSED`; clippy
  `-D warnings` + `cargo fmt --check` clean inside the run; test totals
  **1996 passed / 0 failed / 1 ignored — identical to the task-1 baseline**
  (`task4-check-full.log`, `task4-exit-codes.txt`).
- Compile time A/B (`touch` tests root + `cargo check --all-targets`):
  6.50 s / 6.75 s split vs 26.89 s (fingerprint change) / 6.71 s pre-split —
  neutral.
- Flakes observed while gating (unrelated targets, all green standalone and in
  the final run; recorded, not caused by the move): one
  `third_party_poison_replays_approved_graph_and_restores_providers` `RecvError`
  under load (100 ms service timeout; passed 3/3 standalone and 5/5 full
  `--lib` runs), two `agent_protocol` `ETXTBSY` spawn flakes, one fake-LSP
  early-exit flake, and one desktop-harness hang in
  `manual_smoke_docs::plan118_ui_review_harness_…` after aborted runs (the
  known orphaned-capture-process issue; clearing `/tmp/clay-ui-review.*` and
  capture/clay-desktop processes before the run fixed it).