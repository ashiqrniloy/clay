> **SUPERSEDED — do not execute (2026-09-11).** Clay adopted the Quiet Instrument
> design language (`DESIGN.md`) and the migration scope recorded in
> `decision-logs/2026-09-11-1700-quiet-instrument-migration-scope-no-third-design-system-chat-removal.md`:
> one shipped design system (`@clay/design-instrument`), the four existing themes, no new
> theme packages. The three languages and three paired themes planned below (Graphite
> Precision, Warm Analog, Glass Cockpit) are out of scope; the migration plan is
> `plans/118-Quiet-Instrument-Migration-Component-and-Surface-Adoption.md`. Kept as history.

# Phase 20.10 — Graphite Precision, Warm Analog, and Glass Cockpit Design-System + Paired Theme Packages

Implements the three user-approved UI design languages from the proposal review (`.impeccable/reviews/design-proposals/index.html`, samples 2/4/5) as first-party packages: three `uiDesignSystem` recipe packages and three content themes with identical display names so the pairing is obvious to users. All six packages ride the existing typed recipe boundary (plans 101–104, decision log 2026-08-28-2234): design systems supply geometry/material/motion referencing **theme color roles only**; themes supply the palettes via `clay.contributions.designTokens`; every design system therefore works with every theme, and each named theme is the best-fit palette for its namesake design system.

## Objectives

- Ship `@clay/design-graphite-precision`, `@clay/design-warm-analog`, `@clay/design-glass-cockpit` as inert `uiDesignSystem` recipe packages covering the full canonical slot matrix (controls + chrome surfaces), selectable via `preferences.designSystem`, `theme.setDesignSystem`, and the Settings selector from plan 110.
- Ship `@clay/theme-graphite-precision`, `@clay/theme-warm-analog`, `@clay/theme-glass-cockpit` content themes with the same display names, each providing dark **and** light palettes through a new additive appearance-keyed designTokens channel.
- Make the pairing discoverable: additive `recommendedThemes` metadata on `uiDesignSystem` contributions, surfaced as a hint in the Settings design-system dropdown.
- Preserve all invariants: theme color authority (no literals in recipes), inert data only, additive-only schema, DOM continuity on switch, revocation/fallback, no backdrop-filter outside transient overlays, reduced-motion/transparency fallbacks, forced-colors behavior.
- Prove orthogonality: each new design system conformance-tested against every first-party theme; each new theme against every first-party design system.

## Expected Outcome

- Settings → Design system lists five entries (Core baseline, Neobrutal, Glass, plus the three new) with recommended-theme hints; selecting any new system restyles the whole shell (post plan-110 consumption) in <100ms without remount.
- Appearance light/dark toggle keeps the active theme family (e.g. Glass Cockpit dark ↔ Glass Cockpit light) instead of falling back to canonical Modus defaults.
- `cargo test` + frontend gates green, including the plan-110 consumption/drift gate with all three new packages' recipe keys consumed.
- Screenshot evidence: 3 new DS × {paired theme, modus-vivendi, modus-operandi} × dark/light on the fixture harness.

## Tasks

- [ ] 1. Verify plan-110 prerequisite wiring before package work
  - Acceptance Criteria:
    - Functional: Plan 110 tasks 1–6 and 10 are complete (canonical slot names, chrome-surface recipe consumption in all chrome CSS, consumption/drift gate test, Settings design-system selector + enumeration snapshot). If not complete, execute them first; this plan adds no CSS consumption work of its own.
    - Performance: No additional runtime cost; prerequisite gates already bounded.
    - Code Quality: Record plan-110 completion state in this plan's evidence; do not duplicate its tasks.
    - Security: Unchanged boundaries.
  - Approach:
    - Documentation Reviewed:
      - `plans/110-UI-Design-System-Consistency-Neobrutal-Legibility-Revamp.md` tasks 1–6, 10
      - `docs/development/ui-design-system-recipe-matrix.md`
    - Options Considered:
      - Fold consumption work into this plan: duplicates 110 and risks divergent slot names — rejected.
      - Hard prerequisite with verification step — chosen.
    - Chosen Approach: Check checkboxes/evidence in plan 110; run `frontend/src/test/design-system-consumption.test.ts` and the Settings selector manually; proceed only when green.
    - Files to Create/Edit:
      - None (verification only).
    - References:
      - `plans/110-…md`.
  - Test Cases to Write:
    - None; prerequisite gates serve.

