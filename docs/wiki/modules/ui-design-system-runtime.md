# UI Design System Runtime

**Files:** `src/shell/design_system.rs`, `src/packages/record/theme.rs`, `src/packages/conflict.rs`, `src/server/ops/theme.rs`, `src/server/mod.rs`, `src-tauri/src/bridge/dto.rs`, `frontend/src/theme/design-system-adapter.ts`, `frontend/src/state/design-system-store.ts`, `packages/design-instrument/package.json`  
**Tests:** `tests/theme_packages.rs`, `tests/package_ui_conformance.rs`, `tests/package_loading.rs`, `tests/runtime_update_protocol.rs`, `src-tauri/tests/dto_roundtrips.rs`, `frontend/src/test/design-system-adapter.test.ts`, `frontend/src/test/design-system-conformance.test.tsx`, `frontend/src/test/design-system-consumption.test.ts`  
**Reference Docs:** `docs/reference/clay-js-api/theme/set-design-system.md`, `docs/reference/ui-design-systems.md`, `docs/development/ui-design-system-conformance.md`, `docs/development/ui-design-system-recipe-matrix.md`, `docs/reference/packages/creating-packages.md`, `.agents/skills/clay-execution/references/tokens.md`, `DESIGN.md`  

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

To ensure that any content theme (Gruvbox Material Dark/Light, Modus Operandi/Vivendi) can colorize the shipped design system (`@clay/design-instrument`) cleanly:

1. `ThemeColorRef` parses and validates incoming strings during deserialization.
2. Literal hex codes (`#fff`, `#1e1e1e`), RGB/HSL functional strings (`rgb(0,0,0)`), named CSS colors (`black`, `red`), and design-system local aliases are rejected with a structured `DesignSystemError`.
3. Allowed color references are strictly:
   - Valid core theme color roles: `surface.main`, `surface.panel`, `surface.control`, `surface.overlay`, `surface.hover`, `surface.active`, `surface.disabled`, `surface.scrim`, `text.primary`, `text.muted`, `text.disabled`, `border.subtle`, `border.strong`, `border.focus`, `border.hairline`, `accent.primary`, `diagnostic.error`, `diagnostic.warning`, `focus.ring`, etc.
   - `transparent`.
4. The only exception to normal theme color authority is browser/OS-level `forced-colors: active` (high contrast mode), where the browser maps system colors directly.

---

## 4. Shipped Packages (Plan 118)

Clay ships one first-party UI design system package under `packages/`. The
approved design language is **Quiet Instrument** ([`DESIGN.md`](../../../DESIGN.md)),
and `@clay/design-instrument` is that language's package (plan 118 task 8). The
former Neobrutal and Glass packages were removed in plan 118 task 9; `@clay/core`
keeps supplying the built-in fallback recipes. The four shipped content
themes declare the same thirteen theme-side roles as typed `designTokens`
(task 13), the composited contrast gate refuses a below-floor palette
(task 14), and the design-system choice set is `@clay/core` + this package
(task 20).

### 1. `@clay/design-instrument` (Quiet Instrument — shipped language)
- **Manifest:** `packages/design-instrument/package.json`
- **Visual Personality:** quiet instrument face; one surface, hairline zoning, state-driven accent.
- **Recipe Tokens (profile, `DESIGN.md` §4/§11):**
  - `borderRadius`: `5.0px` (chips/kbd/badges), `8.0px` (controls/rows/tabs), `12.0px` (panels/popovers), `16.0px` (window/sheets/modals), `9999.0px` (pills). No `0px`.
  - `borderWidth`: `1.0px` hairline everywhere; 2px state marks rendered as inset shadow layers.
  - `backgroundOpacity`: `0.55` veil planes (`surface.panel`), `0.15` accent-tinted selected fills, `0.92` primary-button hover.
  - `shadow`: two transient recipes only — `overlay` (`0 24px 60px -20px` @0.42 + `0 2px 10px -4px` @0.22) and `pop` (`0 14px 34px -14px` @0.34 + `0 1px 3px -1px` @0.16); no shadow on static surfaces.
  - `backdropBlur`: `0.0px` except the overlay scrim (3px) and toast (8px).
  - `transitionDuration`: `150.0ms` state changes, `240.0ms` surface entrances, `620.0ms` keyboard-focus pulse.
  - `transformPreset`: `press-shift-down` on buttons only; no hover lift.

