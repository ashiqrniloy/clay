# UI Design-System Recipe Foundation

Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.

## Objectives

- Establish durable product and visual authority before redesign implementation.
- Define a versioned, inert component-recipe contract above Clay's existing typed tokens and host-owned React components.
- Preserve content themes as the sole normal-rendering color authority: recipes may reference semantic theme color roles but may not declare palettes, literal colors, or package-owned color values.
- Add manifest, package-record, validation, fallback, and conformance foundations without exposing raw CSS, JSX, scripts, selectors, or Tauri authority.
- Preserve existing theme packages, typography ownership, component kinds, style variables, and fixed non-color CSS recipes as compatibility fallback; every fallback color resolves through the active theme.

## Expected Outcome

- `PRODUCT.md` records confirmed Clay product truth, operating context, platform, users, constraints, and accessibility commitments before visual-world work starts.
- One reviewed recipe matrix covers every implemented component kind, semantic slot, variant, applicable interaction state, and bounded visual property.
- Rust parses and validates versioned `clay.contributions.uiDesignSystem` data into an inert package record with exact provenance and deterministic fallback.
- Invalid schema versions, component/slot/state/property names, theme-role references, literal/package-owned colors, values, payloads, and prohibited authorities fail before runtime installation.
- Normal UI colors for surfaces, text, borders, focus, selection, diagnostics, overlays, and solid material fallbacks always resolve from the active content theme; browser/OS colors appear only under forced-colors accessibility behavior.
- Catalog, token, package-authoring, and deterministic conformance documentation describe the implemented foundation.

## Tasks

- [ ] Capture product truth and visual-authority prerequisites before redesign implementation
  - Acceptance Criteria:
    - Functional: Run the Impeccable init interview, write `PRODUCT.md` with confirmed product truth, classify Clay as an Operate-mode desktop editor, and record the approved restrained utilitarian Neobrutal default plus Glass replacement requirement as a binding brand commitment without inventing palettes, fonts, claims, or assets.
    - Performance: Product documentation introduces no runtime work and records long-session density, input latency, and low-distraction use as durable constraints.
    - Code Quality: `PRODUCT.md` uses the current Impeccable schema, separates product truth from visual recipes, and leaves unresolved facts explicit rather than guessed.
    - Security: Product constraints retain the separate server, main-webview authority boundary, package provenance, two runtime trust domains, and denial of raw package CSS/JSX/Tauri access.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/impeccable/reference/init.md`
      - `.agents/skills/impeccable/reference/new-work.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
    - Options Considered:
      - Begin implementation without product authority: faster initially, but violates the mandatory redesign workflow and leaves future agents to infer durable product constraints.
      - Write a visual rulebook immediately: premature because Impeccable records `DESIGN.md` from the implemented and reviewed world.
      - Complete product init now and defer final `DESIGN.md` until the reference systems are built and reviewed: selected.
    - Chosen Approach:
      - Run `node .agents/skills/impeccable/scripts/context.mjs`, complete the focused product interview, write `PRODUCT.md`, and preserve the approved architecture decision as the implementation authority. Do not create speculative component styling in this task.
    - API Notes and Examples:
      ```text
      Platform: web
      Surface mode: Operate
      Primary job: edit and extend text-centric workflows for long sessions
      Binding requirement: package-replaceable UI design system with host-owned behavior
      ```
    - Files to Create/Edit:
      - `PRODUCT.md`: Confirmed product truth and durable constraints.
      - `.impeccable/config.json`: Edit only if the user explicitly chooses a standing build path during init.
    - References:
      - `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Product record review: Confirm required schema comment, platform, users, purpose, constraints, principles, and accessibility sections contain confirmed facts only.

- [ ] Review existing UI primitives and finalize the component-recipe matrix
  - Acceptance Criteria:
    - Functional: Inventory all implemented package component kinds, Clay-native surfaces, React Aria parts/slots, CSS Modules, semantic variants, applicable interaction states, and existing token consumers; define stable semantic slots and required fallback resolution for each.
    - Performance: Matrix marks layout-neutral versus layout-affecting properties and prohibits runtime parsing, package execution, selector matching, and unbounded style expansion during React render/input paths.
    - Code Quality: New slots and properties are generic across design systems and components; color-valued properties accept only semantic active-theme role references, no property is named for Neobrutal or Glass styling, and no off-catalog component is introduced without documented need.
    - Security: Matrix excludes raw CSS, selectors, URLs, content injection, arbitrary filters/transforms, JSX, callbacks, scripts, and direct Tauri APIs.
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
      - `docs/development/react-ui-catalog-mapping.md`
      - `docs/wiki/modules/frontend-theme-runtime.md`
      - `docs/wiki/modules/react-sdui-package-ui.md`
      - React Aria styling docs via Context7 `/websites/react-aria_adobe`
    - Options Considered:
      - Expose DOM class names as recipe selectors: flexible but makes markup public and unsafe. Rejected.
      - Define recipes only for package-facing components: misses shell, settings, chat, editor chrome, and overlays. Rejected.
      - Define host-owned semantic component/slot/state identifiers for package-facing and internal surfaces: selected.
    - Chosen Approach:
      - Produce a checked-in matrix with one row per component or internal surface and slot. Record React owner, React Aria state attributes, variants, applicable states, fallback recipe, allowed property families, color source, accessibility invariant, and migration owner. Every color source must be an active-theme role or browser/OS forced-color value.
    - API Notes and Examples:
      ```text
      component=button
      slot=root
      variant=primary
      states=rest,hover,active,focus,disabled
      required_fallback=core.button.primary.root
      behavior_owner=React Aria Button
      ```
    - Files to Create/Edit:
      - `docs/development/ui-design-system-recipe-matrix.md`: Complete component, slot, state, and property inventory.
      - `docs/development/react-ui-catalog-mapping.md`: Link stable recipe slots to existing React owners without changing behavior ownership.
      - `.agents/skills/clay-ui/references/components.md`: Add recipe-slot contract entries after names are finalized.
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
      - `.agents/skills/project-patterns/references/ui-modernization.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Matrix coverage test: Every implemented `ComponentKind` and listed Clay-native surface has at least one recipe owner and fallback.
    - State coverage test: Every applicable React Aria state maps to a semantic recipe state without CSS selector exposure.

