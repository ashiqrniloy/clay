# UI Design System Package Primitive Review

**Date:** 2026-08-30  
**Status:** Approved & Implemented  
**Related Plans:** [Plan 101](../../plans/101-UI-Design-System-Recipe-Foundation.md), [Plan 102](../../plans/102-UI-Design-System-Activation-and-Frontend-Runtime.md), [Plan 103](../../plans/103-UI-Design-System-Component-and-Surface-Migration.md), [Plan 104](../../plans/104-Neobrutal-and-Glass-Design-System-Packages-and-Conformance.md)  
**Decision Source:** [`decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`](../../decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md)  
**Primitive Reference:** [`docs/reference/primitives/registry.md`](../reference/primitives/registry.md)  

---

## 1. Overview and Objectives

Scope note (2026-09-11): the recipe primitives reviewed here carry the approved
design language too ([`DESIGN.md`](../../DESIGN.md), Quiet Instrument). The
migration ships it as a third first-party package, `@clay/design-instrument`,
using exactly these primitives — no new manifest, record, service, or recipe
engine primitive is required (opacity fills, inset-shadow state marks,
spread-shadow focus halos, and negative-spread soft elevation all fit the
existing typed domains).

**Historical record (2026-09-11):** this review was written before Plan 104 shipped `@clay/design-neobrutal` and `@clay/design-glass`; both were removed by the Quiet Instrument migration (plan 118 task 9), and `@clay/design-instrument` now occupies the same primitives. The primitive findings below still hold verbatim — read the two removed package names as "the design-system packages".

Before authoring and shipping the `@clay/design-neobrutal` default design system and the `@clay/design-glass` reference design system packages in Plan 104, this primitive review:
1. Inventories all existing package manifest, package record, service, bundled inventory, adoption, selection, and recipe engine primitives.
2. Identifies and closes generic data-only package gaps so purely declarative packages (design systems, theme packages, and metadata-only packages) do not require dummy JavaScript files or unnecessary runtime worker evaluation.
3. Locks the hard security and authority boundaries governing UI design system packages (strict content-theme color authority, no raw CSS/JSX, payload budgets, and provenance tracking).

---

## 2. Existing Primitive Inventory

| Primitive / Subsystem | Location | Existing Capabilities & Invariants |
| --- | --- | --- |
| **Package Manifest** | `src/packages/manifest.rs` | Schema validation for `name`, `version`, `clay.apiPrefix`, `clay.permissions`, `clay.modes`, and `clay.contributions`. Rejects reserved core prefixes, invalid semver, and prohibited authorities (`script`, `rawCss`, `tauri`, etc.). |
| **Package Record** | `src/packages/record/mod.rs` | Typed representation assembled by `assemble_package_record()`. Validates contribution blocks (`uiDesignSystem`, `textStyles`, `themeTokens`, `commands`, `sdui`, `docs`, `performance`). |
| **Bundled Inventory** | `src/packages/bundled-inventory.toml`, `src/packages/bundled.rs` | Pinned list of first-party packages under `packages/`. `verify_bundled_trust()` enforces exact name, version, root, and integrity classification (`RuntimeDomain::Trusted` vs `RuntimeDomain::ThirdParty`). |
| **Package Service** | `src/packages/service.rs` | Package installation, authorization, adoption (`approve_package`), conflict checking (`check_enabled_packages`), and lifecycle enablement. |
| **Design System Recipe Engine** | `src/shell/design_system.rs` | Closed `ComponentRecipeDeclaration`, bounded non-color properties (radii, border widths/styles, padding, gap, shadows, blur, saturation, inner highlights, opacity, outlines, motion transitions, transform presets), `ThemeColorRef` active-theme color validation, and deterministic 5-step fallback inheritance. |
| **Theme & Design System Ops** | `src/server/ops/theme.rs` | `apply_theme` (`setTheme`), `apply_appearance` (`setAppearance`), and `apply_design_system` (`setDesignSystem`). Resolves package records without evaluating runtime code. |
| **Frontend Adapter & Store** | `frontend/src/theme/design-system-adapter.ts`, `frontend/src/state/design-system-store.ts` | Maps resolved recipes to closed `--clay-ds-*` CSS custom properties on `:root`. Idempotent install with zero hot-path overhead. |

