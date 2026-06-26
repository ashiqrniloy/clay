# All-Target Clippy Cleanup

## Objectives
- Make `cargo clippy --all-targets -- -D warnings` pass without crate-wide warning suppression.
- Keep Phase 18.7 runtime/package authority unchanged while cleaning lint debt.

## Expected Outcome
- Whole-repository clippy is usable as one command.
- Intentional staged primitives have narrow, documented local lint allowances.
- Mechanical Clippy fixes are applied where safe.

## Tasks

- [x] Establish all-target clippy baseline
  - Acceptance Criteria:
    - Functional: Run `cargo clippy --all-targets -- -D warnings` and capture failing lint groups.
    - Performance: Baseline command does not skip hot-path modules or benchmark/test targets.
    - Code Quality: Group warnings by root cause before editing.
    - Security: Security tests and authority-boundary modules stay in scope.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`
    - Options Considered:
      - Add crate-wide `allow(warnings)`. Rejected; hides future regressions.
      - Fix only library target. Rejected; user requested all targets.
    - Chosen Approach:
      - Run the requested command, let Clippy identify current debt, then use mechanical fixes plus narrow source-level justifications for staged primitives.
    - API Notes and Examples:
      ```bash
      cargo clippy --all-targets -- -D warnings
      ```
    - Files to Create/Edit:
      - `plans/032-All-Target-Clippy-Cleanup.md`: cleanup plan and evidence.
    - References:
      - `planning-checklist.md`; `authority-boundaries.md`.
  - Test Cases to Write:
    - Command baseline: fails before cleanup and identifies lint groups.
  - Completion Notes:
    - Baseline found dead staged SDUI/package-UI primitives, explicit hot-path function argument counts, large low-volume enums, a large cold error result, type-complexity, and mechanical style warnings.

- [x] Apply mechanical Clippy fixes
  - Acceptance Criteria:
    - Functional: Safe Clippy rewrites are applied.
    - Performance: No new allocations or IPC/file/runtime work added to hot paths.
    - Code Quality: Prefer standard Clippy suggestions over custom rewrites.
    - Security: No validation branches or authority checks are removed.
  - Approach:
    - Documentation Reviewed:
      - Clippy diagnostics from the baseline command.
    - Options Considered:
      - Hand-edit every suggestion. Rejected; slower and more error-prone for mechanical formatting/lint suggestions.
      - Use `cargo clippy --fix`. Chosen for machine-applicable changes only.
    - Chosen Approach:
      - Run `cargo clippy --fix --all-targets --allow-dirty --allow-staged -- -D warnings`, then manually resolve remaining lints.
    - API Notes and Examples:
      ```bash
      cargo clippy --fix --all-targets --allow-dirty --allow-staged -- -D warnings
      ```
    - Files to Create/Edit:
      - Mechanical fixes touched client behavior routing, registry/doc tests, package parsing helpers, JS runtime loader code, server security descriptor setup, and related test formatting.
    - References:
      - Baseline Clippy output.
  - Test Cases to Write:
    - `cargo fmt --check`
    - `cargo clippy --all-targets -- -D warnings`
  - Completion Notes:
    - Mechanical fixes applied and formatted.

- [x] Add narrow lint justifications for intentional staged/runtime shapes
  - Acceptance Criteria:
    - Functional: Remaining non-mechanical lints are either fixed or locally justified.
    - Performance: Hot-path paint and connection code is not wrapped in heap context bags just to appease argument-count lints.
    - Code Quality: No crate-wide `allow(warnings)` or blanket module suppression unrelated to the warning source.
    - Security: Runtime/package authority remains explicit; no authority is hidden in global context structs.
  - Approach:
    - Documentation Reviewed:
      - `authority-boundaries.md`
      - Existing SDUI/package UI wiki/reference docs describing staged primitives.
    - Options Considered:
      - Refactor hot-path functions into context structs. Rejected; speculative and larger than the lint debt.
      - Box large enum variants. Rejected for low-volume internal event channels without profiling evidence.
      - Delete staged SDUI/package-UI primitives. Rejected; they are documented validation/runtime structures for current and near-term phases.
    - Chosen Approach:
      - Add local `#[allow(..., reason = ...)]` only where the current shape is intentional: staged SDUI/package UI bridge structs, explicit connection/paint argument lists, low-volume large enums, and a cold protocol error result.
    - API Notes and Examples:
      ```rust
      #[allow(clippy::too_many_arguments, reason = "explicit authority wiring beats hidden context bag")]
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui.rs`: staged observability/package UI bridge allowances and paint helper argument-count justification.
      - `src/shell/package_ui.rs`: staged package UI runtime descriptor dead-code justification.
      - `src/server/sdui.rs`: static SDUI fixture dead-code justification.
      - `src/client/mod.rs`, `src/editor/layout.rs`, `src/server/connection.rs`: explicit hot-path/authority wiring argument-count justifications.
      - `src/masonry_editor.rs`, `src/main.rs`, `src/server/js_runtime.rs`: low-volume enum size justifications.
      - `src/server/behavior.rs`: cold `ServerMessage` rejection result justification.
      - `src/server/mod.rs`, `src/server/ui.rs`: narrow staged field/helper justifications and pointer-cast cleanup.
      - `src/packages/record.rs`, `src/perf/baselines.rs`, `src/editor/buffer.rs`, `src/protocol/codec.rs`, `tests/clay_js_api_inventory.rs`: direct lint fixes.
    - References:
      - `authority-boundaries.md`; `planning-checklist.md`.
  - Test Cases to Write:
    - `cargo clippy --all-targets -- -D warnings`: verifies no warnings remain.
    - `cargo test --all-targets`: verifies cleanup did not change behavior.
  - Completion Notes:
    - All remaining lints resolved with direct fixes or narrow local allowances carrying reasons.

