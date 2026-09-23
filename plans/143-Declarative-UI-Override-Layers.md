# Plan 143 — Declarative Override Layers: User Theme, Recipe, and Status Segments

Source: the 2026-09-20 configurability review, deviation D3. The UI layer
contract intentionally fixes geometry/material/motion ownership
(DESIGN.md §3; decision `2026-08-28-2234`), but the result is that a user
wanting one different color must ship a full 48-entry theme package to npm
(plan 141 softens distribution, not authoring), design-system recipes have
no user overlay, and the status bar — Emacs' most-hacked surface — has no
declarative user-contributable segments (the `statusItem` SDUI component and
status-region contributions exist for packages:
`frontend/src/sdui/registry.tsx:124`, `src/packages/record/ui.rs`).

Binding prior decisions:

- `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`:
  typed recipe boundary; content themes sole color authority; validation is
  structural and part of activation, "not review".
- `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`:
  user-owned typography precedent — user config owns concrete values,
  packages declare semantics.
- `decision-logs/2026-07-19-0328-configuration-keymaps-survive-mode-activation.md`
  (pattern): user config as durable overlay over package-selected state.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`:
  contributions stay inert validated data; Clay owns composition.
- `DESIGN.md` §4/§11/§16: Quiet Instrument is the normative language; the
  contrast gate floors (prose 4.5 / non-text 3.0 / hairline 1.2,
  `src/shell/theme.rs:987-1099`) are non-negotiable.

Roadmap position: deepens the "design it in their own way" goal as data, not
renderer authority; depends on plan 140's user-config identity for the
status-segment imperative path; plan 141 makes personal theme *packages*
easy — this plan makes small theme edits not need a package at all.

## Objectives

- `theme.setTokenOverride` (init.js): merge a small set of core-role color
  overrides over the active theme **before** validation — the contrast gate
  runs on the merged result and refuses activation with the pair + ratio
  named, exactly as for packages.
- `theme.setRecipeOverride` (init.js): typed, bounded, non-color recipe
  property overrides over the active design system, validated by the same
  schema/bounds as package recipes; colors remain theme-role references
  only.
- Declarative status-bar segments settable from init.js (via the existing
  status-region contribution family once plan 140's identity makes the
  imperative facade callable), with package-vs-user precedence documented
  and deterministic.
- Zero renderer authority added: all three surfaces are validated inert
  data flowing through existing transport (CSS custom properties, recipe
  projection, SDUI status snapshots).

## Expected Outcome

- `init.js` with `theme.setTokenOverride({ "accent.primary": "#…" })` renders
  the override everywhere the role is consumed, survives theme switches by
  re-merging over the newly selected theme, and fails closed (diagnostic +
  previous generation) when the merged theme misses a contrast floor.
- Recipe overrides change geometry/material properties only; any color-ish
  or out-of-schema key is rejected with the package-validation error shape.
- A user status segment renders in the status bar with cataloged
  `statusItem` semantics; removing the init.js line withdraws it on reload.
- Contrast-gate, design-system-conformance, and visual/a11y reviews pass;
  example config and test plan updated.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: seven-stage gate results recorded under
      `code-reviews/<date>-plan140-baseline/logs/`; current behavior
      captured: no user token override API (theme changes require package
      selection), status bar segments package-only.
    - Performance: design-system conformance suite runtime recorded as the
      baseline to not regress.
    - Code Quality: baseline before edits.
    - Security: n/a (no trust-boundary change in this plan).
  - Approach:
    - Documentation Reviewed: AGENTS.md platform validation;
      planning-checklist.md.
    - Options Considered: share plan-140 baseline — rejected: this plan
      touches theme validation, a different regression surface.
    - Chosen Approach: gates + surface inventory notes.
    - API Notes and Examples:
      ```bash
      cargo test --lib theme
      ```
    - Files to Create/Edit: `code-reviews/<date>-plan140-baseline/README.md`.
    - References: `src/shell/theme.rs`.
  - Test Cases to Write: none (evidence capture).

- [ ] Review Clay UI catalog, theme validation, and design-system recipe primitives before UI work
  - Acceptance Criteria:
    - Functional: inventory with paths — contrast gate
      (`src/shell/theme.rs:987-1099`, `composited_contrast_ratio`,
      required-pair table), theme selection + `setTheme`/`setAppearance`
      ops (`src/server/ops/theme.rs`), package token resolution
      (`theme_resolver_for_package_tokens`, `src/packages/record/theme.rs`),
      design-system recipe validation + projection
      (`src/packages/record/ui.rs`, `frontend/src/components/recipe-attributes.ts`),
      the status-region contribution family (manifest keys, budget, SDUI
      transport), the `statusItem` catalog component
      (`frontend/src/sdui/registry.tsx:124`); state where a user overlay
      merges in each pipeline and which validators see the merged result.
    - Performance: overlay application is generation/selection-time only;
      paint reads stay cached CSS custom properties.
    - Code Quality: overlays reuse validators; no parallel "user theme"
      model.
    - Security: overlays are inert data; no new authority.
  - Approach:
    - Documentation Reviewed:
      `DESIGN.md` (§3 layer contract, §4/§11/§16 values/recipes),
      `.agents/skills/clay-execution/references/ui.md`,
      `references/components.md`, `references/tokens.md`,
      `references/config.md` (UI Design-System Packages, Typography
      ownership), `docs/wiki/modules/rendering-primitives.md`,
      `docs/wiki/modules/package-loading.md`.
    - Options Considered:
      - A user theme *package* auto-generated from overrides — rejected:
        packages are distribution units, not edit sessions; merge-before-
        validate is smaller and keeps the gate as the single check.
      - Free-form CSS variables passthrough — rejected: uncontrolled host
        CSS is prohibited by the layer contract.
    - Chosen Approach: typed overlay facades merging into the existing
      validated pipelines.
    - API Notes and Examples:
      ```rust
      // merge point sketch: user overlay applied, then validate(theme)
      let merged = apply_user_overrides(active_theme, &overrides)?;
      validate_theme_contrast(&merged)?; // existing gate, unchanged floors
      ```
    - Files to Create/Edit:
      `code-reviews/<date>-plan140-baseline/primitive-review.md` (new).
    - References: decision `2026-08-28-2234`;
      `plans/118-…md` (contrast gate introduction).
  - Test Cases to Write: none (review verified by later tasks).

- [ ] Review and implement the typed UI design-system recipe boundary (design-system duty)
  - Acceptance Criteria:
    - Functional: `theme.setRecipeOverride({ component, property, value })`
      entries validate against the same typed property schema, bounds, and
      component/slot/state vocabulary as package recipes; color-valued or
      unknown properties rejected; overrides re-apply on design-system
      switch by re-merging; fallback recipes remain fallback until
      overridden.
    - Performance: recipe projection cache invalidation unchanged
      (selection/generation-time).
    - Code Quality: single validator path; additive properties only.
    - Security: recipes stay non-color; literal colors rejected with the
      package error shape; provenance `user-config` in diagnostics.
  - Approach:
    - Documentation Reviewed: `references/config.md` (UI Design-System
      Packages — every bullet), `references/tokens.md`,
      `docs/reference/packages/creating-packages.md`.
    - Options Considered: expose raw recipe JSON editing — rejected: schema
      validation is the boundary's whole point.
    - Chosen Approach: facade + merge + existing validation.
    - API Notes and Examples:
      ```js
      import { setRecipeOverride } from "clay:theme";
      setRecipeOverride({ component: "button", property: "radius", value: 5 });
      ```
    - Files to Create/Edit: `src/server/ops/theme.rs`, theme/recipe
      pipeline files per review task, `runtime/js/theme.js` facade,
      `runtime/js/theme.d.ts`.
    - References: decision `2026-08-28-2234`.
  - Test Cases to Write:
    - `recipe_override_validates_against_package_schema`.
    - `recipe_override_rejects_color_values`.
    - `recipe_override_reapplies_on_design_system_switch`.

- [ ] Implement `theme.setTokenOverride` with gate-on-merged-theme semantics
  - Acceptance Criteria:
    - Functional: overrides keyed by core color-role token names; merged
      over the active theme before validation; contrast floors enforced on
      the merged result with pair + ratio in the diagnostic; failure
      preserves the previous generation (Phase 19 semantics); theme switch
      re-merges over the new theme and re-validates; appearance
      (light/dark) variants handled by the theme's own structure.
    - Performance: merge + validate at selection/generation time only;
      zero paint-path additions.
    - Code Quality: overlay stored in config state alongside typography;
      one merge function shared by both switch paths.
    - Security: `#rrggbbaa`-style literal validation as for package
      designTokens; no URLs/scripts/transform freedom.
  - Approach:
    - Documentation Reviewed: primitive-review output;
      `src/shell/theme.rs` required-pair table; decision
      `2026-07-11-1418` (user-owned concrete values precedent).
    - Options Considered: per-theme override maps (dark vs light separate) —
      deferred: single overlay re-validated per switch is the smaller
      surface; per-theme maps are a follow-up if users hit genuine need.
    - Chosen Approach: one overlay, re-merged on every selection change.
    - API Notes and Examples:
      ```js
      import { setTokenOverride } from "clay:theme";
      setTokenOverride({ "accent.primary": "#c084fc" }); // refused if floors fail
      ```
    - Files to Create/Edit: `src/server/ops/theme.rs`,
      theme resolution pipeline per review, `runtime/js/theme.js` +
      `.d.ts`, `docs/reference/clay-js-api/theme/set-token-override.md`.
    - References: `src/shell/theme.rs:1097` (validate entry).
  - Test Cases to Write:
    - `token_override_below_contrast_floor_refused_with_pair_named`.
    - `token_override_remerges_and_revalidates_on_theme_switch`.
    - `failed_override_preserves_previous_generation`.