- [ ] Define versioned recipe, theme-role reference, non-color value, inheritance, and fallback types
  - Acceptance Criteria:
    - Functional: Add typed Rust data for design-system identity, schema version, component recipe keys, semantic slots, variants, states, semantic theme color-role references, non-color property values, namespaced non-color values, inheritance, and fully resolved fallback output.
    - Performance: Bound recipe count, slot count, state count, property count, structured shadow layers, string lengths, and serialized size; use ordered maps or deterministic sorting for stable snapshots and tests.
    - Code Quality: Closed enums represent known states/properties; color-valued properties accept only known active-theme color-role references, while typed design-system values represent lengths, opacity, levels, shadow geometry, blur, saturation, border style, easing, and transform presets only where the primitive audit proves a generic consumer.
    - Security: Deserialization denies unknown executable or selector-bearing fields, non-finite values, oversized payloads, URLs, raw CSS strings, hex/RGB/HSL/named colors, design-system color aliases, and values outside domain bounds.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `src/shell/theme.rs`
      - `src/shell/components.rs`
      - `src/packages/manifest.rs`
      - `src/packages/record/theme.rs`
    - Options Considered:
      - Encode recipes as arbitrary JSON maps: small initial implementation but weak typing and poor diagnostics. Rejected.
      - Add a new core token for every component/state/property combination: safe but explodes the global token namespace. Rejected.
      - Use typed namespaced non-color design values, semantic active-theme color-role references, a closed recipe-property schema, and host fallback: selected.
    - Chosen Approach:
      - Introduce the smallest dedicated design-system module needed to keep recipe structure separate from content-theme values. Reuse existing non-color token types where semantics match; make every recipe color property a validated reference to an existing theme `color-role`; add only generic non-color typed value forms required by the matrix. Resolve inheritance before state reaches Tauri or React.
    - API Notes and Examples:
      ```json
      {
        "version": 1,
        "values": {
          "controlBorder": {"type": "dimension", "value": 2}
        },
        "recipes": {
          "button.primary.root.rest": {
            "borderWidth": {"value": "controlBorder"},
            "backgroundColor": {"themeColor": "accent.primary"}
          }
        }
      }
      ```
    - Files to Create/Edit:
      - `src/shell/design_system.rs`: Typed declarations, resolved recipes, limits, inheritance, and fallback resolution.
      - `src/shell/mod.rs`: Export internal design-system types.
      - `src/shell/theme.rs`: Reuse typed value helpers where appropriate without merging theme and design-system ownership.
      - `src/protocol/runtime.rs`: Add inert design-system state types only if required for the later snapshot boundary.
      - `.agents/skills/clay-ui/references/tokens.md`: Document any justified additive value domains or clarify reused domains.
    - References:
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - Recipe round trip: Valid declarations serialize and deserialize deterministically.
    - Color authority: Every color property accepts known theme roles and rejects literal colors, design-system color aliases, and non-color tokens.
    - Bounds table: NaN, infinity, negative blur, excessive saturation, too many shadow layers, excessive strings, and oversized recipes are rejected.
    - Inheritance resolution: Missing package entries resolve through the Clay fallback without partial state, and fallback colors still reference active-theme roles.
    - Unknown schema test: Unsupported versions and unknown property/state names fail with actionable diagnostics.

