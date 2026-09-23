# Plan 133 — baseline gates and file-metric inventory (2026-09-21)

Task: "Baseline gates and file-metric inventory" (plan 133 task 1). No source
edits made before this run.

- Tree: HEAD `6ee3b4c` ("WIP"), `git status --porcelain` empty (clean — plan
  targets untouched).
- Environment: rustc/cargo **1.98.1**, clippy 0.1.98, cargo-audit 0.22.2, node
  v26.8.2, npm 11.19.1. No `rust-toolchain.toml` (CI floats
  `dtolnay/rust-toolchain@stable`), so gates are toolchain-sensitive. Plan 132's
  baseline recorded a 1.98.1 clippy failure (`result_large_err` in
  `src/server/connection/documents.rs`); at this HEAD the allow is present
  (`src/server/connection/documents.rs:264`) and clippy `-D warnings` is green,
  so that lane landed between `36eabd2` and `6ee3b4c`.
- Frontend prerequisite satisfied (`frontend/dist` present) for the desktop and
  bindings stages.

## Gate result

`scripts/check.sh full` → **exit 0**, 149 s, `full check PASSED`. Log:
`baseline-check-full.log`; exit codes: `baseline-exit-codes.txt`. Because the
wrapper runs under `set -eu` with a `FAILED at stage:` trap, reaching the
sentinel proves every stage exited 0:

| Stage | Command | Result |
| --- | --- | --- |
| audit | `cargo audit` | exit 0 — 9 allowed RUSTSEC advisory warnings (pre-existing) |
| fmt | `cargo fmt --check` | exit 0 |
| check | `cargo check --all-targets` | exit 0 |
| clippy | `cargo clippy --all-targets -- -D warnings` | exit 0 — **zero warnings** (1.98.1; see plan 132 note above) |
| test | `cargo test --all-targets --quiet` | exit 0 — 1940 passed, 1 ignored (lib 1415) |
| bench compile | `cargo bench --no-run` | exit 0 |
| desktop-clippy | `cargo clippy -p clay-desktop --all-targets -- -D warnings` | exit 0 |
| desktop-test | `cargo test -p clay-desktop --all-targets --quiet` | exit 0 — 55 passed |
| bindings | `scripts/check-bindings.sh` | exit 0 — 1 passed, 32 filtered; `webview bindings up to date` |

Baseline test volume: **1996 passed, 0 failed, 1 ignored** (1940 root + 55
desktop + 1 bindings guard). Zero default-level clippy warnings means every new
lint surfaced during this refactor is a regression, not pre-existing debt.

## File-metric inventory

Line counts (`wc -l`), plan-quoted values for comparison:

| Target | Measured | Plan §Objectives | Delta |
| --- | --- | --- | --- |
| `src/server/mod.rs` | 6,203 | 6,190 | +13 |
| `src/protocol/mod.rs` | 3,832 | 3,830 | +2 |
| `src/server/ui.rs` | 3,320 | 3,320 | 0 |
| `src/shell/theme.rs` | 3,050 | 3,050 | 0 |
| `src/server/js_runtime/tests.rs` | 12,663 | 11,045 | **+1,618** |
| `src/server/connection/tests.rs` | 8,944 | 8,558 | +386 |
| `src/client/tests.rs` | 3,541 | 3,693 | −152 |
| `src/server/workspace/tests.rs` | 2,605 | 2,564 | +41 |

Related targets named in later tasks: `src/server/tests.rs` 836,
`src/shell/package_ui.rs` 1,409, `src/shell/icons.rs` 1,192,
`src/shell/components.rs` 621, `src/behavior/manifest.rs` 443,
`src/protocol/agent.rs` 1,468.

Type counts — `src/protocol/mod.rs`: **71 top-level struct/enum declarations**
(33 `struct` + 38 `enum`), plus 11 `type` aliases = 82 type declarations. This
matches the plan's "71 types". Existing family modules the split can target:

| File | Lines | struct/enum |
| --- | --- | --- |
| `src/protocol/agent.rs` | 1,468 | 23 |
| `src/protocol/codec.rs` | 1,619 | 3 |
| `src/protocol/completion.rs` | 730 | 11 |
| `src/protocol/decorations.rs` | 1,051 | 12 |
| `src/protocol/diagnostics.rs` | 296 | 3 |
| `src/protocol/editor_control.rs` | 105 | 1 |
| `src/protocol/folding.rs` | 79 | 3 |
| `src/protocol/language_intelligence.rs` | 579 | 19 |
| `src/protocol/menu.rs` | 541 | 6 |
| `src/protocol/parse.rs` | 548 | 14 |
| `src/protocol/runtime.rs` | 607 | 14 |
| `src/protocol/sdui.rs` | 694 | 14 |
| `src/protocol/textobjects.rs` | 485 | 9 |

