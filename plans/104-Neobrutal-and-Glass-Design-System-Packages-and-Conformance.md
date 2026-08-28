# Neobrutal and Glass Design-System Packages and Conformance

Depends on `plans/101-UI-Design-System-Recipe-Foundation.md`, `plans/102-UI-Design-System-Activation-and-Frontend-Runtime.md`, and `plans/103-UI-Design-System-Component-and-Surface-Migration.md`.
Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.

## Objectives

- Ship Clay's default restrained utilitarian Neobrutal design language as a complete first-party UI design-system package, not a stock color theme.
- Ship a materially distinct Glass reference package that uses the same host components, slots, states, recipe schema, and active content-theme colors.
- Prove design-system replacement requires package/configuration changes only, not component source, DOM, behavior, accessibility, or a bundled color palette; every normal-rendering color remains supplied by the selected content theme.
- Harden contrast, forced-color, reduced-motion/transparency, unsupported-effect, performance, provenance, revocation, documentation, and review gates.
- Record the implemented default visual world in durable design documentation after bounded visual review.

## Expected Outcome

- `@clay/design-neobrutal` is the bundled default design system and provides complete fallback-compatible non-color recipes plus semantic theme-color-role mappings for every required component/surface/state.
- `@clay/design-glass` can be installed/adopted and selected in one `init.js` line, producing a clearly different but fully usable UI without host component edits or its own color theme.
- Both packages pass schema, inheritance, state completeness, color-source denial, theme/design-system cross-product, contrast, payload, performance, accessibility, screenshot, and package provenance checks.
- Neither package contains a palette or literal color. Unsupported blur or reduced-transparency environments receive solid, legible fallbacks from the active content theme.
- `PRODUCT.md`, final `DESIGN.md`, public references, package docs, catalogs, API docs, canonical example configuration, manual test plan, and code wiki agree with shipped behavior.

## Tasks