### 2. `@clay/core` (built-in fallback recipes)
- **Source:** `core_design_system_fallbacks()` (`src/shell/design_system.rs`), mirrored in `frontend/src/styles/tokens.css`.
- **Visual Personality:** the same language, not a second one — plan 118 task 16 re-cut the baseline from the old plain defaults to the host-consumed subset of the shipped recipes, so a build with no design system installed already paints Quiet Instrument and activation cannot move geometry, material or motion.
- **Recipe Tokens:** the shipped ladder (`5.0`/`8.0`/`12.0`/`16.0`/`9999.0`px radii, 1px hairline borders, veil `0.55` / selected `0.15` / control `1.0` opacities, `150.0`ms state / `240.0`ms enter / `620.0`ms focus motion, `0.0`px blur) restricted to the component kinds and shell surfaces the frame paints before a snapshot lands. `ResolvedComponentRecipe::default()` is neutral (`outlineOffset: 2`, no transition) so a sparse recipe resolves to the language rather than to a retired value. Equality with the package is asserted both ways in `tests/package_ui_conformance.rs` (`plan118_core_fallbacks_match_the_shipped_language`, `plan118_host_fallback_block_matches_the_resolved_package`).

### 3. Shipped theme-side roles (plan 118 task 13)

The four shipped content themes (`@clay/theme-modus-operandi`,
`@clay/theme-modus-vivendi`, `@clay/theme-gruvbox-material-dark`,
`@clay/theme-gruvbox-material-light`) each declare the same thirteen typed
`designTokens` overrides, built from their own palette — `border.hairline` /
`border.subtle` / `border.strong` (34% border grey / 100% border grey / 100%
ink), `surface.scrim` (whichever of ink and canvas is darker), `surface.hover` /
`surface.active` / `surface.selected`, `accent.primary` / `accent.muted`,
`focus.ring` / `border.focus`, `text.muted` / `text.disabled`. This is how the
colour ladder lands without a Rust vocabulary change: themes stay the colour
authority and the design system only maps roles. Binding values and measured
ratios live in `design-artifacts/approved/quiet-instrument-migration/theme-values.{json,md}`;
`tests/theme_packages.rs` asserts board fidelity, the monotonic hairline <
subtle < strong ladder and the accent floor, and `src/shell/theme.rs` refuses a
theme that fails a composited pair.

#### Why the colour stays in the theme (the `designTokens` path)

The design system language needs roles the legacy theme vocabulary did not have:
a three-step border ladder, three state fills, an accent pair, a focus pair, two
text steps and a scrim. Two ways existed to get them: teach `BaseUiColors` the
new roles in Rust, or let each theme supply values for roles the resolver
already knows. Plan 118 chose the second:

- `clay.contributions.designTokens` is an **inert, typed** contribution on theme
  packages (`src/packages/record/theme.rs`; every value is a
  `#rrggbbaa` colour, scalar, opacity or level — no CSS, no expressions).
- `ResolvedUiTheme` layers them over the base palette, and `ActiveTheme`
  carries the overrides into the wire snapshot (`ThemeUiOverride`), so nothing
  about the mechanism is design-system-specific.
- The invariant is unchanged: a design system may only reference role *names*,
  and the role *values* come from the active content theme. A theme that ships
  no override keeps the core fallback for that role, which is why
  `THEME_UI_ROLES` coverage is asserted identical across all four shipped
  themes — a role that only one theme declares would otherwise resolve to the
  core fallback there and to the theme palette elsewhere.
- `designTokens` is a generic theme-package contribution (the record layer
  accepts it from any theme package); before plan 118 the shipped themes used
  only the legacy `textStyles` path, and task 13 is what moved them onto the
  typed one.

`tests/package_ui_conformance.rs` pins the list (`THEME_UI_ROLES`), the resolved
palette of each bundled theme, and the contrast pairs; `src/shell/theme.rs`
measures them **composited** (alpha over the surface) and refuses a theme whose
structural boundary, state fill, focus ring or hairline visibility misses its
floor.

