# Audit Remediation: Responsibility-Preserving Refactor

Prerequisites: Plans 086–089 complete and full Linux baseline green. Do not mix this refactor with visual redesign or dependency migration.

Source review: P2-1, P2-3, P2-4 and large-file evidence in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`.

Scope: Extract existing responsibilities into plain modules/functions. Preserve authority, protocol, UI behavior, hot paths, and public APIs. No one-implementation traits, factories, plugin architecture, or “future flexibility.”

## Objectives

- Make server connection/runtime, editor/shell, package validation, and app launch/event routing reviewable by responsibility.
- Give command-centre lifecycle/focus/geometry/accessibility one legible presentation owner without changing server-owned session authority.
- Reduce high-cost source-text test churn in favor of compact reusable helpers and behavioral checks.
- Prove behavior/performance/security parity after every extraction.

## Expected Outcome

- Large orchestration files become smaller coordinators with named sibling modules aligned to current ownership.
- Connection cleanup, runtime bootstrap/validation, shell overlays/accessibility, package validators, and app launch/event routing each have one obvious owner.
- Editor typing/paint and server authority boundaries remain unchanged; no protocol/schema/package/API migration occurs.
- Tests cover behavior parity and no duplicate state/cleanup paths remain.

## Tasks

- [ ] Establish module/ownership map, UI primitive constraints, and extraction budgets
  - Acceptance Criteria:
    - Functional: Map state, behavior, execution, persistence, validation, cleanup, and cross-module calls for `connection`, `js_runtime`, `editor/surface`, `server/mod`, `masonry_shell`, `packages/record`, and `main`; include Driver/ClayShellWidget/EditorWidget/PaneDocumentView/PackageOverlayHost/server menu sessions.
    - Performance: Identify typing/paint/layout/IPC/runtime hot paths and current benchmark/test guards before moves.
    - Code Quality: Set per-task extraction boundaries and stop conditions; every new module has at least two coherent responsibilities/callers or owns one state machine, not arbitrary line-count slicing.
    - Security: Mark canonical document/workspace/file/package/runtime/connection identity and cleanup authority; extraction cannot relocate or duplicate enforcement.
  - Approach:
    - Documentation Reviewed:
      - Relevant wiki pages: `server-ipc-skeleton.md`, `embedded-js-runtime.md`, `masonry-editor.md`, `masonry-shell.md`, `pane-document-views.md`, `transient-menu-round-trip.md`, `package-loading.md`.
      - `.agents/skills/clay-ui/SKILL.md` and UI catalogs for shell/editor moves.
      - Project patterns `authority-boundaries.md`, `package-runtime-trust-domains.md`, `package-ui-layout.md`, `protocol-and-performance.md`, `planning-checklist.md`.
    - Options Considered:
      - Split by file size alone: rejected.
      - New service/trait architecture: rejected.
      - Move existing cohesive responsibilities into private sibling modules: chosen.
    - Chosen Approach:
      - Write a one-page ownership graph and dependency direction; each later task extracts one seam and runs focused parity checks before continuing.
    - API Notes and Examples:
      ```text
      coordinator → private responsibility module → existing state/typed result
      no new trait unless multiple current implementations already require one
      ```
    - Files to Create/Edit:
      - `docs/development/architecture-ownership.md` (or existing architecture doc if suitable): one-page map.
      - This plan: exact extraction/file budget table.
    - References:
      - Audit large-file line counts and P2-3.
  - Test Cases to Write:
    - Ownership review: every mutable state/cleanup path has exactly one named owner before extraction.

- [ ] Extract connection dispatch families and one lifecycle/cleanup owner
  - Acceptance Criteria:
    - Functional: Dispatch families (documents/workspace/runtime/menu/tab/package as current code dictates) move to private modules; connection loop remains coordinator; all exit paths invoke one cleanup owner exactly once.
    - Performance: No new global lock/channel/allocation or dispatch indirection in edit acknowledgement; per-document ordering and bounded queues unchanged.
    - Code Quality: Plain functions/structs over existing state; no god context clone, trait hierarchy, or duplicated message match.
    - Security: Canonical connection identity, OutputRouter authorization, grants, leases, close-document, session revocation, and active-connection cap remain enforced at same or stronger shared seams.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection.rs`, Plan 060/061 review remediation evidence, `authority-boundaries.md`, `protocol-and-performance.md`.
    - Options Considered:
      - One module per protocol variant: rejected as fragmentation.
      - Cohesive dispatch families plus lifecycle owner: chosen.
    - Chosen Approach:
      - Move code without semantic edits first; remove duplicate cleanup only after parity tests prove all exits.
    - API Notes and Examples:
      ```rust
      async fn dispatch_menu_message(ctx: &mut ConnectionCtx<'_>, message: MenuMessage) -> Result<DispatchOutcome, ConnectionError>;
      fn cleanup_connection(state: &ServerState, identity: ConnectionIdentity);
      ```
    - Files to Create/Edit:
      - `src/server/connection.rs`: coordinator.
      - `src/server/connection/{documents,workspace,runtime,menus,tabs,packages,lifecycle}.rs` (tentative; consolidate families when small).
      - `src/server/mod.rs`: module declarations/visibility only.
    - References:
      - `code-reviews/2026-07-19-comprehensive-codebase-review.md`, Plan 060/061.
  - Test Cases to Write:
    - Behavior-parity matrix for each message family; disconnect/error/lag/reload/shutdown cleanup exactly once; identity and authorized result routing denial.