- [ ] Enable user status-bar segments through the status-region contribution family
  - Acceptance Criteria:
    - Functional: the imperative status-region facade is callable from
      init.js (plan 140 identity; if the family is manifest-only today, add
      the generic imperative facade with the same declaration validator);
      segments render via the cataloged `statusItem` component; user vs
      package precedence deterministic (documented order, conflict
      diagnostic on duplicate ID); reload withdraws removed segments.
    - Performance: SDUI snapshot budget enforcement unchanged; status
      updates ride existing bounded transport.
    - Code Quality: no new component kinds; font-role/style rules per
      `src/packages/record/ui.rs:1045` apply unchanged.
    - Security: declarations inert; no renderer callbacks.
  - Approach:
    - Documentation Reviewed: `references/components.md` (statusItem),
      `references/ui.md`, `docs/reference/packages/creating-packages.md`
      (status contributions), plan 140 (identity dependency).
    - Options Considered: a bespoke "user statusline" config schema —
      rejected: the contribution family already exists; one more caller is
      the smaller diff.
    - Chosen Approach: verify/complete the imperative facade + docs.
    - API Notes and Examples:
      ```js
      import { serverRegisterStatusRegion } from "clay:ui"; // name per inventory
      serverRegisterStatusRegion({ id: "user.branch", slot: "right",
        text: () => currentBranch(), refreshMs: 2000 });
      ```
      (Exact facade name/shape fixed by the primitive review; the example
      is illustrative.)
    - Files to Create/Edit: `src/server/ops/ui.rs` (if facade missing),
      status-region pipeline per review, facade + docs.
    - References: `frontend/src/sdui/registry.tsx:124`.
  - Test Cases to Write:
    - `user_status_segment_registers_renders_withdraws`.
    - `user_vs_package_status_conflict_diagnostic`.

