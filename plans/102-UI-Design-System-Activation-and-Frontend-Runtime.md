# UI Design-System Activation and Frontend Runtime

Depends on `plans/101-UI-Design-System-Recipe-Foundation.md`.
Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.

## Objectives

- Add one atomic active UI design-system selection beside existing content-theme and typography state.
- Resolve exact adopted or bundled package contributions through existing package provenance and trust-domain rules.
- Carry bounded resolved recipes through the runtime snapshot and Tauri DTO boundary, preserving color properties as semantic active-theme role references rather than concrete colors.
- Install deterministic CSS custom properties in React without package code, selector injection, per-render parsing, or design-system-owned color values.
- Expose and document a one-line `init.js` selection API with safe fallback and revocation behavior.

## Expected Outcome

- Users can select an installed/adopted package through `setDesignSystem("@clay/design-glass")` while `setTheme` and `setTypography` remain independent.
- Server activation validates package identity, current provenance, schema, resolved recipe completeness, contrast, and bounds before replacing active state.
- Runtime generation snapshots carry one coherent `activeDesignSystem` DTO and reject stale, oversized, invalid, or literal-color updates atomically.
- React installs resolved design-system variables once per accepted snapshot and removes obsolete variables on replacement or fallback; color variables remain indirections to the active content-theme CSS roles, so theme switching recolors all components without reinstalling or modifying the design system.
- Missing, revoked, invalid, or removed packages fall back to Clay's built-in recipe without leaving partial styles.

## Tasks

- [x] Revalidate the primitive, authority, and package-loading boundary before activation work
  - Acceptance Criteria:
    - Functional: Re-read Plan 101 artifacts and trace selection from `init.js` through theme ops, package service, runtime generation, protocol snapshot, Tauri projection, frontend session install, and CSS variable ownership.
    - Performance: Baseline snapshot size, install time, CSS variable count, React render count, and theme-switch cost before adding active design-system state.
    - Code Quality: Record exact owners and reuse current package/theme/runtime primitives before adding new state or modules.
    - Security: Confirm trusted classification, adopted-package provenance, revocation, generation checks, and main-webview capability denial remain unchanged.
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
      - `docs/wiki/modules/ui-design-system-runtime.md`
      - `docs/wiki/modules/frontend-theme-runtime.md`
      - `docs/reference/primitives/package-loading.md`
    - Options Considered:
      - Build a second package loader for design systems: duplicates adoption and provenance logic. Rejected.
      - Reuse package records and add one selection/resolution layer: selected.
    - Chosen Approach:
      - Produce a short activation-flow update in the existing matrix/wiki before implementation only when Plan 101 evidence is stale; otherwise record baseline measurements in task completion evidence.
    - API Notes and Examples:
      ```text
      init.js -> theme.setDesignSystem -> package record lookup -> recipe resolution
      -> theme-role references + non-color values -> runtime generation snapshot
      -> Tauri DTO -> frontend atomic install alongside active theme values
      ```
    - Files to Create/Edit:
      - `docs/development/ui-design-system-recipe-matrix.md`: Update activation owner columns if Plan 101 left them unresolved.
      - `docs/development/tauri-react-parity-ledger.json`: Add design-system activation capability and verification owner.
      - `frontend/src/test/theme-adapter.test.ts`: Baseline measurements fixture for CSS variable count and install timing.
      - `src-tauri/tests/dto_roundtrips.rs`: Baseline DTO snapshot size and bounds test.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Baseline measurement fixture: Record runtime snapshot bytes and frontend variable-install timing for current theme state.


- [x] Implement server-owned active design-system selection and fallback lifecycle
  - Acceptance Criteria:
    - Functional: Add `setDesignSystem` selection against exact current package records; install one resolved active design system per runtime generation; preserve built-in fallback when no selection exists; revoke or remove stale selections on package disable, removal, update, or approval loss.
    - Performance: Selection and recipe resolution run during configuration evaluation/generation replacement only; activation remains bounded and does not block editor input or client paint.
    - Code Quality: Keep content theme, typography, and UI design-system fields separate; selection errors are typed, actionable, and leave previous valid generation active.
    - Security: Adopted packages remain third-party, normal approval cannot promote them, replacement carries exact provenance, and selection grants no package execution or renderer authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `src/server/ops/theme.rs`
      - `src/server/configuration.rs`
      - `src/packages/service.rs`
      - `src/packages/approvals.rs`
      - `docs/reference/clay-js-api/theme/set-theme.md`
    - Options Considered:
      - Merge selected design system into `ActiveTheme`: fewer fields but conflates palette/editor style with component recipes and invalidation. Rejected.
      - Store frontend-local selection: breaks server configuration authority and headless validation. Rejected.
      - Add a separate server-owned active design-system record in the same atomic runtime generation: selected.
    - Chosen Approach:
      - Reuse current configuration transaction and package-record lookup. Resolve against built-in fallback, validate complete output, and commit only with the new runtime generation. Preserve previous generation on any error.
    - API Notes and Examples:
      ```rust
      pub struct ActiveDesignSystem {
          pub specifier: String,
          pub schema_version: u32,
          pub generation: u64,
          pub provenance: DesignSystemProvenance,
          pub recipes: ResolvedRecipeTable,
      }
      ```
    - Files to Create/Edit:
      - `src/server/ops/theme.rs`: Selection op and exact package lookup.
      - `src/server/configuration.rs`: Atomic generation state and fallback behavior.
      - `src/packages/service.rs`: Read-only active contribution lookup if no existing generic lookup suffices.
      - `src/shell/design_system.rs`: Active state and resolved fallback builder.
      - `src/protocol/runtime.rs`: Active state representation owned by runtime snapshots.
      - `src/server/js_runtime/tests.rs`: Configuration, stale generation, revocation, and error preservation tests.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Valid bundled selection: Resolves exact bundled provenance and commits atomically.
    - Valid adopted selection: Resolves third-party provenance without promotion or code execution.
    - Invalid package/type: Non-design-system package is rejected with previous generation preserved.
    - Revocation/removal/update: Active selection falls back or requires reselection according to exact current record.
    - Configuration fault isolation: Optional module failure does not partially replace active recipes.

