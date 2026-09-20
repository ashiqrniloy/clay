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

- [ ] Baseline gates and file-metric inventory
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

- [ ] Split `src/protocol/mod.rs` into family modules
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

- [ ] Split `src/server/mod.rs`: runtime assembly vs. accept loops vs. tests
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

- [ ] Extract the giant inline test files (`js_runtime/tests.rs`, `connection/tests.rs`, `client/tests.rs`, `workspace/tests.rs`)
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

- [ ] Table-driven validation in `src/server/ui.rs` (C4)
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

- [ ] String-enum macro (U5) and match-arm dedup (U4)
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

- [ ] Split `src/shell/theme.rs` into resolution and validation modules
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

- [ ] Execute and update the manual test plan (test-plan/)
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

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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

- [ ] Update or verify the code wiki after implementation
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

## Compromises Made
- To be filled after tasks are completed and tests pass.

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
