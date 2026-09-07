# UI Design System Visual Direction Contract

**Date:** 2026-09-05  
**Status:** Approved Direction Contract - Amended for Legibility Revamp (Plan 110)  
**Related Plans:** [Plan 101](../../plans/101-UI-Design-System-Recipe-Foundation.md), [Plan 102](../../plans/102-UI-Design-System-Activation-and-Frontend-Runtime.md), [Plan 103](../../plans/103-UI-Design-System-Component-and-Surface-Migration.md), [Plan 104](../../plans/104-Neobrutal-and-Glass-Design-System-Packages-and-Conformance.md), [Plan 110](../../plans/110-UI-Design-System-Consistency-Neobrutal-Legibility-Revamp.md)  
**Product Reference:** [`PRODUCT.md`](../../PRODUCT.md)  
**Surface Brief:** [`.impeccable/surfaces/operate-visual-direction.md`](../../.impeccable/surfaces/operate-visual-direction.md)  
**Decision Log:** [`decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`](../../decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md)  

---

## 1. Executive Summary

This document establishes the visual direction contract for Clay's default UI design system package (`@clay/design-neobrutal`) and its companion reference package (`@clay/design-glass`).

In accordance with Clay's core architectural invariants, **content themes are the sole color authority**. UI design systems govern **geometry, framing, materials, shadows, spatial rhythm, and motion**, completely decoupled from color palettes, literal hex codes, or user-owned typography.

---

## 2. Direction Contract

### THESIS
A restrained, utilitarian Neobrutal design system engineered for prolonged desktop development. Replaces the generic rounded-gray modern editor aesthetic with blueprint-like compartmentalization, mathematical 90-degree corners, crisp 2px structural ink borders at rest (`borderColor: text.primary`), and physical mechanical feedback, refusing loud web Neobrutal cartoon tropes, garish yellow fills, and excessive non-functional ornamentation while guaranteeing high-contrast structural legibility across both dark and light content themes.

### OWN-WORLD
- **Palette & Material Authority:** Content themes are the sole color authority. Surfaces are opaque, crisp, and high-contrast, reading strictly from semantic theme tokens (`surface.canvas`, `surface.panel`, `surface.control`, `surface.selected`, `accent.primary`, `focus.ring`). List rows maintain solid `surface.control` fills at rest (never transparent); selected rows and panels receive 2px ink borders and solid `surface.selected` fills.
- **Geometry & Structure:** Hard 90-degree corners (`borderRadius: 0px` across all controls, containers, overlays, and dialogs), 2px structural framing borders at rest using the ink role (`borderColor: text.primary`), 2px focus outlines (`outlineColor: text.primary`), and hard offset drop shadows (`3px 3px 0px var(--clay-text-primary)` at rest, extending to `4px 4px 0px` on hover and collapsing to `1px 1px 0px` on active press) without diffuse blur (`blur: 0px`).
- **Spatial Rhythm:** Bimodal density — compact density (4px gap, 2px/4px padding) for toolbars, tab bars, status bars, and breadcrumbs; generous comfortable density (8px/12px padding) for dialogs, modals, and settings.
- **Motion Grammar:** Snappy mechanical translation (`transform: translateY(-1px)` via `hover-lift` on hover, `translateY(1px)` via `press-shift-down` on active press), `100ms ease-out` state transitions, collapsing to `0ms` under `prefers-reduced-motion`. Zero backdrop blur anywhere.

### STORY
The developer opens Clay to an environment that feels like a precision machinist's console. Every pane boundary is sharply defined; active tabs and focused controls announce themselves through solid mechanical offsets and crisp ink borders rather than blurry glows. Interactive elements react instantaneously with subtle physical compression, confirming action without breaking cognitive flow.

### FIRST VIEWPORT
A 3-pane split editor layout with a left workspace tree, central CodeMirror canvas with active tab bar and breadcrumb rail, right git diff pane, and bottom collapsible terminal/status bar. An active command palette overlay sits centered above the canvas, framed in a 2px solid ink border with hard 4px 4px 0px ink drop shadow, its text field and candidate list maintaining perfect 90-degree alignment.