- [x] Add bounded runtime snapshot and Tauri DTO projection
  - Acceptance Criteria:
    - Functional: Runtime snapshots and replacement messages carry one resolved `activeDesignSystem` with string-safe IDs, schema version, source identity, revision/generation, deterministic non-color recipe variables, semantic active-theme color-role references, and capability-neutral provenance metadata needed by UI/help surfaces.
    - Performance: Snapshot remains under the existing 1 MiB runtime-generation ceiling; define lower design-system contribution and resolved-variable budgets, measure serialization/install cost, and avoid per-component recipe duplication.
    - Code Quality: DTO conversion is fallible, typed, deterministic, and tested at Rust and TypeScript boundaries; color-valued entries carry only known theme-role identifiers, and React never receives archived bytes or raw manifest JSON.
    - Security: Tauri projection omits raw package code, filesystem paths, integrity secrets, approval internals, raw selectors, literal colors, package-owned color aliases, and unvalidated declarations.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `src-tauri/src/bridge/dto.rs`
      - `src-tauri/tests/dto_roundtrips.rs`
      - `frontend/src/bridge/types.ts`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
    - Options Considered:
      - Send the original manifest contribution: leaks unvalidated shape and forces frontend resolution. Rejected.
      - Send full resolved property objects per component instance: redundant and large. Rejected.
      - Send one resolved, deduplicated variable table plus stable recipe-key bindings: selected.
    - Chosen Approach:
      - Extend the existing atomic runtime snapshot. Tauri converts resolved Rust enums to a JSON-compatible DTO with explicit tagged values and sorted keys. Frontend validates shape again before install and never reparses package data.
    - API Notes and Examples:
      ```json
      {
        "specifier": "@clay/design-glass",
        "schemaVersion": 1,
        "revision": "7",
        "variables": {
          "button.primary.root.rest.backgroundColor": {
            "type": "theme-color-role",
            "value": "surface.control"
          }
        }
      }
      ```
    - Files to Create/Edit:
      - `src/protocol/runtime.rs`: Snapshot field and wire-safe design-system state.
      - `src-tauri/src/bridge/dto.rs`: Resolved DTO conversion and deny filtering.
      - `src-tauri/tests/dto_roundtrips.rs`: Envelope shape, bounds, and rejection tests.
      - `frontend/src/bridge/types.ts`: Tagged TypeScript DTO types.
      - `frontend/src/test/bridge.test.ts`: Frontend decode and malformed payload tests.
    - References:
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - DTO round trip: Every supported non-color recipe value and theme-color-role reference preserves type and value.
    - Color denial: Hex/RGB/HSL/named colors and package-owned color aliases cannot cross the DTO boundary.
    - Oversized snapshot: Fails before allocation/install and preserves previous frontend state.
    - Stale revision: Frontend session drops stale design-system state with no variable churn.
    - Authority deny: DTO contains no manifest source path, code entry, raw CSS, selector, or Tauri capability.

