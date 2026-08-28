# UI Design-System Component and Surface Migration

Depends on `plans/101-UI-Design-System-Recipe-Foundation.md` and `plans/102-UI-Design-System-Activation-and-Frontend-Runtime.md`.
Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.

## Objectives

- Replace fixed non-color visual recipes across Clay's React client with host-owned semantic recipe variables; replace every normal-rendering color literal with an active content-theme role.
- Keep React Aria/native behavior, accessibility semantics, DOM ownership, package action routing, shell layout, content-theme color authority, and CodeMirror editing authority unchanged.
- Cover every package component, shell surface, transient surface, product surface, and editor-chrome boundary in the recipe matrix.
- Remove visual literals that block design-system replacement while retaining structural CSS and compatible non-color fallback values; every color fallback remains a semantic active-theme reference.
- Prove migration completeness through deterministic source, computed-style, accessibility, screenshot, and performance checks.

## Expected Outcome

- Every non-color visual property identified in the recipe matrix resolves from the active UI design system or built-in fallback; every normal-rendering color resolves from the active content theme, with browser/OS system colors used only in forced-colors mode.
- CSS Modules retain component structure and semantic state selectors but no longer own fixed product-wide radius, material, border geometry, shadow, blur, motion, opacity, component-state mappings, or color literals.
- Package components and Clay-native surfaces use the same versioned recipe contract without exposing DOM selectors to packages.
- CodeMirror content/syntax themes remain separate; only host/editor chrome properties approved by the matrix move to UI design-system recipes.
- Existing themes, typography profiles, package component trees, keyboard behavior, focus semantics, and user layouts remain compatible.

## Tasks

- [ ] Audit every frontend visual declaration and lock structural versus replaceable ownership
  - Acceptance Criteria:
    - Functional: Inventory all frontend CSS/inline styles and classify each declaration as structural layout, active content-theme color role, user typography, non-color UI design-system recipe, accessibility override, browser compatibility rule, or unjustified literal.
    - Performance: Identify expensive filter, backdrop-filter, shadow, transition, containment, and layout declarations; assign explicit paint/layout budgets and fallback ownership.
    - Code Quality: Every replaceable non-color declaration maps to an existing recipe key or creates one generic matrix addition with catalog justification; every color declaration maps to a known active-theme role; structural declarations remain host-owned.
    - Security: Audit confirms no package-controlled selectors, style elements, URLs, CSS imports, generated content, pointer-event overrides, position escapes, or z-index bypasses can enter the main webview.
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
      - `docs/development/react-ui-catalog-mapping.md`
      - `frontend/src/**/*.module.css`
      - `frontend/src/styles/*.css`
    - Options Considered:
      - Replace every numeric CSS value: breaks legitimate structural layout and responsive constraints. Rejected.
      - Migrate only obvious color literals and radii: leaves material, motion, state, elevation, and color-role ownership ambiguous. Rejected.
      - Classify each declaration by owner, migrate colors to content-theme roles, and migrate only non-color visual-system decisions to recipes: selected.
    - Chosen Approach:
      - Generate a checked-in audit table grouped by component/surface and selector. Record current value, target recipe key or theme role, color source, fallback, state source, owner, migration plan, and retained structural reason.
    - API Notes and Examples:
      ```text
      button.module.css | .button | border-radius | recipe | button.default.root.rest.radius
      button.module.css | .button | background | content theme | surface.control
      shell.module.css | .shell | grid-template-rows | structural | React shell owner
      editor.module.css | .cm-content | color | content theme | CodeMirror theme owner
      ```
    - Files to Create/Edit:
      - `docs/development/ui-design-system-css-audit.md`: Complete declaration ownership and migration ledger.
      - `docs/development/ui-design-system-recipe-matrix.md`: Add only generic missing recipe keys found by audit.
      - `.agents/skills/clay-ui/references/components.md`: Add any justified new internal slot.
      - `.agents/skills/clay-ui/references/tokens.md`: Add any justified generic value/property domain.
    - References:
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
      - `.agents/skills/project-patterns/references/ui-modernization.md`
  - Test Cases to Write:
    - CSS audit coverage: Every frontend CSS file and inline style site appears in the ledger.
    - Ownership completeness: Every declaration classified `recipe` has an implemented matrix key and non-color fallback; every color declaration names a valid active-theme role.
    - Color literal deny scan: Hex/RGB/HSL/named product colors are absent outside theme package data and forced-colors system-color rules.

