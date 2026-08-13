# Post-Stability Xilem Compatibility Spike

Prerequisites: Plans 086–090 complete, full Linux gate green, modernized Masonry baseline measured, and no unresolved P0/P1 visual/accessibility defect.

This is an experiment, not an adoption plan. Current recommendation remains **do not adopt Xilem in production now**. Production adoption requires a separate evidence-backed decision log and explicit user approval after this spike.

Source review: Xilem evaluation in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`.

## Objectives

- Determine whether Xilem 0.4 can compose one low-frequency Clay-owned shell surface while preserving the existing bespoke Masonry editor as an opaque retained widget.
- Prove or disprove event-loop, focus, AccessKit, theme-token, command-routing, state-ownership, and performance compatibility.
- Keep editor canvas, pane/document hot paths, package SDUI, and server session authority out of migration scope.
- Delete the experiment if it needs duplicate state ownership, cannot host the editor cleanly, or regresses hot paths.

## Expected Outcome

- A bounded report with reproducible source/commands/measurements recommends reject, defer, or propose a narrowly scoped future adoption.
- Any spike source lives outside production paths and is deleted if acceptance fails; Clay's production `Cargo.toml` remains free of Xilem unless a later approved plan adopts it.
- If technically viable, one noncritical surface demonstrates unified event loop, focus/accessibility, current theme configuration, command routing, and opaque editor hosting without typing-triggered full shell rebuild.
- No claim of production readiness is made from Xilem's alpha/experimental status or a single demo.

## Tasks

- [ ] Re-resolve exact Xilem/Masonry documentation and pin the spike matrix
  - Acceptance Criteria:
    - Functional: Confirm current crates and exact dependency compatibility at execution time; record Clay's locked Masonry/winit/Rust versions and Xilem's matching requirements/APIs.
    - Performance: Documentation/version inspection adds no production dependency/build.
    - Code Quality: Use released-tag source/rustdoc, not `main` examples when APIs differ; record unsupported embedding constraints explicitly.
    - Security: Run `cargo audit`/license/source review for the isolated spike graph before executing code; no dependency enters production lockfile by this task.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/linebender/xilem`: Xilem is a reactive layer over Masonry; experimental status and Masonry backend.
      - `cargo info xilem`: current 0.4.0, Rust 1.88, docs.rs/repository.
      - Exact `xilem-0.4.0` registry `README.md`, `ARCHITECTURE.md`, `Cargo.toml`, `examples/external_event_loop.rs`, `src/widget_view.rs`.
      - Exact local Masonry 0.4.0 rustdoc/source generated separately; Clay depends on `masonry = 0.4.0`, `masonry_winit = 0.4.0`.
      - Xilem 0.4 depends on `masonry = 0.4.0`, `masonry_winit = 0.4.0`, `winit = 0.30.12`; Clay locks winit 0.30.13.
    - Options Considered:
      - Track Xilem `main`: rejected; unreleased API churn and version mismatch.
      - Pin current released 0.4 line in isolated spike: chosen.
      - Upgrade Clay's UI stack first: rejected; broad migration outside spike.
    - Chosen Approach:
      - Re-run `ctx7`, `cargo info`, `cargo tree`, and version-exact local rustdoc at execution; stop if one released dependency graph cannot resolve without production upgrades.
    - API Notes and Examples:
      ```bash
      npx ctx7@latest library Xilem "<spike question>"
      npx ctx7@latest docs /linebender/xilem "<exact coexistence question>"
      cargo info xilem
      cargo tree -p <spike-crate>
      ```
    - Files to Create/Edit:
      - `plans/091-Post-Stability-Xilem-Compatibility-Spike.md`: execution-time version/API matrix.
      - No production files.
    - References:
      - `https://docs.rs/xilem/0.4.0`, exact downloaded registry source.
  - Test Cases to Write:
    - Dependency matrix resolves one Masonry/winit line; report fails gate on duplicate major/minor UI stack or required production upgrade.