- [ ] UI prototype gate coverage check (prototype only if uncovered)
  - Acceptance Criteria:
    - Functional: for each changed visible surface (status bar with a user
      segment; theme/recipe override applied states), check
      `design-artifacts/approved/quiet-instrument-migration/` (and sibling
      approved slugs) for exact surface + state coverage; if any state is
      uncovered (e.g., user segment in narrow layout, long-text overflow),
      build the missing prototype pages under
      `design-artifacts/prototypes/user-override-layers/` per the gate
      (four themes, all states, `file://`-openable) and obtain approval
      into `design-artifacts/approved/user-override-layers/`; cite the
      covering artifact(s) in every implementation task affected.
    - Performance: n/a.
    - Code Quality: coverage decision recorded surface-by-surface with
      artifact paths.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → UI
      Prototype and Explicit User Approval Task; `design-artifacts/README.md`.
    - Options Considered: skip the check because changes are data-driven —
      rejected: the status segment adds visible content to a shipped
      surface; the gate's coverage clause still applies to new states.
    - Chosen Approach: coverage audit first; prototype only the uncovered
      set; no in-place edits of approved artifacts.
    - API Notes and Examples: n/a.
    - Files to Create/Edit:
      `code-reviews/<date>-plan140-baseline/ui-gate-coverage.md` (new);
      conditional `design-artifacts/prototypes/user-override-layers/`,
      `design-artifacts/approved/user-override-layers/`.
    - References: user instruction 2026-09-11.
  - Test Cases to Write: none (gate evidence).