- [ ] 2. Review Clay UI catalog and plan primitive/component reuse for the three languages
  - Acceptance Criteria:
    - Functional: Inventory the canonical slot matrix and the existing recipe property vocabulary (`borderWidth/borderRadius/shadow/motion/opacity/backdropBlur/…` from `src/shell/design_system.rs`); confirm all three languages are expressible with existing typed properties (they are: radii, 1–2px borders, layered shadows, blur-on-overlay, motion durations); list any property gap (expected: none; tick-mark motif and paper texture are out of scope for recipes and must be dropped or approximated with existing typed properties).
    - Performance: Static review only.
    - Code Quality: Output is a short matrix memo appended to `docs/development/ui-design-system-recipe-matrix.md` (three new columns).
    - Security: No new authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`, `.agents/skills/clay-ui/references/components.md`, `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`, `.agents/skills/full-output-enforcement/SKILL.md`, `.agents/skills/high-end-visual-design/SKILL.md`, `.agents/skills/design-taste-frontend/SKILL.md`
      - `.impeccable/reviews/design-proposals/index.html` (approved token sets per style)
      - `src/shell/design_system.rs` (typed property vocabulary)
    - Options Considered:
      - Add new recipe property types for tick motifs/textures: speculative, violates additive-only-when-needed — rejected (YAGNI).
      - Express identities purely with existing properties — chosen.
    - Chosen Approach: Memo + matrix columns; recipes in tasks 7–9 use only existing typed properties.
    - Files to Create/Edit:
      - `docs/development/ui-design-system-recipe-matrix.md`: add Graphite/Analog/Cockpit columns.
    - References:
      - Proposal sample token sets (radii/borders/shadows/motion per style).
  - Test Cases to Write:
    - None (review task); matrix feeds tasks 7–9 and the drift gate.

- [ ] 3. Additive schema: appearance-keyed designTokens + recommendedThemes metadata
  - Acceptance Criteria:
    - Functional: (a) New optional `clay.contributions.designTokensByAppearance` object with keys `dark`/`light`, each an array validated by the exact existing `parse_design_token_contributions` rules (`src/packages/record/theme.rs`); when present and the resolved appearance has a palette, it wins over flat `designTokens`; flat `designTokens` remains the fallback and the only channel for legacy packages. (b) New optional `recommendedThemes: string[]` on `uiDesignSystem` contributions, validated as known `@clay/theme-*` specifiers, bounded length (≤4), projected read-only into the Settings enumeration DTO as a hint. (c) Appearance change re-resolves the active theme palette and bumps the theme revision so the frontend projection updates without remount.
    - Performance: Parse-time validation only; hot path still cached CSS custom properties.
    - Code Quality: Additive, versioned, schema-documented; unknown keys still rejected; prohibited-authority rejection (`reject_ui_prohibited_authority`) applies to new channels.
    - Security: No raw CSS/color injection paths; specifiers validated against bundled inventory; no new authority.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/record/theme.rs` (`parse_design_token_contributions`), `src/shell/theme.rs` (`core_token_type`, resolve layers), `src/server/ops/theme.rs` (`apply_appearance`, canonical defaults), `frontend/src/theme/design-system-adapter.ts`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
    - Options Considered:
      - Two packages per family (`-light` suffix): breaks the user's same-name requirement and doubles inventory — rejected.
      - Appearance-keyed palette channel in one package — chosen; additive and data-driven, keeps `apply_appearance` family-stable.
      - Hardcode family pairing in `canonical_default_specifier`: code-owned pairing rejected in favor of package metadata.
    - Chosen Approach: Extend record parsing + theme resolution + snapshot DTO; adapter unchanged for recipes; theme projection gains appearance-keyed lookup.
    - API Notes and Examples:
      ```json
      "designTokensByAppearance": {
        "dark":  [{ "token": "surface.main", "value": "#0d1117" }],
        "light": [{ "token": "surface.main", "value": "#e9eef4" }]
      },
      "recommendedThemes": ["@clay/theme-glass-cockpit"]
      ```
    - Files to Create/Edit:
      - `src/packages/record/theme.rs`: parse `designTokensByAppearance`.
      - `src/shell/theme.rs`: appearance-aware resolution layer.
      - `src/server/ops/theme.rs`: appearance switch keeps active family when palette exists.
      - `src/shell/design_system.rs` + snapshot DTO: `recommendedThemes`.
      - `frontend/src/settings/SettingsPanel.tsx`: hint text under dropdown items.
    - References:
      - Decision log 2026-08-28-2234 (additive-only rule).
  - Test Cases to Write:
    - Rust: appearance-keyed palette wins over flat; missing appearance falls back; invalid entries rejected with bounded errors; appearance toggle bumps revision and swaps palette without touching designSystem generation.
    - Rust: `recommendedThemes` with unknown specifier / >4 entries rejected.