- [ ] Extract JavaScript runtime source, validation, and trust-domain bootstrap responsibilities
  - Acceptance Criteria:
    - Functional: Runtime service remains facade/coordinator while module source loading, evaluation validation, trusted/adopted bootstrap, and generation result assembly have explicit private owners.
    - Performance: Runtime evaluation/install timings and heap/time bounds do not regress; ordinary typing never waits on runtime work.
    - Code Quality: Preserve existing concrete types and two-domain state; no generic runtime framework, DI container, or dynamic plugin interface.
    - Security: Bundled trust remains exact provenance/integrity; adopted runtime lacks internal ops/modules; cross-domain values remain typed/bounded/inert; revocation/generation cleanup unchanged.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/embedded-js-runtime.md`, `persistent-runtime-hardening.md`, `third-party-runtime-authority.md`.
      - `package-runtime-trust-domains.md`, `extensions-and-ai.md`.
    - Options Considered:
      - Separate crate: rejected; compile/API overhead without current need.
      - Private submodules within `server/js_runtime/`: chosen.
    - Chosen Approach:
      - Extract in order: pure validation/source helpers, bootstrap builders, then generation assembly; keep service/channel ownership in facade.
    - API Notes and Examples:
      ```text
      js_runtime/mod.rs → source.rs + validation.rs + trusted.rs + adopted.rs + generation.rs
      ```
    - Files to Create/Edit:
      - Convert `src/server/js_runtime.rs` to `src/server/js_runtime/mod.rs` only if Rust module move does not create noisy path churn; otherwise use named sibling files and `#[path]` temporarily only if required.
      - Private runtime submodules listed above (tentative).
    - References:
      - Decision `2026-07-21-0001-two-package-runtime-trust-domains.md`.
  - Test Cases to Write:
    - Existing config/load/reload/runtime-domain denial/adoption/revocation/stale-generation/timeout tests pass unchanged; module-source validation unit cases move with owner.

- [ ] Extract package-record contribution-family validators
  - Acceptance Criteria:
    - Functional: UI, mode/grammar, permissions/authority, extension point, behavior, and docs/API validators move into coherent private families while `assemble_package_record` preserves exact validation order/errors/atomicity.
    - Performance: Package validation remains bounded and off editor hot paths; no repeated manifest parse or cloned payload graph.
    - Code Quality: Reuse `ComponentCatalogError` and existing typed records; no validator trait/factory or language/package-specific branch.
    - Security: Validation stays host-authoritative before trusted runtime execution; oversized/raw/internal contributions still reject with same provenance-aware diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/record.rs`, package authoring guide, `package-runtime-trust-domains.md`, `package-ui-layout.md`.
    - Options Considered:
      - One validator file per field: rejected.
      - Contribution-family modules preserving assembly order: chosen.
    - Chosen Approach:
      - Extract pure helpers first, retain one top-level atomic assembly function and one error vocabulary.
    - API Notes and Examples:
      ```rust
      let ui = validate_ui_contributions(raw.ui, &context)?;
      let behavior = validate_behavior_contributions(raw.behavior, &context)?;
      ```
    - Files to Create/Edit:
      - `src/packages/record.rs`: assembly coordinator.
      - `src/packages/record/{ui,behavior,authority,language,documentation}.rs` (tentative; merge small families).
    - References:
      - `docs/wiki/modules/package-loading.md`, `package-primitive-gate.md`.
  - Test Cases to Write:
    - Existing valid package snapshots and exact rejection diagnostics; atomic failure; payload/depth/item caps; trusted/adopted provenance.

- [ ] Extract editor surface composition/input helpers without changing hot-path ownership
  - Acceptance Criteria:
    - Functional: Move cohesive movement/selection, completion/snippet, decoration/status, or geometry helpers only where current state ownership remains in `EditorSurface`; editor behavior is byte-for-byte equivalent at public boundaries.
    - Performance: Typing/local paint/layout proxies do not regress; no new allocation, dynamic dispatch, IPC, JS, or full-document work.
    - Code Quality: `EditorSurface` remains one state owner, not split into mirrored sub-state services; extract pure helpers/state machines with focused tests.
    - Security: Client remains non-authoritative for canonical documents/files/workspaces; stale/version/provenance checks remain before applying server results.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/masonry-editor.md`, editor movement/caret/completion pages; `protocol-and-performance.md`.
    - Options Considered:
      - Break `EditorSurface` into several communicating managers: rejected; duplicates mutable state.
      - Extract pure algorithms and cohesive state machines while retaining one owner: chosen.
    - Chosen Approach:
      - Use profiling/call graph to pick only seams that reduce review burden without adding cross-module choreography.
    - API Notes and Examples:
      ```rust
      // State remains on EditorSurface; helper operates on explicit borrowed fields.
      fn update_completion_state(state: &mut CompletionState, event: CompletionEvent) -> CompletionOutcome;
      ```
    - Files to Create/Edit:
      - `src/editor/surface.rs`.
      - Existing `src/editor/{movement,selection,snippet,accessibility}.rs` or one new cohesive module only when no owner exists.
    - References:
      - Audit P2-1 and P2-3.
  - Test Cases to Write:
    - Typing/edit/IME/selection/completion/decorations/status behavior parity; stale result and local-first invariants.

