# Clay Design System & UI Architecture

## 1. Overview & Core Philosophy

Clay is an engineering-first desktop editor built for density, precision, and tactile responsiveness. The UI architecture enforces a strict separation between **Content Themes** (which own color) and **UI Design Systems** (which own geometry, materials, and motion).

```
┌─────────────────────────────────────────────────────────┐
│                  Active Content Theme                   │
│        (Sole Normal-Rendering Color Authority)          │
│            e.g. Gruvbox, Modus, Tokyo Night             │
└────────────────────────────┬────────────────────────────┘
                             │
                             │ injects var(--clay-*)
                             ▼
┌─────────────────────────────────────────────────────────┐
│                 Active UI Design System                 │
│        (Geometry, Materials, Motion & Token Maps)       │
│           e.g. Neobrutal (Default), Glass, Core         │
└────────────────────────────┬────────────────────────────┘
                             │
                             │ injects var(--clay-ds-*)
                             ▼
┌─────────────────────────────────────────────────────────┐
│                     Clay UI Shell                       │
│  (15 Component Kinds, 35 Recipe Surfaces, Landmarks)    │
└─────────────────────────────────────────────────────────┘
```

Surface counts are mechanical: `ComponentKind` (`src/shell/components.rs`)
defines exactly **15** SDUI component kinds (plus the reserved `table` entry),
and the recipe matrix (`docs/development/ui-design-system-recipe-matrix.md`)
style-stamps **35** distinct surfaces total — the 15 kinds, the reserved
`table` kind, 11 Clay-native internal surfaces, and 8 chrome primitives. It
never declares "25 component kinds"; that was a pre-Phase 20.5 number.

---

## 2. Two First-Party Design Systems

### A. Restrained Neobrutal (`@clay/design-neobrutal` — Default)

Modern, technical, and restrained neobrutalism designed for distraction-free coding and tool density:

- **Border Radii**: Strict `0px` sharp corners across all 15 component kinds and every recipe-styled surface in the matrix.
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

## 3. Spatial System & Typography Hierarchy

### Spacing Scale (4-point Grid)
- `spacing.none`: `0px`
- `spacing.xxs`: `2px`
- `spacing.xs`: `4px`
- `spacing.sm`: `8px`
- `spacing.md`: `12px`
- `spacing.lg`: `16px`
- `spacing.xl`: `24px`
- `spacing.xxl`: `32px`

### Typography Scale
- **UI Base**: 13px / 1.4 line-height (clean system sans-serif or customized via user typography configuration).
- **Code / Mono**: 13px–14px monospace font with tabular figures.
- **Status / Captions**: 11px / 1.3 line-height.
- **Headings / Titles**: 14px–16px medium/semibold.

---

## 4. Accessibility & Invariant Boundaries

1. **WCAG 2.1 AA Conformance**: All text and control pairings maintain a minimum contrast ratio of 4.5:1 (normal text) and 3:1 (large text/UI affordances) across all theme combinations.
2. **Focus Visibility**: Every interactive component exposes a prominent 2px focus ring (`var(--clay-focus-ring)`) with deterministic 1px outline offset.
3. **No Concrete Colors in Design Systems**: Design system package manifests declare zero hex/RGB/HSL literals. All color fields reference semantic theme roles (`surface.control`, `text.primary`, `border.subtle`, `accent.primary`, `focus.ring`).
4. **State Preservation**: Switching design systems dynamically updates CSS variables without unmounting React components, losing editor buffers, or resetting focus.