- [ ] Define time box, isolated worktree/crate, and hard stop criteria
  - Acceptance Criteria:
    - Functional: Spike runs in an isolated branch/worktree or ignored temporary crate with one reproducible command; production source/manifest/lockfile remain unchanged until decision.
    - Performance: Set explicit engineering time box and benchmark cases before implementation; no open-ended migration.
    - Code Quality: Spike contains only one selected shell surface, one editor adapter, and measurement/tests; no generalized compatibility framework.
    - Security: Fixture data only; no package runtime/filesystem/network/shell/AI authority; temporary files/worktree cleanly removable.
  - Approach:
    - Documentation Reviewed:
      - Ponytail/YAGNI guidance; project authority/UI patterns; Plan 090 final ownership map.
    - Options Considered:
      - Add optional `xilem` feature to production crate: rejected; contaminates product graph before adoption.
      - Separate experimental crate/worktree reusing Clay source via minimal explicit adapter: chosen.
    - Chosen Approach:
      - Pin stop criteria: duplicate app/editor state, second event loop/window required for target integration, focus/a11y break, typing rebuild, incompatible theme flow, or meaningful baseline regression → reject/delete.
    - API Notes and Examples:
      ```text
      experiments/xilem-shell-spike/  # isolated branch only; deleted on reject
      ```
    - Files to Create/Edit:
      - Experimental `Cargo.toml`, `src/main.rs`, and README in isolated worktree/crate only.
      - This plan: time box and stop criteria.
    - References:
      - Audit Xilem spike acceptance criteria.
  - Test Cases to Write:
    - Clean-tree check proves production `Cargo.toml`, `Cargo.lock`, `src/`, packages, and public docs unchanged before decision.