---

## 3. Generic Data-Only Package Gap Analysis & Closure

### Identified Gap: Mandatory Executable Entry for Declarative Packages

- **Prior State:** `src/packages/manifest.rs` required `clay.entry` to be present and non-empty for all packages (`required_string_field(clay.get("entry"), "clay.entry")`).
- **Problem:** Data-only packages (such as UI design system packages or `textStyles` theme packages) contain only declarative JSON metadata in `package.json` and have no executable JavaScript code. Forcing an `entry` field required authors to create artificial `./dist/index.js` and `./dist/load.js` placeholder files.
- **Generic Resolution:**
  1. Updated `ClayPackageMetadata.entry` from `String` to `Option<String>`.
  2. In `validate_manifest_value`, `clay.entry` is validated if present; if omitted, validation verifies that `clay.permissions` is empty. If a package requests permissions or executable capabilities, `clay.entry` is strictly required.
  3. Declarative packages declaring only data contributions (`uiDesignSystem`, `textStyles`, `themeTokens`, `docs`, etc.) can omit `clay.entry` and `clay.loadEntry`.
  4. Package enablement in `PackageService` and selection in `apply_design_system` / `apply_theme` enable and read package records directly without invoking Deno/V8 module loaders or JavaScript worker threads.

---

## 4. Hard Security & Authority Boundaries

1. **Strict Content-Theme Color Authority:**
   - Design systems **never** declare concrete color values (hex, RGB, HSL, named CSS colors).
   - Every recipe color property (`backgroundColor`, `textColor`, `borderColor`, `outlineColor`) must parse as a valid `ThemeColorRef` referencing an active semantic content-theme role (`surface.*`, `text.*`, `border.*`, `accent.*`, `diagnostic.*`, `focus.*`, or `transparent`).
   - Content themes own all colors; design systems own structure, materials, and geometry.
2. **Prohibited Runtime Authorities:**
   - Manifest validation unconditionally rejects `rawCss`, `cssText`, `selectors`, `classes`, `className`, `styleString`, `script`, `tauri`, `invoke`, `callback`, and `handler`.
   - Packages cannot inject arbitrary DOM elements, inline style attributes, or bypass the React component registry.
3. **Payload & Effect Budgets:**
   - Maximum contribution payload: `UI_DESIGN_SYSTEM_PAYLOAD_BUDGET_BYTES` (64 KiB).
   - Maximum serialized snapshot: 128 KiB.
   - Strict effect bounds: maximum 3 shadow layers, blur capped at 32px, motion durations capped at 1000ms, border widths capped at 8px.
4. **Provenance & Trust Domains:**
   - First-party design systems bundled in `packages/` are verified against `bundled-inventory.toml`.
   - Third-party design systems require explicit user adoption (`approve_package`) before enablement and remain classified as `RuntimeDomain::ThirdParty`.
   - Revocation demotes active selection back to `@clay/core` fallback cleanly.

---

## 5. Primitive Registry Summary

The primitive registry in [`docs/reference/primitives/registry.md`](../reference/primitives/registry.md) has been updated with:
- `UiDesignSystemContribution`: Package-contributed visual design-system recipe and property declarations (`clay.contributions.uiDesignSystem`).
- `DeclarativePackageManifest`: Declarative data-only package manifests with optional entry points.

---

## 6. Verification

- `cargo test --test security package_loading::` (51 tests passed)
- `cargo test --test security package_primitive_gate::` (29 tests passed)
- `cargo test --test protocol documentation_coverage::` (11 tests passed)