- [ ] Add manifest and package-record validation for inert UI design-system contributions
  - Acceptance Criteria:
    - Functional: `clay.contributions.uiDesignSystem` becomes the sole package registration path for design-system recipes; package records retain exact name, version, integrity/provenance, trust domain, generation, schema version, and validated resolved contribution metadata.
    - Performance: Validation runs at package parse/adoption or configuration reload, never during React render, browser input, layout, or animation frames.
    - Code Quality: Validation shares existing manifest diagnostics and package-record limits, rejects duplicate keys deterministically, and does not add imperative runtime registration APIs.
    - Security: Trusted classification still comes from bundled inventory and integrity, adopted packages remain third-party, and design-system data grants no script, renderer, filesystem, network, shell, process, Tauri, raw-color, or independent-palette authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/packages/creating-packages.md`
      - `docs/reference/primitives/package-security.md`
      - `src/packages/manifest.rs`
      - `src/packages/record/ui.rs`
      - `src/packages/record/theme.rs`
    - Options Considered:
      - Imperative `serverRegisterDesignSystem` facade: duplicates manifest state and violates the single-manifest package pattern. Rejected.
      - Treat design systems as ordinary theme `designTokens`: cannot represent component recipes cleanly. Rejected.
      - Add one declarative manifest contribution parsed into package records: selected.
    - Chosen Approach:
      - Extend manifest and record assembly with a data-only contribution. Reuse exact provenance and package error context. Do not execute a package load entry to discover or register design-system data.
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
      - `src/packages/manifest.rs`: Manifest schema and contribution field.
      - `src/packages/record/mod.rs`: Package-record assembly integration.
      - `src/packages/record/theme.rs` or `src/packages/record/design_system.rs`: Contribution validation and provenance retention; final location follows module cohesion.
      - `src/packages/bundled.rs`: Bundled-inventory validation if data-only packages require classification support.
      - `tests/package_ui_conformance.rs`: Manifest and recipe drift/deny tests.
      - `tests/package_loading.rs`: Data-only contribution load and invalid-package tests.
    - References:
      - `.agents/skills/project-patterns/references/package-manifest-single-source.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
  - Test Cases to Write:
    - Valid contribution: First-party and adopted packages assemble identical inert recipe shapes with distinct provenance.
    - Raw authority denial: CSS, selectors, JSX, script paths, callback names, URLs, literal colors, design-system color aliases, and Tauri fields are rejected.
    - Trust classification: An adopted `@clay/*`-named package is not promoted to trusted.
    - Duplicate and stale schema diagnostics: Errors identify package, field, rejected value, and expected type.

- [ ] Add deterministic recipe, catalog, and documentation conformance gates
  - Acceptance Criteria:
    - Functional: Tests fail when component/slot/state/property catalogs drift from Rust enums, recipe matrix, Clay UI skill references, or package authoring docs.
    - Performance: Conformance tests run offline and add no production runtime work.
    - Code Quality: Failure messages name stale files and repair steps; tests inspect structured registries rather than relying only on fragile whole-file text matches where a typed source is available.
    - Security: Gates ensure prohibited authorities and design-system-owned colors remain absent and every resolved recipe has required focus, disabled, validation, selected, forced-color, reduced-motion, and reduced-transparency handling where applicable.
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
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
    - Options Considered:
      - Rely on reviewer discipline: cheap but allows catalog drift. Rejected.
      - Snapshot generated CSS only: useful later, but does not prove manifest/schema/docs parity. Rejected.
      - Add focused typed parity and deny-list tests now, then CSS snapshots in the frontend-runtime plan: selected.
    - Chosen Approach:
      - Extend current UI conformance tests with exact catalog parity, recipe-key coverage, schema-version coverage, prohibited-field checks, and docs links. Keep checks deterministic and non-mutating.
    - API Notes and Examples:
      ```text
      failure: recipe slot `button.root` missing from docs/development/ui-design-system-recipe-matrix.md
      repair: update matrix, Clay UI component catalog, and package authoring guide together
      ```
    - Files to Create/Edit:
      - `tests/package_ui_conformance.rs`: Recipe/catalog/property parity and prohibited-authority tests.
      - `tests/documentation_coverage.rs`: Documentation links and matrix coverage.
      - `.agents/skills/clay-ui/references/components.md`: Final slot/state entries.
      - `.agents/skills/clay-ui/references/tokens.md`: Final property/value-domain entries.
      - `docs/reference/ui-components.md`: Design-system recipe entry point.
      - `docs/reference/packages/creating-packages.md`: Initial manifest contract and current limitations.
    - References:
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - Catalog drift test: Removing or renaming a documented slot/property fails.
    - Manifest-doc parity test: Contribution keys and schema version match package documentation.
    - Accessibility-state completeness test: Required semantic states resolve after fallback.
    - Color-source conformance: Every normal recipe color resolves through a known active-theme role; only forced-colors rules may resolve browser/OS system colors.