- [ ] Review Clay UI catalog and choose one noncritical shell surface
  - Acceptance Criteria:
    - Functional: Select exactly one low-frequency Clay-owned candidate—welcome/onboarding, settings, package management, or static inspector—based on current ownership and primitive fit; explicitly exclude editor canvas, client/server edit synchronization, pane document hot paths, package SDUI, and shell/tab infrastructure.
    - Performance: Candidate does not rebuild on ordinary typing and has a measurable current Masonry baseline.
    - Code Quality: Map existing Clay token/components/actions to Xilem views without inventing a second public component catalog.
    - Security: Candidate introduces no new authority and continues to emit existing inert command intents.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`, component/token catalogs, `docs/reference/ui-components.md`.
      - `npx ui-skills start`; selected accessibility/design guidance.
      - Project patterns `ui-modernization.md`, `package-ui-layout.md`, `authority-boundaries.md`.
    - Options Considered:
      - Migrate editor or shell root first: rejected; highest flexibility/hot-path/state risk.
      - Welcome/settings static surface: preferred due low frequency and bounded state.
    - Chosen Approach:
      - Pick one candidate after Plan 090 ownership map; default to welcome state if it can coexist without changing pane/editor owner.
    - API Notes and Examples:
      ```text
      Xilem candidate state = projection of existing Clay state
      actions = existing command IDs
      styling = existing resolved Clay tokens
      ```
    - Files to Create/Edit:
      - This plan: candidate/reuse matrix.
      - Experimental source only.
    - References:
      - Plans 087–088 final catalog and baseline evidence.
  - Test Cases to Write:
    - Candidate state/action/theme matrix has one owner each and no new package-facing kind.

- [ ] Build the smallest custom-widget adapter around the existing editor
  - Acceptance Criteria:
    - Functional: One Xilem view tree hosts the selected shell surface and the existing editor widget as an opaque Masonry child in the same window/event loop, or the spike records that released APIs cannot do so and stops.
    - Performance: Typing/editor paint does not recreate/rebuild reactive shell subtree; adapter adds no per-keystroke cloning of document/UI state.
    - Code Quality: Implement the minimum custom `View`/`WidgetView` adapter using Xilem `ViewCtx::create_pod`/`Pod` and Masonry widget identity; no parallel editor implementation or state synchronization layer.
    - Security: Existing editor/client/server authority, native input, clipboard, IME, accessibility, and command routing remain unchanged; no editor state is handed to package JS/Xilem callbacks.
  - Approach:
    - Documentation Reviewed:
      - Exact Xilem 0.4 `View`, `WidgetView`, `Pod`, `ViewCtx`, `AnyView`; `external_event_loop` note that custom embedding needs more design.
      - Exact Masonry 0.4 `Widget`, `NewWidget`, action/event/accessibility contracts.
    - Options Considered:
      - Run Xilem in a separate window alongside Clay: insufficient for target question; reject as compatibility proof.
      - Reimplement editor as Xilem views: rejected.
      - Minimal custom view wrapping existing Masonry widget: chosen if released API supports stable identity/rebuild.
    - Chosen Approach:
      - First prove opaque child identity with a tiny mock widget, then substitute current editor adapter; stop on ownership/event-loop duplication.
    - API Notes and Examples:
      ```rust
      // Spike-only shape; exact signatures must follow Xilem 0.4 rustdoc.
      impl<State> View<State, Action, ViewCtx> for ExistingEditorView { /* build/rebuild/message */ }
      ```
    - Files to Create/Edit:
      - Experimental adapter/view/app files only.
    - References:
      - `xilem-0.4.0/src/widget_view.rs`, `src/view_ctx.rs`, `src/pod.rs`, `examples/external_event_loop.rs`.
  - Test Cases to Write:
    - Stable child WidgetId across unrelated shell state updates; input/IME/focus/actions reach existing editor; editor state is not duplicated in Xilem app state.

- [ ] Prove unified event loop, focus, AccessKit, theme, and command routing
  - Acceptance Criteria:
    - Functional: Selected surface and editor share one window/event loop; Tab/Shift-Tab/Escape/focus restore, AccessKit tree, live status, and existing command actions work across boundary.
    - Performance: Theme/action/focus updates cause targeted rebuilds only; no full root rebuild on typing, caret blink, paint, or server edit ack.
    - Code Quality: Theme adapter reads existing cached `ResolvedUiTheme`/semantic roles; no copied palette or Xilem-only configuration/state owner.
    - Security: `theme.setTheme`/`setTypography` remain current user authority; package action provenance and server/native command checks remain intact.
  - Approach:
    - Documentation Reviewed:
      - Xilem 0.4 app/driver/external-event-loop APIs and AccessKit-through-Masonry architecture.
      - Plans 086/088 stable accessibility and theme contracts.
      - Decision `2026-08-14-0331-ui-modernization-preserves-theme-configuration`.
    - Options Considered:
      - Xilem default styling independent of Clay: rejected.
      - One adapter from existing resolved tokens/properties: chosen.
    - Chosen Approach:
      - Treat Clay state/tokens/commands as source of truth; Xilem is only a view projection for candidate shell UI.
    - API Notes and Examples:
      ```text
      ActiveTheme → existing ResolvedUiTheme → Xilem/Masonry properties
      Xilem action → existing inert Clay command ID → current dispatcher
      ```
    - Files to Create/Edit:
      - Experimental integration/tests only.
      - This plan: compatibility findings.
    - References:
      - `docs/reference/clay-js-api/theme/set-theme.md`, `set-typography.md`.
  - Test Cases to Write:
    - Dark/light/typed override and font-scale changes; focus traversal/restore; AT tree consumer validation; existing action dispatch; no internal anchor/widget exposure.

- [ ] Compare cold start, typing, tab switch, and candidate updates against baseline
  - Acceptance Criteria:
    - Functional: Run identical fixture/commands for current Masonry and spike; collect cold start, typing/local paint proxy, tab switch, candidate state update, allocations/rebuild counts, and binary/build size.
    - Performance: Acceptance requires no meaningful typing/tab regression, no reactive rebuild on typing, and bounded candidate-only rebuilds; exact threshold set from Plan 089 baseline/noise evidence before execution.
    - Code Quality: Reuse existing benchmarks/metrics where possible; spike instrumentation stays experimental.
    - Security: Benchmark fixtures are inert and isolated; no external service/network/user documents.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md`, Plan 089 final baselines; Xilem `memoize` guidance only if measured eager view rebuilding requires it.
    - Options Considered:
      - Claim performance from architecture/docs: rejected.
      - Direct before/after fixture measurement: chosen.
    - Chosen Approach:
      - Instrument rebuild counts first; apply `memoize` only if candidate state has a clear pure key and measurement proves need.
    - API Notes and Examples:
      ```text
      typing event → editor metric changes; candidate Xilem rebuild count must remain unchanged
      ```
    - Files to Create/Edit:
      - Experimental benchmark/metric files.
      - `code-reviews/2026-08-14-xilem-compatibility-spike.md`: measurement table.
    - References:
      - Exact Xilem 0.4 architecture notes on eager view evaluation and `memoize`.
  - Test Cases to Write:
    - Fixed sample/repeated runs, rebuild counter assertions, baseline noise comparison, build size/dependency graph.

