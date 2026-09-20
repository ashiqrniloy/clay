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

- [ ] Baseline gates, clippy inventories, and no-action decision records (R2, R4)
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

- [ ] Replace strict float comparisons in production shell/theme/layout code (D4)
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

- [ ] Adopt and implement the mutex poison policy (D5)
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

- [ ] Move hot-path blocking fs off the async runtime (P3)
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

- [ ] Snapshot swap only on change (R3)
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

- [ ] Execute and update the manual test plan (test-plan/)
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

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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

- [ ] Update or verify the code wiki after implementation
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

## Compromises Made
- To be filled after tasks are completed and tests pass.

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
