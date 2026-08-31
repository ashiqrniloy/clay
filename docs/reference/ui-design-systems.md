# UI Design Systems

Comprehensive public reference for Clay's typed UI design-system recipe architecture, contribution schema, fallback rules, accessibility invariants, bundled design systems, and programmatic activation APIs.

Authoritative decision: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.
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
│           e.g. Neobrutal (Default), Glass, Core          │
└─────────────────────────────┬────────────────────────────┘
                              │
                              │ provides var(--clay-ds-*)
                              ▼
┌──────────────────────────────────────────────────────────┐
│                      Clay UI Shell                       │
│     (25 Component Kinds, Semantic Roles & Landmarks)     │
└──────────────────────────────────────────────────────────┘
```

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

Clay ships with two reference first-party UI design system packages:

### A. Restrained Neobrutal (`@clay/design-neobrutal` — Default)

Modern, technical, and restrained neobrutalism designed for distraction-free coding and high information density:

- **Border Radii**: Strict `0px` sharp corners across all 25 component kinds.
- **Borders**: 1px structural solid borders (`var(--clay-border-subtle)` in rest, `var(--clay-border-strong)` in hover).
- **Shadows**: Hard, crisp 2px–4px offset box-shadows (`box-shadow: 2px 2px 0px var(--clay-border-strong)`) with zero blur.
- **Materials**: 100% opaque, solid backgrounds (`backdropBlur == 0px`).
- **Motion**: Snappy 100ms transitions (`transition-timing-function: cubic-bezier(0, 0, 0.2, 1)`).

### B. Luminous Glass (`@clay/design-glass` — Reference)

Optical, layered frosted-glass material system with specular highlights:

- **Border Radii**: Smooth 4px–14px curves (`4px` list items, `6px` buttons/inputs, `8px` cards/panels, `14px` modals, `9999px` badges/pills).
- **Materials**: Translucent background opacities (`0.7`–`0.85`) combined with localized hardware-accelerated `backdrop-filter: blur(8px–24px) saturate(1.2–1.4)`.
- **Specular Highlights**: 1px inner highlights (`box-shadow: inset 0 1px 0 0 rgba(...)`) simulating physical glass refraction.
- **Shadows**: Soft, multi-layered diffuse shadows (`box-shadow: 0 4px 12px var(--clay-border-strong)`).
- **Performance Invariant**: Text editor canvas, line number gutter, and scroll track remain 100% solid (`backdropBlur == 0.0`) to guarantee 60fps scrolling and sub-millisecond typing latency.
- **Solid Active-Theme Fallbacks**: If `backdrop-filter` is unsupported or `prefers-reduced-transparency` is active, surfaces gracefully fall back to solid active-theme fills with intact contrast.

---

## 3. Package Manifest Contribution Schema

A package declares a UI design system under `clay.contributions.uiDesignSystem` in its `package.json`:

```json
{
  "name": "@vendor/design-glass",
  "version": "0.1.0",
  "type": "module",
  "clay": {
    "apiPrefix": "design-glass",
    "docs": "./docs/index.md",
    "permissions": [],
    "modes": [],
    "contributions": {
      "uiDesignSystem": {
        "schemaVersion": 1,
        "id": "@vendor/design-glass",
        "displayName": "Glass (Reference)",
        "extends": "@clay/design-neobrutal",
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

### Component Kinds (All 25 Supported)
`button`, `textInput`, `dropdown`, `list`, `collapse`, `modal`, `panel`, `label`, `statusItem`, `flex`, `stack`, `overlay`, `portal`, `scroll`, `tab`, `tabBar`, `card`, `badge`, `kbd`, `tooltip`, `popover`, `menu`, `commandCentre`, `chat`, `editor`.

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
| `borderRadius` | Finite float in `[0.0, 32.0]` px or `9999.0` (pill) | `9999.0px` | Corner rounding |
| `padding` / `gap` | Spacing token reference | Valid token | Resolves to spacing grid |
| `shadow` | Structured shadow layers (max 8) | Max 8 layers | Multi-layer shadow stack |
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

## 7. Programmatic Activation API

Activate a design system via `clay:theme` in configuration or scripts:

```javascript
import { setDesignSystem } from "clay:theme";

// Activate the default Neobrutal design system
await setDesignSystem("@clay/design-neobrutal");

// Activate the Glass reference design system
await setDesignSystem("@clay/design-glass");

// Reset to core fallback design system
await setDesignSystem(null);
```

See the [theme.setDesignSystem API Documentation](clay-js-api/theme/set-design-system.md) for parameter types and error handling.