- [ ] 4. Theme package: `@clay/theme-graphite-precision`
  - Acceptance Criteria:
    - Functional: Ships `designTokensByAppearance` dark+light palettes implementing the Graphite sample (dark: bg #0b0d10, surfaces #12151a/#1a1e25, ink #e8ebf0, muted #8b93a1, borders #2a313c/#20262e, accent #58a6ff, focus ring #58a6ff, diagnostics #3fb950/#d29922/#f85149; light: #f6f7f9/#ffffff/#eceef2, ink #1a202c, accent #0969da, …) mapped onto the full role set (`surface.*`, `text.*`, `border.*`, `accent.primary`, `focus.ring`, `diagnostic.*`, `selection.background`, scrollbar roles); ships `textStyles` syntax palette consistent with the chrome (cool blue-grey code colors); displayName "Graphite Precision".
    - Performance: Payload within `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.
    - Code Quality: Every role filled for both appearances (no reliance on core fallback except documented omissions); AA contrast: text roles ≥4.5:1 against surfaces, interactive borders ≥3:1, verified by test.
    - Security: Inert data; no prohibited fields.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references, the four mandatory design skills (stack listed per UI task rule)
      - `packages/theme-modus-vivendi/package.json` (manifest shape), `frontend/src/theme/design-system-adapter.ts` (role list)
    - Options Considered:
      - Reuse gruvbox syntax palette: clashes with cool graphite chrome — own `textStyles` chosen.
    - Chosen Approach: New package mirroring modus manifest shape with the new appearance channel.
    - Files to Create/Edit:
      - `packages/theme-graphite-precision/package.json` (+ docs/ if manifest pattern requires).
    - References:
      - `.impeccable/reviews/design-proposals/index.html` `[data-style="graphite"]` token sets.
  - Test Cases to Write:
    - `tests/theme_packages.rs`: package loads, validates, both palettes resolve all roles; contrast assertions.

- [ ] 5. Theme package: `@clay/theme-warm-analog`
  - Acceptance Criteria:
    - Functional: Same contract as task 4 with the Analog sample palettes (light-first: bg #f4ede3, surface #fdf9f2/#ece2d3, ink #2b2118, muted #7d6c5c, borders #ddcfbc/#e9dfd0, accent #c0562f, diagnostics #3e7c4f/#8a6100/#a63a2b; dark: #181210/#211a16/#2b221c, ink #f0e4d6, accent #e07a52, …); warm syntax `textStyles` (terracotta/olive/amber code colors); displayName "Warm Analog".
    - Performance/Code Quality/Security: Identical gates to task 4.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/impeccable/SKILL.md`; `.agents/skills/full-output-enforcement/SKILL.md`; `.agents/skills/high-end-visual-design/SKILL.md`; `.agents/skills/design-taste-frontend/SKILL.md`
      - `packages/theme-modus-operandi/package.json` (light-theme reference)
    - Chosen Approach: Light palette is the primary identity; dark palette is the espresso variant from the sample.
    - Files to Create/Edit:
      - `packages/theme-warm-analog/package.json`.
    - References:
      - Proposal sample `[data-style="analog"]`.
  - Test Cases to Write:
    - Same as task 4.

- [ ] 6. Theme package: `@clay/theme-glass-cockpit`
  - Acceptance Criteria:
    - Functional: Same contract with Cockpit palettes (dark: #0d1117/#131a22/#1a232e, ink #dbe7f4, muted #7f92a6, borders #243447/#1b2735, accent #35d0ba, diagnostics #35d0ba/#e3b341/#f47067; light: #e9eef4/#f7fafc/#dde5ee, accent #0d8f7f, …); `surface.overlay`/`surface.scrim` values chosen for translucency-friendly compositing (scrim role may carry alpha); syntax `textStyles` teal/amber instrument colors; displayName "Glass Cockpit".
    - Performance/Code Quality/Security: Identical gates; scrim alpha bounded by existing validation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/impeccable/SKILL.md`; `.agents/skills/full-output-enforcement/SKILL.md`; `.agents/skills/high-end-visual-design/SKILL.md`; `.agents/skills/design-taste-frontend/SKILL.md`
    - Chosen Approach: Dark is primary; light is the "day cockpit" variant.
    - Files to Create/Edit:
      - `packages/theme-glass-cockpit/package.json`.
    - References:
      - Proposal sample `[data-style="cockpit"]`.
  - Test Cases to Write:
    - Same as task 4.

- [ ] 7. Design-system package: `@clay/design-graphite-precision`
  - Acceptance Criteria:
    - Functional: Recipes for the full canonical slot matrix (controls + chrome slots from plan 110 task 6): 6px radii (10px modals), 1px borders, layered soft shadows (`0 1px 2px` + `0 8px 24px -12px` class via typed shadow layers), 120ms motion, quiet hover (no translate), selected = `surface.selected` fill + `border.strong`, focus = 2px `focus.ring` outline offset 1; no backdrop blur anywhere; colors only theme roles; `recommendedThemes: ["@clay/theme-graphite-precision"]`; displayName "Graphite Precision".
    - Performance: No filters; paint-class parity with core.
    - Code Quality: Slot keys exactly the consumed canonical set (plan-110 drift gate green); schemaVersion current.
    - Security: Inert; provenance first-party bundled.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/impeccable/SKILL.md`; `.agents/skills/full-output-enforcement/SKILL.md`; `.agents/skills/high-end-visual-design/SKILL.md`; `.agents/skills/design-taste-frontend/SKILL.md`
      - `packages/design-glass/package.json` (recipe shape reference), proposal sample graphite tokens.
    - Chosen Approach: Author recipes from the sample token set translated to role references (e.g. borders → `border.subtle`/`border.strong`, fills → `surface.control`/`surface.main`).
    - Files to Create/Edit:
      - `packages/design-graphite-precision/package.json`.
    - References:
      - Plan 110 task 6 slot list.
  - Test Cases to Write:
    - `tests/package_ui_conformance.rs`: loads/validates; drift gate consumes every key; cross-package slot parity assertion green.

- [ ] 8. Design-system package: `@clay/design-warm-analog`
  - Acceptance Criteria:
    - Functional: Same slot coverage; 10px radii (14px modals), 1px warm borders, soft warm shadows with inset top highlight layer (typed), 140ms ease, hover = subtle brightness/`surface.hover` (no translate), selected fills `surface.selected`, focus 2px ring; no blur; `recommendedThemes: ["@clay/theme-warm-analog"]`; displayName "Warm Analog".
    - Performance/Code Quality/Security: Same gates as task 7.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/impeccable/SKILL.md`; `.agents/skills/full-output-enforcement/SKILL.md`; `.agents/skills/high-end-visual-design/SKILL.md`; `.agents/skills/design-taste-frontend/SKILL.md`
      - Proposal sample analog tokens.
    - Chosen Approach: Tactile softness via radii + warm shadow layers; keeps density identical to core (no layout shift across systems).
    - Files to Create/Edit:
      - `packages/design-warm-analog/package.json`.
    - References:
      - Plan 110 task 6 slot list.
  - Test Cases to Write:
    - Same as task 7.

- [ ] 9. Design-system package: `@clay/design-glass-cockpit`
  - Acceptance Criteria:
    - Functional: Same slot coverage; 8px radii (12px modals), 1px `border.strong` resting chrome with inset top highlight, matte fills (opaque `surface.*` roles) on editor/lists/status/tab strip, translucency + `backdropBlur` **only** on `modal.*`, `menu.*`, `popover.*`, `commandCentre.*` overlay slots (16px blur, saturate 1.3 via typed properties), 150ms motion; reduced-transparency fallback recipes ship opaque (adapter/plan-110 gate); `recommendedThemes: ["@clay/theme-glass-cockpit"]`; displayName "Glass Cockpit".
    - Performance: Conformance invariant — zero backdrop-filter on editor, lists, scroll containers, status bar, tab strip (asserted by test scanning resolved recipes per slot kind).
    - Code Quality/Security: Same gates as task 7.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/impeccable/SKILL.md`; `.agents/skills/full-output-enforcement/SKILL.md`; `.agents/skills/high-end-visual-design/SKILL.md`; `.agents/skills/design-taste-frontend/SKILL.md`
      - `packages/design-glass/package.json` (existing blur usage), `docs/development/ui-design-system-conformance.md` (performance safety).
    - Options Considered:
      - Blur on panels like design-glass: repeats the original perf/a11y tradeoff — rejected; cockpit identity is matte instruments + glass overlays.
    - Chosen Approach: Blur restricted to transient overlay kinds; resting chrome is hairline+highlight matte.
    - Files to Create/Edit:
      - `packages/design-glass-cockpit/package.json`.
    - References:
      - Proposal sample cockpit tokens + palette overlay treatment.
  - Test Cases to Write:
    - Same as task 7 + blur-scope assertion (blur only in allowed kinds) + reduced-transparency opaque fallback.

- [ ] 10. Registration, enumeration, and pairing UX
  - Acceptance Criteria:
    - Functional: All six packages land in `packages/` and the bundled first-party inventory (verify inventory pickup mechanism per existing `design-neobrutal`/`theme-*` packages — directory-based, no hardcoded list); Settings design-system dropdown shows the three new systems with "pairs best with <Theme>" hint from `recommendedThemes`; theme dropdown lists the three new themes; selecting a DS then clicking its hint applies the paired theme (one-click pairing, still two independent settings); command centre `settings.setDesignSystem` accepts the new specifiers.
    - Performance: Enumeration from snapshot; no per-open scans.
    - Code Quality: React presentation-only; server validates.
    - Security: First-party provenance; no authority changes.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/impeccable/SKILL.md`; `.agents/skills/full-output-enforcement/SKILL.md`; `.agents/skills/high-end-visual-design/SKILL.md`; `.agents/skills/design-taste-frontend/SKILL.md`
      - Plan 110 task 10 outputs (selector + snapshot DTO).
    - Chosen Approach: Hint row + one-click apply in existing dropdown; no new surface.
    - Files to Create/Edit:
      - `frontend/src/settings/SettingsPanel.tsx` (hint + apply), snapshot DTO plumbing if task 3 didn't cover hint projection.
    - References:
      - `src/server/ops/theme.rs` setTheme/setDesignSystem.
  - Test Cases to Write:
    - Frontend: dropdown lists 5 DS + 6+ themes; hint applies paired theme; conformance test: switch DS→cockpit keeps DOM state.

- [ ] 11. Cross-product conformance, color-deny, and orthogonality tests
  - Acceptance Criteria:
    - Functional: Matrix tests: each new DS × {paired theme, modus-vivendi, modus-operandi, gruvbox-light, gruvbox-dark} and each new theme × {neobrutal, glass, graphite, analog, cockpit, core}: recipes resolve (no missing role references), contrast floors hold (text 4.5:1, UI 3:1) per combination, color-source deny (no literal colors in any recipe), DOM/state continuity on live switch, revocation to core fallback clean, appearance toggle within each new theme family swaps palette without designSystem invalidation.
    - Performance: Tests static/snapshot-based except one jsdom continuity test; <10s total.
    - Code Quality: Extends `frontend/src/test/design-system-conformance.test.tsx` + `tests/package_ui_conformance.rs` + `tests/theme_packages.rs` with data-driven matrices (no per-combo hand-written tests).
    - Security: Provenance/revocation assertions included.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/ui-design-system-conformance.md`, existing test files above.
    - Chosen Approach: Data-driven matrix over bundled inventory.
    - Files to Create/Edit:
      - The three test files.
    - References:
      - Pattern `ui-design-system-packages.md` (cross-product requirement).
  - Test Cases to Write:
    - The matrix itself; plus negative: recipe with literal color rejected; blur outside allowed kinds rejected (cockpit).

- [ ] 12. Perform visual screenshot and accessibility review of the three languages
  - Acceptance Criteria:
    - Functional: Fixture harness screenshots: `controls`, `splits`, `chat`, `command-centre`, `settings` × {graphite, analog, cockpit} × {paired theme dark, paired theme light} + one cross-theme proof per DS (modus pair); palette overlay open state for cockpit; focus-visible/hover/disabled/invalid states on `controls`; narrow+wide layouts on `splits`. Evidence under `.impeccable/reviews/111-final/` with findings; defects fixed or deferred to Further Actions.
    - Performance: Dev harness only.
    - Code Quality: Findings triaged.
    - Security: computer-use `get_app_state` a11y pass on Settings pairing flow when available; else record blocker per pattern.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/impeccable/SKILL.md`; `.agents/skills/full-output-enforcement/SKILL.md`; `.agents/skills/high-end-visual-design/SKILL.md`; `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
    - Chosen Approach: Same harness as plan 110 task 11; compare against proposal samples as design intent reference.
    - Files to Create/Edit:
      - `.impeccable/reviews/111-final/` evidence.
    - References:
      - `.impeccable/reviews/design-proposals/index.html`.
  - Test Cases to Write:
    - None (review task).

- [ ] 13. Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Docs updated so `theme.setTheme`/`settings.setTheme` and `theme.setDesignSystem`/`settings.setDesignSystem` enumerate the new specifiers with examples; `designTokensByAppearance` and `recommendedThemes` schema documented in package-authoring docs and lookup tags; registry refreshed; doc gates green.
    - Performance/Code Quality/Security: Registry gate defaults; no raw ops.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`, `doc-registry-tests.md`; `docs/reference/clay-js-api/theme/set-design-system.md`, `set-theme.md`.
    - Chosen Approach: Doc amendments + examples; no new API surface needed (commands already generic over specifiers).
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/theme/set-theme.md`, `set-design-system.md`, `docs/index.md` if links change, generated registry.
    - References:
      - Decision logs 2026-05-08-1509/1840.
  - Test Cases to Write:
    - Existing doc-registry conformance tests.

- [ ] 14. Create or verify Clay configuration APIs and update examples/init.js
  - Acceptance Criteria:
    - Functional: `examples/init.js` gains commented examples: `theme.setDesignSystem("@clay/design-glass-cockpit")` + `theme.setTheme("@clay/theme-glass-cockpit")` (and the other two families), annotation explaining DS/theme orthogonality and pairing hint; `node --check` passes; names match server validators.
    - Performance/Code Quality/Security: Commented opt-in; no authority granted.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (configuration + example tasks); `examples/init.js` current structure.
    - Chosen Approach: One commented "Design systems & paired themes" section.
    - Files to Create/Edit:
      - `examples/init.js`.
    - References:
      - Decision log 2026-05-08-1841; user instruction 2026-08-03.
  - Test Cases to Write:
    - `node --check examples/init.js`.

- [ ] 15. Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: New numbered steps: select each new DS from Settings (whole-shell restyle, hint visible, one-click pairing), appearance toggle keeps family palette, cross-theme combo (cockpit DS + modus-operandi) renders coherently, invalid specifier error; run affected modules on Linux build; failures = defects or documented ceilings.
    - Performance: <5min run.
    - Code Quality/Security: No weakening existing steps.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`.
    - Files to Create/Edit:
      - Affected `test-plan/` UI module + index if needed.
    - References:
      - User instruction 2026-08-04.
  - Test Cases to Write:
    - The manual steps.

- [ ] 16. Update package authoring contract, UI reference docs, and clay-ui catalogs
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md` documents `designTokensByAppearance`, `recommendedThemes`, and the three new packages as worked examples; `docs/reference/ui-components.md` + `.agents/skills/clay-ui/references/{components,tokens}.md` list the new DS/theme packages and pairing convention; `docs/development/ui-design-system-{recipe-matrix,package-primitive-review}.md` synced; drift doc gates green.
    - Performance/Code Quality/Security: Docs-only.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` + references; `.agents/skills/project-patterns/references/ui-design-system-packages.md`, `package-ui-layout.md`.
    - Files to Create/Edit:
      - The docs listed above.
    - References:
      - Decision logs 2026-08-28-2234, 2026-06-09-1431.
  - Test Cases to Write:
    - Phase-20.8 documentation-drift `cargo test` gates.

- [ ] 17. Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki pages for theme resolution (appearance-keyed layer), design-system packages (new examples), Settings pairing flow; master index intact.
    - Performance: No runtime work.
    - Code Quality: What/how/invariants/tests documented.
    - Security: Boundaries (inert data, validation, provenance) stated.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`, `.agents/skills/create-plan/references/wiki-task.md`.
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/*.md` as needed.
    - References:
      - Wiki task template.
  - Test Cases to Write:
    - Manual wiki review.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
