# UI Design Systems

Comprehensive public reference for Clay's typed UI design-system recipe architecture, contribution schema, fallback rules, accessibility invariants, bundled design systems, and programmatic activation APIs.

Design language (normative): [`DESIGN.md`](../../DESIGN.md) — Quiet Instrument.
Authoritative decision: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.
Design-language decision: `decision-logs/2026-09-11-1615-quiet-instrument-design-language.md`.
Recipe matrix reference: `docs/development/ui-design-system-recipe-matrix.md`.
Conformance proof: `docs/development/ui-design-system-conformance.md`.

---

## 1. Overview & Authority Boundaries

Clay enforces a strict four-layer separation of concerns across visual aesthetics, geometry, typography, and semantic structure:

```
┌──────────────────────────────────────────────────────────┐
│                   Active Content Theme                   │
│         (Sole Normal-Rendering Color Authority)          │
│            e.g. Gruvbox, Modus, Tokyo Night              │
└─────────────────────────────┬────────────────────────────┘
                              │
                              │ provides var(--clay-*)
                              ▼
┌──────────────────────────────────────────────────────────┐
│                  Active UI Design System                 │
│         (Geometry, Materials, Motion & Token Maps)       │
│      e.g. Quiet Instrument (Default), @clay/core baseline │
└─────────────────────────────┬────────────────────────────┘
                              │
                              │ provides var(--clay-ds-*)
                              ▼
┌──────────────────────────────────────────────────────────┐
│                      Clay UI Shell                       │
│  (16 Component Kinds, 36 Recipe Surfaces, Landmarks)     │
└──────────────────────────────────────────────────────────┘
```

The design language itself — laws, values, per-surface recipes, retired
patterns — is normative in [`DESIGN.md`](../../DESIGN.md) (Quiet Instrument).
This page documents the mechanism that carries it.

1. **Content Themes (Sole Color Authority):** Own all color values, contrast pairs, syntax highlight palettes, and semantic color roles (`surface.main`, `text.primary`, `border.subtle`, `accent.primary`, `focus.ring`). Design systems may map semantic color roles to component slots and states, but **may never declare color palettes, raw color literals (`#hex`, `rgb()`, `hsl()`, named colors), or independent color values**.
2. **Typography Hierarchy:** Owns user-configured font family stacks and scale ratios across UI and editor surfaces.
3. **UI Design Systems:** Own geometry (radii, border widths, dimensions), materials (surface opacity, backdrop blur, structured shadow layers), state mappings, and motion timing.
4. **React Aria & Clay Core:** Own accessibility semantics, keyboard navigation, focus management, ARIA roles/states, and intent dispatch.

### Security and Inertness Guarantees

- Design systems are declarative data contributions (`clay.contributions.uiDesignSystem`).
- Packages **cannot** inject raw CSS strings, CSS selectors, URLs, network assets, arbitrary filter/transform pipelines, JSX, renderer callbacks, client scripts, or direct Tauri APIs into the webview.
- All recipe properties resolve deterministically to host-internal CSS custom properties (`--clay-ds-*`) at design-system install time — never during render or input hot paths.
- Any property or state omitted by a package design system resolves deterministically through Clay's core fallback recipes.

---

## 2. Bundled First-Party Design Systems

Clay ships one first-party UI design system package
(`@clay/design-instrument`), plus the `@clay/core` fallback recipes that fill
any recipe a package omits. Both are inert recipe data over the same component
catalog and the same content themes. The former `@clay/design-neobrutal` and
`@clay/design-glass` packages were removed by the Quiet Instrument migration
(plan 118 task 9); their languages are recorded in `decision-logs/` and
`plans/104-*`, not shipped.

### A. Quiet Instrument (`@clay/design-instrument` — Default)

The approved design language ([`DESIGN.md`](../../DESIGN.md)): one continuous
surface zoned by hairlines and whitespace, two elevations (canvas and overlay),
accent reserved for state, type carrying hierarchy.

