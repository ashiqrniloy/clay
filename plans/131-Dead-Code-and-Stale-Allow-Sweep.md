# 131 — Dead Code and Stale-Allow Sweep

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` §7
(dead-code table), §10 A4 (three locking mechanisms, one live). Pure deletion +
lint-hygiene plan; no behavior changes.

## Objectives

- Delete confirmed-dead production code: `src/server/cross_domain.rs` (468 lines,
  0 production references), `ScopedLockManager` family in `src/server/locks.rs`
  (self-declared dead outside tests), `FoldingRangeRegistry`/`DocumentFolds` in
  `src/server/folding.rs:95–180` (0 instantiations), client event structs
  (`src/client/mod.rs:59–90`), `clay-agent/dbg5.mjs`, `tests/suites/protocol.rs.tmp`.
- Remove the stale `#[allow(dead_code)]` on live modules
  (`src/server/mod.rs:19–38`: `cross_domain` deleted; `git`, `js_runtime`, `ops`,
  `launcher` un-allowed) so the compiler's real dead-code lint is re-armed.
- A4: record the locking-model decision — `RegionLock`/`EditableLease` +
  workspace access teardown are the live ownership mechanisms; the unused
  `ScopedLockManager` generic layer is deleted, not kept "for later".
- Refresh the graft graph after deletion.

## Expected Outcome

- Net deletion ~600+ lines of production code and one debug script; no reference
  remains (grep clean per deleted symbol).
- `cargo check/clippy --all-targets` green with the stale allows removed; the
  compiler now reports any *newly* discovered dead code (triaged in-task).
- Wiki/doc references to deleted items updated (guard suites green).
- `graft build` refreshed; stale nodes gone.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full set passes untouched; record exit codes.
    - Performance: none (deletion work).
    - Code Quality: record current `cargo check` warning baseline (zero expected — allows suppress).
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §7 table (evidence per item).
      - Plan 119 P2-3 task evidence (dead-module deletion method incl. wiki/ledger/doc-guard cleanup and `graft build`).
    - Options Considered:
      - Direct delete: baseline first per house style. (Chosen: baseline.)
    - Chosen Approach:
      - Gates + grep evidence for each doomed symbol stored in task evidence.
    - Files to Create/Edit:
      - None.
    - References:
      - `plans/119-…` P2-3 evidence section.
  - Test Cases to Write:
    - None (evidence-recording task).