- [x] Implement atomic frontend design-system store and CSS custom-property adapter
  - Acceptance Criteria:
    - Functional: Add a dedicated frontend store that validates and installs the complete resolved variable set on the root element, removes stale variables, exposes active identity/revision to diagnostics, and restores built-in fallback on disconnect/revocation according to runtime snapshot semantics.
    - Performance: One accepted runtime generation causes at most one batched root-style mutation phase and bounded React notification; ordinary rendering, typing, pointer input, and state transitions perform no recipe lookup beyond native CSS variable resolution.
    - Code Quality: Content-theme and design-system stores remain separate but install coherently from one accepted runtime snapshot; design-system color variables compile only to references to host-owned content-theme variables, and CSS names are deterministic and collision-free.
    - Security: Adapter emits property values only from typed DTO variants, rejects raw/literal colors and unsafe strings, never creates style elements from package text, and never touches arbitrary selectors or child DOM.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `frontend/src/theme/adapter.ts`
      - `frontend/src/state/theme-store.ts`
      - `frontend/src/app/use-clay-session.ts`
      - React Aria styling docs via Context7 `/websites/react-aria_adobe`
    - Options Considered:
      - Generate package CSS text and append `<style>`: compact but reintroduces raw cascade/selector authority. Rejected.
      - Pass recipe objects through every React component: causes prop churn and render-time work. Rejected.
      - Install host-named CSS custom properties at root and let fixed host CSS consume them: selected.
    - Chosen Approach:
      - Add a pure DTO-to-variable adapter and small external store modeled after current theme runtime. Batch `style.setProperty`/`removeProperty` against a previously installed key set. Install only after full DTO validation.
    - API Notes and Examples:
      ```ts
      installDesignSystem({
        specifier: "@clay/design-glass",
        schemaVersion: 1,
        revision: "7",
        variables: resolvedVariables,
      });
      ```
    - Files to Create/Edit:
      - `frontend/src/theme/design-system-types.ts`: Resolved frontend DTO/value types, unless colocating in `theme/types.ts` is clearer.
      - `frontend/src/theme/design-system-adapter.ts`: Deterministic CSS name/value conversion.
      - `frontend/src/state/design-system-store.ts`: Atomic installation and stale-key removal.
      - `frontend/src/app/use-clay-session.ts`: Install coherent accepted snapshot state.
      - `frontend/src/styles/tokens.css`: Built-in fallback recipe variables only; no package selectors.
      - `frontend/src/test/design-system-adapter.test.ts`: Adapter/install tests.
    - References:
      - `.agents/skills/project-patterns/references/ui-modernization.md`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
  - Test Cases to Write:
    - Atomic install: Components never observe half old/half new variable sets through store notifications.
    - Stale-key removal: Switching from larger to smaller recipe removes obsolete root properties.
    - Unsafe variant: URL, CSS function text, selector text, literal color text, unknown theme roles, and non-finite values are rejected.
    - Theme-only switch: Changing the active content theme recolors every recipe consumer without changing design-system revision, reinstalling recipes, or rerendering component trees.
    - No-op install: Identical revision causes no DOM writes or subscriber notifications.

- [x] Wire fallback recipe variables into representative host components without changing appearance
  - Acceptance Criteria:
    - Functional: Button, text input, dropdown trigger/list, modal shell/scrim, tab, panel, focus ring, and one package component consume the new host-owned recipe variables while rendering identically under built-in fallback and sourcing every color from the active content theme.
    - Performance: Variable consumption adds no React state subscriptions per component and no JavaScript state-style mapping for hover, pressed, focus-visible, selected, invalid, or disabled states.
    - Code Quality: React Aria data attributes and semantic host classes select states; DOM structure and accessibility behavior remain unchanged.
    - Security: Package data can alter only validated property values, not component structure, text, event handlers, roles, or focus behavior.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `frontend/src/components/button.module.css`
      - `frontend/src/components/controls.module.css`
      - `frontend/src/components/modal.module.css`
      - `frontend/src/app/layout/tab-bar.module.css`
      - `frontend/src/sdui/registry.tsx`
    - Options Considered:
      - Migrate every surface in this plan: broad blast radius before runtime is proven. Rejected.
      - Add runtime with no real consumers: cannot prove adapter works. Rejected.
      - Migrate a representative vertical slice and defer exhaustive migration to Plan 103: selected.
    - Chosen Approach:
      - Replace fixed non-color visual declarations only where the recipe matrix already defines variables. Replace color declarations with active-theme role variables; design-system recipe variables may select among those roles but never contain concrete colors. Keep existing non-color values as fallback arguments or root defaults so screenshots remain stable.
    - API Notes and Examples:
      ```css
      .button {
        background: var(
          --clay-recipe-button-default-root-rest-background-color,
          var(--clay-surface-control)
        );
      }
      /* Recipe value must itself be var(--clay-<theme-color-role>). */
      ```
    - Files to Create/Edit:
      - `frontend/src/components/button.module.css`: Recipe variable consumption.
      - `frontend/src/components/controls.module.css`: Dropdown/list state variable consumption.
      - `frontend/src/components/modal.module.css`: Modal/scrim recipe consumption.
      - `frontend/src/app/layout/tab-bar.module.css`: Tab recipe consumption.
      - `frontend/src/sdui/registry.module.css`: Package component recipe consumption.
      - `frontend/src/sdui/registry.test.tsx`: State/data attribute and stable behavior checks.
    - References:
      - `docs/development/ui-design-system-recipe-matrix.md`
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
  - Test Cases to Write:
    - Fallback parity: Computed representative properties match pre-migration values under the same active theme.
    - Theme authority: Switching only the content theme changes every representative component color; design-system DTO and package contain no color literal.
    - State consumption: Hover/pressed/focus-visible/disabled/selected/invalid read distinct variables without React rerender logic.
    - Package boundary: Recipe changes cannot alter rendered label, role, handler, or action intent.