- [ ] Implement shared host recipe selectors and state mapping
  - Acceptance Criteria:
    - Functional: Add minimal host-owned class/data-slot conventions that map component, slot, variant, and React Aria semantic state to recipe variables, with color variables resolving only through active-theme role indirection, without exposing generated class names or DOM structure to packages.
    - Performance: State changes use CSS pseudo-classes/data attributes and native custom-property resolution; no React state bridge, object allocation, selector generation, or JavaScript style update runs on hover/press/focus.
    - Code Quality: One shared convention works for host components and SDUI registry mappings; state precedence matches disabled, invalid, selected, active, hover, focus-visible, and rest contracts from the catalog.
    - Security: Data attributes are set only by host code from validated enums; package values cannot add attributes, selectors, roles, handlers, or arbitrary style properties.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - React Aria styling docs via Context7 `/websites/react-aria_adobe`
      - `frontend/src/sdui/registry.tsx`
      - `frontend/src/components/*.tsx`
    - Options Considered:
      - Inline `style` objects per render: straightforward but causes object churn and state mapping in JavaScript. Rejected.
      - Runtime-generated global selectors: recreates selector injection and lifecycle complexity. Rejected.
      - Static host CSS selectors over stable host-authored classes/data slots and React Aria state attributes: selected.
    - Chosen Approach:
      - Keep DOM private. Add host-internal `data-clay-component`, `data-clay-slot`, and validated variant attributes only where CSS Modules cannot identify a semantic slot. Consume root-level variables from static CSS.
    - API Notes and Examples:
      ```tsx
      <Button
        data-clay-component="button"
        data-clay-slot="root"
        data-variant="primary"
      />
      ```
    - Files to Create/Edit:
      - `frontend/src/components/recipe-attributes.ts`: Closed host-only attribute helpers if repetition justifies one helper.
      - `frontend/src/components/chrome.module.css`: Shared semantic slot/state consumption.
      - `frontend/src/sdui/registry.tsx`: Host-authored component/slot/variant attributes.
      - `frontend/src/sdui/registry.test.tsx`: State/attribute/accessibility preservation tests.
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
      - `.agents/skills/project-patterns/references/ui-skill-stack.md`
  - Test Cases to Write:
    - State precedence: Disabled and invalid states cannot be hidden by hover/active styling.
    - Package data denial: Unknown package style/variant cannot create host data attributes.
    - Render-count test: Hover/focus state changes do not trigger recipe-related React renders.

