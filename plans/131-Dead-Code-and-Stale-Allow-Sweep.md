# 131 — Dead Code and Stale-Allow Sweep

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` §7
(dead-code table), §10 A4 (three locking mechanisms, one live). Pure deletion +
lint-hygiene plan; no behavior changes.

## Objectives

- Delete confirmed-dead production code: `src/server/cross_domain.rs` (468 lines,
  0 production references), `ScopedLockManager` family in `src/server/locks.rs`
  (self-declared dead outside tests — corrected in task 3: only the
  range/document/workspace target layer is dead; the behavior scope is live in
  the runtime-reload commit), `FoldingRangeRegistry`/`DocumentFolds` in
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

- [x] Baseline gates on the unmodified tree
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
  - Evidence (2026-09-20):
    - Environment: HEAD `66648e3` ("WIP", dirty tree — in-flight plans 127–135 work,
      79 entries in `git status --porcelain`, none touching plan-131 targets);
      rustc/cargo 1.96.1, node v24.19.0. All deletion targets still present:
      `src/server/cross_domain.rs` (468 lines), `src/server/locks.rs` (271 lines),
      folding registry `src/server/folding.rs:95–161`, `clay-agent/dbg5.mjs`,
      `tests/suites/protocol.rs.tmp`.
    - Gates: `scripts/check.sh full` **exit 0** in 331 s — audit (9 allowed RUSTSEC
      advisory warnings, pre-existing), fmt, `cargo check --all-targets`, clippy
      `-D warnings`, `cargo test --all-targets --quiet` (1421 lib + 62 + 227 + 75 +
      152 + 9 + 1 passed, 0 failed, 1 ignored), `cargo bench --no-run`, and
      `scripts/check-bindings.sh` all pass. Raw log and exit code:
      `test-plan/artifacts/131-dead-code-sweep/baseline-check-full.log`,
      `…/baseline-exit-codes.txt`; summary `…/baseline.md`.
    - Code-quality baseline: `cargo check --all-targets --message-format short`
      replays **0** warnings — the stale module allows hide everything, so any
      lint surfaced after task 4 removes them is a genuinely new finding.
    - Grep evidence (raw `grep -rn`, excludes target/.git/graft/node_modules;
      `graft grep` agrees): `cross_domain` 62 hits — production in-edge only via
      the live `absorb_cross_domain_evaluation` (`src/server/ops/mod.rs:493`,
      called at `src/server/ops/packages.rs:529`); `ScopedLockManager` 20 /
      `ScopedLockTarget` 21 hits — all in `locks.rs`, `src/server/mod.rs:117,788`,
      `src/server/tests.rs:91` and the module's own tests; `FoldingRangeRegistry`
      12 / `DocumentFolds` 5 hits — `folding.rs` only, 0 in-edges.
    - Two findings that change later tasks (no action taken here):
      `tests/rust_visibility_api_mapping.rs:89` allowlists
      `"src/server/cross_domain.rs"`, so task 2 must also edit that guard list;
      and `src/server/locks.rs` contains only `ScopedLock*` items plus its own
      tests (no `RegionLock`/`EditableLease`), confirming task 3's whole-file
      deletion against the live ownership model.
    - Doc pages carrying deleted-symbol references, for tasks 2/3/7:
      `docs/development/architecture-ownership.md`,
      `docs/reference/primitives/package-security.md`,
      `docs/wiki/flows/document-leases-and-region-locks.md`,
      `docs/wiki/modules/third-party-runtime-authority.md`,
      `docs/wiki/modules/embedded-js-runtime.md`,
      `docs/wiki/modules/persistent-runtime-hot-reload.md`,
      `docs/wiki/modules/react-sdui-package-ui.md`,
      `docs/reference/clay-js-api/folding/server-publish-folding-ranges.md`,
      `docs/reference/clay-js-api/api-inventory.toml`,
      `docs/generated/clay-js-api-registry.json`.

- [x] Delete `src/server/cross_domain.rs` and decide the locking-model record (A4)
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
  - Evidence (2026-09-20):
    - Deleted `src/server/cross_domain.rs` (468 lines, 6 of its own tests), its
      `pub(crate) mod cross_domain;` + stale `#[allow(dead_code)]` and pre-mod
      comment in `src/server/mod.rs`, and the dangling
      `("src/server/cross_domain.rs", "pub struct CrossDomainRequestEnvelope")`
      row in `tests/rust_visibility_api_mapping.rs` (the guard edit flagged by
      baseline). Removing the attributed module re-exposed `menu_sessions` to
      rustfmt's alphabetical `mod` run, so `cargo fmt` moved it after `mod locks`.
    - Also deleted the now-orphaned `CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES` in
      `src/perf/budgets.rs` (4 lines): the deleted validator was its only consumer,
      so keeping it would ship a budget for a mechanism with no implementation
      (not caught by the dead-code lint — `perf` is a public module).
    - A4 decision (task 3 executes it): **delete `ScopedLockManager`, do not
      promote.** No production consumer exists — baseline `graft grep`/raw grep show
      only construction sites (`src/server/mod.rs:788`, `src/server/tests.rs:91`)
      and the module's own tests; the live ownership mechanisms are
      `RegionLock`/`EditableLease` (`src/server/document.rs:21,27`; 33 references)
      plus workspace access-count teardown. Durable record lands on
      `docs/wiki/flows/document-leases-and-region-locks.md` in task 7.
      **Correction (task 3):** this conclusion was wrong. `commit_runtime_generation`
      (`src/server/mod.rs:1577–1583`) acquires the behavior scope and holds the
      guard across the generation compare-and-swap; it is called from
      `load_default_configuration` (`src/server/mod.rs:1055`) and the reload path
      (`src/server/mod.rs:1251`), so the manager and `ScopedLockTarget::Behavior`
      are live production code. The `expect(dead_code)` was satisfied by the three
      never-constructed range/document/workspace targets (rustc expectations cover
      nested items), which is what review §7/A4 read as "the manager is dead".
      Task 3 therefore deleted only the never-constructed generic target layer
      (`Range`/`Document`/`Workspace`) and kept the live behavior scope — see task 3
      "Scope correction".
    - Design-intent record is an append-only note, not a new decision:
      `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md` →
      "## Appended note (2026-09-20, plan 131 dead-code sweep)": the validator was
      pre-wired for plan 061 task 8's extension surfaces, which never arrived; the
      rule stays binding; any future invocation re-adds validator + consumer
      together. Durable docs updated: `docs/reference/primitives/package-security.md`
      (design record + live load-bridge behavior),
      `docs/reference/primitives/registry.md` row `CrossDomainRequestEnvelope`
      (`New` → `Deferred`, keeping the ledger↔registry guard green),
      `docs/wiki/modules/third-party-runtime-authority.md`,
      `docs/wiki/modules/embedded-js-runtime.md`,
      `docs/wiki/modules/react-sdui-package-ui.md`.
    - Gates after deletion: `scripts/check.sh full` **exit 0** in 353 s (baseline
      331 s), 0 compiler warnings, clippy `-D warnings` clean; lib tests 1421 → 1415
      (−6 = exactly the deleted module's own tests, no unrelated coverage loss);
      targeted guards `security::rust_visibility_api_mapping`,
      `protocol::primitives_docs`, `protocol::documentation_coverage` green. Log:
      `test-plan/artifacts/131-dead-code-sweep/task2-check-full.log`.
    - Grep `cross_domain` after deletion: no `mod` or envelope-type references
      remain; only the live `absorb_cross_domain_evaluation`
      (`src/server/ops/mod.rs:493`, `src/server/ops/packages.rs:529`), the live
      bridge test (`src/server/js_runtime/tests.rs:874`), and the doc/decision-log
      records above.

- [x] Delete `ScopedLockManager` (locks.rs) and `FoldingRangeRegistry` (folding.rs)
  - Scope correction (task 3, evidence-based): the planned whole-file deletion of
    `src/server/locks.rs` was not executable — `commit_runtime_generation`
    (`src/server/mod.rs:1577`) calls `try_acquire(ScopedLockTarget::Behavior, …)`
    on the live config-load/reload commit path, and `RegionLock::overlaps`
    (`src/server/document.rs:1040`) calls `locks::ranges_overlap`. The dead part is
    the never-constructed generic target layer (`ScopedLockTarget::Range`/
    `Document`/`Workspace` and its conflict/validation machinery), not the
    manager. Actual scope: keep the live behavior-scope lock (single non-blocking
    scope, RAII release); delete the dead targets, the `ranges_overlap` helper
    (inlined into its only caller), and — as planned —
    `FoldingRangeRegistry`/`DocumentFolds`. Acceptance criteria below are read
    accordingly: `src/server/locks.rs` and `mod locks` survive, `grep` is clean for
    the deleted symbols, and both live paths stay untouched.
  - Acceptance Criteria:
    - Functional: the never-constructed generic target layer deleted from `src/server/locks.rs` (the file and `mod locks` stay: the behavior scope is live on the runtime-generation commit path, and `ranges_overlap` is inlined into `RegionLock::overlaps`); `FoldingRangeRegistry`/`DocumentFolds` and their impl removed from `src/server/folding.rs` while the live `validate_folding_publication`/`op_clay_folding_publish_ranges` path is untouched; suites green.
    - Performance: none.
    - Code Quality: grep clean for the deleted symbols; the live folding op's tests still cover publication validation; no `#[allow(dead_code)]` left in `src/server/folding.rs`.
    - Security: no permission/validation path deleted (registry was never wired).
  - Approach:
    - Documentation Reviewed:
      - `src/server/locks.rs` (self-declared `expect(dead_code)`), `src/server/folding.rs:95–180`, `src/server/ops/folding.rs` (the live path).
    - Options Considered:
      - Promote the whole `ScopedLockManager` to real use (A4): the behavior scope already has a consumer (runtime-reload commit), so keep it; delete only the unused range/document/workspace acquisition layer rather than carrying generic primitives "for later".
    - Chosen Approach:
      - Delete the dead target layer + `ranges_overlap` (inlined) + `FoldingRangeRegistry`/`DocumentFolds`; A4 disposition recorded (live model: behavior-scope commit guard + `EditableLease`/`RegionLock` + workspace access teardown).
    - Files to Create/Edit:
      - `src/server/locks.rs`, `src/server/mod.rs`, `src/server/document.rs`, `src/server/folding.rs`, `docs/reference/clay-js-api/{api-inventory.toml,folding/server-publish-folding-ranges.md}`, `docs/generated/clay-js-api-registry.json`.
    - References:
      - Review §7, A4.
  - Test Cases to Write:
    - Existing folding publication tests remain green.
  - Evidence (2026-09-20):
    - Premise check before deleting (review §7/A4 called this file dead on the
      strength of the self-declared `expect(dead_code)` attribute, whose reason
      string names "range/document/workspace acquisition … for later scoped
      mutations"): `ScopedLockManager`, `ScopedLockTarget`, and the RAII
      `ScopedLockGuard` are live on the runtime-generation commit path
      (`commit_runtime_generation`, `src/server/mod.rs:1577–1583`, called from
      `load_default_configuration` `:1055` and the reload path `:1251`);
      `ranges_overlap` is live in `RegionLock::overlaps`
      (`src/server/document.rs:1040`). Whole-file deletion would have broken the
      server config-load/reload commit and region-lock overlap.
    - `src/server/locks.rs` **271 → 67 lines**: deleted `ScopedLockTarget`
      (`Range`/`Document`/`Workspace`), `ScopedLockConflict`,
      `ScopedLockError::InvalidTarget`, `validate`, the `conflicts_with` matrix,
      `ranges_overlap`, the `expect(dead_code)` attribute, and the two
      generic-target tests (the wiki pages listed below still describe the deleted
      generic semantics and need the task 5/7 pass). Kept live behavior scope,
      reshaped to what it actually is: one
      non-blocking scope (`held: Arc<Mutex<bool>>`), `try_acquire()` (no target or
      owner — both were only ever read by the deleted conflict payload), unit
      error `BehaviorLocked`, `ScopedLockGuard` releasing on drop; one test
      `behavior_lock_rejects_concurrent_acquire_and_releases_on_drop` replaces the
      two generic tests. Caller-visible behavior unchanged: `mod.rs` still maps
      `Err(_)` to `runtime.behavior_locked`.
    - `src/server/mod.rs`: import `locks::ScopedLockManager` only, call
      `.try_acquire()`, drop the now-unused protocol `LockOwner` import.
      `src/server/document.rs`: drop the `locks::ranges_overlap` import and inline
      the predicate (`start < self.end && self.start < end`) in
      `RegionLock::overlaps` — no other caller existed.
    - `src/server/folding.rs` **354 → 297 lines**: deleted `FoldingRangeRegistry`,
      `DocumentFolds`, their `impl` block (incl. unused `publish_ranges`/`store`/
      `merged`), 3 `#[allow(dead_code)]` attributes (file now has 0), and the
      orphaned `BTreeMap` import. Live path untouched:
      `validate_folding_publication`/`validate_folding_set`,
      `folds_from_syntax_tree`, and its 5 tests (budget deny, stale version,
      missing/insufficient permission, generic tree walk).
    - Doc metadata that named the deleted symbol was fixed in-task (it is
      guard-checked): `docs/reference/clay-js-api/api-inventory.toml` and
      `docs/reference/clay-js-api/folding/server-publish-folding-ranges.md`
      front-matter `backing_rust`/`current_rust_owner`
      `FoldingRangeRegistry::publish_ranges` → live
      `src/server/folding.rs::validate_folding_publication`;
      `docs/generated/clay-js-api-registry.json` regenerated with
      `cargo run --bin update-doc-registry` (its diff also carries the pre-existing
      uncommitted plans-127–135 doc updates, i.e. the state the artifact already had
      before this task; `generated_registry_is_current`,
      `set_typography_api_is_linked_and_generated_registry_is_current`, and
      `inventory_rust_paths_name_existing_source_files` green).
    - Still to fix in task 5/7 (wiki prose, no guard covers it):
      `docs/wiki/flows/document-leases-and-region-locks.md:9,34,83` (source bullet,
      "range/document/workspace" lock semantics, `cargo test --lib locks::tests`
      wording), `docs/wiki/modules/persistent-runtime-hot-reload.md:43,53,62,76,103`
      (`ScopedLockTarget::Behavior`/generic-scope prose),
      `docs/wiki/modules/command-registry.md:93,95` (same). Corrections needed: one
      behavior scope only, no lock ids/owners/typed conflict payload,
      `ranges_overlap` now inlined in `RegionLock::overlaps`; `mod locks` and
      `src/server/locks.rs` remain (the file still has one test).
    - A4 disposition after this task: live locking/ownership =
      `RegionLock`/`EditableLease`, the behavior-scope commit guard
      (`ScopedLockManager`, behavior only), and workspace access-count teardown;
      the dead part was the generic range/document/workspace target layer. Review
      §10 A4's "three mechanisms, one live" is corrected accordingly.
    - Gates after deletion: `scripts/check.sh full` **exit 0** in 264 s (task-2 run
      353 s), 0 compiler warnings, clippy `-D warnings` clean; lib tests 1415 →
      1414 (−1 = the removed generic-target test; the folding registry had no tests
      of its own); protocol 227, security 152, presentation 62, runtime 75 green;
      targeted `locks::` (1), `folding::` (5), `region_lock` (7) green. Log:
      `test-plan/artifacts/131-dead-code-sweep/task3-check-full.log`.
    - Grep evidence: `ScopedLockTarget|ScopedLockConflict|ScopedLockError|
      FoldingRangeRegistry|DocumentFolds|ranges_overlap` → 0 hits across `src/` and
      `tests/`; the only remaining mentions are the wiki prose lines listed above
      (task 5/7) plus the archived review/plan text.

- [x] Delete stray artifacts and client dead structs; re-arm the dead-code lint
  - Findings note (task 4, evidence-based): removing the four module allows
    surfaced **14** lint findings, not the single expected `trigger_characters`,
    because those allows were also hiding test-only accessors and one never-used
    `#[cfg(test)]` accessor. Triage is per item: **delete** (zero references),
    **`#[cfg(test)]`** (reachable only from tests — it must not exist in the
    shipped lib, so no allow is needed), or one **narrow allow with a `reason`**
    for a capability whose production caller is still missing. No module-wide
    allow remains.
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
      - Remove allows; fix whatever the compiler surfaces within this task (surface expected to be small: module-level allows were stale). One known finding from the plan-136 audit: `src/server/ops/completion.rs::trigger_characters` is a private helper with zero callers — it is the vestige of the pre-Phase-27 options-driven completion descriptor, and the re-armed lint is expected to report it.
    - Files to Create/Edit:
      - `src/server/mod.rs`, `src/client/mod.rs`, deletions above.
    - References:
      - Review §7.
  - Test Cases to Write:
    - Existing suites green.
  - Evidence (2026-09-20):
    - Stray artifacts: deleted `clay-agent/dbg5.mjs` (2,856 bytes, debug script
      committed in the `WIP` commit) and `tests/suites/protocol.rs.tmp` (0 bytes);
      `grep -rn` finds no reference to either.
    - Client dead structs (the plan's named scope): deleted
      `EditorCompletionRequestEvent`, `EditorLanguageIntelligenceRequestEvent`,
      `EditorSelectionQueryRequestEvent` and, with them, the only code that
      referenced the three: `ClientEditQueue::{enqueue_completion_request,
      enqueue_language_intelligence_request, enqueue_selection_query_request}`,
      `record_completion_accept`, `recent_completion_texts`, the
      `completion_recency` field plus its two initializers, and the four
      `src/client/tests.rs` tests (completion enqueue, bounded accept recency,
      optimistic-after-edit version, language-intelligence enqueue). Lib tests
      1414 → 1410 — exactly those four, no other coverage change.
      `src/client/mod.rs` −265 net lines, `src/client/tests.rs` −152. Three
      protocol imports became unused (`CompletionRequest`,
      `LanguageIntelligenceRequest`, `SelectionQueryRequest`) and were removed.
    - Removing the `#[allow(dead_code)]` on `impl ClientEditQueue` surfaced 12 more
      queue methods with no lib caller: 11 have **zero** references and were
      deleted (`client_id`, `enqueue_menu_query_update`,
      `enqueue_menu_backspace`, `enqueue_menu_selection_move`,
      `enqueue_menu_activate`, `enqueue_menu_cancel`,
      `enqueue_runtime_generation_installed`, `enqueue_close_document`,
      `enqueue_reload_document`, `enqueue_request_resync`, `sync_snapshot_for`);
      3 are test-only (`enqueue_viewport_render_request`,
      `enqueue_command_intent`, `enqueue_save_document`) and are now
      `#[cfg(test)]`, with a comment recording that the React frontend emits those
      frames itself (`frontend/src/editor/sync/messages.ts`).
    - Module allows removed from `src/server/mod.rs`: `git`, `js_runtime`,
      `launcher`, `ops` (launcher had no findings — genuinely stale). A fifth,
      stale field allow on `IpcServer::parse_coordinator` (used by live code) was
      removed as well; the file's only remaining allow is the `#[cfg(windows)]`
      security descriptor. This removed the last of the suppressions that hid the
      `ops`/`git`/`js_runtime` dead surfaces.
    - Deleted zero-reference findings: `js_runtime/error.rs`
      `ClayRuntimeError::Join` + its three match arms + the `tokio::task` import
      (never constructed) and `configuration_runtime_detail` (unused);
      `js_runtime/mod.rs::shutdown_generation_resources` (superseded by
      `shutdown_trusted_generation_resources`; zero refs) and
      `load_default_configuration` (unused even by tests — production calls
      `load_default_configuration_with_workspace`); `ops/mod.rs::has_current_package`
      (its doc pointer updated to `in_package_activation`) and the
      `#[cfg(test)]`-gated `domain()` accessor (unused in every build);
      `ops/commands.rs::string_or` (zero refs; `modes.rs` keeps its own private
      copy); `ops/completion.rs::trigger_characters` (the plan's expected
      finding); `git.rs::refresh_workspace` (zero refs).
    - `#[cfg(test)]`-gated test-only support (kept out of the shipped lib instead
      of carrying an allow): `js_runtime` `evaluate_controlled_module`,
      `load_configuration_from_root`, `load_configuration_from_root_for_document`,
      `open_activation_evaluation_count` (thin wrappers/accessors over the live
      `*_with_workspace`/`*_for_document` variants); `ops`
      `explicit_icon_pack_active`, `explicit_design_system_active`,
      `validate_set_cursor_style` (the live op calls
      `validate_set_cursor_style_value`), `PackageLoadEntryAllowlist::record`
      (production records through `record_for_package`); `git`
      `discover_workspace_statuses`, `refresh_stale_workspace`, `should_poll`,
      `GIT_STATUS_POLL_INTERVAL`, and the `tokio::task::JoinSet` import (only the
      test-only helpers use it).
    - One justified narrow allow remains:
      `PackageLoadEntryAllowlist::revoke_package` carries
      `#[cfg_attr(not(test), allow(dead_code, reason = "withdrawal capability for
      the disable/revoke path; no production caller yet (plan 131 task 4
      finding)"))]`. Its test shows the withdrawal works, but no production
      disable/revoke path calls it, so a disabled package's recorded
      `clay://packages/...` module entries are never withdrawn and stay resolvable
      for the current generation. Recorded in Further Actions (high priority);
      wiring it needs a package-disable → runtime hook, which is not a dead-code
      sweep.
    - Gates: `cargo check --lib` **0 warnings** and `cargo check --all-targets`
      **0 warnings** with the allows gone; `scripts/check.sh full` **exit 0** in
      280 s (task-2 run 353 s), clippy `-D warnings` clean; lib tests 1414 → 1410
      (−4 = the deleted client tests only); protocol 227, security 152,
      presentation 62, runtime 75 green; bench compile and webview bindings green.
      Log: `test-plan/artifacts/131-dead-code-sweep/task4-check-full.log`.

- [x] Refresh graft graph and verify docs consistency
  - Findings note (task 5, evidence-based): the graph was rebuilt and `graft check`
    is clean; the docs sweep found **11 evergreen documents still describing
    plan-131-deleted code as live** (the largest cluster: the generic lock-target
    story, repeated in four pages). Two of those docs asserted behaviour the code
    never had — a production Git stale-poll loop and production
    `PackageLoadEntryAllowlist::revoke_package` withdrawal — so the corrections
    record the task-4 allowlist finding and a new Git-poll finding instead of
    re-asserting them. Archive pages and explicit deletion records were left
    untouched.
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
  - Evidence (2026-09-20):
    - Graph: `graft build` rebuilt the wiring graph — 11,579 nodes / 20,857 edges /
      626 cards from 634 parsed files (16 parsed, 618 replayed from cache).
      `graft check` reports “graph check: OK — the wiring graph is in sync with the
      code”. `graft/src/server/cross_domain.md` is gone, `graft/INDEX.md` contains
      no `cross_domain`/`FoldingRangeRegistry` reference, and no card under
      `graft/src` mentions a deleted symbol (`cross_domain.rs`,
      `FoldingRangeRegistry`, `DocumentFolds`, `ScopedLockTarget`,
      `enqueue_completion_request`, `ranges_overlap`,
      `configuration_runtime_detail`, `trigger_characters`,
      `shutdown_generation_resources`).
    - Docs consistency sweep (evergreen pages only; `docs/wiki/archive/` is
      pull-only history and was left alone, since its links still resolve):
      - `docs/wiki/flows/document-leases-and-region-locks.md`: the Phase 19
        paragraph and its test bullet now describe the behavior-only
        `ScopedLockManager` / typed `BehaviorLocked` / RAII `ScopedLockGuard` and
        record that plan 131 deleted the generic `ScopedLockTarget`
        range/document/workspace variants plus `ranges_overlap` (this page carries
        the A4 locking-model record).
      - `docs/wiki/modules/persistent-runtime-hot-reload.md`:
        `ScopedLockTarget::Behavior` → `ScopedLockManager::try_acquire` (3 spots),
        `shutdown_generation_resources` → `shutdown_trusted_generation_resources`
        (2 spots, with the reason: the shared third-party domain survives a trusted
        reload), the scoped-lock primitive bullet, the `locks::tests` coverage
        line, and the allowlist invariant (withdrawal hook exists, no caller yet).
      - `docs/wiki/modules/command-registry.md`: reload command now names
        `ScopedLockManager::try_acquire`, and the stale
        “range/document/behavior/workspace lock acquisition … workspace locks
        conflict with every scope” paragraph is replaced by the single behavior
        lock and its typed conflict.
      - `docs/wiki/modules/language-server-process-service.md`:
        `shutdown_trusted_generation_resources` (2 spots).
      - `docs/wiki/modules/configuration-runtime.md`: live entry point is
        `load_configuration_from_root_with_workspace` (the bare wrapper is now a
        `#[cfg(test)]` helper); the Rust example follows.
      - `docs/wiki/modules/embedded-js-runtime.md`: public async method list now
        names `evaluate_controlled_module_for_document`,
        `load_configuration_from_root_with_workspace`, and
        `load_default_configuration_with_workspace`.
      - `docs/wiki/modules/git-discovery-service.md`: the periodic stale-polling
        path (`GIT_STATUS_POLL_INTERVAL`, `refresh_stale_workspace`,
        `discover_workspace_statuses`) is recorded as having no production caller
        (plan 131 gated it `#[cfg(test)]`); the live refresh path is the explicit
        `refresh_root` used by `clay:git`/command execution.
      - `docs/wiki/modules/package-loading.md` and
        `docs/reference/primitives/package-loading.md`: the `revoke_package`
        claim now says the hook is unit-tested but unwired (matches the task-4
        finding) instead of asserting live withdrawal.
      - `docs/wiki/modules/completion-snippet-expansion.md`: completion requests
        are stamped with the optimistic version by the React completion extension
        (`frontend/src/editor/extensions/completion.ts`); the Rust
        `ClientEditQueue` path was deleted in plan 131.
      - `docs/development/architecture-ownership.md`: the `ClayJsRuntimeService`
        behaviour list now names the live methods.
    - Intentionally untouched (they describe the deletion themselves, each naming
      plan 131): `docs/wiki/archive/*`, `docs/reference/primitives/package-security.md`,
      `docs/reference/primitives/registry.md` (`CrossDomainRequestEnvelope` row is
      `Deferred`), `docs/wiki/modules/third-party-runtime-authority.md`,
      `docs/wiki/modules/react-sdui-package-ui.md`, and
      `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
    - Gates: `cargo test --test protocol` **152 passed** (covers
      `documentation_coverage` — including
      `wiki_navigation_is_complete_and_current_page_paths_resolve` —
      `primitives_docs`, `manual_smoke_docs`, `package_loading_docs`);
      `cargo test --test security` **227 passed**; `graft check` OK. No code changed
      in this task, so the task-4 `scripts/check.sh full` exit 0 run still holds.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Findings note (task 6, verification only): **no public programmatic surface
    changed**. Plan 131 deleted exclusively private/`pub(crate)` Rust items, so no
    Rust public function lost its Clay JS API (decision 2026-05-08-1509 boundary
    intact), and the only Clay JS API metadata edit was a *backing-pointer*
    correction on an API whose name, module, and export are unchanged
    (`folding.serverPublishFoldingRanges`). The sweep did surface one pre-existing
    inventory defect (a planned row pointing at a nonexistent op file), recorded in
    Further Actions.
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
  - Evidence (2026-09-20):
    - Identity check: `docs/reference/clay-js-api/api-inventory.toml` has **163
      entries in `HEAD` and 163 now**; **zero removed / added stable IDs**, and the
      set of `(js_module, js_export, facade fn)` tuples is identical in both
      directions. The only plan-131 field change is
      `folding.serverPublishFoldingRanges` (`backing_rust` + `current_rust_owner`:
      `FoldingRangeRegistry::publish_ranges` → `validate_folding_publication`). The
      other entries in the diff (`completion.serverRegisterCompletionProvider`,
      `language.serverRegisterLanguageIntelligenceProvider`,
      `agent.setFullAutonomy`) are pre-existing in-flight edits from plans 127–135
      that touch only `custom_properties`/`security_notes` — no name or export
      moves in any of them.
    - No deleted symbol survives on the Clay JS surface: grep over
      `docs/reference/clay-js-api/` (including `api-inventory.toml` and
      `inventory.md`), `docs/generated/clay-js-api-registry.json`, and `runtime/js/`
      finds no `publish_ranges`, `FoldingRangeRegistry`, `DocumentFolds`,
      `ScopedLockTarget`, `ranges_overlap`, `enqueue_*_request`,
      `Editor*RequestEvent`, `trigger_characters`, `configuration_runtime_detail`,
      `has_current_package`, `validate_set_cursor_style`, `GIT_STATUS_POLL_INTERVAL`,
      `refresh_stale_workspace`, `discover_workspace_statuses`, or `cross_domain`.
      `op_clay_folding_publish_ranges` remains correctly: the op wrapper in
      `src/server/ops/folding.rs` is live; only its unused registry type was deleted.
    - Reference integrity: every non-`planned:` file reference in the inventory was
      existence-checked against `HEAD` and against the worktree — **78 missing
      references before, the same 78 after, zero regressions**. The missing set is
      pre-existing (unwritten `status = "planned"` docs/ops such as
      `docs/reference/clay-js-api/packages/*.md`, plus prose fragments inside
      `current_rust_owner` fields). No entry references
      `src/server/cross_domain.rs` or any other file this plan deleted.
    - Native/public facade files untouched: `git status --porcelain runtime/js/
      packages/ frontend/src/` shows no plan-131 edit (only unrelated in-flight
      `packages/lsp-shared/*` churn from another plan).
    - Rust visibility: every deleted item was private or `pub(crate)` —
      `string_or`, `trigger_characters`, `validate_set_cursor_style_value`,
      `ranges_overlap`, `ScopedLockManager`, `FoldingRangeRegistry`,
      `DocumentFolds`, `has_current_package`, the `js_runtime/mod.rs` wrappers, the
      `git.rs` polling items, and the client event structs plus `ClientEditQueue`
      methods (`pub(crate)` in `src/client/mod.rs`) — so neither the JS surface nor
      the library's Rust API shrank.
    - Guards: `cargo test --test protocol clay_js` → **74 passed** (covers
      `clay_js_api_inventory::*`, `clay_js_doc_registry::*` including
      `generated_registry_is_current`, and `clay_js_facade_layout::*`); the full
      `--test protocol` (152 passed) and `--test security` (227 passed, including
      `rust_visibility_api_mapping` and `primitives_docs`) runs from task 5 still
      hold because no code or registry source changed since.

- [x] Update or verify the code wiki after implementation
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
  - Evidence (2026-09-20): covered by task 5's sweep and guards — 11 evergreen
    pages corrected, no wiki page references a deleted module
    (`FoldingRangeRegistry`/`publish_ranges`/`cross_domain`/`ScopedLockTarget`
    all absent from `docs/wiki/`), the A4 locking-model record lives on
    `docs/wiki/flows/document-leases-and-region-locks.md`, validation intactness is
    documented on `docs/wiki/modules/folding-ranges.md` (step 1 still names
    `validate_folding_publication`’s permission/version/provenance/budget checks),
    and `wiki_navigation_is_complete_and_current_page_paths_resolve` passes in the
    152-test `--test protocol` run. No further wiki edit needed after task 6.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- **`application.quit` inventory row points at a nonexistent op file** (found by
  task 6's reference-integrity check): `api-inventory.toml` lists
  `deno_op_path = "src/server/ops/application.rs::op_clay_application_quit"`, but
  `src/server/ops/application.rs` does not exist and no
  `op_clay_application_quit` is defined anywhere; the entry's `status = "planned"`
  means the inventory existence guard skips it. It is the only planned row with an
  unmarked bogus path — the other seven (`packages.*`) prefix theirs with
  `planned:`. Either mark planned paths the same way or point the row at the real
  client-command owner (`src/client_commands.rs::EditorClientCommand`).
  Priority: low. Delegated → plan 136 Further Actions (inventory-guard task there
  is still open).
- **`PackageLoadEntryAllowlist::revoke_package` has no production caller** (found
  by task 4's re-armed lint): a disabled/revoked package's recorded
  `clay://packages/...` module entries are never withdrawn, so they remain
  resolvable in the current runtime generation. Wire it into the package
  disable/revoke path (needs a package → runtime hook), or revalidate package
  enablement at module resolution and delete the method. Priority: high
  (security-adjacent). The method keeps a narrow `allow(dead_code, reason = ...)`
  until then. Delegated → plan 136 Further Actions (owns the grant/revoke/disable
  lifecycle verbs).
- **Git stale-polling surface has no production caller** (found while making the
  wiki truthful in task 5): `GIT_STATUS_POLL_INTERVAL`,
  `GitStatusCache::refresh_stale_workspace`, and
  `GitDiscoveryService::discover_workspace_statuses` are `#[cfg(test)]` since plan
  131, and the live paths are `list_cached` + explicit `refresh_root`
  (`clay:git`, `git.refreshStatus`). Either wire a real poll (control-center/Git
  panel interval) or delete the polling surface with its tests — do not leave the
  wiki describing a poll that never runs. Priority: low (docs now record reality).
  Delegated → plan 134 Further Actions (hygiene pass).
- **Native Rust client queue is test-only for three enqueue paths**
  (`enqueue_viewport_render_request`, `enqueue_command_intent`,
  `enqueue_save_document`), now `#[cfg(test)]`: the React frontend emits those
  protocol frames itself. Decide whether the Rust client keeps those capabilities
  (and the tests) or the remaining queue surface is retired with them.
  Priority: medium (test weight only). Delegated → plan 133 Further Actions
  (`src/client/tests.rs` extraction).
- Left as-is (already justified in code, out of this plan's scope):
  `src/server/language_server.rs::revoke_for_package` carries its own
  `ponytail`/Phase-18.21 note, `src/server/behavior.rs::validate_message_version`
  keeps its deliberate `cfg_attr(not(test), allow(dead_code))`, and the
  `#[cfg(windows)]` security-descriptor allow stays (Windows-only, unverifiable
  on the Linux primary host).