- [ ] Perform visual screenshot and accessibility review of the compatibility fallback
  - Acceptance Criteria:
    - Functional: Launch the real Linux React client with no active package recipe under at least two materially different content themes, confirm current appearance and behavior remain unchanged, confirm all component colors follow theme selection, and exercise representative button, text input, dropdown, modal, tab, package panel, Command Centre, and editor-chrome states.
    - Performance: Review records startup/render regressions and confirms schema/record work causes no per-frame package parsing or visible transition churn.
    - Code Quality: Store default, focused, disabled, validation, modal, narrow, and wide screenshots under `.impeccable/review/plan-101/` with written findings.
    - Security: Accessibility inspection confirms host semantics, focus order, names, roles, states, modal containment, and package intent routing remain host-owned.
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
      - Source inspection only: cannot prove visual or accessibility non-regression. Rejected.
      - Screenshot only: misses role, focus, and state regressions. Rejected.
      - Real GUI screenshot plus computer-use accessibility review: selected.
    - Chosen Approach:
      - Call `get_app_state` first through `computer-use-linux`, inspect the accessibility tree, navigate with keyboard only, capture every named state, and record exact blockers instead of claiming success if the GUI or tool is unavailable.
    - API Notes and Examples:
      ```text
      .impeccable/review/plan-101/default-wide.png
      .impeccable/review/plan-101/focus-controls.png
      .impeccable/review/plan-101/modal-narrow.png
      ```
    - Files to Create/Edit:
      - `.impeccable/review/plan-101/**`: Screenshot evidence and findings.
      - `docs/development/ui-design-system-recipe-matrix.md`: Correct only evidence-backed omissions found by review.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`
  - Test Cases to Write:
    - Keyboard path: Reach and operate every representative control without pointer input.
    - Accessibility tree: Verify role, name, disabled/selected/expanded/invalid state, and modal containment.
    - Compatibility image review: Compare baseline and fallback screenshots at matching dimensions.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory public Rust functions added by this plan; expose none unless they represent a public capability, and record that design-system selection remains deferred to Plan 102 while manifest contribution parsing is documented as package data.
    - Performance: No public op or facade adds runtime work to rendering or input paths.
    - Code Quality: Internal helpers remain private or `pub(crate)`; any unavoidable public API follows Clay naming/schema/documentation rules and updates generated registry artifacts.
    - Security: No API exposes raw recipe internals, validation bypasses, package promotion, CSS/color injection, independent palette authority, or renderer authority.
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
    - Options Considered:
      - Add imperative recipe-registration APIs: rejected by the single-manifest package decision.
      - Keep schema/validation internal and expose selection later: selected.
    - Chosen Approach:
      - Run a public-function inventory, narrow visibility, update package manifest reference docs, and use `cargo run --bin update-doc-registry` only if authoritative Clay JS API Markdown changes.
    - API Notes and Examples:
      ```text
      Public callable added in Plan 101: none expected
      Package declaration path: package.json -> clay.contributions.uiDesignSystem
      Selection API owner: Plan 102
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`: Public manifest contribution documentation.
      - `docs/index.md`: Link new public design-system reference when introduced.
      - `src/docs/generated.rs` or current generated registry artifact: Update only through project generator if API Markdown changes.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - Rust visibility inventory: New public server functions have a documented facade or are narrowed.
    - Registry freshness: `cargo test` detects stale API docs artifacts.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Confirm this foundation plan adds no user-selectable behavior yet and therefore does not add an undocumented configuration key; preserve `setTheme` as the sole concrete color-selection path and reserve design-system selection implementation for Plan 102.
    - Performance: No configuration reload work is added beyond package manifest validation already required at load time.
    - Code Quality: Configuration ownership remains `~/.config/clay/init.js`; no environment variable, JSON settings file, or frontend-local preference becomes a competing source.
    - Security: Configuration cannot bypass package adoption, provenance, schema validation, trust-domain rules, or content-theme color authority.
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
      - `docs/reference/clay-js-api/theme/set-theme.md`
      - `examples/init.js`
    - Options Considered:
      - Add an inactive placeholder `setDesignSystem`: creates a documented API that cannot work. Rejected.
      - Add no setting until atomic activation exists: selected.
    - Chosen Approach:
      - Record the Plan 102 dependency in docs and ensure schema work has no hidden user-facing option.
    - API Notes and Examples:
      ```js
      // Plan 101 intentionally adds no callable selection API.
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`: State that declaration is implemented before activation.
      - `examples/init.js`: No change expected; verify no premature design-system call appears.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Configuration surface scan: No undocumented `designSystem` key or frontend-local selection exists.

- [ ] Execute and update the manual test plan
  - Acceptance Criteria:
    - Functional: Run relevant Linux steps for launch, configuration, packages, themes, and representative UI states; add bounded recipe-manifest validation steps and expected failures.
    - Performance: Record that package recipe validation occurs during load/reload and does not change typing or UI interaction latency.
    - Code Quality: New manual steps use stable module/step IDs, expected results, negative checks, and known ceilings without weakening existing coverage.
    - Security: Manual negative cases cover raw CSS/selector/script rejection, literal/design-system-owned color rejection, and adopted-package trust classification.
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
      - `test-plan/01-launch-and-connection.md`
      - `test-plan/02-configuration-init-js.md`
      - `test-plan/09-packages-and-modes.md`
      - `test-plan/11-performance.md`
    - Options Considered:
      - Defer all manual coverage until visible switching: misses package-validation security checks. Rejected.
      - Add foundation-level negative tests now and expand visual switching coverage later: selected.
    - Chosen Approach:
      - Extend existing package/configuration modules unless the coverage matrix shows a dedicated UI design-system module is clearer; update `test-plan/index.md` only when module coverage changes.
    - API Notes and Examples:
      ```text
      Expected failure: package contribution containing `rawCss` is rejected before activation.
      Expected fallback: no active design-system package preserves current host recipe behavior while all colors continue resolving from the selected content theme.
      ```
    - Files to Create/Edit:
      - `test-plan/02-configuration-init-js.md`: No hidden setting and reload behavior.
      - `test-plan/09-packages-and-modes.md`: Design-system contribution acceptance and denial steps.
      - `test-plan/11-performance.md`: Load-time validation and hot-path non-regression.
      - `test-plan/index.md`: Coverage matrix update if needed.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Manual valid package: Confirm data-only contribution loads without executing renderer code.
    - Manual invalid package: Confirm raw CSS, literal/package-owned colors, and unsupported schema fail with actionable diagnostics.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki documents recipe declarations, semantic active-theme color-role references, rejection of design-system color values, validation, fallback, package-record flow, limits, and current activation boundary after all implementation and verification tasks pass.
    - Performance: Wiki explains install-time resolution and absence of package parsing/execution in frontend hot paths.
    - Code Quality: Pages explain responsibilities, data flow, invariants, source/test paths, examples, extension guidance, and links from the master wiki index.
    - Security: Wiki records prohibited authorities, provenance, trust-domain classification, validation, and fallback without exposing secrets.
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
      - Update after each implementation task: likely to churn as schema names settle.
      - Update once after tests and review pass: selected.
    - Chosen Approach:
      - Add one implementation-focused wiki page and update related theme/package UI pages plus the master index once final code is known.
    - API Notes and Examples:
      ```text
      package.json -> manifest parser -> package record -> resolved fallback recipe
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Link design-system foundation page.
      - `docs/wiki/modules/ui-design-system-runtime.md`: Schema, validation, fallback, and package authority.
      - `docs/wiki/modules/frontend-theme-runtime.md`: Clarify content-theme versus UI design-system ownership.
      - `docs/wiki/modules/react-sdui-package-ui.md`: Link component recipe slots to host component behavior.
    - References:
      - `.agents/skills/project-wiki/references/page-template.md`
  - Test Cases to Write:
    - Wiki navigation review: Every new page is linked from `docs/wiki/index.md`.
    - Documentation coverage: Deterministic tests fail for missing design-system wiki/reference links.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