- [ ] Create or verify Clay JS APIs for the new facades
  - Acceptance Criteria:
    - Functional: `theme.setTokenOverride`, `theme.setRecipeOverride` (and
      the status facade if new) have full schema-compliant docs (stable ID,
      user-facing name, key bindings `[]`, `custom_properties` for every
      behavior-changing option, security notes stating inert-data boundary,
      lookup tags), master-index links, regenerated registry; `cargo test`
      fails on drift.
    - Performance: n/a.
    - Code Quality: naming per `references/js-api.md` (no `clay.` prefix in
      IDs; batch/table form where options repeat).
    - Security: docs state overrides grant no renderer authority.
  - Approach:
    - Documentation Reviewed: `references/js-api.md`;
      `docs/reference/clay-js-api/schema.md`.
    - Options Considered: fold both overrides into one
      `theme.setOverride` — rejected: different validation vocabularies
      (color roles vs recipe properties) deserve distinct schemas.
    - Chosen Approach: two facades, one docs section.
    - API Notes and Examples:
      ```toml
      # api-inventory.toml
      [[api]]
      id = "theme.setTokenOverride"
      js_module = "clay:theme"
      ```
    - Files to Create/Edit: `docs/reference/clay-js-api/theme/**`,
      `api-inventory.toml`, `docs/index.md`,
      `docs/generated/clay-js-api-registry.json` (via
      `cargo run --bin update-doc-registry`).
    - References: decisions `2026-05-08-1509`, `2026-05-08-1840`.
  - Test Cases to Write: existing registry coverage gates extended to the
    new entries.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: the three override surfaces are documented init.js
      configuration APIs (not hidden keys); `getConfigurationState()`
      reports active overrides; no undocumented behavior-changing settings
      shipped.
    - Performance: state snapshot bounded.
    - Code Quality: configuration docs follow the canonical example file's
      style.
    - Security: configuration grants no filesystem/network/shell/extension
      authority (statement in docs).
  - Approach:
    - Documentation Reviewed: `references/config.md`.
    - Options Considered: a `theme.json` sidecar — rejected: decision
      1841 makes init.js the entry point.
    - Chosen Approach: init.js facades.
    - API Notes and Examples: see facade tasks.
    - Files to Create/Edit: `docs/reference/clay-js-api/configuration.md`.
    - References: decision `2026-05-08-1841`.
  - Test Cases to Write: `configuration_state_reports_overrides`.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: "Theme and design overrides" section with one active,
      copy-safe token override (a shipped-theme-safe value verified by the
      launch test) and commented recipe/status examples, each annotated
      once; `node --check` passes.
    - Performance: n/a.
    - Code Quality: canonical file style.
    - Security: active example passes the contrast gate on all four shipped
      themes (verified in the launch test).
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Example
      Configuration Maintenance Task.
    - Options Considered: commented-only section — rejected: the canonical
      file should exercise the surface actively when copy-safe.
    - Chosen Approach: minimal active override + commented variants.
    - API Notes and Examples:
      ```js
      theme.setTokenOverride({ "text.muted": "#8b949e" }); // illustrative
      ```
    - Files to Create/Edit: `examples/config/init.js`.
    - References: user instruction 2026-08-03.
  - Test Cases to Write: `node --check` gate.

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: scratch HOME + copied example; GUI launch; Connected;
      clean generation; the active token override visually applies across
      theme switches (all four shipped themes exercised); the recipe
      override shows its geometry effect; a user status segment renders;
      invalid-override variant produces the named-pair diagnostic and the
      previous generation stays active.
    - Performance: startup budgets unchanged.
    - Code Quality: commands + results in evidence.
    - Security: scratch profile.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Live
      Launch-Test Task.
    - Options Considered: automated conformance only — insufficient; the
      gate lesson from plan 109 is precisely visual silent-fallback.
    - Chosen Approach: full GUI launch with theme-switch pass.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: evidence in plan file.
    - References: decision `2026-08-28-2234` (silent-fallback caution).
  - Test Cases to Write: manual evidence.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: real Linux GUI, representative data; exercise default,
      overridden, and invalid-override-recovery states; user status segment
      in narrow and wide layouts; screenshots inspected and stored;
      compare against the plan's covering approved artifacts surface by
      surface with per-deviation disposition; keyboard-only pass over the
      status bar; contrast re-verified through the gate (not eyeball).
    - Performance: n/a.
    - Code Quality: evidence paths recorded.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Mandatory
      UI Visual and Accessibility Review Task; `references/ui.md`.
    - Options Considered: source-inspection-only — prohibited by the duty.
    - Chosen Approach: `computer-use-linux` when available; else record
      blocker, keep structural checks, leave visual acceptance unresolved.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `code-reviews/<date>-plan140-visual/`.
    - References: decision `2026-08-14-0200`.
  - Test Cases to Write: manual evidence.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: theme/settings modules gain numbered steps: override
      apply/switch/refuse flows, recipe override, status segment add/
      withdraw; results recorded on Linux.
    - Performance: n/a.
    - Code Quality: no weakened steps.
    - Security: negative step: below-floor override refused with pair
      named.
  - Approach:
    - Documentation Reviewed: `test-plan/index.md`.
    - Options Considered: automated-only — rejected (user-visible).
    - Chosen Approach: extend theme + settings modules.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `test-plan/theme*.md`, `test-plan/settings*.md`,
      `test-plan/index.md`.
    - References: user instruction 2026-08-04.
  - Test Cases to Write: the added steps.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki documents overlay merge points, gate-on-merged
      semantics, precedence, and budgets; index updated.
    - Performance: none added; performance-relevant details documented.
    - Code Quality: docs-as-code bar.
    - Security: layer-contract boundary restated.
  - Approach:
    - Documentation Reviewed: `references/docs-as-code.md`.
    - Options Considered: per-task updates — rejected (churn).
    - Chosen Approach: one post-pass.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `docs/wiki/modules/theme-*.md`,
      `docs/wiki/modules/package-ui-*.md` as located, `docs/wiki/index.md`.
    - References: docs-as-code reference.
  - Test Cases to Write: manual wiki review.

## Compromises Made
- To be filled after execution.

## Further Actions
- To be filled after execution. Known candidates: per-theme (dark/light)
  override maps; moving the status-segment path earlier if plan 140 lands
  late; pane-content contribution points (terminal, image viewer) —
  separate plans with their own primitive reviews.