- [ ] Extract shell tab/window layer and overlay coordinator with one presentation owner
  - Acceptance Criteria:
    - Functional: Tab/window composition, overlay reconciliation, and command-centre presentation bridge become legible private modules; server still owns menu sessions while one client presentation owner controls geometry, focus restoration, visual host, and accessibility projection.
    - Performance: No duplicate overlay reconciliation, per-frame state mirroring, or extra full-tree invalidation; existing tab/pane/menu budgets hold.
    - Code Quality: Reuse shared virtual-a11y helper from Plan 086 and overlay primitives from Plans 087–088; no second state model or custom widget framework.
    - Security: Packages cannot request centered/internal anchors, mutate shell layout, or bypass server menu activation/provenance.
  - Approach:
    - Documentation Reviewed:
      - `masonry-shell.md`, `centered-command-centre-surface.md`, `transient-menu-round-trip.md`, `package-ui-layout.md`.
      - Clay UI catalogs and Plan 088 final contracts.
    - Options Considered:
      - Keep state mirrored across Driver/editor/host with docs only: rejected; duplication remains.
      - Move presentation bridge behind one retained client owner while preserving server session authority: chosen.
    - Chosen Approach:
      - Extract data movement/reconciliation first; delete duplicated mirrored fields only after lifecycle/focus tests pass.
    - API Notes and Examples:
      ```text
      server TransientMenuSession → client snapshot → OverlayPresentationOwner → PackageOverlayHost
      ```
    - Files to Create/Edit:
      - `src/masonry_shell.rs`, `src/masonry_editor.rs`, `src/masonry_pane_document.rs`.
      - `src/shell/overlay_coordinator.rs`, `src/shell/window_tabs.rs` (tentative).
      - `src/main.rs` Driver bridge callers.
    - References:
      - `decision-logs/2026-08-11-1711-command-centre-surface-path-mode-and-sequence-keybindings.md`.
  - Test Cases to Write:
    - Open/filter/select/cancel/reload/tab-switch/disconnect lifecycle, geometry, single host, modal input containment, focus restore, accessibility identity.

- [ ] Split app launch/CLI/window creation from event and action routing
  - Acceptance Criteria:
    - Functional: CLI parsing/launch, server/client startup, window creation, app event dispatch, and native dialog/action routing have explicit private owners; all current modes/help/endpoint behavior remain unchanged.
    - Performance: Event dispatch adds no allocation/dynamic registry/async hop on input/action path; startup remains within baseline.
    - Code Quality: Plain modules and direct matches; no command bus/factory/service locator.
    - Security: Endpoint directory ownership, dialog-to-server validation, connection identity, and no remote listener remain unchanged.
  - Approach:
    - Documentation Reviewed:
      - `src/main.rs`, `docs/development/launch-and-gui-smoke.md`, CLI docs/tests.
    - Options Considered:
      - Clap/new CLI dependency: rejected; unrelated.
      - Move existing parsing/launch/routing functions into private modules: chosen.
    - Chosen Approach:
      - Preserve `main` as thin composition root and current exhaustive matches as direct code.
    - API Notes and Examples:
      ```text
      main.rs → cli.rs + launch.rs + app_driver.rs + native_dialogs.rs
      ```
    - Files to Create/Edit:
      - `src/main.rs`.
      - `src/app/{cli,launch,driver,native_dialogs}.rs` or minimal equivalent (tentative).
    - References:
      - `docs/wiki/modules/server-ipc-skeleton.md`.
  - Test Cases to Write:
    - CLI help/mode parsing, endpoint safety, launch/restart/smoke fixtures, command action routing, dialog success/cancel/error.