Module landmarks (measured, supersede plan estimates):

- `src/server/mod.rs`: module decls L1–43, `ServerConfig` L123,
  `RuntimeGenerationStore` L163, `ActiveTypographyState` L266, accept loops
  `accept_unix_loop` L996 (plus `spawn_connection` L1768), `mod tests;` **L6132**.
  Only ~70 lines of test wiring are inline; the test body already lives in
  `src/server/tests.rs` (836 lines, 12 test attributes). The plan's "tests from
  ~L3100" is stale.
- The other three giant test modules are also already `mod tests;` siblings
  (`js_runtime/mod.rs:1683`, `connection/mod.rs:1437`, `client/mod.rs:1756`,
  `workspace/mod.rs:3383`), so task 4/5 work is relocating sibling files, not
  cutting blocks out of `mod.rs`.
- Raw test-attribute counts in those unit modules (baseline for the
  "no test lost" check; `#[test]` + `#[tokio::test]`): js_runtime 16+225=241,
  connection 12+75=87, client 2+46=48, workspace 30+50=80 (= 456), plus
  `src/server/tests.rs` 3+9=12. Proxy only — macro-generated tests are not
  counted by attribute grep.
- `StateFanout<T>` exists at `src/server/fanout.rs:68` (`pub(crate)`), so plan
  132's lane work has landed enough for task 3's store extraction; current
  ownership is documented in `docs/wiki/modules/server-state-fanout.md`
  (`src/server/mod.rs` still owns `ActiveRuntimeStateFanout`,
  `ActiveTypographyState`).

## Clippy `match_same_arms` inventory (U4)

Command: `cargo clippy --all-targets --message-format=json -- -W clippy::pedantic`
(clippy 0.1.98). Full per-file list with line numbers:
`baseline-clippy-match-same-arms.txt`.

**38 unique sites** — 32 production + 6 test-side (the whole of
`src/client/tests.rs` 4 and `tests/editor_performance.rs` 1, plus
`src/server/mod.rs:5890` inside a `#[cfg(test)]` item). Top files: `src/shell/theme.rs` 12,
`src/server/agent.rs` 2, `src/server/ops/modes.rs` 2, `src/shell/package_ui.rs`
2, `src/client/tests.rs` 4, then one each in 16 other files
(`src/behavior/manifest.rs`, `src/client/mod.rs`, `src/packages/{service,verbs}.rs`,
`src/packages/record/documentation.rs`, `src/protocol/{agent,mod}.rs`,
`src/server/{document,language_server,menu_sessions,mod}.rs`,
`src/server/connection/{delivery,runtime}.rs`, `src/server/ops/packages.rs`,
`src/shell/components.rs`, `tests/editor_performance.rs`).

Reconciliation with the plan: the plan quotes 39 sites and "theme.rs 6";
measured is 38 and theme.rs 12 (each site is a distinct `match`; a site is
emitted twice when the lib is compiled as rlib and test harness — the list is
de-duplicated). A sample verified by hand: `src/shell/theme.rs:389`
(`"text.primary"`) and `:445` (`"border.strong"`) share the body
`Color::from_rgb8(0xee, 0xea, 0xff)`. The measured list is the work order for
task 6.

## Findings that change later tasks

1. **Source review missing.** All of plans 126–134 cite
   `code-reviews/2026-09-18-comprehensive-implementation-review.md`; that file
   does not exist in the repo (or in git history — `git log --all --diff-filter=A`
   has only the 2026-06-21/07-19/08-14 reviews). Plan-quoted counts (39 sites,
   "theme.rs 6", the inline-test line counts) cannot be reconciled against their
   source; use the measured inventory here.
2. **Task 4 (and task 3's test step) are sibling-file moves.** All four giant test
   modules are already `mod tests;` files, and `src/server/mod.rs`'s test body is
   `src/server/tests.rs`. Note those unit tests may use private items, so a full
   move to `tests/` may need to keep a thin `#[cfg(test)] mod tests;` shim.
3. **Suite registry guard.** `tests/suites/protocol.rs` contains
   `integration_suite_inventory_assigns_every_source_once`, which asserts every
   `tests/*.rs` root file is `#[path]`-assigned exactly once across the four
   suite files. Any new root test file created by task 4 must be registered in a
   suite file in the same change; files placed under `tests/suites/` or
   `tests/common/` are not scanned.
4. **`src/shell/theme.rs` | `src/server/ui.rs` line targets are unaffected** by
   the deltas above (both match the plan exactly), so the plan's shrink targets
   (`ui.rs` < ~2,000; theme split < ~1,600/file) stand as written.