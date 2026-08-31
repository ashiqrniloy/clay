# UI Design System Runtime

**Files:** `src/shell/design_system.rs`, `src/packages/record/theme.rs`, `src/packages/conflict.rs`, `src/server/ops/theme.rs`, `src/server/mod.rs`, `src-tauri/src/bridge/dto.rs`, `frontend/src/theme/design-system-adapter.ts`, `frontend/src/state/design-system-store.ts`, `packages/clay-design-neobrutal/package.json`, `packages/clay-design-glass/package.json`  
**Tests:** `tests/theme_packages.rs`, `tests/package_ui_conformance.rs`, `tests/package_loading.rs`, `tests/runtime_update_protocol.rs`, `src-tauri/tests/dto_roundtrips.rs`, `frontend/src/test/design-system-adapter.test.ts`, `frontend/src/test/design-system-conformance.test.tsx`  
**Reference Docs:** `docs/reference/clay-js-api/theme/set-design-system.md`, `docs/reference/ui-design-systems.md`, `docs/development/ui-design-system-conformance.md`, `docs/development/ui-design-system-recipe-matrix.md`, `docs/reference/packages/creating-packages.md`, `.agents/skills/clay-ui/references/tokens.md`, `DESIGN.md`  

---

## 1. Overview and Responsibilities

The UI Design System Runtime is the host-owned system for declaring, validating, resolving, and falling back visual recipes for Clay components and shell surfaces.

It establishes a strict separation of concerns across presentation systems:
- **Content Themes (`clay:theme`, `setTheme`):** Sole normal-rendering color authority for surfaces, text, borders, focus rings, selections, and diagnostics.
- **User-Owned Typography (`setTypography`):** Concrete font family fallback stacks, sizes, and relative scale hierarchies.
- **UI Design Systems (`clay.contributions.uiDesignSystem`):** Geometry (radii, border widths, padding, gap), materials (background opacity, backdrop blur, saturation, inner highlight), shadows, focus/outline geometry, motion transitions, and hover/press transform presets.

Under normal rendering, design systems **never** declare colors or custom palettes. Every visual recipe slot references semantic color roles provided by the active content theme.

---

## 2. Architecture and Data Structures

### Core Types (`src/shell/design_system.rs`)

```rust
/// A validated recipe key identifying a component, variant, slot, and state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecipeKey {
    pub component: String,
    pub variant: String,
    pub slot: String,
    pub state: RecipeState,
}

/// Bounded interaction states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecipeState {
    Rest,
    Hover,
    Active,
    Focus,
    Disabled,
    Selected,
    Expanded,
}

/// A validated reference to an active content-theme color role.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ThemeColorRef(String);
```

### Component Recipe Declarations and Resolved Recipes

```rust
/// Package-contributed visual recipe declaration (all properties optional).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentRecipeDeclaration {
    pub background_color: Option<ThemeColorRef>,
    pub background_opacity: Option<f64>,
    pub text_color: Option<ThemeColorRef>,
    pub border_color: Option<ThemeColorRef>,
    pub border_width: Option<f64>,
    pub border_style: Option<BorderStyle>,
    pub border_radius: Option<f64>,
    pub padding: Option<String>,
    pub gap: Option<String>,
    pub shadow: Option<Vec<ShadowLayer>>,
    pub backdrop_blur: Option<f64>,
    pub backdrop_saturate: Option<f64>,
    pub inner_highlight: Option<InnerHighlight>,
    pub opacity: Option<f64>,
    pub outline_color: Option<ThemeColorRef>,
    pub outline_width: Option<f64>,
    pub outline_offset: Option<f64>,
    pub outline_style: Option<OutlineStyle>,
    pub transition_duration: Option<f64>,
    pub transition_timing: Option<TransitionTiming>,
    pub transform_preset: Option<TransformPreset>,
}

/// Fully resolved, non-empty, validated recipe for a component slot and state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedComponentRecipe { ... }
```

---

## 3. Strict Content-Theme Color Authority Invariant

To ensure that any theme (e.g. Gruvbox Dark, Gruvbox Light, Modus Operandi, Modus Vivendi) can colorize any design system (e.g. Neobrutal, Glass) cleanly:

1. `ThemeColorRef` parses and validates incoming strings during deserialization.
2. Literal hex codes (`#fff`, `#1e1e1e`), RGB/HSL functional strings (`rgb(0,0,0)`), named CSS colors (`black`, `red`), and design-system local aliases are rejected with a structured `DesignSystemError`.
3. Allowed color references are strictly:
   - Valid core theme color roles: `surface.main`, `surface.panel`, `surface.control`, `surface.overlay`, `surface.hover`, `surface.active`, `surface.disabled`, `surface.scrim`, `text.primary`, `text.muted`, `text.disabled`, `border.subtle`, `border.strong`, `border.focus`, `border.hairline`, `accent.primary`, `diagnostic.error`, `diagnostic.warning`, `focus.ring`, etc.
   - `transparent`.
4. The only exception to normal theme color authority is browser/OS-level `forced-colors: active` (high contrast mode), where the browser maps system colors directly.

---

## 4. Shipped Packages (Plan 104)

Clay ships two first-party UI design system packages under `packages/`:

### 1. `@clay/design-neobrutal` (Default Restrained Neobrutal)
- **Manifest:** `packages/clay-design-neobrutal/package.json`
- **Visual Personality:** Utilitarian, tactile, architectural precision.
- **Recipe Tokens:**
  - `borderRadius`: `0.0px` across all controls and surfaces.
  - `borderWidth`: `1.0px` structural borders (`border.subtle` / `border.strong`).
  - `shadow`: Hard 2px offset box-shadows (`2px 2px 0px 0px var(--clay-border-subtle)`), `blur: 0.0px`.
  - `backdropBlur`: `0.0px` (100% solid opacity).
  - `transitionDuration`: `100.0ms` snappy ease-out transitions.
  - `transformPreset`: `subtlePress` (`translate(1px, 1px)`) on active state.

### 2. `@clay/design-glass` (Luminous Reference Frosted Glass)
- **Manifest:** `packages/clay-design-glass/package.json`
- **Visual Personality:** Ethereal, layered depth, frosted optical translucency.
- **Recipe Tokens:**
  - `borderRadius`: `6.0px` (small badges/tags), `8.0px` (inputs/buttons), `10.0px` (dropdowns/cards), `12.0px` (modals/dialogs), `14.0px` (floating overlays).
  - `borderWidth`: `1.0px` hairline borders (`border.hairline`).
  - `backgroundOpacity`: `0.70`–`0.85` frosted translucency.
  - `backdropBlur`: `8.0px`–`24.0px` gaussian backdrop filter with `1.2`–`1.4` saturation boost.
  - `innerHighlight`: Specular 1px top inner highlight (`0 1px 0 0 rgba(255, 255, 255, 0.12)`).
  - `shadow`: Diffuse soft drop shadows (`0 8px 32px 0 rgba(0, 0, 0, 0.36)`).
  - `transitionDuration`: `150.0ms`–`200.0ms` smooth ease-out curves.

### Strict Editor Performance Invariant
Both design systems enforce:
- Editor canvas, gutter, and scroll track MUST have `backdropBlur: 0.0` and `backgroundOpacity: 1.0`.
- Sub-millisecond typing latency and 60fps scrolling are preserved without GPU filter compositing overhead.

---

## 5. Deterministic 5-Step Fallback Inheritance Algorithm

When resolving a recipe for a `(component, variant, slot, state)` key in `resolve_single_recipe()`:

```text
┌─────────────────────────────────────────────────────────────┐
│ 1. Exact match in package declaration                       │
│    (e.g., button.danger.root.active)                        │
└──────────────────────────────┬──────────────────────────────┘
                               │ (missing / partial)
┌──────────────────────────────▼──────────────────────────────┐
│ 2. Rest state in package declaration                        │
│    (e.g., button.danger.root.rest)                          │
└──────────────────────────────┬──────────────────────────────┘
                               │ (missing / partial)
┌──────────────────────────────▼──────────────────────────────┐
│ 3. Default variant in package declaration                   │
│    (e.g., button.default.root.active or .rest)              │
└──────────────────────────────┬──────────────────────────────┘
                               │ (missing / partial)
┌──────────────────────────────▼──────────────────────────────┐
│ 4. Extended parent design system (via `extends` field)      │
└──────────────────────────────┬──────────────────────────────┘
                               │ (missing / partial)
┌──────────────────────────────▼──────────────────────────────┐
│ 5. Clay core default fallback catalog                       │
│    (core_design_system_fallbacks() — Neobrutal baseline)    │
└─────────────────────────────────────────────────────────────┘
```