### Strict Editor Performance Invariant
All design systems enforce:
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
│    (core_design_system_fallbacks() — host-consumed subset    │
│     of the shipped language, plan 118 task 16)              │
└─────────────────────────────────────────────────────────────┘
```

This guarantees that:
- Packages can override as little as a single property (e.g. `borderRadius: 8.0`) on a single variant without duplicating the entire catalog.
- Missing properties inherit values from the base fallback without undefined or partial states.

---

## 6. Activation, Generation Handshake, and DOM Continuity

The full activation chain:

```text
init.js: setDesignSystem("@clay/design-instrument") (runtime/js/theme.js facade)
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
  -> RuntimeSnapshot.ui_choices (UiChoicesSnapshot: themes, design_systems, appearance)
       built by IpcServer::enumerate_ui_choices (src/server/mod.rs): enabled @clay/theme-*
       packages sorted by specifier; @clay/core pinned first in design_systems followed by
       enabled uiDesignSystem contributors with manifest display names; the persisted
       appearance preference; validated against RUNTIME_STATE_SNAPSHOT_MAX_UI_CHOICES (64)
  -> frontend designSystemStore.setDesignSystem(snapshot)
  -> designSystemCssVariables -> one root-style batch write per generation (<0.3ms)
```

### DOM & State Continuity Invariant
Switching design systems mutates only CSS custom properties on `:root`. It causes **zero** React component unmounting, tree re-creation, focus loss, or scroll jump in open editor documents.

### Settings command surface (interactive selection)

The Settings panel selects a design system through the `settings.setDesignSystem` command ([API doc](../../reference/clay-js-api/settings/set-design-system.md)):

```text
Settings dropdown / command centre: settings.setDesignSystem("@clay/design-instrument")
  -> execute_settings validator   (src/server/command_execution.rs)
       accepts @clay/core or a bundled @clay/design-* contributor; else InvalidArguments
  -> persist_settings_change      (src/server/connection/runtime.rs)
       designSystem preference -> ~/.clay/preferences.json + reload_runtime_generation()
  -> apply_persisted_preferences  (src/server/js_runtime/evaluation.rs)
       re-applies through apply_design_system on every reload (preference wins over init.js)
