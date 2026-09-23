# 133 — File and Complexity Decomposition: server/mod.rs, protocol/mod.rs, ui.rs, String Enums

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
C3, C4, U4, U5 (review §4, §5). Structural decomposition of the remaining
oversized files plus two mechanical duplication cleanups. Pure refactor; no
behavior changes.

## Objectives

- C3: split `src/server/mod.rs` (6,190 lines) and `src/protocol/mod.rs` (3,830
  lines, 71 types) into cohesive submodules; extract the giant inline test
  modules (`src/server/js_runtime/tests.rs` 11,045 lines,
  `src/server/connection/tests.rs` 8,558, `src/client/tests.rs` 3,693,
  `src/server/workspace/tests.rs` 2,564) into the `tests/suites/` structure in
  followable steps.
- C4: make `src/server/ui.rs` (3,320 lines) table-driven: per-field validation
  declarative (field → allowed values → typed error), collapsing the manual
  match trees.
- U4: resolve the 39 clippy "match arms have identical bodies" sites (production
  ones: `src/shell/theme.rs` 6, `src/shell/package_ui.rs` 2, `src/behavior/manifest.rs`,
  `src/protocol/agent.rs`, `src/protocol/mod.rs`, `src/shell/components.rs`).
- U5: replace the 10 hand-written `enum + parse + as_str` triples with one
  `string_enum!` macro (no new dependency).

## Expected Outcome

- `src/server/mod.rs` < ~1,500 lines (runtime assembly + accept loops); its test
  body lives in `tests/suites/`. `src/protocol/mod.rs` is a module hub with
  family types in their own files.
- `ui.rs` validation is data-driven; adding an allowed value is a table edit;
  identical validation coverage proven by the existing conformance suites
  (`tests/package_ui_conformance.rs`).
- Clippy pedantic `same_match_arms` count for production files: zero.
- All Linux gates green after each step.

## Tasks