- **Border Radii**: 5px (chips, kbd, badges), 8px (controls, rows, tabs),
  12px (panels, popovers, composers), 16px (window, sheets, modals), 9999px
  (tab items, segments, toasts, progress tracks, status dots). No 0px corners.
- **Borders**: 1px `border.hairline` only. State is fill plus typography: a
  selected row has no leading bar and no underline (retired pattern).
- **Materials**: single canvas surface; grouped content on a veil
  (`surface.panel` at 0.55); insets on `surface.control`; transient layers on
  `surface.overlay`. Zero backdrop blur outside the overlay scrim (3px) and
  toast (8px).
- **Shadows**: exactly two recipes — `overlay`
  (`0 24px 60px -20px` @0.42 + `0 2px 10px -4px` @0.22) and `pop`
  (`0 14px 34px -14px` @0.34 + `0 1px 3px -1px` @0.16) — on transient surfaces
  only. Static regions never float.
- **Motion**: 150ms `ease-out` state changes, 240ms `spring-snappy` surface
  entrances, one-shot 620ms accent pulse for keyboard focus moves; press is
  `press-shift-down`; no hover lift.
- **Accent**: `accent.primary` at 0.15 opacity for selected/active fills, and
  `focus.ring` for focus — never as decoration.

The package declares **165 recipe keys** (the recorded 142-key reference set minus
the 12 `chat.default.*` keys of the removed chat surface, plus the launcher /
tab-model / auxiliary families). Slots the host renders without a declared recipe —
`dropdown.triggerLabel`/`indicator`, `list.rowTitle`/`rowDetail`,
`collapse.title`/`chevron`, `panel.title`/`body`, `modal.title`/`body`,
`commandCentre.input`/`listBox`/`item`, `editorChrome.*`, `focusRing.ring` and the
rest — are marked `†` in
[`docs/development/ui-design-system-recipe-matrix.md` §1–3](../development/ui-design-system-recipe-matrix.md)
and in the
[component catalog](../../.agents/skills/clay-execution/references/components.md#ui-design-system-recipe-slots-plan-101);
they resolve to the core fallback instead of a package variable, and the markers
are drift-tested (`plan118_recipe_matrix_marks_undeclared_slots`).

### B. Core baseline (`@clay/core` — built-in fallback)

The host-owned fallback set (`core_design_system_fallbacks` in
`src/shell/design_system.rs`, mirrored by the `--clay-ds-*` block in
`frontend/src/styles/tokens.css`) that fills every recipe a package omits — and
the values the frame paints before a snapshot exists.

It is **the same language as the shipped system, not a second one**: the
host-consumed subset of `@clay/design-instrument` at the approved ladder (radii
5/8/12/16/pill, 0 only for full-bleed regions and non-boxes), one hairline border
weight, the two soft elevation stacks, and the language motion tiers (150ms state
changes, 240ms surface entrances, nothing linear). A build with no design system
installed therefore paints Quiet Instrument, and activating the package cannot
move geometry, material or motion; `tests/package_ui_conformance.rs` asserts both
equalities. It stays selectable, and is what a revoked or invalid design system
falls back to.

**Solid Active-Theme Fallbacks**: if `backdrop-filter` is unsupported or
`prefers-reduced-transparency` is active, veiled surfaces fall back to solid
active-theme fills with intact contrast, in both the shipped system and the
baseline.

---

## 3. Package Manifest Contribution Schema

A package declares a UI design system under `clay.contributions.uiDesignSystem` in its `package.json`:

```json
{
  "name": "@vendor/design-example",
  "version": "0.1.0",
  "type": "module",
  "clay": {
    "apiPrefix": "design-example",
    "docs": "./docs/index.md",
    "permissions": [],
    "modes": [],
    "contributions": {
      "uiDesignSystem": {
        "schemaVersion": 1,
        "id": "@vendor/design-example",
        "displayName": "Vendor Example",
        "extends": "@clay/design-instrument",
        "values": {
          "radius.smooth": { "type": "radius", "value": 6.0 },
          "radius.modal": { "type": "radius", "value": 14.0 },
          "blur.panel": { "type": "backdrop-blur", "value": 12.0 },
          "blur.modal": { "type": "backdrop-blur", "value": 24.0 },
          "saturate.standard": { "type": "backdrop-saturate", "value": 1.2 },
          "motion.smooth": { "type": "motion-duration", "value": 150.0 }
        },
        "recipes": {
          "button.default.root.rest": {
            "backgroundColor": "surface.control",
            "backgroundOpacity": 0.8,
            "textColor": "text.primary",
            "borderColor": "border.subtle",
            "borderWidth": 1.0,
            "borderStyle": "solid",
            "borderRadius": 6.0,
            "padding": "spacing.sm",
            "gap": "spacing.xs",
            "backdropBlur": 8.0,
            "backdropSaturate": 1.2,
            "innerHighlight": {
              "colorRole": "text.primary",
              "opacity": 0.12,
              "width": 1.0
            },
            "shadow": [
              { "x": 0.0, "y": 2.0, "blur": 6.0, "spread": 0.0, "colorRole": "border.strong", "opacity": 0.15 }
            ],
            "transitionDuration": 150.0,
            "transitionTiming": "ease-out"
          }
        }
      }
    }
  }
}
```

---

## 4. Recipe Key Structure

Recipe keys are 4-tuples formatted as:

$$\text{component}.\text{variant}.\text{slot}.\text{state}$$

### Component Kinds (All 16 Implemented + Reserved `table`)

Component kind identifiers are frozen on the `ComponentKind` enum (`src/shell/components.rs`): `editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, `statusItem`, `dropdown`, `collapse`, `modal`, `textInput` — plus the reserved `table` entry the recipe matrix carries for future phases. Recipe keys additionally address Clay-native internal surfaces (`chatPanel`, `commandCentre`, `completion`, `editorChrome`, `fileBrowser`, `paneSplitTree`, `settingsPanel`, `statusBar`, `tabBar`, `transientMenu`, `welcome`) and chrome primitives (`badge`, `divider`, `focusRing`, `iconSlot`, `kbd`, `scrim`, `scrollChrome`, `tooltip`); the full 35-surface inventory lives in the recipe matrix, which is authoritative.

### Semantic Slots
`root`, `field`, `input`, `label`, `description`, `error`, `trigger`, `popover`, `list`, `item`, `row`, `header`, `body`, `dialog`, `scrim`, `scrollbarTrack`, `scrollbarThumb`, `container`, `gutter`, `activeLine`, `selection`, `matchingBracket`, `findMatch`.

### States
`rest`, `hover`, `active`, `focus`, `selected`, `disabled`, `invalid`.

---

## 5. Property Families and Value Domains

| Property | Allowed Types / Ranges | Safety Ceiling | Behavior |
| --- | --- | --- | --- |
| `backgroundColor` | Theme color role reference | Valid theme role | Resolves to `var(--clay-*)` |
| `backgroundOpacity` | Finite float in `[0.0, 1.0]` | `1.0` | Material alpha |
| `textColor` | Theme color role reference | Valid theme role | Resolves to `var(--clay-*)` |
| `borderColor` | Theme color role reference | Valid theme role | Resolves to `var(--clay-*)` |
| `borderWidth` | Finite float in `[0.0, 8.0]` px | `8.0px` | Structural border width |
| `borderStyle` | Enum (`none`, `solid`, `dashed`, `dotted`) | N/A | Border style |
| `borderRadius` | Finite float in `[0.0, 32.0]` px or `9999.0` (pill) | `9999.0px` | Corner rounding; the shipped language uses the 5/8/12/16/pill ladder, and a package may declare any value in range |
| `padding` / `gap` | Spacing token reference | Valid token | Resolves to spacing grid |
| `shadow` | Structured shadow layers (max 3) | Max 3 layers | Multi-layer shadow stack |
| `backdropBlur` | Finite float in `[0.0, 32.0]` px | `32.0px` | Localized frosted blur |
| `backdropSaturate` | Finite float in `[1.0, 2.0]` | `2.0` | Backdrop color vibrance |
| `innerHighlight` | Structured rim highlight (`colorRole`, `opacity`, `width`) | `width: [1..4]`, `opacity: [0..1]` | Specular optical refraction |
| `outlineColor` | Theme color role reference | Valid theme role (`focus.ring`) | Accessibility focus ring |
| `outlineWidth` | Finite float in `[0.0, 8.0]` px | `8.0px` | Focus outline width |
| `outlineOffset` | Finite float in `[-16.0, 16.0]` px | `16.0px` | Focus outline separation |
| `outlineStyle` | Enum (`none`, `solid`) | N/A | Focus outline style |
| `transitionDuration` | Finite float in `[0.0, 1000.0]` ms | `1000.0ms` | Motion duration |
| `transitionTiming` | Enum (`linear`, `ease-out`, `spring-snappy`, `spring-smooth`) | N/A | Motion curve |
| `transformPreset` | Enum (`none`, `press-subtle`, `press-shift-down`, `hover-lift`) | Bounded | Interactive micro-motion |

---

## 6. Accessibility & Performance Fallbacks

1. **Forced Colors Mode (`@media (forced-colors: active)`):** System high-contrast colors (`Highlight`, `HighlightText`, `GrayText`, `CanvasText`) override custom recipe styling for focus outlines, selections, borders, and disabled elements.
2. **Reduced Motion (`@media (prefers-reduced-motion: reduce)`):** All transition durations collapse to `0.01ms !important` and `transform: none !important`, ensuring instant state transitions without latency.
3. **Reduced Transparency (`@media (prefers-reduced-transparency: reduce)`):** Disables `backdrop-filter` and sets material opacity to `1.0`, falling back directly to solid active-theme surface roles (`var(--clay-surface-overlay)`).
4. **Unsupported Backdrop Filter (`@supports not (backdrop-filter: blur(1px))`):** Automatically falls back to opaque surface roles with preserved text contrast.
5. **Editor Canvas & Scroll Safety**: Editor typing canvas, line gutters, and scroll containers enforce `backdropBlur == 0.0` at all times.

---

## 7. Content-Theme Contrast Gate (implemented)

Content themes are the sole colour authority, so the themes themselves are gated.
`src/shell/theme.rs` owns the policy; `theme_meets_contrast` walks it, and
`validate_active_theme_contrast` runs it on every activation path (the `setTheme`
apply path and the canonical-default resolver) — a failing pair rejects the
activation atomically, leaving the previous theme in place, and a role a theme
does not declare falls back to the core catalog and is measured there, so the
gate cannot be passed by declaring fewer tokens.

**Every pair is measured composited.** The foreground role is alpha-blended over
the background role first (`editor::theme::composited_contrast_ratio`), because
alpha is what the eye sees: a 34 % hairline scores 4:1 from its raw bytes on dark
chrome and 21:1 on light, but reads 1.4:1 and 1.5:1 respectively. The
pre-compositing gate therefore approved hairlines and state fills that were
invisible.

| Group | Roles (threshold) | Why this floor |
| --- | --- | --- |
| Prose | `text.primary` / `text.muted` / `text.tooltip` on their surface, `text.badge`, `text.disabled` on the canvas (4.5:1); `text.kbd` on `surface.kbd` (3.0:1) | WCAG 2.1 SC 1.4.3 normal text; a kbd chip is non-text UI (SC 1.4.11) |
| Affordance | `accent.primary`, `accent.muted`, `focus.ring`, `border.focus` on the canvas (3.0:1) | SC 1.4.11: keyboard focus and an affordance must be perceivable |
| Structural boundary | `border.subtle` on the canvas **and** the panel, `border.strong` on the canvas (3.0:1) | a control outline that cannot be seen is a broken control, not a style choice; both surfaces because a boundary is drawn against either |
| Hairline | `border.hairline` on the canvas and the panel (**1.2:1**) | `DESIGN.md` §10.1 makes the hairline quieter than `border.subtle` by definition (34 % of the border grey), which cannot reach 3:1; it is exempt from that floor but never allowed to vanish |
| State fill | `text.primary` on `surface.hover` / `surface.active` / `surface.selected` as painted (3.0:1) | a fill is drawn *under* its text, so the fill is composited over its surface first — the pre-migration uniform 40 %-alpha selection colour scored 1.03–1.43:1 against its own text |

The border ladder itself is monotonic wherever it is role-driven — the theme's
border grey at 34 % (`border.hairline`), the same grey at 100 % (`border.subtle`)
and the theme's ink (`border.strong`). `tests/theme_packages.rs` asserts
`hairline < subtle < strong` on both surfaces for every shipped theme, and the
core catalog is asserted the same way in `src/shell/theme.rs`.

### Measured worst pair per group (composited)

Values are re-derived by `tests/theme_packages.rs` (shipped themes) and the
in-crate core-catalog test; the shipped-theme column values are identical to the
approved board in
`design-artifacts/approved/quiet-instrument-migration/theme-values.md`, which
records every pair per theme.

| Palette | Prose (≥4.5) | Affordance (≥3.0) | Boundary (≥3.0) | Hairline (≥1.2) | Fill (≥3.0) |
| --- | --- | --- | --- | --- | --- |
| `@clay/core` (baseline) | 5.13 | 5.13 | 3.70 | 3.70 | 8.83 |
| Modus Operandi | 5.33 | 5.48 | 3.21 | 1.42 | 14.73 |
| Modus Vivendi | 6.49 | 5.07 | 3.32 | 1.39 | 9.32 |
| Gruvbox Material Dark | 5.56 | 3.98 | 4.02 | 1.61 | 5.63 |
| Gruvbox Material Light | 5.21 | 3.56 | 3.31 | 1.43 | 5.97 |

Two notes on the baseline row and on what is *not* gated:

- The baseline column is the palette as the frame paints it before any snapshot
  (`StyleRegistry::default()` layered over the core catalog): borders resolve from
  the legacy base-ui `scrollbar` colour, one flat value for all three roles — the
  *default*, not a ceiling: since plan 118 task E7 a theme may declare
  `accent` and the ladder keys `borderHairline` / `borderSubtle` / `borderStrong`
  in `textStyles`, and the projection prefers them over the `caret` / `scrollbar`
  stand-ins (the same gate then judges them, so a declared ladder that cannot be
  seen is refused by name). Themes that declare nothing keep today's behaviour
  exactly.
- The floors are checked against the roles, not against the attenuated paint: a
  disabled control that also applies `opacity.disabled` renders below the prose
  floor, which WCAG exempts for disabled elements.

The host-owned theme-role block in `frontend/src/styles/tokens.css` must mirror
the core catalog value each role resolves to, or the window repaints on the first
snapshot; `src/shell/theme.rs` asserts that equality for every role the block
declares.

---

## 8. Programmatic Activation API

Activate a design system via `clay:theme` in configuration or scripts:

```javascript
import { setDesignSystem } from "clay:theme";

// Activate the shipped Quiet Instrument design system
await setDesignSystem("@clay/design-instrument");

// Activate the built-in core baseline recipes instead
await setDesignSystem("@clay/core");

// Reset to core fallback design system
await setDesignSystem(null);
```

See the [theme.setDesignSystem API Documentation](clay-js-api/theme/set-design-system.md) for parameter types and error handling.
