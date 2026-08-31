# UI Design System Visual Direction Contract

**Date:** 2026-08-30  
**Status:** Approved Direction Contract  
**Related Plans:** [Plan 101](../../plans/101-UI-Design-System-Recipe-Foundation.md), [Plan 102](../../plans/102-UI-Design-System-Activation-and-Frontend-Runtime.md), [Plan 103](../../plans/103-UI-Design-System-Component-and-Surface-Migration.md), [Plan 104](../../plans/104-Neobrutal-and-Glass-Design-System-Packages-and-Conformance.md)  
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
A restrained, utilitarian Neobrutal design system engineered for prolonged desktop development. Replaces the generic rounded-gray modern editor aesthetic with blueprint-like compartmentalization, mathematical 90-degree corners, crisp 1px/2px structural borders, and physical mechanical feedback, refusing loud web Neobrutal cartoon tropes, garish yellow fills, and excessive non-functional ornamentation.

### OWN-WORLD
- **Palette & Material Authority:** Content themes are the sole color authority. Surfaces are opaque, crisp, and high-contrast, reading strictly from semantic theme tokens (`surface.canvas`, `surface.panel`, `surface.control`, `border.subtle`, `border.strong`, `accent.primary`, `focus.ring`).
- **Geometry & Structure:** Hard 90-degree corners (`borderRadius: 0px` across all controls, containers, overlays, and dialogs), 1px structural framing borders, 2px borders for active/focus states, and hard offset drop shadows (`2px 2px 0px var(--clay-border-strong)`) without diffuse blur.
- **Spatial Rhythm:** Bimodal density — compact density (4px gap, 2px/4px padding) for toolbars, tab bars, status bars, and breadcrumbs; generous comfortable density (8px/12px padding) for dialogs, modals, and settings.
- **Motion Grammar:** Snappy mechanical translation (`transform: translate(-1px, -1px)` on hover, `translate(1px, 1px)` on active press), `100ms ease-out` state transitions, collapsing to `0ms` under `prefers-reduced-motion`.

### STORY
The developer opens Clay to an environment that feels like a precision machinist's console. Every pane boundary is sharply defined; active tabs and focused controls announce themselves through solid mechanical offsets and crisp focus borders rather than blurry glows. Interactive elements react instantaneously with subtle physical compression, confirming action without breaking cognitive flow.

### FIRST VIEWPORT
A 3-pane split editor layout with a left workspace tree, central CodeMirror canvas with active tab bar and breadcrumb rail, right git diff pane, and bottom collapsible terminal/status bar. An active command palette overlay sits centered above the canvas, framed in a 2px solid border with hard 3px 3px 0px drop shadow, its text field and candidate list maintaining perfect 90-degree alignment.

### FORM
Restrained Utilitarian Blueprint Neobrutal (Candidate #3 in the Operate-mode grounded system matrix, Seed Key 87634504), raised by the density discipline of technical ruling engines and the strict baseline rhythm of typographic specimen books.

### FINISH CONDITION
Unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping package recipe carrying its proven conformance across all 25 component recipes.

---

## 3. Visual System Comparison & Conformance Grammar

| Attribute | Default Neobrutal (`@clay/design-neobrutal`) | Reference Glass (`@clay/design-glass`) | Conformance Rule / Invariant |
| --- | --- | --- | --- |
| **Corner Radius** | `0px` (Strict 90° right angles across all controls and surfaces) | `8px` (controls), `12px` (panels), `16px` (dialogs/modals) | Enforced via `--clay-ds-*-radius` recipe variables; zero CSS overrides in components |
| **Borders** | `1px solid var(--clay-border-subtle)` (rest), `2px solid var(--clay-focus-ring)` (focus) | `1px solid var(--clay-border-subtle)` with subtle alpha | All border colors reference active theme semantic tokens |
| **Surfaces & Materials** | Opaque solid fills (`var(--clay-surface-*)`) | Translucent tinted fills (`opacity: 0.85` / `0.92`), `backdrop-filter: blur(12px)` | Glass provides solid opaque fallback under `prefers-reduced-transparency` and unsupported filters |
| **Shadows & Elevation** | Hard offset shadows (`2px 2px 0px var(--clay-border-strong)`), 0px blur | Diffused layered ambient shadows (`0 8px 24px -4px rgba(...)`), 1px top highlight | Max 3 shadow layers; zero literal colors in Neobrutal shadows |
| **Hover Feedback** | `translate(-1px, -1px)` with matching shadow extension | Subtle surface brightness lift (`brightness(1.05)`), `translateY(-1px)` | Snappy `100ms ease-out` response |
| **Active / Press Feedback** | `translate(1px, 1px)` with collapsed shadow (`0px 0px 0px`) | `scale(0.99)` or `translateY(0px)` with softened shadow | Tactile physical click verification |
| **Focus State** | Crisp 2px solid offset ring or 2px inset border | 2px smooth glow/ring around squircle contour | Clear WCAG 2.1 AA focus indication |
| **Density** | Bimodal (Compact for code/navigation chrome; Comfortable for forms/settings) | Bimodal (Compact for code/navigation chrome; Comfortable for forms/settings) | Identical layout geometry and hit-target metrics prevent layout thrash |

---

## 4. Accessibility and Platform Constraints

1. **Forced Colors Mode (`forced-colors: active`):**
   - Both design systems yield to system-enforced canvas and highlight colors.
   - Neobrutal 1px/2px borders ensure structure remains 100% visible even when backgrounds are replaced by the OS.
   - Glass translucent layers and blur filters are completely bypassed in favor of native OS system colors.
2. **Reduced Motion (`prefers-reduced-motion: reduce`):**
   - All translation, spring, scale, and elevation transitions collapse to `0ms` (instant state change).
3. **Reduced Transparency (`prefers-reduced-transparency: reduce`):**
   - Translucent glass panels render as solid opaque surfaces using `var(--clay-surface-panel)` and `var(--clay-surface-control)`.
4. **Keystroke Latency & Rendering Budget:**
   - Recipe styles compile to static CSS custom properties on `:root`.
   - Zero DOM measurements, JavaScript hooks, or mutation observers during keyboard, scroll, or layout operations.

---

## 5. Traceability and Next Steps

- **Task 1 (Completed):** Package primitives reviewed, data-only package manifests enabled.
- **Task 2 (Completed):** Visual direction established and recorded in `.impeccable/surfaces/operate-visual-direction.md` and this document.
- **Task 3 (Next):** Author `@clay/design-neobrutal` package manifest (`packages/design-neobrutal/package.json`), documentation, and complete 25-recipe schema.
- **Task 4 (Upcoming):** Author `@clay/design-glass` reference package and fallbacks.