- [ ] Review reusable package/design primitives and close only generic data-only package gaps
  - Acceptance Criteria:
    - Functional: Inventory package manifest, bundled inventory, adoption, package record, data-only contribution, configuration selection, theme package, documentation, fixture, and conformance primitives before creating either package.
    - Performance: Confirm data-only design-system packages require no persistent JavaScript worker execution or runtime module evaluation when they contain no executable behavior.
    - Code Quality: Any missing capability is implemented generically for declarative data-only packages, not named for Neobrutal or Glass, and is documented/tested before package manifests depend on it; no package primitive permits design-system-owned color values.
    - Security: Preserve exact bundled integrity classification, adopted third-party provenance, two runtime trust domains, revocation, no automatic promotion, and no cross-domain V8 values.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/primitives/index.md`
      - `docs/reference/primitives/registry.md`
      - `docs/reference/primitives/package-loading.md`
      - `docs/wiki/modules/primitive-architecture.md`
      - `docs/wiki/modules/package-loading.md`
      - `docs/wiki/modules/ui-design-system-runtime.md`
    - Options Considered:
      - Add no-op JavaScript entries to data-only packages: works around loader assumptions but adds needless execution and files. Rejected if generic declarative-only package support is practical.
      - Add a design-system-specific loader path: duplicates package authority. Rejected.
      - Reuse generic manifest/package-record flow and permit no-entry declarative packages when existing invariants allow: selected.
    - Chosen Approach:
      - Document an inventory first. If current package records require executable entries, add the smallest generic optional-entry rule for contribution-only packages with validation that rejects runtime-only declarations lacking an entry.
    - API Notes and Examples:
      ```text
      package.json contribution only -> package record -> active selection
      no load entry -> no JavaScript execution -> no runtime op set required
      ```
    - Files to Create/Edit:
      - `docs/development/ui-design-system-package-primitive-review.md`: Existing primitive inventory, generic gaps, and final disposition.
      - `src/packages/manifest.rs`: Generic optional entry for data-only packages only if required.
      - `src/packages/record/mod.rs`: Generic contribution-only validation only if required.
      - `src/packages/service.rs`: Reuse normal enable/adopt lifecycle without runtime execution only if required.
      - `tests/package_loading.rs`: Declarative-only package tests.
      - `docs/reference/primitives/registry.md`: Add generic primitive only after implementation.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/package-manifest-single-source.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
  - Test Cases to Write:
    - Data-only bundled package: Loads contribution without evaluating JavaScript.
    - Data-only adopted package: Retains third-party provenance and no trusted-runtime presence.
    - Invalid no-entry package: Runtime behavior declaration without executable entry is rejected.
    - Revocation: Data-only contribution is withdrawn and active selection falls back.

- [ ] Establish and record the default Operate-mode visual direction before package implementation
  - Acceptance Criteria:
    - Functional: With `PRODUCT.md` present, run the Impeccable new-work direction workflow for a replacement-capable Operate-mode visual system; treat the user-approved restrained utilitarian Neobrutal default as binding, use the required concept seed/quality-bar process to challenge execution quality, and record the selected direction contract without changing product behavior or information architecture.
    - Performance: Direction defines dense long-session use, minimal non-functional motion, bounded effects, and no reduction in editor workspace.
    - Code Quality: Direction specifies thesis, own-world, task story, first representative viewport, form, finish condition, component/state grammar, and Glass contrast case; it defines no palette, literal color, stock theme, hardcoded font family, or point size, and instead names semantic active-theme roles where color contrast matters.
    - Security: Direction respects host-owned semantics, recipe schema, package provenance, no raw CSS/JSX, and no arbitrary renderer behavior.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/impeccable/reference/new-work.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `PRODUCT.md`
      - `docs/development/ui-design-system-recipe-matrix.md`
      - `docs/development/ui-design-system-css-audit.md`
    - Options Considered:
      - Preserve current CSS literally: misses opportunity to make default coherent and premium. Rejected.
      - Apply loud web Neobrutal tropes everywhere: harms density and long-session use. Rejected.
      - Use restrained mechanical Neobrutal structure with precise hierarchy, strong focus using the active theme's semantic accent/focus roles, compact density, and limited tactile state shifts: selected by approved direction.
    - Chosen Approach:
      - Follow Impeccable's mandatory direction process, keeping the approved aesthetic pinned. Store the direction contract and any approved visual decision artifacts under `.impeccable/`; do not write final `DESIGN.md` until implementation and review establish ground truth.
    - API Notes and Examples:
      ```text
      Default design system: restrained utilitarian Neobrutal
      Color source: currently selected content theme only
      Shape: mechanically consistent, predominantly sharp
      Density: expert desktop editor
      Motion: state feedback only
      Alternate proof: translucent layered Glass using theme colors with solid theme-role fallback
      ```
    - Files to Create/Edit:
      - `.impeccable/mocks/decision/**`: Direction artifacts produced by the approved workflow when image generation is available.
      - `.impeccable/surfaces/**`: Surface brief or direction contract if the workflow records one.
      - `docs/development/ui-design-system-visual-direction.md`: Implementation-facing summary of the approved direction and conformance contrast case.
    - References:
      - `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`
      - `.agents/skills/project-patterns/references/ui-skill-stack.md`
  - Test Cases to Write:
    - Direction contract review: Every non-color visual commitment maps to existing recipe/property capabilities or an explicitly approved generic gap; every color commitment maps to an existing semantic content-theme role.
    - Product-boundary review: No direction item changes host behavior, information architecture, package authority, or user typography ownership.

- [ ] Implement the complete restrained Neobrutal default design-system package
  - Acceptance Criteria:
    - Functional: Package provides complete resolved non-color recipes and semantic active-theme color-role mappings for all required component/surface/slot/state combinations, including empty/loading/error/recovery, validation, selection, modal, focus-visible, disabled, and narrow/wide behavior where visual treatment changes.
    - Performance: Default adds no blur, large scrolling shadows, perpetual animation, layout-property animation, or excess paint layers; payload/install size remains within Plan 102 budgets.
    - Code Quality: Manifest is declarative, recipe keys are generic, fallback is complete, comments/docs explain design intent, all color properties reference known theme roles, and no host source branch checks package identity.
    - Security: Package has no unnecessary permissions or executable entry, cannot inject CSS/JSX/selectors/scripts/palettes/literal colors, and bundled trust comes only from exact inventory/integrity.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/ui-design-system-visual-direction.md`
      - `docs/reference/ui-design-systems.md`
      - `docs/development/ui-design-system-recipe-matrix.md`
    - Options Considered:
      - Keep fallback in CSS and ship a partial default package: makes package replacement incomplete. Rejected.
      - Duplicate all current CSS values without design refinement: technically complete but fails approved default-quality goal. Rejected.
      - Express the approved restrained system through complete non-color recipes, typed non-color values, and semantic active-theme color-role references: selected.
    - Chosen Approach:
      - Author one declarative manifest and package docs. Use typed non-color values, consistent border/shape/motion rules, semantic theme-role mappings for focus/error/state color, and restrained physical active feedback. Keep all concrete colors content-theme owned and typography user-owned.
    - API Notes and Examples:
      ```json
      {
        "name": "@clay/design-neobrutal",
        "clay": {
          "permissions": [],
          "contributions": {
            "uiDesignSystem": {
              "version": 1,
              "values": {},
              "recipes": {}
            }
          }
        }
      }
      ```
    - Files to Create/Edit:
      - `packages/design-neobrutal/package.json`: Complete data-only manifest.
      - `packages/design-neobrutal/docs/index.md`: Purpose, selection, compatibility, accessibility, and customization limits.
      - `packages/design-neobrutal/README.md`: Package development/testing entry if project package convention uses one.
      - `src/packages/bundled-inventory.toml`: Exact bundled package identity/integrity classification.
      - `tests/theme_packages.rs` or `tests/design_system_packages.rs`: Package conformance and default selection tests.
    - References:
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Complete resolution: No required recipe falls through to an undocumented non-color value or color role.
    - Color denial: Package contains no hex/RGB/HSL/named colors, palette, or namespaced color value; every color property references an active-theme role.
    - State contrast: Text and UI affordance pairs pass required ratios across representative content themes.
    - Default identity: Fresh configuration selects built-in Neobrutal fallback/package deterministically.
    - No host branch: Source scan finds no `design-neobrutal` conditional outside bundled inventory/tests/docs.

- [ ] Implement the complete Glass reference design-system package with solid active-theme fallbacks
  - Acceptance Criteria:
    - Functional: Glass package changes material, layering, border geometry, shadows, blur/saturation, radii where approved, semantic theme-role mapping, and state treatment across the complete surface matrix while preserving legibility, focus, validation, and dense editor usability; it defines no concrete color.
    - Performance: Backdrop blur is limited to approved fixed/transient surfaces and bounded areas; no blur on large scrolling containers, no unbounded shadow layers, and no continuous decorative animation.
    - Code Quality: Glass uses the same manifest/schema/recipe keys as Neobrutal, provides explicit solid material fallback through semantic active-theme roles and reduced-transparency behavior, and needs no host component or CSS selector changes.
    - Security: Package remains declarative and third-party-compatible; values cannot escape validated effect/property bounds, declare literal/package-owned colors, or alter event/focus/role/layout authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/ui-design-system-visual-direction.md`
      - MDN `backdrop-filter` and reduced-motion documentation.
      - `docs/reference/ui-design-systems.md`
    - Options Considered:
      - Apply glass to every surface: visually noisy and GPU-expensive. Rejected.
      - Use opacity only without backdrop interaction: not a meaningful Glass proof. Rejected.
      - Bundle a Glass palette or stock theme: violates content-theme color ownership. Rejected.
      - Use bounded glass effects over active-theme color roles on overlays, panels, transient/raised surfaces with solid theme-role base regions and mandatory fallback: selected.
    - Chosen Approach:
      - Author complete recipes with glass material applied where hierarchy warrants it. Use only active-theme color-role references plus typed opacity/effects; preserve solid editor/content canvas, clear boundaries, one coherent radius rule, and strong contrast.
    - API Notes and Examples:
      ```js
      import { setDesignSystem } from "clay:theme";
      setDesignSystem("@clay/design-glass");
      ```
    - Files to Create/Edit:
      - `packages/design-glass/package.json`: Complete data-only manifest.
      - `packages/design-glass/docs/index.md`: Selection, material strategy, fallbacks, performance, and accessibility.
      - `packages/design-glass/README.md`: Package development/testing entry if project convention uses one.
      - `src/packages/bundled-inventory.toml`: Include only if Glass ships bundled; otherwise keep it as adopted test/reference package with exact source fixture.
      - `tests/theme_packages.rs` or `tests/design_system_packages.rs`: Glass resolution, effect bounds, and fallback tests.
    - References:
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Same schema: Glass introduces no component/slot/property identifier absent from shared catalog.
    - Color denial: Glass contains no palette, literal color, or package-owned color alias; translucent and solid states both derive from active-theme roles.
    - Unsupported blur: Solid active-theme fallback remains legible and layered.
    - Reduced transparency/motion: Effects collapse without hiding state.
    - Scroll performance: Large package/editor scroll regions contain no backdrop blur.

- [ ] Prove package-only replacement and source-independent conformance
  - Acceptance Criteria:
    - Functional: Automated fixtures select Neobrutal and Glass against the same runtime data, active content theme, and component trees; switching changes only non-color recipe identity/variables and semantic theme-role mappings while preserving DOM semantics, stable keys, action intents, tabs, panes, editor state, package state, and content-theme color authority.
    - Performance: Switching either direction remains within snapshot/install/render budgets and performs no full app/editor remount.
    - Code Quality: A source guard fails if host components branch on design-system package names or if package manifests require undocumented keys.
    - Security: Third-party-equivalent Glass fixture receives no extra op/module/Tauri authority and revocation restores fallback atomically.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/ui-design-system-recipe-matrix.md`
      - `frontend/src/sdui/registry.test.tsx`
      - `tests/package_ui_conformance.rs`
    - Options Considered:
      - Compare package JSON only: cannot prove runtime/UI replacement. Rejected.
      - Maintain separate component snapshots per package with manual comparison: easy to drift. Rejected.
      - Run the same semantic fixture twice and compare behavior/DOM invariants plus allowed computed-style differences: selected.
    - Chosen Approach:
      - Build one reusable conformance harness that loads any design-system package. Assert complete recipes, allowed non-color computed values, theme-role-only color sources, unchanged semantics/state, bounded metrics, and no package-name branching.
    - API Notes and Examples:
      ```text
      semantic fixture hash: unchanged
      accessibility tree: unchanged
      recipe revision: changed
      non-color computed visual properties: changed within allowed property set
      concrete color sources: active content-theme variables only
      ```
    - Files to Create/Edit:
      - `frontend/src/test/design-system-conformance.test.tsx`: Shared semantic/state/style harness.
      - `frontend/src/test/shell.test.tsx`: App-state preservation across switch.
      - `frontend/src/test/editor.test.tsx`: Editor-state preservation across switch.
      - `tests/package_ui_conformance.rs`: Package-name branch and complete recipe guards.
      - `tests/design_system_packages.rs`: Rust manifest/resolution/provenance conformance.
      - `docs/development/ui-design-system-conformance.md`: Harness contract and evidence format.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Same DOM/accessibility contract: Both packages expose identical roles/names/states.
    - State continuity: Input values, disclosure state, selection, modal focus, editor history, tabs, panes, and scroll survive switch.
    - Source independence: Host source contains no package-name visual branches.
    - Color independence: Both manifests contain no color values; under the same content theme, all computed colors trace to host-owned theme variables.
    - Cross-product: Both design systems pass against at least two materially different content themes, and theme-only switching recolors both without recipe reinstall.
    - Revocation: Adopted Glass withdrawal restores default with no stale variables.

- [ ] Harden validation, accessibility, performance, and package security across both systems
  - Acceptance Criteria:
    - Functional: Validate all required text/UI contrast pairs and interaction states across the Neobrutal/Glass by light/dark content-theme cross-product, compact/default/spacious density, and representative typography sizes; every concrete color must trace to the active theme.
    - Performance: Enforce measured limits for manifest bytes, resolved variables, install time, style writes, React renders, blur surfaces/area, shadow layers, and transition count/duration.
    - Code Quality: Error diagnostics identify package, recipe key, property, rejected value, expected type/bound, and fallback action; no warning-only path activates invalid state.
    - Security: Tests cover untrusted package classification, internal-op/module denial, stale generation, adoption/revocation, replacement rollback, shared third-party cohort disclosure, and absence of raw styling authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/ui-design-system-conformance.md`
      - `docs/development/performance.md`
      - `docs/reference/primitives/package-security.md`
    - Options Considered:
      - Validate only default package: permits unsafe adopted systems. Rejected.
      - Apply identical static limits to every property without surface context: simple but may allow large blur area or reject safe small effects. Rejected.
      - Combine schema-wide value bounds with host-known per-surface effect budgets: selected.
    - Chosen Approach:
      - Enforce generic server bounds, strict literal/package-color denial, and frontend host-surface budgets, with hard fallback/rejection rather than silent degradation. Pin performance budgets and color-source rules in tests and docs.
    - API Notes and Examples:
      ```text
      recipe error: modal.backdrop.rest.blur = 96 rejected
      expected: finite blur within host overlay bound
      action: active design system unchanged
      ```
    - Files to Create/Edit:
      - `src/shell/design_system.rs`: Final bounds, contrast/state checks, and diagnostics.
      - `src/server/ops/theme.rs`: Activation failure preservation and sanitized diagnostics.
      - `tests/design_system_packages.rs`: Complete hardening matrix.
      - `tests/package_ui_conformance.rs`: Trust/raw-authority/source guards.
      - `frontend/src/test/design-system-conformance.test.tsx`: Accessibility/effect/performance matrix.
      - `frontend/src/editor/performance.test.ts`: Editor switch continuity budget.
      - `docs/development/performance.md`: Durable budgets and measurement method.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
  - Test Cases to Write:
    - Theme/design-system matrix: Neobrutal and Glass pass with representative light/dark content themes; theme-only changes recolor all UI and design-system-only changes add no concrete color.
    - Typography/density matrix: Focus, labels, hit targets, and layout remain usable.
    - Malicious recipe matrix: Raw CSS, URLs, literal colors, package palettes/color aliases, excessive effects, hidden focus, zero-opacity controls, and off-scale z values are rejected.
    - Third-party denial: Adopted package has no trusted/internal ops or renderer capability.

- [ ] Run complete automated Linux and frontend release validation
  - Acceptance Criteria:
    - Functional: All package, runtime, protocol, Tauri, frontend, docs-registry, conformance, and existing regression suites pass with both design systems.
    - Performance: Bundle, runtime snapshot, adapter install, React render, editor continuity, and effect budgets pass recorded thresholds.
    - Code Quality: Linux formatting, check, clippy, tests, frontend typecheck/lint/format/tests/build, bundle budget, JavaScript package tests, docs registry, and documentation coverage pass without warnings or stale generated artifacts.
    - Security: Existing package trust-domain, capability, raw-op, and Tauri boundary suites remain green; dependency changes are absent unless separately justified and documented.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/build-and-test.md`
      - `frontend/package.json`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
    - Options Considered:
      - Run only new tests: misses repository-wide package/configuration regressions. Rejected.
      - Run targeted suites during work and full blocking/release gates at completion: selected.
    - Chosen Approach:
      - Execute full Linux validation once package tests and conformance harness pass. Record exact commands, versions, measurements, and failures in plan completion evidence.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo run --bin update-doc-registry
      git diff --exit-code -- docs src
      npm --prefix frontend run typecheck
      npm --prefix frontend run lint
      npm --prefix frontend run format:check
      npm --prefix frontend test
      npm --prefix frontend run build
      npm --prefix frontend run check:budget
      node --check examples/init.js
      ```
    - Files to Create/Edit:
      - `plans/104-Neobrutal-and-Glass-Design-System-Packages-and-Conformance.md`: Record completion evidence and exact checks.
      - Generated registry artifacts: Update only through project generator.
      - No production file solely for test orchestration unless existing scripts cannot express the matrix.
    - References:
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - Full release matrix: All blocking commands pass on Linux.
    - Generated artifact cleanliness: Rerunning registry generation produces no diff.
    - Dependency audit: Confirm no new styling runtime dependency was introduced.

- [ ] Perform final visual screenshot, accessibility, and Impeccable finish review
  - Acceptance Criteria:
    - Functional: Capture every representative surface/state under the Neobrutal/Glass by representative light/dark content-theme cross-product, default/large typography, compact/default/spacious density, narrow/wide windows, reduced motion, reduced transparency or strongest browser-supported substitute, forced colors, unsupported blur fallback, loading/empty/error/recovery, and all interactive states; confirm each normal UI color follows the selected theme.
    - Performance: Exercise typing, scrolling, split resize, tab switching, package panel use, Command Centre, completion, modal, and design-system switching while observing repaint/jank, editor continuity, flash, and effect cost.
    - Code Quality: Store valid screenshots under `.impeccable/review/plan-104/`, inspect each file, run the detector once, batch fixes, confirm once, and invoke the fresh Impeccable finish reviewer with original request, direction contract, screenshots, quality-bar references, and findings.
    - Security: Start with `get_app_state`; verify keyboard flow, focus visibility/order, roles, names, values/states, errors, modal containment, announcements, forced-color affordances, and unchanged package intent semantics in both systems.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/impeccable/reference/new-work.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
      - `docs/wiki/modules/ui-review-harness.md`
      - `docs/development/ui-design-system-visual-direction.md`
    - Options Considered:
      - Self-review only: insufficient for a new default visual world. Rejected.
      - One bounded screenshot/a11y pass, one batched correction, one confirmation, then fresh finish reviewer: selected.
    - Chosen Approach:
      - Extend capture fixtures for a two-system matrix, validate every screenshot, run detector once over changed UI targets, and use the shipped finish reviewer. Treat `recapture`, `rebuild`, `fix`, and `ship` dispositions exactly as Impeccable defines. After final corrections, run the documenter to write `DESIGN.md` from shipped ground truth.
    - API Notes and Examples:
      ```text
      .impeccable/review/plan-104/neobrutal-shell-wide.png
      .impeccable/review/plan-104/neobrutal-settings-focus.png
      .impeccable/review/plan-104/glass-shell-wide.png
      .impeccable/review/plan-104/glass-solid-fallback.png
      .impeccable/review/plan-104/glass-forced-colors.png
      ```
    - Files to Create/Edit:
      - `tests/fixtures/configuration/ui-review-design-neobrutal/init.js`: Default review configuration.
      - `tests/fixtures/configuration/ui-review-design-glass/init.js`: Glass review configuration.
      - `scripts/capture-ui-review.sh`: Parameterize design-system matrix only if current fixture selection cannot.
      - `.impeccable/review/plan-104/**`: Screenshot evidence and findings.
      - `DESIGN.md`: Generated after final corrections by Impeccable documenter.
      - `.impeccable/**`: Direction/documentation sidecar artifacts produced by the approved workflow.
      - Changed package/frontend files: One batched evidence-driven correction pass.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`
  - Test Cases to Write:
    - Screenshot matrix completeness: Every required system/theme/state/viewport/fallback has a valid image and finding.
    - Accessibility-tree parity: Roles/names/states stay equivalent across systems.
    - Color-source review: Neobrutal and Glass contain no stock palette; switching content theme visibly recolors every reviewed surface under both systems, while forced-colors uses browser/OS system colors.
    - Reviewer disposition: Final evidence records reviewer outcome and unresolved findings without overstating scope.
    - Design documentation: `DESIGN.md` describes final built default, not pre-build intention.

- [ ] Update final public design-system, package, catalog, and contributor documentation
  - Acceptance Criteria:
    - Functional: Document package selection, authoring, complete schema, non-color value/property types, semantic theme-color-role references, recipes, slots, states, inheritance, fallback, Neobrutal default design system, Glass reference design system, package installation/adoption, revocation, compatibility, testing, and limitations.
    - Performance: Docs specify payload/effect/install/render budgets and safe material guidance.
    - Code Quality: `PRODUCT.md`, `DESIGN.md`, Clay UI catalogs, public references, package docs, developer conformance docs, and master index agree with shipped code; generated registry is current.
    - Security: Docs clearly distinguish declarative styling from arbitrary renderer code and color themes, describe trust domains and shared third-party cohort, and prohibit raw CSS/JSX/selectors/scripts/Tauri access, palettes, literal colors, and package-owned color values.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-design-systems.md`
      - `docs/reference/packages/creating-packages.md`
      - `docs/reference/ui-components.md`
      - `DESIGN.md`
    - Options Considered:
      - Document only bundled packages: fails user-authoring goal. Rejected.
      - Document only schema without full package examples: too abstract. Rejected.
      - Use both shipped packages as complete reference examples and keep schema authoritative in one public page: selected.
    - Chosen Approach:
      - Finalize one authoritative design-system reference, link package examples, update catalogs and development conformance docs, then regenerate/check docs registry.
    - API Notes and Examples:
      ```js
      import { setDesignSystem } from "clay:theme";
      setDesignSystem("@clay/design-glass");
      ```
    - Files to Create/Edit:
      - `docs/reference/ui-design-systems.md`: Complete public authoring and runtime contract.
      - `docs/reference/ui-components.md`: Component/slot/state catalog links.
      - `docs/reference/packages/creating-packages.md`: Design-system package workflow and security.
      - `docs/index.md`: Master documentation links.
      - `.agents/skills/clay-ui/references/components.md`: Final recipe coverage/status.
      - `.agents/skills/clay-ui/references/tokens.md`: Final values/properties/bounds.
      - `packages/design-neobrutal/docs/index.md`: Default package reference.
      - `packages/design-glass/docs/index.md`: Glass package reference.
      - `docs/development/ui-design-system-conformance.md`: Test/review harness and budgets.
      - `docs/development/ui-design-system-css-audit.md`: Close every migration row.
      - `docs/development/tauri-react-parity-ledger.json`: Mark capability verified only with named evidence.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Docs/source parity: Schema, package names, recipe keys, theme-color-role authority, bounds, and commands match source.
    - Master link coverage: All public/package docs are indexed.
    - Conformance evidence: Parity ledger cannot become verified without automated/manual/visual evidence.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Verify `theme.setDesignSystem` remains the only public selector needed; document bundled default and Glass example; inventory new/changed Rust public functions and narrow any internal-only helpers.
    - Performance: API remains generation-time and same-selection no-op behavior is documented/tested.
    - Code Quality: Stable ID, JS export, user-facing name, keybindings, custom properties, backing Rust/op/facade paths, errors, return behavior, tags, and generated registry entries are complete.
    - Security: API docs state prior installation/adoption, exact provenance, no permission expansion, revocation/fallback, no arbitrary renderer authority, and no design-system palette/color authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/theme/set-design-system.md`
    - Options Considered:
      - Add convenience APIs per bundled package: redundant and package-specific. Rejected.
      - Keep one generic selector and normal package identifiers: selected.
    - Chosen Approach:
      - Update authoritative API docs/examples/paths, regenerate registry, and run inventory/lookup coverage.
    - API Notes and Examples:
      ```js
      import { setDesignSystem } from "clay:theme";
      setDesignSystem("@clay/design-neobrutal");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/theme/set-design-system.md`: Final examples/defaults/security/performance.
      - `docs/index.md`: Verify link.
      - `api-inventory.toml`: Verify metadata.
      - Generated registry artifacts: Update through `cargo run --bin update-doc-registry`.
      - `tests/clay_js_doc_registry.rs`: API inventory/docs/lookup coverage.
    - References:
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - API metadata: Registry exposes all required fields and lookup tags.
    - Rust inventory: Every public capability has an API or internal visibility.
    - Generator idempotence: Second update produces no diff.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Fresh configuration uses the Neobrutal default design system with colors from the current content theme; one-line Glass selection works after installation/adoption without changing theme selection; reload, removal, revocation, and invalid configuration follow documented fallback/fault-isolation behavior.
    - Performance: Same selection is a no-op and switches remain within recorded budget.
    - Code Quality: Theme, typography, appearance, and design-system selection stay separate; `setTheme` remains the concrete color-selection path, and no stock design-system theme, palette setting, or undocumented per-recipe override surface is added.
    - Security: Configuration cannot install, adopt, promote, grant permissions, bypass package validation, or supply design-system colors.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/clay-js-api/configuration.md`
      - `docs/reference/clay-js-api/theme/set-design-system.md`
    - Options Considered:
      - Automatically activate newly installed design-system packages: surprising behavior and authority coupling. Rejected.
      - Default to bundled Neobrutal and require explicit `setDesignSystem` for alternatives: selected.
    - Chosen Approach:
      - Verify parser/runtime/docs behavior and add no granular overrides until a separate demonstrated need and decision.
    - API Notes and Examples:
      ```js
      import { setDesignSystem } from "clay:theme";
      setDesignSystem("@clay/design-glass");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Final appearance ownership and selection links.
      - `src/server/configuration.rs`: Final default/no-op/reload behavior if tests expose a gap.
      - `tests/clay_js_doc_registry.rs`: Configuration metadata coverage.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Fresh default: No explicit design-system selection uses bundled Neobrutal recipes while the configured/default content theme supplies every color.
    - One-line alternate: Glass activates from `init.js` after adoption without replacing or mutating active theme state.
    - Theme-only selection: `setTheme` recolors both systems without changing design-system generation.
    - Invalid/revoked: Previous valid generation or built-in fallback remains coherent as documented.

- [ ] Update the canonical example configuration (`examples/init.js`)
  - Acceptance Criteria:
    - Functional: Canonical appearance section documents default Neobrutal design-system behavior and one commented Glass selection exactly once, alongside separate content-theme and typography configuration; comments state that content themes supply all colors.
    - Performance: Example leaves optional Glass effects inactive by default.
    - Code Quality: File is comprehensive, valid JavaScript, safe to copy, correctly ordered, and synchronized with API docs/inventory.
    - Security: Comments explain installation/adoption and no authority grant from selection.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `examples/init.js`
      - `docs/reference/clay-js-api/theme/set-theme.md`
      - `docs/reference/clay-js-api/theme/set-typography.md`
      - `docs/reference/clay-js-api/theme/set-design-system.md`
    - Options Considered:
      - Show both packages as active calls: invalid because only one design system is active. Rejected.
      - Keep default implicit and show Glass as commented optional selection: selected.
    - Chosen Approach:
      - Update one appearance section with ownership comments and exact package/API names.
    - API Notes and Examples:
      ```js
      // Content theme supplies every concrete UI/editor color.
      // Bundled restrained Neobrutal recipes are the default design system.
      // setDesignSystem("@clay/design-glass"); // Keeps current theme colors.
      ```
    - Files to Create/Edit:
      - `examples/init.js`: Final design-system example and comments.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Syntax: `node --check examples/init.js` passes.
    - Surface uniqueness: Design-system selection appears exactly once.
    - Doc parity: Names/defaults/security and active-theme-only color notes match authoritative docs.

- [ ] Execute and update the manual test plan
  - Acceptance Criteria:
    - Functional: Run all UI design-system steps on real Linux for default startup, package adoption, one-line selection, reload switching, restart, complete component/surface states, the Neobrutal/Glass by light/dark content-theme cross-product, theme-only recoloring, typography/density variants, reduced effects, unsupported blur fallback, revocation, and invalid package recovery.
    - Performance: Record switch responsiveness, typing/scrolling continuity, blur fallback, and absence of full remount or sustained repaint.
    - Code Quality: Keep numbered steps and coverage matrix current; cross-link deep references and screenshot evidence.
    - Security: Verify third-party package disclosure, no promotion/internal ops/Tauri access, raw styling and color/palette denial, active-theme-only color sourcing, and atomic fallback after revocation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `test-plan/index.md`
      - `test-plan/02-configuration-init-js.md`
      - `test-plan/07-caret-and-typography.md`
      - `test-plan/09-packages-and-modes.md`
      - `test-plan/11-performance.md`
      - `test-plan/15-ui-design-systems.md`
    - Options Considered:
      - Treat screenshots as the manual plan: screenshots do not cover package/configuration lifecycle. Rejected.
      - Run workflow steps and cross-link visual evidence from the review task: selected.
    - Chosen Approach:
      - Expand module 15 into the complete end-user/design-author workflow, then execute every affected module and record pass/fail.
    - API Notes and Examples:
      ```text
      UI-DS-05 Neobrutal complete state matrix
      UI-DS-06 Glass complete state matrix
      UI-DS-07 reduced/unsupported effect fallback
      UI-DS-08 revoke active adopted package
      ```
    - Files to Create/Edit:
      - `test-plan/15-ui-design-systems.md`: Complete package/style/lifecycle matrix.
      - `test-plan/index.md`: Coverage matrix and evidence links.
      - `test-plan/02-configuration-init-js.md`: Selection/reload/restart cross-links.
      - `test-plan/07-caret-and-typography.md`: Typography ownership matrix.
      - `test-plan/09-packages-and-modes.md`: Adoption/revocation/security cross-links.
      - `test-plan/11-performance.md`: Effect/switch budgets.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Real Linux run: Record pass/fail for every affected numbered step.
    - Package-author smoke: Install/adopt/select a local data-only design-system fixture without JavaScript execution.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki documents shipped package structure, default/alternate resolution, content-theme-only color authority, conformance harness, visual review fixtures, lifecycle, fallback, and contributor extension path after all work passes.
    - Performance: Wiki records measured budgets, effect limits, no-execution data-only path, and frontend install/render behavior.
    - Code Quality: Pages explain what/how/why, source/test paths, examples, tradeoffs, known ceilings, and links from master index; public usage links to authoritative reference docs.
    - Security: Wiki documents bundled/adopted provenance, trust domains, revocation, no raw styling authority, and isolated custom-surface boundary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
    - Options Considered:
      - Create separate wiki pages for each visual recipe: too granular. Rejected.
      - Document package examples in one runtime page plus package-loading/theme/review cross-links: selected.
    - Chosen Approach:
      - Update wiki once after final automated/manual/visual review and `DESIGN.md` generation, then run deterministic coverage.
    - API Notes and Examples:
      ```text
      @clay/design-neobrutal -> default ActiveDesignSystem
      @clay/design-glass -> same schema and components, different resolved recipes
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Master navigation.
      - `docs/wiki/modules/ui-design-system-runtime.md`: Shipped packages, selection, fallback, conformance.
      - `docs/wiki/modules/frontend-theme-runtime.md`: Runtime install and effect fallbacks.
      - `docs/wiki/modules/package-loading.md`: Declarative-only package lifecycle if implemented.
      - `docs/wiki/modules/react-sdui-package-ui.md`: Source-independent styling proof.
      - `docs/wiki/modules/ui-review-harness.md`: Two-system screenshot/accessibility matrix.
    - References:
      - `.agents/skills/project-wiki/references/page-template.md`
  - Test Cases to Write:
    - Wiki index coverage: Every new/updated implementation page is discoverable.
    - Public/internal boundary review: Wiki links public authoring/API docs and does not duplicate them as competing authority.
    - Final documentation coverage: `cargo test` fails for stale wiki/reference/catalog links.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