- [ ] Migrate cataloged controls and package-facing components to recipes
  - Acceptance Criteria:
    - Functional: Migrate button, label/text, text input, dropdown/select, list/list row, collapse/disclosure, modal/dialog, panel, overlay, scroll, status item, flex, stack, portal, and editor-view host chrome according to the catalog and matrix; every component color remains supplied by the active content theme.
    - Performance: No component subscribes independently to design-system state; large package trees retain stable reconciliation keys and bounded render counts.
    - Code Quality: Reuse React Aria/native primitives and existing catalog kinds; no custom off-catalog component is introduced. All applicable rest/hover/active/focus-visible/disabled/selected/expanded/invalid states resolve through recipes and fallback.
    - Security: Package actions remain inert intents, values remain validated token/recipe references, and component behavior/semantics cannot be replaced by the active design system.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - `frontend/src/components/button.module.css`
      - `frontend/src/components/controls.module.css`
      - `frontend/src/components/modal.module.css`
      - `frontend/src/components/text-field.module.css`
      - `frontend/src/sdui/registry.module.css`
    - Options Considered:
      - Rewrite components around a new third-party design-system library: breaks Clay catalog and package compatibility. Rejected.
      - Migrate each existing host component in place: selected.
    - Chosen Approach:
      - Replace non-color visual literals with recipe variables and color literals with active-theme role variables in coherent component groups. Recipe color selectors may choose a theme role but cannot carry a concrete color. Keep DOM, React Aria composition, test selectors, intent routing, and package-facing schema stable.
    - API Notes and Examples:
      ```css
      .textInput[data-focus-visible] {
        /* Installed recipe value is var(--clay-focus-ring), never a color literal. */
        outline-color: var(--clay-recipe-text-input-default-root-focus-outline-color);
        outline-width: var(--clay-recipe-text-input-default-root-focus-outline-width);
      }
      ```
    - Files to Create/Edit:
      - `frontend/src/components/button.module.css`: Full button variants and states.
      - `frontend/src/components/controls.module.css`: Select/list/disclosure states and slots.
      - `frontend/src/components/modal.module.css`: Dialog/backdrop/scrim slots.
      - `frontend/src/components/text-field.module.css`: Field/label/description/error slots.
      - `frontend/src/components/text.module.css`: Semantic text slots without concrete fonts/sizes.
      - `frontend/src/components/chrome.module.css`: Shared focus/divider/scrollbar/badge/kbd/icon/tooltip chrome.
      - `frontend/src/sdui/registry.module.css`: Package-facing kind mapping.
      - `frontend/src/sdui/registry.tsx`: Semantic slot attributes only where required.
      - `frontend/src/sdui/registry.test.tsx`: Kind/state/accessibility tests.
      - `frontend/src/settings/SettingsPanel.test.tsx`: Compiled trusted surface composition parity.
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
  - Test Cases to Write:
    - Per-kind computed style: Required states resolve non-empty values after fallback.
    - Theme recoloring: Switching only the content theme recolors every migrated kind without changing design-system identity or component source.
    - Accessibility parity: Roles, labels, descriptions, errors, selected/expanded/disabled states, and modal focus trap remain intact.
    - Stable reconciliation: Unrelated recipe changes do not reset input, disclosure, selection, or scroll state.