- [ ] Delete `src/server/cross_domain.rs` and decide the locking-model record (A4)
  - Acceptance Criteria:
    - Functional: module file, `mod` declaration, and any test-only references removed; grep for `cross_domain` (module refs; the unrelated `absorb_cross_domain_evaluation` op-method stays) is clean; suites green.
    - Performance: build time marginally down; nothing else.
    - Code Quality: deletion recorded in the wiki page that mentions the module (if any); doc-guard suites green.
    - Security: removes an unwired trust-boundary surface from the shipped set (the live cross-domain enforcement is `absorb_cross_domain_evaluation` + op validation — unaffected).
  - Approach:
    - Documentation Reviewed:
      - `src/server/cross_domain.rs` header (Plan 061 task 7 provenance), `src/server/ops/mod.rs:462` (the live `absorb_cross_domain_evaluation` — NOT part of the dead module).
      - `.agents/skills/create-plan/skills` n/a; decision-log skill for the A4 record if user-approval conventions require it — record decision in task evidence and, if material, a decision log entry.
    - Options Considered:
      - Wire it up later ("later" = never; git remembers): delete. (Chosen.)
      - Keep as documented future design: move the design intent to `docs/design/` note like plan 119 did for the sandbox, delete the code.
    - Chosen Approach:
      - Delete code; one-paragraph design note appended to the trust-domains decision log entry (append-only note, not a new decision).
    - Files to Create/Edit:
      - `src/server/cross_domain.rs` (delete), `src/server/mod.rs` (remove `mod` + its `allow`), decision-log note.
    - References:
      - Review §7, A4; `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
  - Test Cases to Write:
    - Existing suites green; no new tests for deleted code.

- [ ] Delete `ScopedLockManager` (locks.rs) and `FoldingRangeRegistry` (folding.rs)
  - Acceptance Criteria:
    - Functional: `src/server/locks.rs` deleted with `mod locks`; `FoldingRangeRegistry`/`DocumentFolds` and their impl/tests removed from `src/server/folding.rs` while the live `validate_folding_set`/`op_clay_folding_publish_ranges` path is untouched; suites green.
    - Performance: none.
    - Code Quality: grep clean for both symbols; the live folding op's tests still cover publication validation.
    - Security: no permission/validation path deleted (registry was never wired).
  - Approach:
    - Documentation Reviewed:
      - `src/server/locks.rs` (self-declared `expect(dead_code)`), `src/server/folding.rs:95–180`, `src/server/ops/folding.rs` (the live path).
    - Options Considered:
      - Promote `ScopedLockManager` to real use (A4): no consumer exists; workspace access + `RegionLock` cover current needs. Delete; revisit via a real requirement.
    - Chosen Approach:
      - Delete both; A4 disposition recorded (live model: `EditableLease`/`RegionLock` + workspace access teardown).
    - Files to Create/Edit:
      - `src/server/locks.rs` (delete), `src/server/mod.rs`, `src/server/folding.rs`.
    - References:
      - Review §7, A4.
  - Test Cases to Write:
    - Existing folding publication tests remain green.

- [ ] Delete stray artifacts and client dead structs; re-arm the dead-code lint
  - Acceptance Criteria:
    - Functional: `clay-agent/dbg5.mjs` and `tests/suites/protocol.rs.tmp` removed; the three `#[allow(dead_code)]` event structs in `src/client/mod.rs` deleted with their test-only references; the `#[allow(dead_code)]` attributes on live modules in `src/server/mod.rs` (`git`, `js_runtime`, `ops`, `launcher`) removed and `cargo check --all-targets` compiles clean — any newly-surfaced dead code is triaged in this task (delete or justify with a named upcoming plan).
    - Performance: none.
    - Code Quality: `cargo check`/`clippy --all-targets -D warnings` green without the suppressions.
    - Security: none of the deleted items gate permissions or validation.
  - Approach:
    - Documentation Reviewed:
      - Review §7; `src/server/mod.rs:19–38`, `src/client/mod.rs:59–90`.
    - Options Considered:
      - Keep allows "harmless": they disable the compiler's real lint for live modules — the exact anti-pattern. (Chosen: remove.)
    - Chosen Approach:
      - Remove allows; fix whatever the compiler surfaces within this task (surface expected to be small: module-level allows were stale).
    - Files to Create/Edit:
      - `src/server/mod.rs`, `src/client/mod.rs`, deletions above.
    - References:
      - Review §7.
  - Test Cases to Write:
    - Existing suites green.

- [ ] Refresh graft graph and verify docs consistency
  - Acceptance Criteria:
    - Functional: `graft build` succeeds; stale nodes for deleted files gone; `graft/INDEX.md` consistent.
    - Performance: graph rebuild is offline; no runtime effect.
    - Code Quality: wiki pages referencing deleted symbols updated; doc-guard/wiki-navigation suites green.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `AGENTS.md` (graft maintenance duty).
    - Options Considered:
      - Skip rebuild: stale graph misleads future `graft ask` results. (Chosen: rebuild.)
    - Chosen Approach:
      - `graft build` + wiki-navigation guard suite.
    - Files to Create/Edit:
      - `graft/` (generated), wiki pages if guard flags.
    - References:
      - `AGENTS.md` graft section.
  - Test Cases to Write:
    - None (guard suites cover).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed (pure deletion); verify via diff.
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
    - Functional: wiki no longer references deleted modules; A4 locking-model disposition recorded on the relevant page.
    - Performance: none.
    - Code Quality: linked from `docs/wiki/index.md`; navigation guard green.
    - Security: records that no permission/validation path was removed.
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
- To be filled after task completion with improvements, rationale, and priority.