This guarantees that:
- Packages can override as little as a single property (e.g. `borderRadius: 8.0`) on a single variant without duplicating the entire catalog.
- Missing properties inherit values from the base fallback without undefined or partial states.

---

## 6. Activation, Generation Handshake, and DOM Continuity

The full activation chain:

```text
init.js: setDesignSystem("@clay/design-glass")    (runtime/js/theme.js facade)
  -> op_clay_theme_set_design_system              (src/server/ops/theme.rs)
  -> apply_design_system:
       trim/empty check -> "@clay/core" short-circuits to the built-in baseline
       first-party package: ensure_first_party_record enables it from bundled inventory
       third-party specifier: resolved only through already-enabled package records
           (selection never installs, adopts, or promotes — fail closed)
       contributing package must expose contributions.uiDesignSystem,
       else the op throws theme.invalid_design_system
       missing/unenabled package -> theme.load_failed
  -> ActiveDesignSystem { specifier, schema_version, generation,
                          provenance, recipes: BTreeMap<RecipeKey, ResolvedComponentRecipe> }
  -> generation commit in src/server/mod.rs:
       revalidate selection against enabled_records(); on revocation fallback to core_fallback()
  -> RuntimeSnapshot.active_design_system -> DesignSystemSnapshotDto projection
  -> frontend designSystemStore.setDesignSystem(snapshot)
  -> designSystemCssVariables -> one root-style batch write per generation (<0.3ms)
```

### DOM & State Continuity Invariant
Switching design systems mutates only CSS custom properties on `:root`. It causes **zero** React component unmounting, tree re-creation, focus loss, or scroll jump in open editor documents.

---

## 7. Package Security and Authority Boundaries

1. **Zero Permissions Required:** Design system packages request `permissions: []` in `package.json`.
2. **Zero Code Execution:** Declarations are purely inert JSON records; no client-side or server-side scripts are executed during recipe installation.
3. **No Direct Renderer Access:** Packages cannot inject raw CSS strings, `<style>` elements, class names, or DOM hooks.
4. **Compile-Time Bounds:**
   - Maximum contribution JSON: `64 KiB`.
   - Maximum backdrop blur: `32.0px`.
   - Maximum transition duration: `1000.0ms`.
   - Maximum border width: `8.0px`.
   - Maximum shadow layers: `3`.
   - Maximum backdrop saturation: `2.0`.

---

## 8. Testing Strategy & Conformance Matrix

- `tests/theme_packages.rs`: Validates full 25-component recipe coverage for `@clay/design-neobrutal` and `@clay/design-glass`.
- `tests/package_ui_conformance.rs`: Hard color authority denial (`plan104_design_system_packages_cover_all_25_components_and_enforce_color_authority`), bounds enforcement, and source-independence guards.
- `frontend/src/test/design-system-conformance.test.tsx`: End-to-end component rendering and DOM continuity across design system switches.
- `frontend/src/test/design-system-adapter.test.ts`: CSS custom property projection, color role translation, and install idempotence.
- `.impeccable/review/plan-104/`: Visual review fixtures and AT-SPI dumps across 8 desktop states.

Public documentation:
- API reference: [`docs/reference/clay-js-api/theme/set-design-system.md`](../../reference/clay-js-api/theme/set-design-system.md)
- Public specification: [`docs/reference/ui-design-systems.md`](../../reference/ui-design-systems.md)
- Conformance report: [`docs/development/ui-design-system-conformance.md`](../../development/ui-design-system-conformance.md)
- Recipe matrix: [`docs/development/ui-design-system-recipe-matrix.md`](../../development/ui-design-system-recipe-matrix.md)
- Package creation guide: [`docs/reference/packages/creating-packages.md`](../../reference/packages/creating-packages.md)