- [ ] Migrate shell, layout, transient, product, and editor-chrome surfaces
  - Acceptance Criteria:
    - Functional: Migrate app shell, top bar, tab strip, working area, pane tree/dividers, workspace panels, package workspace, Command Centre, chat, settings, fixture/workspace routes, status surfaces, completion/overlay chrome, editor gutters/scrollbars/focus/selection-adjacent host chrome, and responsive containers identified in the audit.
    - Performance: Preserve CodeMirror-local typing and viewport rendering; design-system changes do not recreate `EditorView`, pane state, tab state, package trees, or transient-menu sessions.
    - Code Quality: Structural grid/split/layout CSS stays host-owned; only non-color visual-system decisions move to recipes. All shell, component, editor, syntax, text, border, focus, selection, status, diagnostic, overlay, and solid fallback colors remain content-theme owned, and typography remains user-profile owned.
    - Security: Shell/layout ownership, fixed-slot constraints, modal boundaries, package overlay anchors, z-level ordering, and narrow Tauri authority remain unchanged.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/ui-design-system-css-audit.md`
      - `docs/wiki/modules/react-codemirror-editor.md`
      - `docs/wiki/modules/react-sdui-package-ui.md`
      - `docs/wiki/modules/slot-aware-package-ui.md`
    - Options Considered:
      - Let design systems alter split topology, breakpoints, or slot ownership: exceeds visual authority and risks usability. Rejected.
      - Keep shell/editor chrome fixed: prevents whole-application replacement. Rejected.
      - Recipe-drive non-color visual chrome and semantic theme-role selection while retaining host structure, geometry clamps, and universal content-theme color ownership: selected.
    - Chosen Approach:
      - Migrate by surface group, using the audit as a deletion checklist. Preserve existing responsive behavior and only replace style decisions covered by recipe keys.
    - API Notes and Examples:
      ```text
      UI design system owns: pane divider geometry/material/effects and theme-role mapping
      Shell owns: split ratio, hit target, drag behavior, min/max geometry
      Content theme owns: every concrete UI/editor color
      Typography owns: UI/document font families and base sizes
      ```
    - Files to Create/Edit:
      - `frontend/src/app/layout/shell.module.css`: Shell chrome recipes.
      - `frontend/src/app/layout/tab-bar.module.css`: Tab slots/states.
      - `frontend/src/shell/pane-tree.module.css`: Divider and focus chrome.
      - `frontend/src/shell/workspace-panes.module.css`: Slot/panel chrome.
      - `frontend/src/packages/package-workspace.module.css`: Package surface chrome.
      - `frontend/src/command-centre/command-centre.module.css`: Dialog/list/input chrome.
      - `frontend/src/chat/chat.module.css`: Chat product-surface recipes.
      - `frontend/src/settings/settings-panel.module.css`: Settings surface recipes.
      - `frontend/src/editor/editor.module.css`: Host/editor chrome recipe boundaries; keep content-theme and structural rules separate.
      - `frontend/src/routes/fixture.module.css`: Review fixture recipes.
      - `frontend/src/routes/workspace.module.css`: Workspace route chrome.
      - `frontend/src/sdui/renderer.module.css`: SDUI surface chrome.
      - `frontend/src/styles/global.css`: Global focus/forced-color/reset rules only.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/typography-role-ownership.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Editor persistence: Switching design system preserves document, selection, history, viewport, folds, completion, and diagnostics.
    - Shell persistence: Tabs, panes, split ratios, panel visibility, and modal/menu sessions remain stable.
    - Ownership test: No concrete shell/component/editor color comes from UI design-system package data; CodeMirror syntax variables and all other colors resolve from active-theme variables.
    - Responsive test: Narrow/wide geometry remains usable under compact/spacious density and large UI typography.

- [ ] Add forced-color, reduced-motion, reduced-transparency, unsupported-effect, and paint-budget fallbacks
  - Acceptance Criteria:
    - Functional: Every effect-capable recipe property has host-defined fallback behavior for forced colors, reduced motion, reduced transparency where supported, missing `backdrop-filter`, and invalid/unsupported values; every normal solid fallback color references the active content theme.
    - Performance: Bound blur area/count, shadow layers, transition properties/durations, and active animated elements; prohibit blur on large scrolling content and layout-property animation.
    - Code Quality: Fallbacks are centralized by property/slot class rather than copied inconsistently across components; required focus/error/selection contrast cannot be disabled by recipes.
    - Security: Recipes cannot use arbitrary CSS functions, animation names, keyframes, URLs, masks, blend modes, custom cursors, pointer suppression, or z-index outside host levels.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - MDN `backdrop-filter`, `prefers-reduced-motion`, `forced-colors`, and CSS custom-property documentation.
      - `frontend/src/styles/global.css`
    - Options Considered:
      - Let each package provide fallback recipes: flexible but cannot guarantee accessibility/performance. Rejected.
      - Host enforces mandatory fallback layers after active recipe values: selected.
    - Chosen Approach:
      - Add static host media/support rules and server bounds. Design-system packages may provide preferred non-color fallback values and semantic theme-role mappings inside the schema, but cannot provide colors. Clay supplies safe non-color defaults and mandatory affordances; normal fallback colors come from the active theme, while forced-colors mode uses browser/OS system colors.
    - API Notes and Examples:
      ```css
      @supports not (backdrop-filter: blur(1px)) {
        [data-clay-material="glass"] {
          backdrop-filter: none;
          background: var(
            --clay-recipe-material-glass-solid-fallback-color,
            var(--clay-surface-overlay)
          );
        }
      }
      ```
    - Files to Create/Edit:
      - `src/shell/design_system.rs`: Effect/property bounds and mandatory fallback validation.
      - `frontend/src/styles/global.css`: Forced-color, reduced-motion/transparency, and unsupported-effect fallbacks.
      - `frontend/src/components/chrome.module.css`: Shared material/effect fallback slots.
      - `frontend/src/test/design-system-adapter.test.ts`: Capability/fallback value tests.
      - `docs/development/ui-design-system-recipe-matrix.md`: Accessibility/performance fallback column.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Forced colors: Focus, invalid, selected, disabled, and modal boundaries remain distinguishable.
    - Reduced motion: Non-essential transitions become instant without hiding state changes.
    - Unsupported blur: Glass material resolves a solid active-theme surface role with passing contrast and no design-system color value.
    - Effect bounds: Oversized blur/shadow/transition requests are rejected before install.

- [ ] Add exhaustive automated migration and non-regression checks
  - Acceptance Criteria:
    - Functional: Tests prove every audit `recipe` row has a consumer, every recipe key has a fallback and consumer, every component/surface state renders, no fixed non-color visual literal remains outside approved fallback/token files, and no normal-rendering color literal remains outside content-theme package data.
    - Performance: Frontend performance tests cover switch render counts, style-install cost, editor continuity, large package trees, and expensive-effect ceilings.
    - Code Quality: All Linux Rust and frontend blocking checks pass; CSS audit and catalog drift tests provide actionable failures.
    - Security: Static and runtime tests retain raw CSS/selector/script/Tauri denial, package tree inertness, and host semantic ownership.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `tests/package_ui_conformance.rs`
      - `tests/documentation_coverage.rs`
      - `frontend/src/test/theme-adapter.test.ts`
      - `frontend/src/editor/performance.test.ts`
    - Options Considered:
      - Snapshot all generated CSS text: brittle and obscures semantic gaps. Rejected as sole check.
      - Combine typed registry parity, targeted computed styles, source deny scans, and end-to-end fixtures: selected.
    - Chosen Approach:
      - Add the smallest check at each failure boundary and retain one representative end-to-end switch test. Run full blocking commands after targeted suites pass.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      npm --prefix frontend run typecheck
      npm --prefix frontend run lint
      npm --prefix frontend run format:check
      npm --prefix frontend test
      npm --prefix frontend run build
      npm --prefix frontend run check:budget
      ```
    - Files to Create/Edit:
      - `tests/package_ui_conformance.rs`: Recipe consumer/fallback/catalog/source checks.
      - `tests/documentation_coverage.rs`: Audit and docs parity.
      - `frontend/src/sdui/registry.test.tsx`: Package component states.
      - `frontend/src/test/shell.test.tsx`: Shell/surface states and persistence.
      - `frontend/src/test/editor.test.tsx`: Editor chrome/content-theme ownership.
      - `frontend/src/editor/performance.test.ts`: Switch continuity and render budgets.
      - `frontend/src/test/design-system-adapter.test.ts`: Install/fallback/effect checks.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Audit closure: No unresolved recipe or theme-color-role row remains.
    - Literal deny scan: Unapproved raw visual decisions and every non-theme color literal fail with file/line guidance.
    - Theme/design-system cross-product: Every migrated surface passes with at least two materially different content themes and two design systems; theme-only switching changes colors while design-system-only switching never introduces colors.
    - Large tree: Design-system switch preserves local SDUI state and bounded renders.
    - Full Linux validation: All blocking checks pass.

