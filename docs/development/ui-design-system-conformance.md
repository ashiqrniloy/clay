# UI Design System Conformance & Replacement Proof

## 1. Overview and Invariant Contract

This document records the conformance proof and verification contract for package-defined UI design systems in Clay (Plan 104 Task 5).

Clay enables full UI design system replacement purely through declarative data contributions (`clay.contributions.uiDesignSystem`), with zero JavaScript execution and zero host component branching.

### Core Conformance Invariants

| Invariant | Requirement | Verification Method |
| --- | --- | --- |
| **DOM & State Continuity** | Switching design systems MUST NOT remount the DOM or reset user input, focus, scroll, selection, or disclosure states. | `frontend/src/test/design-system-conformance.test.tsx` |
| **Zero Host Branching** | Host components and backend code MUST NOT branch conditionally on design-system package specifiers. | `tests/package_ui_conformance.rs` (`plan104_source_independence_guard_rejects_package_name_branching`) |
| **Color Authority** | Design systems MUST NOT declare concrete color palettes or literal hex/RGB/HSL colors. All colors trace to active content-theme tokens (`var(--clay-*)`). | `tests/package_ui_conformance.rs` & `tests/theme_packages.rs` |
| **Theme Orthogonality** | Switching active content themes recolors the UI instantaneously through CSS custom properties with zero design system recipe modifications. | `frontend/src/test/design-system-conformance.test.tsx` |
| **Performance Safety** | Large scroll viewports, editor typing canvas, and line gutters MUST NOT apply backdrop filters (`backdropBlur == 0.0`). | `tests/theme_packages.rs` (`design_glass_bundled_package_validates_as_inert_data`) |
| **Clean Revocation** | Revoking or disabling an active design system cleanly restores default/core CSS variables with no orphaned properties. | `frontend/src/test/design-system-conformance.test.tsx` |

---

## 2. Package Replacement Verification Matrix

The two reference first-party design systems (`@clay/design-neobrutal` and `@clay/design-glass`) exhibit contrasting visual treatments across all 25 component kinds while adhering to the identical semantic schema:

| Component Kind | Neobrutal Characteristic | Glass Characteristic | Semantic Token Source |
| --- | --- | --- | --- |
| `button` | `0px` radius, `2px` hard offset shadow, `100ms` snappy motion | `6px` radius, `8px` backdrop blur, diffuse shadow, inner highlight | `var(--clay-surface-control)`, `var(--clay-text-primary)` |
| `textInput` | `0px` radius, `1px` structural border, `100ms` transition | `6px` radius, `8px` backdrop blur, translucent control fill | `var(--clay-surface-control)`, `var(--clay-border-subtle)` |
| `dropdown` | `0px` radius, solid `surface.overlay` popover, `3px` offset shadow | `8px` radius, `16px` backdrop blur, inner highlight, diffuse shadow | `var(--clay-surface-overlay)`, `var(--clay-border-strong)` |
| `list` | `0px` radius, `0px` blur, solid panel background | `8px` radius, `8px` backdrop blur, translucent fill | `var(--clay-surface-panel)`, `var(--clay-border-subtle)` |
| `collapse` | `0px` radius, solid header, `1px` crisp border | `8px` radius, `8px` backdrop blur, rounded header | `var(--clay-surface-panel)`, `var(--clay-text-primary)` |
| `modal` | `0px` radius, `2px` border, `4px` offset shadow, `surface.scrim` | `14px` radius, `24px` backdrop blur, `40px` diffuse shadow, inner highlight | `var(--clay-surface-overlay)`, `var(--clay-surface-scrim)` |
| `panel` | `0px` radius, solid `surface.panel` fill | `8px`–`10px` radius, `12px`–`16px` backdrop blur | `var(--clay-surface-panel)`, `var(--clay-border-subtle)` |
| `card` | `0px` radius, `2px` offset shadow, hover lift | `8px` radius, `12px` backdrop blur, diffuse shadow, hover lift | `var(--clay-surface-panel)`, `var(--clay-border-strong)` |
| `badge` | `0px` radius, `1px` border, solid fill | `9999px` pill radius, `6px` backdrop blur | `var(--clay-accent-primary)`, `var(--clay-surface-main)` |
| `editor` | `0px` radius, `0px` blur, solid `surface.main` canvas | `0px` radius, `0px` blur, solid `surface.main` canvas | `var(--clay-surface-main)`, `var(--clay-text-primary)` |

---

## 3. Automated Test Coverage

### Rust Integration & Conformance Tests
- **`tests/package_ui_conformance.rs`**:
  - `plan104_source_independence_guard_rejects_package_name_branching`: Static AST/source scan confirming zero conditional branches on package specifiers in host source code.
  - `plan104_design_system_packages_cover_all_25_components_and_enforce_color_authority`: Asserts complete 25-component recipe presence and literal color absence.
- **`tests/theme_packages.rs`**:
  - `design_neobrutal_bundled_package_validates_as_inert_data`: Asserts Neobrutal package loads as inert data with `0px` border radius across all recipes.
  - `design_glass_bundled_package_validates_as_inert_data`: Asserts Glass package loads as inert data, validates `backdrop_blur == 0.0` on editor/scroll paths, and verifies modal dialog frosted glass properties.

### Frontend Conformance Suite
- **`frontend/src/test/design-system-conformance.test.tsx`**:
  - Validates CSS custom property projection (`--clay-ds-*`) without literal colors.
  - Proves user-typed buffer text, modal visibility, and collapse states survive live design system switching without DOM remounts.
  - Proves content-theme token updates recolor the UI without modifying design-system recipes.
  - Proves clean variable revocation on reset.
