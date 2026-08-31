# Surface Direction: Operate Mode UI Design System

<!-- impeccable:surface-brief 1 -->

## Scope & Operating Context

- **Surface Scope:** Global desktop application shell, multi-pane editor, toolbars, overlays, and SDUI component catalog.
- **Visitor / Operator Mode:** Operate. Software engineers and systems programmers engaged in long, focused editing and debugging sessions.
- **Audience Job:** Fast keyboard navigation, distraction-free code reading/editing, unambiguous pane focus, immediate tactile feedback on interactive controls.
- **Primary Constraints:**
  - Strict separation of content-theme color authority (`var(--clay-*)`) from design-system geometry/materials.
  - Zero literal hex/RGB color values in design-system recipes.
  - Sub-millisecond keystroke responsiveness; zero layout shift or JavaScript recalculation on state changes.
  - Full accessibility conformance: forced-colors, prefers-reduced-motion, prefers-reduced-transparency.

---

## Direction Contract

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

### FINISH
Unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping package recipe carrying its proven conformance across all 25 component recipes.

---

## Contrast Case: Reference Glass Design System (`@clay/design-glass`)

To prove the architecture's modularity without modifying host components or breaking theme invariants:
- **Geometry:** Fluid squircle radii (`borderRadius: 8px` to `16px`).
- **Material:** Layered translucent surfaces (`surface.panel` with opacity), backdrop blur (`8px` to `16px`), subtle inner highlight (`inset 0 1px 0 rgba(...)`), and smooth diffuse shadows.
- **Motion:** Floating elevation transitions (`translateY(-2px)`, `200ms cubic-bezier(0.16, 1, 0.3, 1)`).
- **Fallbacks:** Guaranteed solid-panel fallback under `prefers-reduced-transparency`, `forced-colors`, or `@supports not (backdrop-filter: blur(1px))`.