### FORM
Restrained Utilitarian Blueprint Neobrutal (Candidate #3 in the Operate-mode grounded system matrix, Seed Key 87634504), raised by the density discipline of technical ruling engines and the strict baseline rhythm of typographic specimen books.

### FINISH CONDITION
Unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping package recipe carrying its proven conformance across every recipe-styled surface in the matrix (15 component kinds plus the 19 Clay-native internal/chrome surfaces and the reserved `table` kind).

---

## 3. Visual System Comparison & Conformance Grammar

| Attribute | Default Neobrutal (`@clay/design-neobrutal`) | Reference Glass (`@clay/design-glass`) | Conformance Rule / Invariant |
| --- | --- | --- | --- |
| **Corner Radius** | `0px` (Strict 90° right angles across all controls and surfaces) | `8px` (controls), `12px` (panels), `16px` (dialogs/modals) | Enforced via `--clay-ds-*-radius` recipe variables; zero CSS overrides in components |
| **Borders** | `2px solid var(--clay-text-primary)` (rest and active; ink role ensures high contrast on both dark and light themes), `2px solid var(--clay-text-primary)` (focus outline, offset 1px) | `1px solid var(--clay-border-subtle)` with subtle alpha | All border colors reference active theme semantic tokens |
| **Surfaces & Materials** | Opaque solid fills (`var(--clay-surface-*)`). List rows maintain solid `var(--clay-surface-control)` at rest (never transparent) and solid `var(--clay-surface-selected)` on selection with 2px ink border. Zero backdrop blur anywhere. | Translucent tinted fills (`opacity: 0.85` / `0.92`), `backdrop-filter: blur(12px)` | Glass provides solid opaque fallback under `prefers-reduced-transparency` and unsupported filters |
| **Shadows & Elevation** | Hard offset shadows (`3px 3px 0px var(--clay-text-primary)` at rest, `4px 4px 0px` on hover, `1px 1px 0px` on active press), 0px blur, strictly using the ink role (`text.primary`) rather than low-contrast `border.strong` | Diffused layered ambient shadows (`0 8px 24px -4px rgba(...)`), 1px top highlight | Max 3 shadow layers; zero literal colors in Neobrutal shadows |
| **Hover Feedback** | `translateY(-1px)` (`hover-lift`) with matching shadow extension to `4px 4px 0px` | Subtle surface brightness lift (`brightness(1.05)`), `translateY(-1px)` | Snappy `100ms ease-out` response |
| **Active / Press Feedback** | `translateY(1px)` (`press-shift-down`) with collapsed shadow to `1px 1px 0px` | `scale(0.99)` or `translateY(0px)` with softened shadow | Tactile physical click verification |
| **Focus State** | Crisp 2px solid offset ink ring (`outlineColor: text.primary`, offset 1px) or 2px inset ink border | 2px smooth glow/ring around squircle contour | Clear WCAG 2.1 AA focus indication |
| **Density** | Bimodal (Compact for code/navigation chrome; Comfortable for forms/settings) | Bimodal (Compact for code/navigation chrome; Comfortable for forms/settings) | Identical layout geometry and hit-target metrics prevent layout thrash |

---

## 4. Accessibility and Platform Constraints

1. **Forced Colors Mode (`forced-colors: active`):**
   - Both design systems yield to system-enforced canvas and highlight colors.
   - Neobrutal 2px ink borders ensure structure remains 100% visible even when backgrounds are replaced by the OS.
   - Glass translucent layers and blur filters are completely bypassed in favor of native OS system colors.
2. **Light and Dark Theme Contrast (Plan 110 Legibility Revamp):**
   - Neobrutal structural borders and drop shadows utilize the semantic ink role (`text.primary`) rather than `border.subtle` or `border.strong`, guaranteeing high contrast and crisp structural definition on light themes (e.g., Modus Operandi, Gruvbox Light) as well as dark themes (e.g., Modus Vivendi, Gruvbox Dark).
   - List rows maintain solid `surface.control` fills at rest and solid `surface.selected` fills when selected, preventing low-contrast washed-out rows.
3. **Reduced Motion (`prefers-reduced-motion: reduce`):**
   - All translation, spring, scale, and elevation transitions collapse to `0ms` (instant state change).
4. **Reduced Transparency (`prefers-reduced-transparency: reduce`):**
   - Translucent glass panels render as solid opaque surfaces using `var(--clay-surface-panel)` and `var(--clay-surface-control)`.
5. **Keystroke Latency & Rendering Budget:**
   - Recipe styles compile to static CSS custom properties on `:root`.
   - Zero DOM measurements, JavaScript hooks, or mutation observers during keyboard, scroll, or layout operations.

---

## 5. Traceability and Next Steps

- **Task 1 (Completed):** Package primitives reviewed, data-only package manifests enabled.
- **Task 2 (Completed):** Visual direction established and recorded in `.impeccable/surfaces/operate-visual-direction.md` and this document.
- **Task 3 (Completed):** Author `@clay/design-neobrutal` package manifest (`packages/design-neobrutal/package.json`), documentation, and complete recipe schema.
- **Task 4 (Completed):** Author `@clay/design-glass` reference package and fallbacks.
- **Plan 110 Task 7 (Completed):** Rewrite `@clay/design-neobrutal` into legible, proper neobrutalism (2px structural rest borders with `text.primary` ink role, hard offset shadows using ink color with blur 0, solid `surface.control` list row fills, 0px radius everywhere, tactile hover/active presets).