- [x] Final verification and Phase 18.7 plan update
  - Acceptance Criteria:
    - Functional: `cargo clippy --all-targets -- -D warnings` and `cargo test --all-targets` pass.
    - Performance: No benchmark/perf guard tests are skipped.
    - Code Quality: Plan 031 follow-up task is marked complete with evidence.
    - Security: Phase 18.7 security tests remain in the all-target suite.
  - Approach:
    - Documentation Reviewed:
      - `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`
    - Options Considered:
      - Leave Plan 031 task open and point to this plan. Rejected; user asked to update the plan once done.
    - Chosen Approach:
      - Record this plan as the dedicated cleanup plan and update Plan 031 completion notes after checks pass.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      ```
    - Files to Create/Edit:
      - `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`: mark follow-up complete.
      - `plans/032-All-Target-Clippy-Cleanup.md`: final evidence.
      - `docs/wiki/modules/maintenance-validation.md` and `docs/wiki/index.md`: document the all-target validation gate and link it from the wiki index.
    - References:
      - Plan 031 follow-up task.
  - Test Cases to Write:
    - Final command sequence above.
  - Completion Notes:
    - `cargo fmt --check` passed.
    - `cargo clippy --all-targets -- -D warnings` passed with no issues.
    - `cargo test --all-targets` passed: 772 tests across 20 suites.
    - Wiki maintenance page added and linked from `docs/wiki/index.md`.

## Compromises Made
- Kept narrow local lint allowances for staged SDUI/package UI primitives and explicit hot-path/authority wiring instead of refactoring into speculative context structs or deleting documented near-term primitives.
- Did not run Criterion benchmarks; this cleanup is lint/test focused and does not change protocol benchmark code beyond a Clippy-suggested match simplification.

## Further Actions
- P1: Keep `cargo clippy --all-targets -- -D warnings` in the Phase 18.7 final verification checklist now that it passes.
- P2: Revisit staged SDUI/package UI `dead_code` allowances when dynamic package UI publication callsites are wired; remove allowances as callsites become live.