- [x] Run automated activation, frontend, security, and performance verification
  - Acceptance Criteria:
    - Functional: Rust, Tauri, TypeScript, frontend, package, and runtime tests cover selection, snapshot, install, switch, failure, revocation, and fallback.
    - Performance: Measured snapshot bytes, adapter conversion time, root variable writes, React notifications, and representative component render counts remain within recorded budgets.
    - Code Quality: Linux `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, relevant Rust tests, frontend typecheck/lint/format/tests/build, and bundle budget pass.
    - Security: Tests prove no raw CSS/selector/script/color authority, no internal op/module availability to third-party packages, stale-generation rejection, and exact-provenance fallback.
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
      - Validate only unit layers: misses end-to-end generation replacement. Rejected.
      - Add one bounded integration fixture through server, Tauri DTO, and frontend adapter plus focused unit tests: selected.
    - Chosen Approach:
      - Run targeted checks during implementation, then full Linux blocking checks once. Record exact performance measurements and compare with Task 1 baseline.
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
      - `tests/theme_packages.rs`: Selection and fallback integration.
      - `tests/package_ui_conformance.rs`: Security/property boundaries.
      - `src-tauri/tests/dto_roundtrips.rs`: DTO and size tests.
      - `frontend/src/test/design-system-adapter.test.ts`: Install and performance checks.
      - `frontend/src/test/shell.test.tsx`: Representative switched rendering.
      - `docs/development/performance.md`: Record measured design-system install budget if durable.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - End-to-end switch: Valid package selection reaches frontend and changes only approved variables.
    - Invalid update: Previous coherent design system remains installed.
    - Slow/absent consumer: Server and editor remain responsive.
    - Bundle budget: No new styling runtime dependency is added.
  - Completion Evidence (2026-09-01):
    - Focused activation/security/runtime checks passed: three `setDesignSystem` init.js flows (core, adopted third-party, rejection preservation); package-manifest conflict; two presentation conformance/color-authority tests; four atomic runtime-snapshot tests; and four Tauri DTO projection/authority/size tests.
    - Frontend gates passed: typecheck, lint, formatting, 174 Vitest tests, production build, and bundle budget. Focused bridge/design-system/shell tests: 26 passed.
    - Linux release gate passed: `scripts/check.sh full` completed Cargo audit, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, all-target tests, and bench compilation. Cargo audit reported 19 configured allowed dependency warnings and no failing advisory.
    - Measured frontend adapter fixture (300 variables, 100 conversions): 0.295 ms/snapshot local average, 300 initial root writes, zero writes for identical revision, and one subscriber notification. DTO size-budget test enforces the core resolved snapshot below 128 KiB; bundle output is 167.4/180 kB shell gzip and 356.3/400 kB total gzip.

- [x] Perform visual screenshot and accessibility review of activation and fallback states (2026-08-30)
  - Completion Evidence (2026-08-30):
    - Six real-client states captured and reviewed under `.impeccable/review/plan-102/` (all PASS, see `findings.md`): default fallback shell, loading SDUI delivery, explicit `@clay/core` activation on `@clay/theme-gruvbox-material-dark` and `-light` (two materially different themes; panel/action/enabled+disabled list rows/editor verified visually and via AT-SPI), invalid-selection recovery (reload-time failure falls back to the last valid generation, client stays connected, sanitized `JavaScript runtime evaluation failed.` status diagnostic, no paths or internals), and server-stop recovery (`Session lost` alert with `Reconnect session`).
    - Accessibility dumps (`accessibility.txt` per state) show correct roles/names (workspace landmark, SDUI panel/button/list box/list items, alert, status bar); package-controlled values limited to theme variables with host-authored semantics; no secrets (private mode-700 roots).
    - Impeccable detector run once over changed frontend targets: 0 findings (`detector.json`).
    - Harness hardening landed in `scripts/capture-ui-review.sh`: loading-tree wait fix, error fixture now exercises valid-boot → invalid-reload (surfaces the sanitized diagnostic), AT-SPI probe skips stale dead-pid registrations (orphaned windows previously shadowed the live window), and new `ui-review-design-system`/`ui-review-design-system-light` fixtures.
    - Limits recorded in `findings.md`: focus-order driving and modal/completion captures need interactive input synthesis (unavailable here; those fixtures remain interactive); layout captured at the fixed 900×600 logical size.
  - Acceptance Criteria:
    - Functional: Review built-in fallback and one test design-system override against at least two materially different active content themes, plus invalid-selection recovery, revoked-package fallback, default/focus/disabled/invalid/modal states, and narrow/wide layouts in the real Linux client.
    - Performance: Observe switching for flash of unstyled content, partial variable application, excessive repaint, animation churn, or editor interaction stalls.
    - Code Quality: Store screenshots and findings under `.impeccable/review/plan-102/`; run the Impeccable detector once on changed frontend targets and resolve mechanical findings.
    - Security: Use `get_app_state` before interaction; verify roles, names, states, focus order/visibility, modal containment, and package-controlled values without package-controlled semantics.
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
      - `docs/wiki/modules/ui-review-harness.md`
    - Options Considered:
      - Synthetic DOM screenshots only: cannot prove real Tauri session switching. Rejected.
      - Real runtime fixture with package/configuration reload and accessibility inspection: selected.
    - Chosen Approach:
      - Use existing UI review fixtures and capture script, add a design-system fixture, inspect all named screenshots, and perform one confirmation round after batching fixes.
    - API Notes and Examples:
      ```text
      .impeccable/review/plan-102/fallback-wide.png
      .impeccable/review/plan-102/override-focus.png
      .impeccable/review/plan-102/revoked-fallback.png
      ```
    - Files to Create/Edit:
      - `tests/fixtures/configuration/ui-review-design-system/init.js`: Runtime-switch review fixture.
      - `scripts/capture-ui-review.sh`: Add fixture only if existing parameterization cannot express it.
      - `.impeccable/review/plan-102/**`: Screenshots and findings.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`
  - Test Cases to Write:
    - Keyboard-only switch/reload flow: Focus remains visible and stable.
    - Invalid/revoked recovery: No inaccessible partial state or stale package appearance remains.

- [x] Create or verify Clay JS APIs for public programmatic surfaces (2026-08-30)
  - Completion Evidence (2026-08-30):
    - Full chain verified: `runtime/js/theme.js::setDesignSystem` facade export (string or `{ specifier }`, validates non-empty specifier) → registered op `op_clay_theme_set_design_system` (`src/server/ops/mod.rs`) → `apply_design_system` (`src/server/ops/theme.rs`: `@clay/core`/`clay:core`/`core` built-in baseline, first-party `@clay/*` via `ensure_first_party_record`, third-party adoption only through the package service's existing enable path) → `resolve_design_system` core-fallback resolution → frontend adapter projection on snapshot revision.
    - Created authoritative doc `docs/reference/clay-js-api/theme/set-design-system.md` (stable ID `theme.setDesignSystem`, searchable name/user-facing name, empty default key bindings, `specifier` custom-property metadata, sync return `{ specifier, recipeCount, schemaVersion }`, error codes `theme.invalid_request`/`theme.load_failed`/`theme.invalid_design_system`, security/denied sections covering adoption/provenance, no automatic trust promotion, no raw CSS or color authority, active-theme-only color sourcing, fallback and revocation semantics) and linked it in `docs/index.md`.
    - Regenerated `docs/generated/clay-js-api-registry.json` via `cargo run --bin update-doc-registry` (131 entries); added the `api-inventory.toml` row and the parity-ledger `packages.modes.settings.themes` reference.
    - New test `plan102_set_design_system_is_registered_public_theme_api` (tests/clay_js_doc_registry.rs via protocol suite) asserts registry lookups by id and facade export, visibility/module/export/op/stability/async, empty key bindings and permissions, `specifier` metadata, required security markers, and generated-registry currency.
    - Performance: activation resolves only during configuration generation (op runs at init.js evaluation; snapshot shipped via `RuntimeStateSnapshot`); adapter rewrites recipe CSS custom properties only on revision change — prior measured fixture: 0.295 ms/snapshot, zero writes for identical revision, one subscriber notification.
    - Gates: `cargo test --test protocol` 200 passed (includes `generated_registry_is_current`, inventory, facade-layout, and documentation-coverage suites), `cargo test --test presentation` 32 passed, `cargo fmt --check` and `cargo clippy --all-targets` clean.
  - Acceptance Criteria:
    - Functional: Implement and document `theme.setDesignSystem` through an explicit Rust function, `deno_core` op, `clay:theme` JS/TS facade export, stable ID, searchable name, empty default keybindings, and complete custom property metadata.
    - Performance: API resolves during configuration generation and returns without introducing per-frame or per-component work.
    - Code Quality: Callable name is concise, stable ID is `theme.setDesignSystem`, errors/async behavior are documented, and generated registry lookup exposes the API.
    - Security: Documentation states package adoption/provenance requirements, no automatic trust promotion, no raw CSS or color authority, active-theme-only color sourcing, fallback behavior, and revocation semantics.
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
      - `docs/reference/clay-js-api/theme/set-theme.md`
    - Options Considered:
      - Add a new `clay:design` module for one function: unnecessary API surface. Rejected.
      - Add `setDesignSystem` to existing `clay:theme` while keeping state separate internally: selected.
    - Chosen Approach:
      - Implement one explicit facade and authoritative Markdown page, then regenerate docs registry with the project command.
    - API Notes and Examples:
      ```js
      import { setDesignSystem } from "clay:theme";

      setDesignSystem("@clay/design-glass");
      ```
    - Files to Create/Edit:
      - `src/server/ops/theme.rs`: Backing op.
      - `runtime/js/theme.js`: `setDesignSystem` export.
      - `runtime/js/theme.d.ts`: Type declaration.
      - `docs/reference/clay-js-api/theme/set-design-system.md`: Authoritative API documentation.
      - `docs/index.md`: Master index link.
      - `api-inventory.toml`: API metadata if required by current registry architecture.
      - Generated registry artifacts: Update through `cargo run --bin update-doc-registry`.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - Facade test: JS export invokes only the expected op with exact string input.
    - API coverage: Missing Markdown/index/registry/lookup metadata fails `cargo test`.
    - Raw op boundary: User docs contain no direct `Deno.core.ops` usage.

- [x] Create or verify Clay configuration APIs (2026-08-30)
  - Completion Evidence (2026-08-30):
    - One-line setup verified: `set_design_system_core_via_init_js` and `set_design_system_adopted_third_party_via_init_js` (src/server/js_runtime/tests.rs) activate the design system from `init.js` via `setDesignSystem` with no imperative recipe registration; `persisted_preferences_design_system_applied` covers the persisted-preference path; live `init.js` fixtures captured in the Plan 102 UI review (design-system dark/light states).
    - Missing-package safety now pinned by a new server test `reload_with_missing_design_system_preserves_previous_generation_and_reports_diagnostic` (src/server/mod.rs): after a valid `@clay/core` generation, an uncaught `setDesignSystem("@vendor/never-installed-ds")` on reload leaves the generation ID, the active design system value, and the `theme.load_failed` diagnostic exactly as specified — reload rejected, previous generation retained, actionable diagnostic recorded.
    - Same-selection no-op verified: frontend adapter idempotence test (frontend/src/test/design-system-adapter.test.ts "identical revision causes no DOM writes or notifications") plus measured fixture evidence (zero writes on identical generation, one subscriber notification); server stamps `active_design_system.generation = generation_id` once per generation commit and the welcome/resnapshot path re-delivers the same generation without churn.
    - Omission fallback verified in code and tests: no explicit selection preserves the previous valid selection (revocation-safe revalidation against enabled records) and otherwise resolves `ActiveDesignSystem::core_fallback`; `@clay/core`/`clay:core`/`core` specifiers resolve to the built-in baseline without package access.
    - Security re-verified: selection never installs — third-party resolution goes through the package service's existing enable graph (fail-closed on replaced/revoked targets, rollback on failure, approvals untouched); first-party selection uses `ensure_first_party_record`; revoked/missing records fail validation before commit. No JSON/env/local-storage/frontend-only/stock-palette setting introduced; `specifier` custom-property metadata documents string type, `@clay/core` default fallback, and `theme.invalid_request`/`theme.load_failed` error behavior (docs/reference/clay-js-api/theme/set-design-system.md).
    - `docs/reference/clay-js-api/configuration.md` gained a "Plan 102 design-system selection configuration" section linking the API doc and summarizing one-line selection, atomic reload, fallback/revocation, and no-op semantics.
    - Gates: `cargo test --lib reload_with_missing_design_system` passed; `cargo test --test protocol` 200 passed; presentation 32 passed; security 131 passed; `cargo fmt --check` and `cargo clippy --all-targets` clean.
  - Acceptance Criteria:
    - Functional: `~/.config/clay/init.js` can select one design system in one line; configuration reload switches atomically; omission uses built-in fallback; invalid selection preserves previous generation with actionable diagnostics.
    - Performance: Repeated selection of the same exact package/revision is a no-op and does not churn runtime/frontend revisions.
    - Code Quality: No competing JSON, environment, local-storage, frontend-only setting, stock color theme, or design-system palette setting is introduced; custom property metadata documents string package specifier, default fallback, and error behavior.
    - Security: Selection cannot install/adopt a package, expand package permissions, promote trust, or bypass revocation.
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
      - `src/server/configuration.rs`
    - Options Considered:
      - Require `loadPackage` plus selection: unnecessary for a declarative data-only contribution and creates two-step common setup. Rejected.
      - Let `setDesignSystem` install packages: mixes package management with configuration and authority. Rejected.
      - Require prior installation/adoption, then one-line selection: selected.
    - Chosen Approach:
      - Treat `setDesignSystem` as a normal configuration API over existing installed/adopted records. Keep package acquisition and approval separate.
    - API Notes and Examples:
      ```js
      import { setDesignSystem } from "clay:theme";
      setDesignSystem("@clay/design-glass");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/theme/set-design-system.md`: Configuration semantics and custom properties.
      - `docs/reference/clay-js-api/configuration.md`: Link design-system selection.
      - `src/server/configuration.rs`: Reload/no-op/fault-isolation behavior.
      - `tests/clay_js_doc_registry.rs`: Configuration metadata coverage.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - One-line setup: Installed/adopted package activates from `init.js` without imperative recipe registration.
    - Missing package: Configuration fails safely with previous generation retained.
    - Same selection: No revision or DOM mutation churn.

- [x] Update the canonical example configuration (`examples/init.js`) (2026-08-30)
  - Completion Evidence (2026-08-30):
    - `examples/init.js` section 2 (Theme + appearance) gained a design-system block: inert-recipe/color-authority explanation, built-in `@clay/core` baseline needing no call by default, install-and-adopt-before-select note, explicit "selection itself installs nothing and grants no new package authority" clarification, exactly one commented non-default selection (`// setDesignSystem("@clay/design-glass");`), and invalid/revoked-selection behavior (previous generation kept, diagnostic, startup never blocked).
    - Facade import extended to `import { setTheme, setTypography, setAppearance, setDesignSystem } from "clay:theme";` so uncommenting the selection line works without editing imports; no expensive optional effects enabled by default (the section is fully commented).
    - Pinned by extended `canonical_example_covers_theme_typography_and_modular_configuration` (tests/clay_js_doc_registry.rs): exactly one `setDesignSystem(` occurrence, required comment markers (`@clay/core baseline`, `install and adopt its package first`, `grants no new package authority`), updated import assertion.
    - Code Quality gates: `node --check examples/init.js` passes (same check as CI `scripts/package-smoke.sh`); `cargo test --test protocol` 200 passed (includes the copy-safe executed-lines scan); `cargo fmt --check` clean.
  - Acceptance Criteria:
    - Functional: Add one documented design-system section showing built-in fallback and a commented non-default package selection exactly once.
    - Performance: Example does not enable expensive optional effects by default.
    - Code Quality: File remains valid JavaScript, ordering is correct, active content is safe to copy, and API names/defaults match authoritative docs and parsers.
    - Security: Comments explain installation/adoption before selection and clarify that selection grants no new package authority.
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
      - `docs/reference/clay-js-api/theme/set-design-system.md`
      - `api-inventory.toml`
    - Options Considered:
      - Activate Glass in canonical defaults: changes product default prematurely. Rejected.
      - Keep the fallback implicit and show optional selection as commented configuration: selected.
    - Chosen Approach:
      - Place the design-system section beside existing theme/typography appearance configuration and annotate package adoption and fallback.
    - API Notes and Examples:
      ```js
      // import { setDesignSystem } from "clay:theme";
      // setDesignSystem("@clay/design-glass");
      ```
    - Files to Create/Edit:
      - `examples/init.js`: Canonical design-system configuration example.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - JavaScript syntax: `node --check examples/init.js` passes.
    - Example-doc parity: API name, argument, default, and security notes match authoritative docs.

- [x] Execute and update the manual test plan (2026-08-30)
  - Completion Evidence (2026-08-30):
    - Created `test-plan/15-ui-design-systems.md` with steps UI-DS-01…UI-DS-10 plus performance rows UI-DS-P1…P3, stable step IDs, and a dated Linux execution record; updated `test-plan/index.md` (module map row 15 + coverage-matrix row) and cross-referenced `02-configuration-init-js.md` (selection/reload + watcher caveat), `09-packages-and-modes.md` (adoption/revocation), `11-performance.md` (switch budget).
    - Executed live on Linux via isolated server/client sessions (temp HOME/XDG_CONFIG_HOME, explicit socket; evidence in `code-reviews/screenshots/2026-08-30-plan102-design-system-manual/`):
      - UI-DS-01 startup fallback PASS (normal workspace, built-in baseline, clean log; `ds01-fallback-window.png`).
      - UI-DS-02 valid selection + watcher reload PASS (one visible atomic swap ≈2.0 s; log clean; `ds02-core-light-window.png`).
      - UI-DS-03 repeat switch PASS (second atomic swap ≈2–4.5 s incl. poll granularity; no partial frames; `ds03-core-dark-window.png`).
      - UI-DS-04 invalid selection recovery PASS (`runtime reload failed [theme.load_failed]` in server stderr; sanitized `JavaScript runtime evaluation failed.` status text; previous generation intact — `ds04-invalid-statusbar.png`).
      - UI-DS-06 denied adoption PASS (`configuration failed [theme.load_failed]` at startup; packages store unchanged — nothing installed/adopted).
      - UI-DS-10 restart persistence PASS (identical baseline after full stop/start; `e3a`/`e3b` captures).
      - UI-DS-05/07/08/09 PASS via automated suites where the live step needs a second real design system (none bundled): protocol 200, presentation 32, security 131 passed on the same tree.
    - Performance recorded: switching latency dominated by the documented ~2 s watcher debounce, atomic single swap per generation, zero writes for identical generations (automated adapter idempotence); live typing-feel remains UNRESOLVED on this host (no synthetic keyboard input — documented ceiling), covered by the automated no-write test.
    - Two new findings recorded in module 15 (follow-ups, predate Plan 102 UI): F-1 startup `setDesignSystem` on a non-design-system first-party package wedges startup evaluation (client "timed out waiting for server snapshot", no diagnostic; `e1-markdown-window.png`); F-2 watcher reloads stop firing after the first failed reload until restart (reproduced twice). Semantics for the single-failure path remain pinned by `reload_with_missing_design_system_preserves_previous_generation_and_reports_diagnostic`.
  - Acceptance Criteria:
    - Functional: Run and document Linux steps for startup fallback, valid selection, reload switching, invalid selection recovery, package revoke/remove fallback, and app restart persistence through `init.js`.
    - Performance: Record visible switching latency and absence of typing/input stalls, partial styles, or unbounded repaint.
    - Code Quality: Add a dedicated `test-plan/15-ui-design-systems.md` module if design-system coverage would otherwise be fragmented; update coverage matrix and stable step IDs.
    - Security: Steps verify adopted-package provenance, denied automatic adoption/promotion, raw styling/color authority rejection, theme-owned color resolution, and fallback after revocation.
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
      - `test-plan/09-packages-and-modes.md`
      - `test-plan/11-performance.md`
    - Options Considered:
      - Scatter activation checks across three modules: avoids a new file but obscures full workflow. Rejected once the feature is user-visible.
      - Add one focused module and cross-link package/configuration/performance prerequisites: selected.
    - Chosen Approach:
      - Add `15-ui-design-systems.md` with numbered end-to-end steps and update the index coverage matrix.
    - API Notes and Examples:
      ```text
      UI-DS-01 built-in fallback
      UI-DS-02 valid adopted selection
      UI-DS-03 configuration reload switch
      UI-DS-04 invalid/revoked fallback
      ```
    - Files to Create/Edit:
      - `test-plan/15-ui-design-systems.md`: End-to-end activation and recovery steps.
      - `test-plan/index.md`: Module link and coverage matrix.
      - `test-plan/02-configuration-init-js.md`: Cross-reference selection/reload.
      - `test-plan/09-packages-and-modes.md`: Cross-reference adoption/revocation.
      - `test-plan/11-performance.md`: Cross-reference switch/install budget.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Real Linux workflow: Execute every new numbered step and record pass/fail.
    - Negative workflow: Verify invalid package never produces partial frontend variables.

- [x] Update or verify the code wiki after implementation (2026-08-30)
  - Completion Evidence (2026-08-30):
    - `docs/wiki/modules/ui-design-system-runtime.md`: added the Plan 102 activation/lifecycle section — full `setDesignSystem` chain (facade → `op_clay_theme_set_design_system` → `apply_design_system` resolution/fail-closed rules → `ActiveDesignSystem` → generation-commit revalidation with `core_fallback` demotion on revocation → `DesignSystemSnapshotDto` → frontend store install), debugging semantics (init.js/preferences selection, failed-reload preservation with sanitized diagnostics, revalidation-not-event revocation, idempotent install, `resetToFallback` key cleanup), budgets (64 KiB contribution payload compiled; 128 KiB serialized-snapshot test ceiling; measured ≈0.3 ms / 300 variables install), updated file/test header, and a test map covering the new protocol/DTO/server/frontend suites; links the authoritative API doc instead of duplicating usage.
    - `docs/wiki/modules/frontend-theme-runtime.md`: documented separate theme vs design-system stores with coherent install order (`design-system-adapter.ts`, `design-system-store.ts`, `use-clay-session.ts` wiring, `--clay-ds-*` naming, installedKeys removal, color-role indirection to `--clay-*` theme variables, measured install cost) and added the adapter test row.
    - `docs/wiki/modules/desktop-typed-bridge.md`: added the design-system DTO section — `DesignSystemSnapshotDto`/`DesignSystemProvenanceDto` shape, deny fields (no source paths/entry points/manifest JSON/approval state), provenance-is-informational authority note, 128 KiB serialization bound, core-fallback bootstrap default; extended the dto_roundtrips test entry with the design-system round-trip/authority/size pins.
    - `docs/wiki/modules/configuration-runtime.md`: added the Plan 102 selection/reload semantics section (one-line init.js or persisted `designSystem` preference, ui-session precedence, fail-closed resolution errors, watcher reload with previous-generation preservation, generation-commit revalidation, no new configuration keys) with the four server/JS-runtime tests; corrected the stale persisted-preferences text (now four keys including `designSystem`).
    - `docs/wiki/index.md`: refreshed the Frontend Theme Runtime and UI Design System Runtime rows to state the Plan 102 coverage (master index remains complete and linked).
    - Documentation coverage tests green: `cargo test --test protocol documentation_coverage` (11 passed), `cargo test --test protocol clay_js_doc_registry` (50 passed), `cargo test --test protocol primitives_docs` (34 passed) — these pin wiki/index links, reference-doc paths, and the canonical example alignment.
  - Acceptance Criteria:
    - Functional: Wiki explains active selection, package lookup, configuration transaction, runtime snapshot, Tauri DTO, frontend adapter/store, active-theme color indirection, fallback, switch, and revocation after all tasks pass.
    - Performance: Wiki records snapshot/install budgets, no-op behavior, and hot-path exclusion.
    - Code Quality: Pages link source/tests/public API docs and explain extension/testing procedures; master index remains complete.
    - Security: Wiki documents exact provenance, trust-domain status, DTO deny fields, main-webview boundary, and failure preservation.
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
      - Duplicate public usage in wiki: causes drift. Rejected.
      - Explain internals and link authoritative API/package docs: selected.
    - Chosen Approach:
      - Update implementation pages once final code and measurements are stable, then run documentation coverage tests.
    - API Notes and Examples:
      ```text
      setDesignSystem -> configuration generation -> ActiveDesignSystem
      -> RuntimeSnapshotDto -> installDesignSystem -> root CSS variables
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Navigation update.
      - `docs/wiki/modules/ui-design-system-runtime.md`: Activation, lifecycle, and fallback.
      - `docs/wiki/modules/frontend-theme-runtime.md`: Separate stores and coherent install.
      - `docs/wiki/modules/desktop-typed-bridge.md`: DTO shape and bounds.
      - `docs/wiki/modules/configuration-runtime.md`: Selection/reload semantics.
    - References:
      - `.agents/skills/project-wiki/references/page-template.md`
  - Test Cases to Write:
    - Wiki index coverage: Every changed/new page is linked.
    - Docs cross-link review: Wiki links authoritative `setDesignSystem` and package authoring pages instead of duplicating usage docs.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