```

The panel that renders those choices is the tab's fixed right slot (plan 118's
settings task): eyebrow-labelled `collapse` groups of hairline-separated rows,
mono values for data, an actions row whose note states whether anything can be
committed, and no radius or elevation because a slot is not a floating surface.
The `@clay/settings` SDUI tree mirrors the same rows and labels for hosts that
paint the package's own tree. `frontend/src/test/settings-panel-choices.test.tsx`
holds both the enumeration contract and the composition (sections, mono values,
no nested panel, Escape + `esc` affordance).

The persisted choice survives restarts; a stored specifier that later fails activation (revoked or invalid package) preserves the previous valid design system and records a sanitized diagnostic. The Settings panel renders its Theme/Design system/Appearance dropdowns from the snapshot's `ui_choices` list instead of hardcoded options.

---

## 7. Canonical Recipe Slots and Consumption Gate (Plan 110)

### One canonical name per slot

Plan 110 task 3 unified one canonical name per component-kind slot and deleted every alias. Packages must ship these canonical keys (the fallback-resolution chain resolves only these names):

| Canonical key family | Renamed from | Notes |
| --- | --- | --- |
| `tab.default.item.{rest,hover,selected,focus,disabled}` | `tab.default.root.*` | Tab items in `ClayTabStrip`; `tabBar.default.root.rest` remains the strip-chrome key |
| `list.default.row.{rest,hover,active,focus,selected}` | `list.default.root.*` | List rows; the `list.default.item.*` alias was deleted |
| `modal.default.{dialog,scrim}.rest` | `modal.default.root.*` / `modal.default.surface.*` | `dialog` is the canonical surface slot |
| `textInput.default.input.*` | `textInput.default.root.*` | The field wrapper keeps `textInput.default.field.rest` |

### Chrome and agent surface coverage

Plan 110 tasks 6 and 8 extended package-facing recipes beyond the interactive controls to the whole shell; plan 118 task 8 re-cut that set for the target information architecture. The shipped package declares **165 recipe keys**: the recorded 142-key baseline (`tests/fixtures/design-system-reference-keys.txt`) minus the 12 `chat.default.*` keys, plus 35 keys for `seg` (the tab view switcher), `agentPicker`, `recentRow`, `sessionRow`, `toast`, `empty`, `statusDot`, `keyHint`, `swatch`, and `statRow` — covering `shell.*` (root/header/brand/workingArea/footer), `editor.*` (10 chrome slots: root, container, chrome, gutter, activeLine, selection, findMatch, matchingBracket, path, tooltip), `menu`, `card`, `popover`, `badge`, `kbd`, `divider`, `tooltip`, `tab`/`tabBar`, `commandCentre`, `settingsPanel`, `paneSplitTree`, `fileBrowser`, `statusBar`, and `statusItem`. `plan118_design_instrument_keys_are_the_reference_set_minus_chat_plus_the_new_families` (`tests/package_ui_conformance.rs`) pins that delta. Core fallbacks stay at `<surface>.default.root.rest` granularity (`chatPanel`, `editorChrome`, `tabBar`, …), so a skipped chrome surface falls through the 5-step chain to the core baseline rather than erroring — a visible downgrade, not a validation failure.

### Consumption-tested contract

`frontend/src/test/design-system-consumption.test.ts` makes recipe-key drift a test failure instead of a review catch. It scans every `*.module.css` file plus `tokens.css` and asserts, bidirectionally:

1. Every `tokens.css` `--clay-ds-*` fallback variable is consumed by at least one CSS module (zero unconsumed fallbacks — this is what let task 3 delete 21 dead speculative fallbacks).
2. Every CSS-consumed recipe variable is backed by either a host fallback recipe or a shipped package recipe.
3. Canonical keys (e.g. `tab.default.item.*`) are declared in both package manifests and consumed by the owning component CSS.

The host recipe matrix (`docs/development/ui-design-system-recipe-matrix.md`) is the source of truth for slot names; misspelled or speculative package keys are drift, not extensibility. The table marks the mirror case with `†`: a host slot the shipped package declares no recipe for (`dropdown.triggerLabel`, `list.rowTitle`, `modal.body`, `editorChrome.*`, …) resolves to the core fallback, and a CSS module must not expect a `--clay-ds-*` variable for it.

### Legible-neobrutal baseline (plan 110 task 9 — historical record, its package is removed)

Historical record: this baseline improved the then-default Neobrutal package's contrast and row legibility (0px radii, 1px structural borders, 2px hard offset shadows with 0px blur, 100ms snappy motion; text inputs filling `surface.main`, transparent list rows with hairline separators and `surface.hover`/`surface.selected` fills, 1px `border.subtle` ghost buttons, slightly larger UI type). The approved direction is now Quiet Instrument (`DESIGN.md`), and plan 118 task 9 removed the Neobrutal package entirely. The legibility lessons survive as invariants in `DESIGN.md` §13, and the `@clay/core` fallback in `core_design_system_fallbacks()` still mirrors the host defaults so pre-bootstrap paint matches post-activation rendering.

## 8. Package Security and Authority Boundaries

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

## 9. Testing Strategy & Conformance Matrix

- `tests/theme_packages.rs`: Validates the four shipped content themes (inert data, full mapping, distinct palettes, contrast).
- `tests/package_ui_conformance.rs`: Hard color authority denial and required-component coverage (`plan118_design_instrument_covers_required_components_and_enforces_color_authority`), the plan-118 key delta, bounds enforcement, removed-package absence (`plan118_removed_design_systems_are_absent`), source-independence guards (`plan118_source_independence_guard_rejects_package_name_branching`), and the catalog-currency pair: `plan118_recipe_matrix_marks_undeclared_slots` (`†` markers recomputed from the shipped manifest, in both directions, plus the generated index in `components.md`) and `plan118_catalog_pages_do_not_ship_removed_systems` (a catalog line may name a removed system only while marking it removed).
- `frontend/src/test/design-system-consumption.test.ts`: Bidirectional recipe-key ↔ CSS-consumption ↔ tokens.css-fallback gate (plan 110 task 3).
- `frontend/src/test/core-baseline-hierarchy.test.ts`: Core-fallback + typography-default gate for the *current* core baseline (the Quiet Instrument language as of plan 118 task 16; the file name is a plan-110-era label).
- `frontend/src/test/settings-panel-choices.test.tsx`: Server-enumerated `ui_choices` dropdowns and design-system switching (plan 110 task 10), plus the settings composition gates — eyebrow sections, mono values, no nested panel/elevation, `esc` affordance and Escape dismissal (plan 118's settings task).
- `frontend/src/agent-settings/AgentSettingsPanel.test.tsx`: the agent Settings tab — delivered-file rows as `list.default.row`s, mono sizes, provenance badges, the designed empty state and the source caption.
- `frontend/src/test/design-system-conformance.test.tsx`: End-to-end component rendering and DOM continuity across design system switches.
- `frontend/src/test/design-system-adapter.test.ts`: CSS custom property projection, color role translation, and install idempotence.
- `.impeccable/reviews/110-final/`: Plan 110 visual review (historical) — 61 captures plus CDP accessibility trees across the design systems shipped at that time.
- `design-artifacts/approved/quiet-instrument-migration/`: the approved plan-118 prototype set (surfaces, catalog, theme values, harness states).

Public documentation:
- API reference: [`docs/reference/clay-js-api/theme/set-design-system.md`](../../reference/clay-js-api/theme/set-design-system.md)
- Public specification: [`docs/reference/ui-design-systems.md`](../../reference/ui-design-systems.md)
- Conformance report: [`docs/development/ui-design-system-conformance.md`](../../development/ui-design-system-conformance.md)
- Recipe matrix: [`docs/development/ui-design-system-recipe-matrix.md`](../../development/ui-design-system-recipe-matrix.md)
- Package creation guide: [`docs/reference/packages/creating-packages.md`](../../reference/packages/creating-packages.md)

Plan 112: icon packs reuse this module's selection lifecycle end to end —
`setIconPack` mirrors `setDesignSystem` resolution, generation-commit
revalidation, revocation fallback, DTO projection, and idempotent frontend
install, while icons stay consumers of theme color roles (never contributors).
See [Icon Pack Runtime](icon-pack-runtime.md).

## 10. Provenance, cost, and where the rest lives

**Provenance.** `@clay/design-instrument` is a bundled first-party root in
`src/packages/bundled-inventory.toml`; trust is granted only by
`verify_bundled_trust` after exact name/version/canonical-root match and an
FNV-1a-64 fingerprint of the shipped `package.json` bytes
(`src/packages/bundled.rs`, generated by `build.rs`). The fingerprint is exact
binding plus drift detection, not a cryptographic boundary: the checked-in
source tree is the trust root. Every bundled package must also declare at least
one extension point whose scopes name only real contributions of that package
(`bundled_extension_points_match_real_contributions`), which is why the
manifest carries `design-instrument.recipes` (`replace`, `uiDesignSystem`).
Design-system trust is separate from *panel* trust: a host-rendered compiled
panel (launcher, agent view) requires trusted provenance **and** an exact package
name — see [Launcher Landing Surface](launcher-landing-surface.md).

**Cost.** Removing the two retired systems (plan 118 task 9) shrank the shipped
design-system payload from 89,135 B to 57,218 B raw (−35.8%) and 5,345 B to
3,110 B gzip (−41.8%). Plan 118's surface work moved the shell budget from
164.5 kB to 169.8 kB gzip (limit 180 kB) and total from 381.0 kB to 390.9 kB
(limit 400 kB). Recipe resolution is a `BTreeMap` lookup on a cached struct: no
parsing, no JS, no color math on the paint path; the frontend applies one
`document.documentElement.style` batch per generation. `frontend/src/styles/tokens.css`
carries a generated host fallback block (512 `--clay-ds-*` declarations today,
projected from the manifest by the same rules as `design-system-adapter.ts`) so a
build before the first snapshot paints the language instead of UA defaults; it must
stay a superset of every consumed variable, which
`frontend/src/test/design-system-consumption.test.ts` asserts in both
directions.

**The artifact gate.** Recipe values, the component specimen and the theme board
are designed and frozen before implementation. See
[Design Artifact Gate](design-artifact-gate.md) for the approved set
(`design-artifacts/approved/quiet-instrument-migration/`), the generators, and
the verification tools (`capture-prototypes.mjs`,
`verify-component-conformance.mjs`).