- [ ] Replace redundant source-text assertions with compact helpers or behavioral tests
  - Acceptance Criteria:
    - Functional: Review `editor_performance_invariants.rs` and `rust_visibility_api_mapping.rs`; retain source-text checks only for unique absence/visibility contracts and replace duplicate prose needles with behavior/type/registry checks.
    - Performance: Test compile/run time and linked binary size do not regress; ideally decrease.
    - Code Quality: Delete more assertion boilerplate than added; one helper centralizes repeated file lookup/assertion diagnostics.
    - Security: Do not weaken trust-boundary visibility, no-hot-path, docs/API coverage, or denial checks.
  - Approach:
    - Documentation Reviewed:
      - Project patterns `maintenance-validation.md`, `documentation-as-code.md`, `doc-registry-tests.md`.
    - Options Considered:
      - Delete all static checks: rejected; some enforce otherwise unobservable contracts.
      - Keep all duplicated assertions: rejected.
      - Classify unique vs redundant and shrink surgically: chosen.
    - Chosen Approach:
      - Prefer compiler visibility tests, behavioral tests, generated registries, and small reusable source-policy helpers.
    - API Notes and Examples:
      ```rust
      assert_source_absent("src/editor/surface.rs", &["Deno.core", "std::fs::"]);
      ```
    - Files to Create/Edit:
      - `tests/editor_performance_invariants.rs`, `tests/rust_visibility_api_mapping.rs`.
      - Existing test support helper module if shared.
    - References:
      - Audit P2-4.
  - Test Cases to Write:
    - Mutation-style check for every retained unique contract; before/after test count, runtime, and source-line delta.

- [ ] Verify behavior, performance, security, and UI parity after each extraction
  - Acceptance Criteria:
    - Functional: Focused suites pass after each task; final all-target Linux gates and smoke fixtures match pre-refactor behavior.
    - Performance: No sustained regression in typing, runtime reload, menu filtering, tab switching, package validation, startup, or target shape.
    - Code Quality: `git diff --stat` shows responsibility moves/deletions rather than abstraction growth; no cyclic modules, duplicate owners, or unjustified public visibility.
    - Security: Existing package/runtime/IPC/file/workspace/connection/accessibility denial and cleanup suites remain blocking.
  - Approach:
    - Documentation Reviewed:
      - Plans 086–089 validation commands and `docs/development/performance.md`.
    - Options Considered:
      - Refactor all then test: rejected.
      - Per-seam focused checks plus final gate: chosen.
    - Chosen Approach:
      - Stop/revert any extraction whose only result is more indirection or whose parity cannot be demonstrated.
    - API Notes and Examples:
      ```bash
      scripts/check.sh quick
      scripts/check.sh full
      cargo bench --bench window_baselines --no-run
      cargo bench --bench runtime_sdui_baselines --no-run
      ```
    - Files to Create/Edit:
      - Changed source/tests from prior tasks; `docs/development/performance.md` only for measured evidence.
    - References:
      - Ponytail ladder and Karpathy surgical-change guidance.
  - Test Cases to Write:
    - All focused parity matrices plus required Linux gates.