- [ ] Perform visual screenshot and accessibility review of every migrated surface
  - Acceptance Criteria:
    - Functional: Launch representative real Linux UI states for shell, editor, tabs/panes, package panel, settings, chat, Command Centre, completion, dropdown, collapse, text input validation, modal, empty/loading/error/recovery, and narrow/wide layouts.
    - Performance: Inspect switching, scrolling, typing, modal opening, and transient state changes for flash, jank, blur repaint, layout shift, stale styles, or recreated editor/package state.
    - Code Quality: Capture and inspect named screenshots under `.impeccable/review/plan-103/`, run the detector once on all changed CSS/TSX targets, batch fixes, and confirm once.
    - Security: Start with `get_app_state`; verify keyboard-only flow, focus order/visibility, names, roles, states, invalid/error descriptions, modal containment, list/select semantics, and live status announcements.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
      - `scripts/capture-ui-review.sh`
      - `docs/wiki/modules/ui-review-harness.md`
    - Options Considered:
      - Review only default shell screenshot: misses state and product-surface migration defects. Rejected.
      - One bounded matrix across all changed surfaces/states with a single fix batch and confirmation: selected.
    - Chosen Approach:
      - Extend review fixtures only where existing fixtures cannot reach required state. Record screenshot path, state, viewport, theme, typography/density, accessibility findings, and disposition.
    - API Notes and Examples:
      ```text
      .impeccable/review/plan-103/shell-wide.png
      .impeccable/review/plan-103/settings-validation-focus.png
      .impeccable/review/plan-103/command-centre-narrow.png
      .impeccable/review/plan-103/editor-completion.png
      ```
    - Files to Create/Edit:
      - `tests/fixtures/configuration/ui-review-*/init.js`: Add only missing representative state fixtures.
      - `scripts/capture-ui-review.sh`: Add targets only when current parameterization cannot express them.
      - `.impeccable/review/plan-103/**`: Screenshots and findings.
      - Changed frontend files from Tasks 2-5: One batched evidence-driven correction pass.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`
  - Test Cases to Write:
    - Full keyboard matrix: Operate every changed interactive surface without pointer input.
    - Large typography/density: No clipped labels, hidden focus, unreachable controls, or unusable editor region.
    - Light/dark content themes: Every component and surface color follows the selected content theme under the same design system; design-system switching preserves theme-owned color values/roles and contrast.

- [ ] Update UI, package authoring, catalog, and migration documentation
  - Acceptance Criteria:
    - Functional: Document stable recipe slots, non-color property types, semantic theme-color-role references, state rules, fallback/inheritance, package manifest examples, host-owned behavior, structural CSS boundary, migration status, and unsupported authorities.
    - Performance: Docs state install-time resolution, cached root variables, effect budgets, and absence of package work in render/input hot paths.
    - Code Quality: Catalog, token reference, UI entry point, package guide, migration ledger, and docs index agree; no planned feature is described as implemented.
    - Security: Docs clearly reject raw CSS/JSX/selectors/scripts/URLs/Tauri access, literal/design-system-owned colors, and stock color themes inside design-system packages; explain isolated custom-surface boundary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - `docs/reference/packages/creating-packages.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
    - Options Considered:
      - Put all recipe docs only in package guide: hard for host contributors to navigate. Rejected.
      - Create one public design-system reference and link it from component/package docs: selected.
    - Chosen Approach:
      - Keep public package declaration and usage authoritative under `docs/reference/`; keep implementation rationale in `docs/development/` and wiki.
    - API Notes and Examples:
      ```json
      {
        "clay": {
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
      - `docs/reference/ui-design-systems.md`: Public recipe, state, fallback, performance, and security contract.
      - `docs/reference/ui-components.md`: Component/slot/state navigation.
      - `docs/reference/packages/creating-packages.md`: Authoring, validation, adoption, selection, and limitations.
      - `docs/index.md`: Master links.
      - `.agents/skills/clay-ui/references/components.md`: Final component/slot migration status.
      - `.agents/skills/clay-ui/references/tokens.md`: Final value/property domain and fallback rules.
      - `docs/development/ui-design-system-css-audit.md`: Mark migrated/retained rows with evidence.
      - `docs/development/react-ui-catalog-mapping.md`: Record recipe-driven React target.
    - References:
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - Documentation parity: Every recipe component/slot/property/state and theme-color-role rule appears consistently across source catalogs and public docs.
    - Link coverage: New pages are linked from `docs/index.md` and package guide.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Verify migration changes no public behavior beyond existing `theme.setDesignSystem`; any new Rust public helper is narrowed or documented through the existing API boundary.
    - Performance: API behavior remains generation-time only and no new UI hot-path facade is introduced.
    - Code Quality: `theme.setDesignSystem` docs, metadata, registry, lookup, Rust backing path, op, and facade remain current after module/file movement.
    - Security: API cannot select component implementations, arbitrary recipe keys, raw CSS, or unadopted package content.
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
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/theme/set-design-system.md`
    - Options Considered:
      - Expose per-component recipe mutation APIs: bypasses package validation and atomicity. Rejected.
      - Keep one package-level selector API and declarative manifest recipes: selected.
    - Chosen Approach:
      - Audit changed public Rust items, update paths in authoritative API Markdown, regenerate registry, and run coverage tests.
    - API Notes and Examples:
      ```js
      import { setDesignSystem } from "clay:theme";
      setDesignSystem("@clay/design-glass");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/theme/set-design-system.md`: Update implementation paths or behavior notes if changed.
      - `docs/index.md`: Verify link.
      - Generated registry artifacts: Update through `cargo run --bin update-doc-registry` if authoritative docs changed.
      - `tests/clay_js_doc_registry.rs`: Coverage if needed.
    - References:
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - Public-function inventory: Every changed server public function has a facade/doc or reduced visibility.
    - Registry freshness: Generated entries match Markdown source.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Verify component/surface migration adds no hidden settings and existing `setTheme`, `setTypography`, `setAppearance`, and `setDesignSystem` remain distinct and compatible; `setTheme` remains the only configuration path that changes concrete UI colors.
    - Performance: Configuration reload causes one coherent install and does not rebuild component/editor state.
    - Code Quality: No CSS class, local-storage key, environment variable, or recipe-property override becomes a public configuration shortcut.
    - Security: User configuration cannot bypass package adoption, validation, component catalog, host-owned accessibility behavior, or content-theme color authority.
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
      - `examples/init.js`
      - `docs/reference/clay-js-api/theme/*.md`
    - Options Considered:
      - Add per-user raw recipe overrides now: increases schema and precedence without a demonstrated need. Rejected.
      - Keep selection package-level and defer granular user overrides until a separate approved decision: selected.
    - Chosen Approach:
      - Cross-check configuration docs, parser behavior, example configuration, and runtime install tests. Record no new configuration surface unless implementation proves one unavoidable.
    - API Notes and Examples:
      ```js
      setTheme("@clay/theme-modus-vivendi");
      setTypography({ui: {families: ["system-ui"], size: 13}});
      setDesignSystem("@clay/design-neobrutal");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Verify separate ownership description.
      - `examples/init.js`: Verify canonical example remains complete and valid; edit only if migration changes documented behavior.
      - `tests/clay_js_doc_registry.rs`: Custom-property and docs coverage.
    - References:
      - `.agents/skills/project-patterns/references/typography-role-ownership.md`
      - `.agents/skills/project-patterns/references/ui-modernization.md`
  - Test Cases to Write:
    - Combined configuration: Theme, typography, appearance, and design system install coherently; theme-only changes recolor all recipe consumers and design-system-only changes introduce no color values.
    - No hidden setting: Search source/docs for undocumented recipe override controls.
    - Example validity: `node --check examples/init.js` passes.

- [ ] Execute and update the manual test plan
  - Acceptance Criteria:
    - Functional: Execute every `test-plan/15-ui-design-systems.md` step plus affected shell, tabs, splits, editor, package, configuration, and performance steps on a real Linux build.
    - Performance: Record typing, scrolling, pane/tab switching, package-tree rendering, and design-system switching behavior with no new hot-path stalls.
    - Code Quality: Add exact steps for each migrated surface/state and cross-link deep references instead of duplicating architecture prose.
    - Security: Verify packages cannot alter roles, actions, layout ownership, overlay anchors, or Tauri authority through recipe values.
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
      - `test-plan/04-core-editing.md`
      - `test-plan/07-caret-and-typography.md`
      - `test-plan/09-packages-and-modes.md`
      - `test-plan/11-performance.md`
      - `test-plan/13-window-splits.md`
      - `test-plan/14-tabs.md`
      - `test-plan/15-ui-design-systems.md`
    - Options Considered:
      - Add duplicate per-surface design-system steps to every module: fragments ownership. Rejected.
      - Keep core switching/state coverage in module 15 and add cross-references from affected modules: selected.
    - Chosen Approach:
      - Update coverage matrix and run all affected existing steps plus new migration steps. Record exact blockers and known ceilings.
    - API Notes and Examples:
      ```text
      UI-DS component matrix: default, hover, active, focus-visible, disabled, selected, invalid
      UI-DS surface matrix: shell, package panel, settings, chat, Command Centre, completion, editor chrome
      ```
    - Files to Create/Edit:
      - `test-plan/15-ui-design-systems.md`: Component/surface migration matrix.
      - `test-plan/index.md`: Coverage matrix.
      - `test-plan/04-core-editing.md`: Editor continuity cross-reference.
      - `test-plan/07-caret-and-typography.md`: Ownership separation cross-reference.
      - `test-plan/09-packages-and-modes.md`: Package component security cross-reference.
      - `test-plan/11-performance.md`: Install/render budget cross-reference.
      - `test-plan/13-window-splits.md`: Shell geometry cross-reference.
      - `test-plan/14-tabs.md`: Tab state cross-reference.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Real Linux completion: Record pass/fail for all affected numbered steps.
    - Negative package recipe: Confirm visual values cannot change behavior or structure.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki documents static host selector convention, component/surface consumers, structural-versus-recipe ownership, active-theme-only color ownership, editor boundary, accessibility fallbacks, migration ledger, and testing.
    - Performance: Wiki explains zero per-state JavaScript recipe work, root-variable install, render-count guarantees, and effect budgets.
    - Code Quality: Internal details, source/test paths, extension guidance, tradeoffs, and master-index links are complete after implementation settles.
    - Security: Wiki records package-data limits, host semantic ownership, prohibited authorities, and isolated custom-surface boundary.
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
      - Add per-CSS-file wiki pages: too granular and costly to maintain. Rejected.
      - Update architecture/module pages organized by runtime, package UI, shell, and editor boundaries: selected.
    - Chosen Approach:
      - Update wiki once after automated/manual/visual verification, link public docs, and run deterministic wiki coverage checks.
    - API Notes and Examples:
      ```text
      React Aria state attribute -> static host selector -> root recipe variable -> computed style
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Master navigation.
      - `docs/wiki/modules/ui-design-system-runtime.md`: Component/surface consumer and fallback flow.
      - `docs/wiki/modules/react-sdui-package-ui.md`: Package component mapping and inert boundary.
      - `docs/wiki/modules/frontend-theme-runtime.md`: Store and CSS variable ownership.
      - `docs/wiki/modules/react-codemirror-editor.md`: Content-theme versus UI-chrome ownership.
      - `docs/wiki/modules/ui-review-harness.md`: Design-system review fixture coverage.
    - References:
      - `.agents/skills/project-wiki/references/page-template.md`
  - Test Cases to Write:
    - Wiki navigation coverage: Every updated page is indexed.
    - Implementation accuracy review: Paths, states, properties, commands, and test names match final code.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