- [ ] Perform visual screenshot and accessibility review of spike UI
  - Acceptance Criteria:
    - Functional: Capture candidate/editor default, interaction, focus, error/recovery, dark/light/typed override, narrow/wide, and font-scale states; inspect unified accessibility tree.
    - Performance: No visible rebuild flicker, focus jump, clipping, or editor input lag.
    - Code Quality: Store evidence separate from production modernization artifacts and record every mismatch.
    - Security: Fixture-only data; no absolute path/secret exposure; package provenance remains clear.
  - Approach:
    - Documentation Reviewed:
      - `ui-visual-review.md`, Plan 087 harness, computer-use-linux workflow.
    - Options Considered:
      - Demo screenshot only: rejected.
      - State/theme/accessibility comparison to current Masonry baseline: chosen.
    - Chosen Approach:
      - Use same dimensions/themes/fixtures as Plan 088 and `get_app_state` before/after interactions.
    - API Notes and Examples:
      ```text
      current Masonry artifact ↔ spike artifact, same state/theme/dimensions
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-14-xilem-spike/*.png`.
      - Spike report findings.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Visual/theme/focus/roles/names/states/modal/announcement comparison matrix.

- [ ] Decide reject/defer/propose and delete failed spike source
  - Acceptance Criteria:
    - Functional: Report every acceptance criterion pass/fail, dependency/API risk, ownership result, measurements, screenshots, and recommendation. Failed/rejected spike source and dependencies are deleted; only report/evidence remain.
    - Performance: Recommendation cites measured values/rebuild counts, not expected framework performance.
    - Code Quality: “Propose adoption” names one narrow production scope and migration/rollback boundary; it does not silently modify production.
    - Security: Recommendation covers alpha status, dependency advisories, accessibility/focus, package authority, and theme configuration; adoption waits for explicit user approval and decision log.
  - Approach:
    - Documentation Reviewed:
      - `create-decision-log` skill; audit recommendation and all spike evidence.
    - Options Considered:
      - Reject: cannot host editor or duplicates state/event loop.
      - Defer: technically viable but alpha/API/performance/maintenance risk outweighs current benefit.
      - Propose narrow adoption: all gates pass and measured boilerplate/ownership benefit is material.
    - Chosen Approach:
      - Default to reject/defer unless every hard criterion passes; ask user for explicit approval before any production adoption decision/log/plan.
    - API Notes and Examples:
      ```text
      Outcome: reject | defer | propose narrow adoption
      Production changes: none in this plan
      ```
    - Files to Create/Edit:
      - `code-reviews/2026-08-14-xilem-compatibility-spike.md`.
      - Delete experimental source on reject/defer unless user explicitly asks to retain a branch.
      - New decision log only after explicit user approval of production direction.
    - References:
      - `.agents/skills/create-decision-log/SKILL.md`.
  - Test Cases to Write:
    - Clean-tree/dependency check proves rejected/deferred spike leaves production manifest/lock/source unchanged.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Spike adds no production Clay JS API; existing candidate actions/theme/typography IDs remain unchanged.
    - Performance: No JS facade becomes reactive view state or typing dependency.
    - Code Quality: Experimental helpers stay outside public Rust/API inventory; report records no-new-API result.
    - Security: No Xilem/Masonry handle, widget/view, raw style, event-loop, or debug action reaches packages/configuration.
  - Approach:
    - Documentation Reviewed:
      - API boundary/naming/schema/documentation project patterns.
    - Options Considered:
      - Expose spike controls: rejected.
      - Existing APIs only: chosen.
    - Chosen Approach:
      - Run production visibility/doc-registry checks if any production file was touched; otherwise verify unchanged.
    - API Notes and Examples:
      ```text
      No production API expected.
      ```
    - Files to Create/Edit:
      - None expected; report records verification.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
  - Test Cases to Write:
    - Production API inventory/registry unchanged.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Existing `setTheme`, `setTypography`, keybindings, and candidate behavior configuration work unchanged in spike; no Xilem-specific config.
    - Performance: Config reload updates cached projection atomically without editor rebuild on unrelated changes.
    - Code Quality: One Clay config model; no parallel framework settings.
    - Security: Existing validation/allowlists/authority remain source of truth.
  - Approach:
    - Documentation Reviewed:
      - `configuration-system.md`, `ui-modernization.md`, current theme API docs/example.
    - Options Considered:
      - Xilem-specific appearance config: rejected.
      - Adapt existing Clay state: chosen.
    - Chosen Approach:
      - Treat configuration parity as hard spike acceptance.
    - API Notes and Examples:
      ```javascript
      setTheme("@clay/theme-gruvbox-material-dark");
      ```
    - Files to Create/Edit:
      - Experimental adapter/tests only; no production config docs expected.
    - References:
      - `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`.
  - Test Cases to Write:
    - Theme/typography reload parity, invalid theme rejection, no duplicate state.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Execute relevant modules 01, 02, 04, 07, 10, 11 for the spike; production manual plan changes only if an approved adoption later changes product behavior.
    - Performance: Record comparative feel/metrics and rebuild observations.
    - Code Quality: Spike evidence stays in report; do not pollute production manual plan with rejected experimental behavior.
    - Security: Verify no new authority/debug surface and fixture isolation.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`.
    - Options Considered:
      - Add permanent steps before adoption: rejected.
      - Execute existing steps against spike and record report: chosen.
    - Chosen Approach:
      - Update `test-plan/` only if production adoption is separately approved; otherwise record execution in spike report.
    - API Notes and Examples:
      ```text
      Existing manual step IDs + “spike” result column in report
      ```
    - Files to Create/Edit:
      - `code-reviews/2026-08-14-xilem-compatibility-spike.md`.
      - `test-plan/**` only under later approved production adoption.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
  - Test Cases to Write:
    - Existing manual scenarios run against baseline and spike.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: If spike is rejected/deferred and production unchanged, explicitly verify wiki unchanged; if a later approved adoption occurs, update wiki only under that production plan.
    - Performance: Spike report, not wiki, owns experimental measurements.
    - Code Quality: No dead experimental wiki page or index link.
    - Security: No experimental authority model is documented as production fact.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Document experiment in production wiki: rejected unless adopted.
      - Verify unchanged; keep report in code-reviews: chosen.
    - Chosen Approach:
      - Perform final wiki audit and record “unchanged” when source is deleted.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md  # unchanged for rejected/deferred spike
      ```
    - Files to Create/Edit:
      - None expected for reject/defer; production wiki only in later approved adoption plan.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
  - Test Cases to Write:
    - Wiki index contains no link to deleted experimental implementation.

## Compromises Made

- Spike is limited to one low-frequency surface and one opaque editor adapter. It cannot prove whole-app migration economics, only whether coexistence is technically and operationally credible.

## Further Actions

- Production adoption, if recommended, requires explicit user approval, a decision log, a new numbered plan, and a rollback-compatible narrow scope. Otherwise close Xilem evaluation as rejected/deferred with no production dependency.