- [ ] Perform visual screenshot and accessibility review of refactored UI paths
  - Acceptance Criteria:
    - Functional: Re-run Plan 088 representative shell/editor/menu/completion/dialog/tab/pane states; screenshots and accessibility trees match intended behavior.
    - Performance: No visible focus loss, overlay duplication, layout churn, or interaction delay.
    - Code Quality: Record artifact paths/findings; refactor-induced defect blocks completion.
    - Security: Labels remain sanitized and modal/package boundaries remain clear.
  - Approach:
    - Documentation Reviewed:
      - `ui-visual-review.md`, Plan 087 harness.
    - Options Considered:
      - Skip because “refactor only”: rejected; ownership moves touch presentation lifecycle.
      - Representative live regression review: chosen.
    - Chosen Approach:
      - Use same fixtures/dimensions/themes as pre-refactor for direct comparison and `get_app_state` checks.
    - API Notes and Examples:
      ```text
      same fixture + same theme + same window size → before/after evidence
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-14-plan090-refactor-parity/*.png`.
      - This plan: findings.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Focus/order/roles/names/modality/announcements and visual parity for moved paths.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory every moved/changed `pub` server function; stable JS API IDs/facades/docs remain unchanged; internal helpers are private/`pub(crate)`.
    - Performance: No new JS boundary or op is added for internal refactoring.
    - Code Quality: Rust paths in authoritative docs/registry are updated when moved; generated registry stays current.
    - Security: No internal state/context/validator/runtime/widget handle becomes public.
  - Approach:
    - Documentation Reviewed:
      - API boundary/naming/schema/documentation/registry project patterns.
    - Options Considered:
      - Preserve old Rust paths through public re-exports: rejected unless required by actual Rust public contract.
      - Update internal paths and docs, preserve JS surface: chosen.
    - Chosen Approach:
      - Run visibility mapping and doc registry after each major move.
    - API Notes and Examples:
      ```bash
      cargo test --test security rust_visibility_api_mapping::
      cargo run --bin update-doc-registry
      cargo test --test protocol clay_js_doc_registry::
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`, `docs/index.md`, generated registry where backing paths move.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
  - Test Cases to Write:
    - Every server public function maps to a documented API or becomes non-public; stable IDs/exports unchanged.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Existing `init.js` behavior, theme/typography/keybindings/packages/reload remain unchanged; no internal module boundary leaks into configuration.
    - Performance: Runtime reload timing remains within baseline and atomically installs one generation.
    - Code Quality: No new hidden config; moved Rust paths reflected only in backing metadata/docs.
    - Security: Trust domains/grants/config-root isolation remain unchanged.
  - Approach:
    - Documentation Reviewed:
      - `configuration-system.md`, `examples/init.js`, configuration runtime wiki/docs.
    - Options Considered:
      - Add switches for refactored modules: rejected.
      - Configuration-parity only: chosen.
    - Chosen Approach:
      - Run canonical example/config reload tests and record no-new-API result.
    - API Notes and Examples:
      ```bash
      node --check examples/init.js
      cargo test --lib server::runtime_generation_tests::example_configuration_loads_cleanly_and_applies_effects -- --exact
      ```
    - Files to Create/Edit:
      - Configuration docs/example only if backing path metadata changes.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`.
  - Test Cases to Write:
    - Canonical config behavior and atomic reload parity.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Execute affected modules 01, 02, 03, 04, 09, 10, 13, 14; record that no user-visible behavior changed. Add steps only if module ownership changes alter debugging commands/workflow.
    - Performance: Execute module 11 representative checks for moved hot paths.
    - Code Quality: Record pure-refactor rationale rather than inventing redundant steps.
    - Security: Include package/runtime/file/workspace/modal/accessibility negative checks covering moved enforcement.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` coverage matrix.
    - Options Considered:
      - Omit manual task silently: rejected by project rule.
      - Execute parity matrix and update only when needed: chosen.
    - Chosen Approach:
      - Preserve existing steps; record pass/fail evidence against stable IDs.
    - API Notes and Examples:
      ```bash
      scripts/check.sh full
      ```
    - Files to Create/Edit:
      - Affected `test-plan/*.md` only when execution finds changed instructions/coverage; this plan records evidence otherwise.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
  - Test Cases to Write:
    - Manual parity and negative checks listed above.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki ownership/data-flow pages match final modules and explain responsibilities, state, cleanup, extension points, and tests; index remains navigable.
    - Performance: Hot-path and benchmark ownership is documented.
    - Code Quality: Remove stale paths/ownership descriptions; link authoritative API docs rather than duplicating them.
    - Security: Trust/authority/validation/cleanup boundaries follow final source paths.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`; page template for substantial rewrites.
    - Options Considered:
      - Preserve old pages as historical: rejected if misleading.
      - Update once after final module layout: chosen.
    - Chosen Approach:
      - Synchronize relevant wiki pages and master index after all extraction/parity gates pass.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<responsibility>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md` and all relevant runtime/server/editor/shell/package pages identified in task 1.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
  - Test Cases to Write:
    - Manual wiki link/path/ownership review and documentation coverage tests.

## Compromises Made

- No production crate split. Private modules are the smallest boundary that improves reviewability without new package/API/link costs.

## Further Actions

- Reconsider a crate boundary only after module ownership stabilizes and measured compile/reuse benefits justify it.