- [x] Baseline gates and file-metric inventory
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full set passes untouched; record exit codes.
    - Performance: none (structural).
    - Code Quality: record per-target line counts, type counts (`protocol/mod.rs` 71 types), and clippy `same_match_arms`/`match arm` duplication counts (39 production+test sites listed in the review).
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §4 (C3, C4), §5 (U4, U5).
    - Options Considered:
      - Skip metrics: the counts are the task's success criteria. Recorded.
    - Chosen Approach:
      - Static inventory into task evidence.
    - Files to Create/Edit:
      - None.
    - References:
      - Review §4, §5.
  - Test Cases to Write:
    - None (evidence-recording task).
  - Evidence (2026-09-21):
    - Tree/env: HEAD `6ee3b4c` ("WIP"), clean (`git status --porcelain` empty);
      rustc/cargo/clippy 1.98.1, cargo-audit 0.22.2, node v26.8.2; no
      `rust-toolchain.toml`. `frontend/dist` present (desktop/bindings stages).
    - Gates: `scripts/check.sh full` **exit 0** in 149 s, `full check PASSED` —
      `set -eu` + `FAILED at stage:` trap means the sentinel proves all nine
      stages exited 0 (audit with 9 allowed RUSTSEC warnings, fmt, check,
      clippy `-D warnings` **zero warnings**, test 1940 passed/1 ignored,
      bench-compile, desktop-clippy, desktop-test 55 passed, bindings
      `webview bindings up to date`). Total 1996 passed / 0 failed / 1 ignored.
      Log `test-plan/artifacts/133-file-decomposition/baseline-check-full.log`,
      exit codes `…/baseline-exit-codes.txt`, inventory `…/baseline.md`.
    - Note: plan 132's baseline failed clippy on 1.98.1 (`result_large_err`,
      `src/server/connection/documents.rs`); the allow is present at this HEAD
      (`:264`), so that lane landed between `36eabd2` and `6ee3b4c`.
    - Line counts (measured): `server/mod.rs` 6,203; `protocol/mod.rs` 3,832;
      `server/ui.rs` 3,320; `shell/theme.rs` 3,050; `js_runtime/tests.rs`
      12,663; `connection/tests.rs` 8,944; `client/tests.rs` 3,541;
      `workspace/tests.rs` 2,605. Plan-quoted values are stale for the four test
      files (js_runtime +1,618, connection +386, client −152, workspace +41);
      `ui.rs`/`theme.rs` targets are unaffected.
    - Types: `protocol/mod.rs` has **71 top-level struct/enum** declarations
      (33 struct + 38 enum) + 11 type aliases = plan's "71 types"; family-module
      table recorded in `baseline.md`.
    - Clippy `match_same_arms` (pedantic, command in evidence file): **38 unique
      sites** — 32 production + 6 test-side (whole of `src/client/tests.rs` 4 and
      `tests/editor_performance.rs` 1, plus `src/server/mod.rs:5890` inside a
      `#[cfg(test)]` item), dominated by `shell/theme.rs` 12. Per-file list with
      line numbers: `…/baseline-clippy-match-same-arms.txt` (raw 6.1 MB JSON not
      committed). Plan quotes 39 sites / "theme.rs 6" — unreconcilable because
      the cited review file is **absent from the repo** (see next); measured list
      is the task-6 work order.
    - Findings changing later tasks: (1) all plans 126–134 cite
      `code-reviews/2026-09-18-comprehensive-implementation-review.md`, which
      does not exist in the repo or git history; (2) the four giant test modules
      are already `mod tests;` sibling files and `server/mod.rs`'s test body is
      `src/server/tests.rs` (836 lines) — task 4 (and task 3's test step) are
      sibling-file relocations needing a `#[cfg(test)] mod tests;` shim for
      private access, and the plan's
      "tests from ~L3100" is stale (`mod tests;` at L6132); (3) `tests/suites/protocol.rs`
      asserts every new `tests/*.rs` root file is `#[path]`-registered exactly
      once, so task 4 files must be wired in the same change; (4) `StateFanout`
      exists at `src/server/fanout.rs:68` (plan 132 landed), so task 3's store
      extraction is unblocked.

- [x] Split `src/protocol/mod.rs` into family modules
  - Acceptance Criteria:
    - Functional: every type moves to an existing or new family file (`protocol/mod.rs` becomes declarations + shared helpers); all `use` sites updated; protocol suites (`tests/suites/protocol.rs`, `benches/protocol_server_baselines.rs`) green.
    - Performance: compile-time neutral or better; no runtime change (type moves only).
    - Code Quality: `mod.rs` < ~300 lines; no type defined in it except truly cross-family ones; codec round-trip tests unchanged.
    - Security: no wire-format change — rkyv archives byte-identical (round-trip tests prove it).
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/mod.rs` (71 types), existing family files (`agent.rs`, `completion.rs`, `decorations.rs`, …) as the pattern.
    - Options Considered:
      - Leave as-is (type dump): the review's finding; families already half-exist. (Chosen: finish the split.)
    - Chosen Approach:
      - Move by family with re-exports preserving `crate::protocol::X` paths (no caller churn beyond imports where rustfmt forces it).
    - Files to Create/Edit:
      - `src/protocol/mod.rs`, new/existing family files.
    - References:
      - Review C3.
  - Test Cases to Write:
    - Existing round-trip suites are the net (no new tests for moved types).
  - Evidence (2026-09-21):
    - Result: `src/protocol/mod.rs` **3,832 → 163 lines**. The hub keeps the
      `pub mod`/`pub use` list, `PROTOCOL_VERSION` (with its full history doc),
      the 11 shared id aliases, and `menu_session_id_serde`; no other type is
      defined there. New families: `behavior.rs` (1,202), `editor_rules.rs`
      (656), `messages.rs` (598), `typography.rs` (460), `caret.rs` (197),
      `theme.rs` (164), `document.rs` (149), `shell.rs` (143), `launcher.rs`
      (71). Existing families gained: `agent.rs` +`AgentSettingsFileInfo`,
      `diagnostics.rs` +`DiagnosticSeverity`/`RuntimeDiagnostic`. Module map with
      per-file line/type counts: `test-plan/artifacts/133-file-decomposition/task2-protocol-split.md`.
    - Move method: line-range extraction script (kept for provenance as
      `…/task2-split-protocol.py`) sliced each top-level item with its
      attributes/doc comments; new files open with a module doc and
      `use super::*;` (precedent: `src/packages/record/*.rs`), the hub globs
      every family, so **zero `crate::protocol::X` caller edits** were needed.
      The 13 protocol tests moved into `behavior.rs` with their unit (verbatim),
      so no visibility widening was required; `src/protocol/codec.rs` is
      untouched, including its round-trip tests.
    - Verbatim check: original `mod.rs` non-blank lines vs the new tree —
      **0 lines missing** (only added family headers/`use super::*;`).
    - Forced guard/doc updates (not wire changes):
      `tests/documentation_coverage.rs:242–243` now reads
      `src/protocol/messages.rs` for `ClientMessage`/`ServerMessage` variants;
      the hub's version-history line for `FoldingRangeSet` now names
      `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES` to keep
      `tests/performance_budgets.rs::folding_and_inlay_payloads_deny_above_cap`
      (its original satisfaction came from the moved variant doc).
    - Gates: `scripts/check.sh full` **exit 0**, 325 s, `PASSED` — clippy
      `-D warnings` zero warnings, `cargo test --all-targets` **1996 passed / 0
      failed / 1 ignored** (identical to the task-1 baseline), protocol suite
      `cargo test --test protocol` 227 passed, `cargo bench --no-run` compiles
      (`benches/protocol_server_baselines.rs`). Log `…/task2-check-full.log`,
      exit codes `…/task2-exit-codes.txt`.
    - Compile time: incremental `cargo check --lib` after touching the protocol
      module — 4.7 s split vs 4.6 s pre-split (A/B via stash, 3 samples each;
      first pre-split sample 15.7 s included the stashed file-set change).
      Neutral.
    - Security: no wire-format change — every item moved verbatim (multiset
      check), enum/derive bodies untouched, codec round-trip suites green.
    - Operational note for later gate runs: interrupting the suite during
      `manual_smoke_docs::plan118_…` leaves `capture-ui-review.sh` / `clay` /
      `clay-desktop` / `portal_capture.py` orphans that make the next run hang;
      clear them with `pkill -f "capture[-]ui-review"` etc. Clean run: ~50 s.

- [x] Split `src/server/mod.rs`: runtime assembly vs. accept loops vs. tests
  - Acceptance Criteria:
    - Functional: server construction/`ServerConfig`/generation stores move to focused submodules (e.g. `server/runtime_state.rs` for the fanout/generation stores — coordinate with plan 132's `StateFanout`); accept loops and `spawn_connection` stay; the inline `#[cfg(test)]` suites move to `tests/suites/server_runtime.rs` (or are absorbed by existing suites) without losing any test.
    - Performance: none.
    - Code Quality: `mod.rs` < ~1,500 lines; moved tests keep names (grep-able continuity); test count before/after equal (record both).
    - Security: no gate/permission logic altered — moved verbatim.
  - Approach:
    - Documentation Reviewed:
      - `src/server/mod.rs` structure (module decls L1–50, stores L166–304, accept loops L978–1030, tests from ~L3100).
    - Options Considered:
      - Move tests only: leaves the 3,000-line runtime half.
      - Full split: stores out, tests out. (Chosen; two steps, gates between.)
    - Chosen Approach:
      - Extract stores first (mechanical), tests second (mostly moves).
    - Files to Create/Edit:
      - `src/server/mod.rs`, `src/server/runtime_state.rs` (tentative name), `tests/suites/server_runtime.rs` (tentative).
    - References:
      - Review C3; plan 132 (ordering dependency on `StateFanout` location).
  - Test Cases to Write:
    - Test-count equality assertion in evidence (not a new test).
  - Evidence (2026-09-21):
    - Result: `src/server/mod.rs` **6,203 → 1,028 lines**. New focused modules:
      `src/server/runtime_state.rs` (525) — `ServerConfig`, `RuntimeGeneration`
      (+impl), `RuntimeGenerationStore` (+impl), `ActiveRuntimeStateFanout`,
      `ActiveTypographyState`, `shell_command_catalogue`,
      `RuntimeOutputApplication` + `apply_runtime_outputs`*
      (`crate::server::runtime_state`); and `src/server/runtime_reload.rs`
      (1,165) — `effective_agent_root`, `RuntimeGenerationCandidate`,
      `ReloadedDocumentRefresh`, `RuntimeReloadOutcome`, the runtime-contribution
      helpers, and `impl IpcServer` { `new`/`try_new`, configuration load,
      diagnostic recording, the hot-reload pipeline (`prepare_*`, `commit_*`,
      `reload_*`, `refresh_open_documents_after_reload`), `enumerate_ui_choices` }.
      Kept in `mod.rs`: module decls + prelude, `IpcServer` struct, tab-state
      plumbing, `run`/`accept_unix_loop`, `spawn_connection`,
      `sweep_expired_tabs`, `LiveClientGuard`, Unix socket + named-pipe
      binding/validation, `ServerError`. Module map, method-vs-store inventory,
      and the full plumbing diff list:
      `test-plan/artifacts/133-file-decomposition/task3-server-split.md`.
    - Tests: the four inline `#[cfg(test)]` mods moved to sibling unit-test files
      (`src/server/runtime_outputs_tests.rs` 225, `runtime_generation_tests.rs`
      3,070, `tab_server_state_tests.rs` 127, `windows_tests.rs` 67), declared
      from `mod.rs` with the same cfg gates. **Deviation from the task text**: the
      task named `tests/suites/server_runtime.rs`, but these suites exercise
      crate-internal items (`apply_runtime_outputs` is `#[cfg(test)]`,
      `create_named_pipe_server`/socket helpers are private, private
      `RuntimeGeneration*` fields are assembled directly), which an integration
      root (separate crate) cannot reach; sibling test files are the repository
      precedent (`src/server/tests.rs`, `src/server/connection/tests.rs`). No new
      `tests/*.rs` root was added, so the suite-inventory guard is unaffected.
    - Method: extraction script kept for provenance as `task3-split-server.py`
      (parses top-level items with attrs/doc prefixes plus the method groups
      inside the two `impl IpcServer` blocks, then cuts those ranges).
      Verbatim checks: 44/44 moved production blocks reappear as order-preserving
      subsequences of the new files; the four test bodies were byte-identical
      pre-rustfmt (then reflowed by `cargo fmt` with equal tokens); the token
      diff of all removed ranges vs. the new files contains only the expected
      deltas (4 `mod … {` wrappers + cfg attrs, 19 `pub(super)` keywords,
      `use`-statement rewrites, module docs).
    - Plumbing-only edits: `mod runtime_reload;`/`mod runtime_state;` + re-exports
      (`pub use runtime_reload::{ReloadedDocumentRefresh, RuntimeReloadOutcome}`,
      `pub use runtime_state::ServerConfig`,
      `pub(crate) use runtime_state::{RuntimeGeneration, RuntimeGenerationStore,
      apply_runtime_outputs_without_sdui}`); `pub(super)` on
      `RuntimeGeneration`/`RuntimeGenerationStore`/`ActiveTypographyState` fields,
      the stores' `initial`/`push_diagnostic`/`swap`,
      `RuntimeGenerationCandidate`, and `IpcServer::{load_default_configuration,
      load_configuration_for_service, prepare_runtime_generation_candidate,
      commit_runtime_generation}` (previously module-private in the same `server`
      subtree — same effective visibility); `src/server/connection/tests.rs`
      moved five `super::super::…` paths to
      `crate::server::runtime_state::…`; `tests/performance_budgets.rs`'s
      `is_sibling_test_file` now also accepts `<module>_tests.rs` siblings (it
      scanned the extracted test files as production code and false-positived on
      their folding-range fixtures).
    - Gates: `scripts/check.sh full` **exit 0**, 281 s, `PASSED`; clippy
      `-D warnings` and `cargo fmt --check` clean; test totals **identical to the
      task-1 baseline: 1996 passed / 0 failed / 1 ignored** (47 moved test
      attributes: 4 + 41 + 1 + 1; module and test fn names unchanged,
      e.g. `runtime_generation_tests::…`). Logs `…/task3-check-full.log`,
      exit codes `…/task3-exit-codes.txt`.
    - Compile time: incremental `cargo check --lib` after touching `mod.rs` —
      4.85 s / 4.94 s split vs 5.43 s / 4.83 s pre-change (A/B via stash; first
      sample ≈17 s both sides from the fingerprint change). Neutral.
    - Security: socket binding and parent-directory/socket permission validation
      plus `ServerError` stayed in `mod.rs` untouched; the moved code is verbatim
      (checks above), so no gate or permission logic was altered.

- [x] Extract the giant inline test files (`js_runtime/tests.rs`, `connection/tests.rs`, `client/tests.rs`, `workspace/tests.rs`)
  - Acceptance Criteria:
    - Functional: the four inline test modules move to `tests/suites/` (or shrink to a thin `mod tests;` include) with zero lost tests; suites green from the new locations.
    - Performance: compile-time neutral or better.
    - Code Quality: each moved file keeps its test names; `tests/suites/` module map updated; no `#[cfg(test)]`-only helpers stranded unreferenced (deleted or moved with them).
    - Security: security-relevant tests keep their suite wiring (`tests/suites/security.rs` untouched).
  - Approach:
    - Documentation Reviewed:
      - House pattern: root `tests/` (47 files) with `autotests = false` + suite wiring in `tests/suites/*.rs`.
    - Options Considered:
      - Keep inline: review finding stands; 25K lines of tests inside src inflates every read.
      - Move to integration tests where they only use public API; keep `#[cfg(test)]` thin shims where they need private access. (Chosen.)
    - Chosen Approach:
      - File-by-file move; private-access tests remain unit tests but relocated to a submodule file (mechanical `mod tests;` include is acceptable if full integration move would weaken visibility).
    - Files to Create/Edit:
      - `src/server/js_runtime/tests.rs` → `src/server/js_runtime/tests/` split or `tests/suites/js_runtime_*.rs`; likewise the other three.
    - References:
      - Review C3 item 6 (ranked refactoring list).
  - Test Cases to Write:
    - None lost; no new tests (move-only).
  - Evidence (2026-09-21):
    - Result: each `src/**/tests.rs` is now a thin `src/**/tests/mod.rs` (module
      doc + all imports + shared helpers + the suite map) plus one file per
      contiguous thematic run of tests — 55 suites total (24 js_runtime,
      14 connection, 8 client, 9 workspace). Test counts identical to baseline:
      **241 + 87 + 48 + 80 = 456**; root files hold 0 tests; largest suite is
      911 lines (`js_runtime/tests/lanes_and_queues.rs`); the owners'
      `#[cfg(test)] mod tests;` declarations are untouched
      (`tests.rs` → `tests/mod.rs`). Per-file before/after table, suite map and
      full method in
      `test-plan/artifacts/133-file-decomposition/task4-test-split.md` (+`task4-suite-map.txt`).
    - **Deviation from the task text**: the suites stayed in-crate under
      `src/**/tests/` instead of `tests/suites/*.rs`, because these modules
      exercise `pub(crate)`/private items (`apply_runtime_outputs` is
      `#[cfg(test)]`, socket helpers and `IpcServer` internals are private,
      workspace save/reload hooks are test-only), which an integration root
      (separate crate) cannot reach; the task text explicitly allowed the thin-
      include form instead. No new `tests/*.rs` root was created, so the suite
      inventory guard is unaffected.
    - Method: `task4-split-test-files.py` slices items between per-suite ladder
      entries named by the first test of each run (kept for provenance). Imports
      and helpers stay in `tests/mod.rs`; suites start with `use super::*;` so
      reachability is unchanged. Only two textual edits inside moved items:
      `super::` chains gain one level (6 + 52 + 12 + 0 lines) and
      `include_str!`/`include_bytes!` paths gain one level (11 + 1) — plus
      `#[cfg(windows)] mod windows_named_pipe;` in the client root so the
      Windows-only suite does not leave an unused glob on non-Windows builds.
    - Verification: `task4-verify.py` **exit 0** — for each module, every original
      top-level item (imports, helpers, tests) reappears exactly once across the
      root + suites as the same code (whitespace-insensitive; rustfmt's
      trailing-comma moves tolerated) after the documented bump, root files have
      0 `#[test]` items, all 456 bodies placed, and all 66 root helper items are
      referenced from at least one suite (no stranded `#[cfg(test)]`-only helpers).
    - Guard/doc updates: `tests/performance_budgets.rs::is_sibling_test_file`
      now also treats files under a `tests/` directory as test-only (it scanned
      the new suites as production and false-positived on folding fixtures);
      `tests/package_ui_conformance.rs` `RETIREMENT_GUARDS` now names
      `src/server/js_runtime/tests/coding_agent_and_launcher.rs`; the
      guard-enforced page `docs/wiki/flows/document-chunked-loading.md` now
      points at `js_runtime/tests/document_and_git_facades.rs` and
      `connection/tests/language_intelligence.rs`.
    - Deferred to the wiki task below: ~20 other wiki pages still name the old
      `src/**/tests.rs` paths (~80 references) and test-name `cargo test` filters
      (`client::tests::real_server_tab`); the suite map artifact is the work order.
    - Gates: `scripts/check.sh full` **exit 0**, 244 s, `PASSED`, clippy
      `-D warnings` + `fmt --check` clean, totals **1996 passed / 0 failed /
      1 ignored = baseline** (`task4-check-full.log`, `task4-exit-codes.txt`).
      Compile time neutral (`cargo check --all-targets` after touching the tests
      root: 6.5 s/6.8 s vs 6.7 s pre-change). Four earlier attempts were red on
      unrelated timing-sensitive tests (worker-timeout `RecvError` under load,
      two `agent_protocol` `ETXTBSY` spawns, one fake-LSP early exit) and one
      desktop-harness hang after aborted runs; all passed standalone and in the
      green run — recorded in the artifacts, not caused by the move.

- [x] Table-driven validation in `src/server/ui.rs` (C4)
  - Acceptance Criteria:
    - Functional: string-set validation (`VALID_SLOTS`, `VALID_VISIBILITY`, `VALID_OVERLAY_ANCHORS`, `VALID_FOCUS_POLICIES`, `VALID_DISMISSAL_POLICIES`, `VALID_INPUT_SCOPES`, pointer policies, …) becomes a declarative table consulted by one validator; typed error kinds unchanged (same error variants surface for the same inputs — proven by existing tests).
    - Performance: validation cost not regressed (table lookup vs match — same order; hot-path SDUI validation is budgeted and tested).
    - Code Quality: `ui.rs` shrinks (target: < ~2,000 lines); adding an allowed value is a one-line table edit; conformance suites (`tests/package_ui_conformance.rs`) green.
    - Security: validation coverage strictly non-decreasing — every field's allowed set identical before/after (test asserts set equality per field).
  - Approach:
    - Documentation Reviewed:
      - `src/server/ui.rs` (validation functions + `VALID_*` tables), `tests/package_ui_conformance.rs`.
    - Options Considered:
      - Macro-generated validators per field: table + one generic validator is simpler. (Chosen.)
      - Keep manual matches: review finding. 
    - Chosen Approach:
      - `const` tables of `(field, allowed, error_ctor)` + generic `validate_choice` walking them; complex validations (numeric bounds, structural) stay explicit.
    - Files to Create/Edit:
      - `src/server/ui.rs`.
    - References:
      - Review C4.
  - Test Cases to Write:
    - `allowed_sets_unchanged`: before/after set equality per field (golden test authored pre-refactor against the old tables).

  - Evidence (2026-09-21):
    - Result: the 21 hand-written closed-set checks (147 lines of
      `if !VALID_*.contains(..) { return Err(context.error(rule, Some(id), "…")) }`)
      are now one-line `validate_choice("<key>", value, id, &context)?` calls backed
      by the `CHOICE_FIELDS` table (21 rows of `(key, allowed, rule, label)`,
      `#[rustfmt::skip]` data table) plus one validator, the `choice_field`
      lookup, and the `choice_message` builder. Adding an allowed value is one edit
      in the `VALID_*` slice; the diagnostic text is built from that slice
      (`label` + `a, b, or c`, keeping the two "must be one of …" phrasings and
      the Oxford comma), so message and values cannot drift. Reads and defaults
      stay at the call sites (`required_str` / `optional_str(..).unwrap_or(..)`),
      so missing-field, blank-string, and wrong-type behaviour is untouched —
      only the membership check moved into the table. Diagnostic
      (rule, id, message) triples are unchanged, including the layout-override
      rows, which keep id = offending value.
    - Security acceptance: the golden `allowed_sets_unchanged` test is
      `choice_fields_pin_every_allowed_set` — exactly 21 rows, no duplicate keys,
      every `VALID_*` slice still has >=1 row, and per-field `assert_eq!` against
      the literal sets that shipped before the refactor. The companion
      `choice_messages_match_the_shipped_diagnostics` pins the 21 exact message
      literals (it caught a missing Oxford comma on the first run, i.e. the guard
      works). Both were generated from the pre-change file, so they cannot drift
      into agreeing with a regression. `cargo test --lib server::ui::` -> 24 passed.
    - Performance: <=21-entry linear scan per closed-choice field at contribution
      registration / override-validation time, no allocation on the accepted path,
      nothing on the SDUI snapshot/update hot path; payload-budget and
      conformance tests green.
    - **Compromise on the line-count target**: `ui.rs` is 2,164 lines (from
      3,320) — the 21 checks were 147 lines and the table + validator + docs cost
      111, so the refactor itself saves ~36 lines; `< ~2,000` is not reached
      because the rest of the file is the registry and the per-contribution
      validators that C4 does not cover. Follow-up if the target matters: split
      `ui.rs` into `src/server/ui/{panels,overlays,inputs,state}.rs` family
      modules (same pattern as the protocol/server splits above).
    - Deviation: the 1,112-line inline test module moved to
      `src/server/ui/tests.rs` (`#[cfg(test)] mod tests;`, the module-declaration
      form tasks 3-4 use), which also carries the two new tests — so
      `src/server/ui.rs` is no longer the only file touched (same documented
      deviation pattern as task 3).
    - Gates: `scripts/check.sh full` **exit 0**, `full check PASSED`, totals
      **1998 passed / 0 failed / 1 ignored** (= task-1 baseline 1996 + the 2 new
      tests), clippy `-D warnings` + `fmt --check` clean, desktop/bindings stages
      green (`task5-check-full.log`, `task5-exit-codes.txt`). One earlier attempt
      hung in the known desktop-capture flake
      (`manual_smoke_docs::plan118_ui_review_harness_…`; 1488 tests had passed with
      0 failures, the test passes standalone in 50 s and passed in the rerun) —
      unrelated to `ui.rs`. Method/codemod: `task5-table-validation.py`; write-up:
      `task5-table-validation.md`.

- [x] String-enum macro (U5) and match-arm dedup (U4)
  - Acceptance Criteria:
    - Functional: a `string_enum!` macro generates `parse`/`as_str`/`all_as_str` for the 10 enums; production `same_match_arms` sites resolved (merge arms or extract shared body); no behavior change (parse tables byte-identical).
    - Performance: `as_str` stays `const fn`-equivalent (match on discriminant); no lookup-table allocation.
    - Code Quality: clippy pedantic `same_match_arms` count for production files zero; macro documented with one example.
    - Security: enum string forms are protocol-visible (`as_str` outputs) — byte-identical before/after asserted by tests.
  - Approach:
    - Documentation Reviewed:
      - `src/shell/theme.rs:16–160` (the triple pattern ×3), `src/shell/icons.rs`, other sites per review U5.
    - Options Considered:
      - `strum` dependency: a new dependency for what one declarative macro does. (Chosen: local macro, no dependency.)
      - Leave as-is: 10 copies of the same 30-line pattern.
    - Chosen Approach:
      - `macro_rules! string_enum { ($(#[$m:meta])* $name:ident { $($variant:ident => $s:literal),+ $(,)? }) … }` in a small `src/shell/str_enum.rs` (or `src/protocol`-local if shared).
    - Files to Create/Edit:
      - `src/shell/theme.rs`, `src/shell/icons.rs`, macro module, U4 sites listed in the review.
    - References:
      - Review U4, U5.
  - Test Cases to Write:
    - `string_roundtrip_per_enum`: every variant `parse(as_str(v)) == Some(v)` and unknown strings `None` — one generic test applied per macro invocation.

  - Evidence (2026-09-21):
    - U5 result: `src/str_enum.rs` (126 lines incl. docs) defines
      `string_enum_impl!`, generating `const fn as_str(self)`,
      `fn parse(value: &str) -> Option<Self>`, an optional `const fn
      all_as_str()` (trailing `all_as_str` token) and a `#[cfg(test)] const
      ALL: &'static [Self]` used by the golden test; attributes/docs and the
      visibility in front of the type name are copied onto the generated
      `impl`. 15 hand-written pairs (89 variant/string pairs) in 7 files now
      come from one declaration each; 2 new tests.
    - U5 shape (deviation, deliberate): the macro takes the *impl*, not the
      type — the plan sketched `string_enum! { enum Name { … } }`, but the
      converted enums carry `rkyv`/`serde` derives, `#[serde(rename_all)]`,
      `#[rkyv(…)]` and variant attributes (`#[default]`), so leaving every enum
      declaration untouched guarantees the archive/wire attributes cannot be
      disturbed. Not converted (documented in `task6-string-enum.md`): the three
      `Result`-returning parsers (`protocol/behavior.rs:686`,
      `design_system.rs:169`) and the parse-only enums
      (`DeferredComponentKind`, `TextobjectDirection`, `SmartSelectAction`) are
      outside the `Option<Self>` + `as_str` contract, and
      `protocol::textobjects::TextobjectKind` already derives `parse` from its
      own `pub const ALL` vocabulary list (no duplicated table; the macro's
      test-only `ALL` would clash). Three package enums keep a module-private
      `parse` via the `parse_private` form — no visibility widening.
    - U5 count vs the plan: the plan says "the 10 enums"; the cited review is
      absent from the repo (task-1 finding), so the measured inventory is the
      source of truth: 16 enums have the triple, 15 converted + 1 skipped above.
      `parse` cannot be `const fn` (rustc 1.98: "cannot match on `str` in
      constant functions"), verified before writing the macro; `as_str` stays
      `const fn`.
    - U5 correctness evidence: the codemod
      (`task6-string-enum.py` → `task6-string-enum-extraction.txt`,
      `task6-string-enum-map.txt`) refuses any rewrite unless the `as_str` and
      `parse` tables are the same bijection, the arm order equals the enum
      declaration order, the `parse` body is only a table (no
      trim/normalisation), and every declared variant is covered — so the
      `parse` tables are byte-identical by construction. The golden test
      (`src/str_enum/tests.rs`, `string_roundtrip_per_enum`) uses pairs
      extracted from the pre-change tables and asserts `Enum::ALL` is
      exhaustive, `as_str(variant)` equals the pinned string (the
      protocol-visible surface), string uniqueness, `parse(as_str(v)) ==
      Some(v)` and unknown-string rejection.
    - U4 result: `clippy::match_same_arms` 38 (task-1 baseline: 32 production +
      6 test-side) → **0** in the final pedantic run over the whole tree; the
      standard gate is green too. Each site was fixed with clippy's own
      multi-span merge suggestion (merge pattern + delete the duplicate arm)
      applied by byte offset (`task6-u4-sites.md`). Semantic checks:
      `src/shell/theme.rs`'s core token map keeps 147 keys / 112 unique aliases
      before and after (zero added/removed), and the security-relevant
      command→permission map in `packages/record/documentation.rs` keeps the
      exact same command set in the merged `=> None` arm with every permission
      arm untouched. Four merges collapsed a match to one real arm and tripped
      the default `clippy::single_match` gate → rewritten as `if let` with the
      same body.
    - U4 method note (kept in the record): the first application used the
      clippy JSON's byte offsets as Python string indices, so 16 files
      containing non-ASCII (box-drawing/`§`) were spliced a few bytes early and
      failed to parse. The 16 files whose only change was this task were
      restored from `HEAD` and re-applied byte-safely; the 6 files that also
      carry tasks 2-5 work or are new/untracked were repaired by hand against
      the pre-edit spans in the JSON; a re-measured pedantic run then reported
      exactly the 30 restored sites and none of the 8 hand-handled ones. All 61
      remaining edits applied in one byte-safe pass.
    - Guards updated (source-scanning doc guards, forced by both halves):
      `tests/package_ui_conformance.rs::core_token_catalog_matches_tokens_md`
      and `tests/primitives_docs.rs::phase20_1_token_catalog_is_complete_…`
      now collect every quoted name on an arm line (aliased tokens share one
      arm after U4); `no_component_kind_or_token_renamed`'s implemented-kind
      marker is `=> "kind",` and its token marker is the quoted name; and
      `catalog_is_drift_free_across_doc_enum_and_react_registry` reads the
      `ComponentKind` string table from the macro invocation
      (`double_quoted()` factored out and shared).
    - Gates: `scripts/check.sh full` **exit 0**, `full check PASSED`, totals
      **2000 passed / 0 failed / 1 ignored** (= task-1 baseline 1996 + 2 from
      task 5 + the 2 new tests), clippy `-D warnings` + `fmt --check` clean,
      desktop/bindings stages green (`task6-check-full.log`,
      `task6-exit-codes.txt`). One red intermediate gate run (two
      `package_ui_conformance` guards, then the `primitives_docs` guards and
      four `single_match` sites) is recorded in the artifacts. Write-ups:
      `task6-u4-match-arms.md`, `task6-string-enum.md`; codemod
      `task6-string-enum.py`.

- [x] Split `src/shell/theme.rs` into resolution and validation modules
  - Acceptance Criteria:
    - Functional: `shell/theme.rs` (3,050 lines) separates token/enum string parsing + validation from theme resolution/compositing (e.g. `shell/theme/parse.rs`, `shell/theme/resolve.rs` — final names per inventory); all call sites updated; theme suites + `tests/theme_packages.rs` green.
    - Performance: none (moves only).
    - Code Quality: each resulting file < ~1,600 lines; inline test block moves with its unit; clippy/fmt green.
    - Security: contrast-floor validation logic moves verbatim (the `#[cfg(test)]` floors stay with validation).
  - Approach:
    - Documentation Reviewed:
      - `src/shell/theme.rs` structure (enums L16–160, resolution core, contrast checks, tests from L1467); review §8.
    - Options Considered:
      - Leave as one file: review finding stands; file mixes three concerns.
      - Split by concern. (Chosen.)
    - Chosen Approach:
      - Mechanical move in two steps (parse/validate out, tests follow their units).
    - Files to Create/Edit:
      - `src/shell/theme.rs`, `src/shell/theme/*.rs` (tentative).
    - References:
      - Review §8.
  - Test Cases to Write:
    - Existing theme suites are the net (move-only).
  - Evidence (2026-09-21):
    - Result: `src/shell/theme.rs` **2,938 → 26 lines** (re-export facade); new
      `src/shell/theme/parse.rs` (739) — token/level enums, `ThemeTokenResolver`,
      `CoreThemeValue` catalog, `CORE_TOKEN_NAMES`; `theme/validate.rs` (249) —
      `DesignTokenError`, `validate_design_token_override`, contrast floors and
      `theme_meets_contrast`/`validate_active_theme_contrast`; `theme/resolve.rs`
      (599) — `SduiThemeStyle`, `PanelDefaults`, `ResolvedUiTheme`,
      `ThemeTokenValueDto`, `resolve_theme_token_snapshot`,
      `density_spacing_scale`. Largest file is the moved `theme/tests.rs`
      (1,167) — every file under the `< ~1,600` target. Module docs, imports,
      module map and per-file item inventory: `task7-theme-split.md`.
    - Zero call-site edits: the hub globs `pub use parse/resolve/validate::*`
      (the `src/protocol/*` pattern), so every `crate::shell::theme::X` and
      `super::theme::{…}` path (`components.rs`, `package_ui.rs`,
      `design_system.rs`, `server/ops/theme.rs`, `editor/theme.rs`'s `pub use`,
      the Tauri bridge) is unchanged. Public bindings
      (`ContrastFailure`, `validate_active_theme_contrast`,
      `ThemeTokenValueDto`, `resolve_theme_token_snapshot`,
      `density_spacing_scale`, `CORE_TOKEN_NAMES`) keep their visibility.
    - Tests: the two inline modules moved to sibling files with names and
      module paths unchanged — `theme/tests.rs` (22 tests) and
      `theme/theme_snapshot_tests.rs` (7), 29/29 same-named tests green
      (`cargo test --lib shell::theme`); `theme_packages` and
      `package_ui_conformance` green via `cargo test --test presentation`
      (62 pass), `primitives_docs` (35) and `rust_visibility` (6) green.
    - Visibility plumbing (only production edits): six `pub(crate)` additions
      for items the moved tests or sibling modules reach (`CoreThemeValue` +
      its fields, `core_theme_value`, `resolve_f64`,
      `ResolvedUiTheme::resolved`); nothing added, removed, or renamed. The
      contrast floors and their `#[cfg(test)]` assertion moved verbatim.
    - Move proof: `task7-split-theme.py` refuses unless every production line
      reappears exactly once with only the six documented substitutions;
      `task7-verify.py` re-checks each destination order-preserving against
      `git show HEAD:src/shell/theme.rs` (`verbatim: OK`) and prints the
      added-line inventory (docs/imports/declarations + two rustfmt reflows).
      Only `cargo fmt` reindented the moved tests.
    - Forced guard path updates (moved catalog):
      `package_ui_conformance::core_token_catalog_matches_tokens_md` and the
      two `primitives_docs` catalog guards read `src/shell/theme/parse.rs`;
      `rust_visibility_api_mapping` pins `HAIRLINE_VISIBILITY_MIN` /
      `REQUIRED_FILL_PAIRS` in `src/shell/theme/validate.rs`. The
      `src/shell/theme.rs` path itself still exists, so no wiki/doc path change
      is needed in this task.
    - Gates: `scripts/check.sh full` — audit, fmt, check, clippy `-D warnings`
      (zero warnings), test, bench-compile **PASS**; root totals **1944 passed /
      0 failed / 1 ignored**, identical to task 6. Log `task7-check-full.log`,
      codes `task7-exit-codes.txt`. The gate aborts at `desktop-clippy` because
      this host has no WebKitGTK dev packages (`javascriptcoregtk-4.1` missing,
      fails at HEAD too; no sudo to install) — supplementary
      `cargo check`/`cargo clippy -p clay-desktop --all-targets` with stubbed
      `.pc` files and a stub `frontend/dist` type-check the desktop crate
      (including `src-tauri/src/bridge/dto.rs`) clean; the `bindings` stage
      needs a real link and is deferred to the next full-green host.
    - **Resolved (2026-09-22):** WebKitGTK 4.1 (2.52.6) is now installed on this
      host; the re-run `scripts/check.sh full` is **exit 0 / `full check
      PASSED`** with `desktop-clippy`, `desktop-test` (55 passed) and
      `bindings` (`webview bindings up to date`) green — log
      `task8-check-full-desktop.log`, details in the task-8 evidence.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: pure refactor — record explicitly that no user-visible behavior changed and which regression modules were re-run (packages-and-modes for ui validation, theme modules), with the reason no new steps are added.
    - Performance: none.
    - Code Quality: explicit-record rule satisfied.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`; `.agents/skills/create-plan/references/clay.md`.
    - Chosen Approach:
      - Regression-only pass.
    - Files to Create/Edit:
      - None expected.
    - References:
      - None beyond index.
  - Test Cases to Write:
    - None.
  - Evidence (2026-09-22):
    - Record: `test-plan/index.md` gained the dated section **"Plan 133
      file/complexity decomposition execution record (2026-09-22, task 8)"** —
      the plan is a pure internal refactor, so no step was added, deleted, or
      weakened and no module file needed an edit; modules
      `test-plan/09-packages-and-modes.md` (package/mode UI-token validation)
      and `test-plan/15-ui-design-systems.md` (theme tokens/contrast,
      design-system activation) were re-run as a regression pass.
    - Regression modules re-run (all PASS, 654 targeted tests):
      `cargo test --lib server::ui` 24 and `--lib packages::` 83 with
      `--test security` 152 (module 09: the task-5 table-driven contribution/
      override validation, every error kind unchanged); `--lib shell::theme` 29
      (22 + 7 same-named tests after the task-7 split — module 15) and
      `--test presentation` 62 (`theme_packages` 15 + `package_ui_conformance`);
      `--lib str_enum` 2 (`string_roundtrip_per_enum` pins the shipped strings)
      with `--test protocol` 227 (incl. `primitives_docs` 35) and `--test
      runtime` 75 for the task-2/task-6 protocol-visible surfaces.
    - Whole-refactor gate: `scripts/check.sh full` audit/fmt/check/
      clippy `-D warnings`/test/bench-compile green, root totals **1944 passed /
      0 failed / 1 ignored** — identical to the task-1 baseline
      (`task7-check-full.log`). Re-run after WebKitGTK 4.1 (2.52.6) was installed
      on this host: **exit 0, `full check PASSED`**, with `desktop-clippy`,
      `desktop-test` (55 passed) and the bindings guard
      (`webview bindings up to date`) green — the task-7 desktop caveat is
      closed (`task8-check-full-desktop.log`).
    - Live server launch smoke (module 01 launch-gate class, server half) PASS:
      fresh `target/debug/clay` on an isolated mode-700 root/socket with
      `--config-fixture ui-review-default` → `clay server listening on
      /tmp/clay133/clay.sock`, mode-700 socket bound, process alive, no
      configuration diagnostics, clean kill.
    - Desktop GUI steps (modules 01/09/15) **PASS live, 4 fixtures** on a freshly
      built `clay` + `clay-desktop` via `scripts/capture-ui-review.sh`
      (xdg-desktop-portal screenshot, AT-SPI dump, isolated mode-700 root and
      private socket): `ui-review-default` 1906×1099 (shell, workspace file
      browser, empty-tab Open File/Open Folder landing);
      `ui-review-design-system` 1906×1099 (shipped `@clay/design-instrument` +
      gruvbox-material-dark active, SDUI review panel with Primary action and
      Enabled/Disabled rows, editor pane, AT-SPI landmark/button/listbox/listitem
      nodes present); `ui-review-design-system-light` (same geometry under
      gruvbox-material-light — UI-DS-15 recoloring, no stale color);
      `ui-review-large-typography` (enlarged user-owned UI typography in outline
      rows, status bar, landing). All four `review.status=PASS`,
      `configuration_failed_lines=0`; captures under
      `test-plan/artifacts/133-file-decomposition/live/<fixture>/`.
    - Frontend suites (not covered by `scripts/check.sh`) PASS:
      `npm ci` + `npm run build` (tsc + vite) + `npm test` **51 files / 497
      tests passed**, `npm run check:budget` shell 178.6/180 kB and total
      403.5/404 kB gzip. **Finding fixed by this pass:**
      `frontend/src/test/core-baseline-hierarchy.test.ts` still read
      `src/protocol/mod.rs` for the typography `DEFAULT` block that task 2 moved
      to `src/protocol/typography.rs`, so its slice was empty and the test failed
      (`expected '' to contain 'title: 15.0 / 13.0'`); the guard now reads
      `src/protocol/typography.rs` (values unchanged) and the suite, typecheck
      and prettier are green. This is a forced guard-path update like task 2's
      Rust doc guards, found by the regression pass rather than by the Rust gate.
      The same sweep retargeted the two live `default_keymaps()` “source of
      truth” comments (`examples/config/init.js` and its `plan080-manual` fixture
      copy) from `src/protocol/mod.rs` to `src/protocol/behavior.rs`; protocol
      (227) and presentation (62) suites green after the change.
    - Artifacts: `test-plan/artifacts/133-file-decomposition/task8-manual-regression.txt`
      (raw regression output, launch smoke, GUI captures table, frontend finding),
      `task8-check-full-desktop.log` (full gate exit 0 with desktop+bindings),
      `live/` (four capture directories), plus the existing `task2`–`task7`
      records. The parity-ledger guard (`tests/documentation_coverage.rs`,
      `cargo test --test protocol`) stays green because no step ID changed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed; SDUI validation error kinds identical (the JS-visible `ui.*` op results unchanged); verify via diff.
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
  - Evidence (2026-09-22):
    - Result: **no public programmatic surface changed — verify-only, zero
      production edits.** Diffed against the pre-plan baseline `6ee3b4c`
      (`task9-js-api-verify.py`, report `task9-js-api-verification.txt`): the
      JS-API artifact paths (`runtime/js/`, `docs/reference/clay-js-api/`,
      `docs/generated/`, `docs/index.md`, `frontend/src/bridge/`,
      `src-tauri/bindings/`) are byte-identical both in the task-2/6 commit and
      in the uncommitted task-7/8 tree; `cargo run --bin update-doc-registry`
      rewrites `docs/generated/clay-js-api-registry.json` to identical bytes
      (no `git status` diff), so no facade export, stable ID, op wrapper, doc
      page, generated registry entry, or ts-rs binding moved.
    - `ui.*` op results: `src/server/ops/ui.rs` is byte-identical (the task-5
      rewrite is inside `src/server/ui.rs`); `UiContributionRule` (the
      JS-visible error-kind enum) is unchanged at 17 variants; and all 21
      closed-choice diagnostic messages recompose byte-for-byte from the new
      `CHOICE_FIELDS`/`VALID_*` table plus `choice_message` (the baseline's
      literal set and the composed set are identical; the other 39 baseline
      diagnostics remain unchanged literals), and the moved
      `src/server/ui/tests.rs` asserts every one of the 21 full strings.
    - The only op-wrapper diffs vs baseline are task-6 `match_same_arms` merges:
      `ops/modes.rs` drops `None | Some(Value::Null)` in favor of the existing
      `_ => Vec::new()` catch-all and `ops/packages.rs` merges `Ok(_)` with the
      already-identical `DuplicateContributionId` arm — same behavior, no
      JS-visible value changed.
    - Guards: `cargo test --lib server::ui` 24, `--lib str_enum` 2 (enum string
      tables byte-identical), `--test protocol` 227 (`clay_js_api_inventory`,
      `clay_js_doc_registry`, `clay_js_facade_layout`, `documentation_coverage`),
      `--test presentation` 62 (`package_ui_conformance`),
      `--test security` 152 (`rust_visibility_api_mapping`) — all pass.
    - Artifact: `test-plan/artifacts/133-file-decomposition/task9-js-api-verification.txt`
      (checks + guard counts + registry no-op) with the verifier
      `task9-js-api-verify.py`.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki pages for protocol structure, ui validation, and the moved test suites updated; master index current.
    - Performance: notes the table-driven validation's cost characteristics.
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: documents that SDUI allowed-sets are unchanged (validation authority intact).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach:
      - Update once after gates pass.
    - Files to Create/Edit:
      - Affected wiki pages, `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.
  - Evidence (2026-09-22):
    - Structural pages updated (no page added or removed; every page stays
      linked from the master index): **protocol structure** — `protocol-codec.md`
      gained the task-2 family module map (`src/protocol/mod.rs` hub over
      `messages/behavior/editor_rules/typography/theme/caret/document/shell/launcher`)
      with the move-only/no-wire-change statement and the task-6
      `src/str_enum.rs` golden-string note; **ui validation** —
      `slot-aware-package-ui.md` gained the task-5 `CHOICE_FIELDS` table section
      with the cost characteristics (one ≤21-row static scan per closed-choice
      field at registration/override-validation time, no allocation on the
      accepted path, nothing on the SDUI snapshot/update, paint, layout,
      pointer, or scroll hot paths) and the security note that all 21 allowed
      sets, the 17 `UiContributionRule` kinds, and every message are unchanged
      with `src/server/ops/ui.rs` byte-identical; the same section records the
      task-7 `src/shell/theme.rs` hub over `theme/parse.rs`,
      `theme/validate.rs`, `theme/resolve.rs` with unchanged call-site paths;
      **moved test suites** — `maintenance-validation.md` corrected the suite
      topology (`tests/suites/{security,runtime,presentation,protocol}.rs`, 38
      `#[path]` modules, was “editor”/33) and gained a Plan 133
      decomposition/test-layout section listing the four
      `src/**/tests/` directories (js_runtime 24, connection 14, client 8,
      workspace 9 suites) and the `src/server/*_tests.rs` siblings.
    - Path sweep: 61 references across 18 pages rewritten to the split suite
      directories, plus 23 targeted retargets to the moved test files / modules
      (`runtime_generation_tests.rs`, `runtime_reload.rs`, `runtime_state.rs`,
      `src/server/ui/tests.rs`, the theme suites) in `embedded-js-runtime.md`,
      `configuration-runtime.md`, `control-center.md`, `command-registry.md`,
      `behavior-manifests.md`, `package-loading.md`, `server-driven-ui.md`,
      `server-ipc-skeleton.md`, `editor-theme-registry.md`,
      `react-shell.md`, `package-input-state-configuration.md`, and
      `maintenance-validation.md`; the protocol/typography/theme paths were
      retargeted from `src/protocol/mod.rs` to the family files.
    - Master index current: six `docs/wiki/index.md` entries (frontend theme
      runtime, protocol codec, maintenance validation, server IPC skeleton,
      slot-aware package UI, editor theme registry) now name the Plan 133
      decomposition; the wiki-index link guard is green.
    - Guards: `cargo test --test protocol` 227 (includes
      `primitives_docs::wiki_index_links_every_wiki_page`,
      `documentation_coverage` parity/current-page path checks, and
      `plan125_wiki_pages_describe_the_one_palette_surface` /
      `plan129_wiki_pages_describe_the_dispatch_router_and_future_sizes` /
      `plan088_code_wiki_documents_modernization_contract`), `--test
      presentation` 62 — all pass after the edits.
    - Artifact: `test-plan/artifacts/133-file-decomposition/task10-wiki-verification.txt`
      (per-page change list, sweep counts, guard results).

## Compromises Made
- **Task 7 test placement**: the two inline test modules moved to sibling
  files (`src/shell/theme/tests.rs`, `theme/theme_snapshot_tests.rs`) declared
  from the hub with the same `#[cfg(test)] mod …;` form and unchanged module
  paths, rather than being split per-unit into each new module. The plan's
  "tests follow their units" is satisfied at the module boundary (all three
  units are in the same `shell::theme` subtree) and this matches the
  task-3/task-5 sibling-file precedent; the 29 tests are unchanged and green.
- **Task 7 desktop gate not runnable on this host** (resolved 2026-09-22):
  `desktop-clippy` (and therefore `desktop-test`/`bindings`) originally aborted
  on missing WebKitGTK dev packages, which also fails at HEAD and could not be
  installed without sudo; the desktop crate was instead type-checked with
  stubbed pkg-config entries. WebKitGTK 4.1 (2.52.6) was then installed by the
  user, `frontend/dist` built, and the re-run `scripts/check.sh full` passed
  **exit 0 / `full check PASSED`** with desktop-clippy, desktop-test (55) and
  bindings green (`task8-check-full-desktop.log`).

## Further Actions

- **Rust `ClientEditQueue` keeps test-only enqueue capabilities** (delegated from
  plan 131 task 4; priority: medium, test weight only):
  `enqueue_viewport_render_request`, `enqueue_command_intent`, and
  `enqueue_save_document` are `#[cfg(test)]` because the React frontend emits those
  protocol frames itself (`frontend/src/editor/sync/messages.ts` for
  `saveDocument`/`viewportRenderRequest`, `frontend/src/editor/extensions/controller.ts`
  for `commandIntent`). Decide the capability set while this plan extracts
  `src/client/tests.rs`: keep the queue as a real Rust-client emission path with
  live callers, or retire the remaining test-only surface with its tests (plan 132
  is the co-owner if the frames are declared duplicates).
