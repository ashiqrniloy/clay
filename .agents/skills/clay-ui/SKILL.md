---
name: clay-ui
description: Clay app UI conventions, mandatory project-local design-skill stack, reusable primitives, component catalog, theme tokens, typography hierarchy, and shell layout model. Use for ANY Clay UI work — revamping or adding components, panels, pop-ups, dropdowns, menus, text inputs, multi-selects, completion pop-ups, dialogs, tooltips, split layouts, theme/token/typography changes, or package UI contributions. Enforces the required design-skill stack plus Clay layout/spatial-engineering directives, primitives-first implementation, complete output, and visual/accessibility proof.
---

# Clay UI

Clay currently ships a native Rust GUI (Masonry/winit + Vello/Parley) and is migrating to a Tauri v2 + React client. Clay core owns host visuals in both renderers; packages declare inert components and typed tokens only. This skill is the single source of truth for reusable Clay UI primitives and components.

## Step 0 (mandatory): Load the complete UI skill stack

Before reviewing existing UI, planning, designing, or implementing each UI task, read:

1. `.agents/skills/clay-ui/SKILL.md`
2. `.agents/skills/clay-ui/references/components.md`
3. `.agents/skills/clay-ui/references/tokens.md`
4. `.agents/skills/impeccable/SKILL.md`
5. `.agents/skills/full-output-enforcement/SKILL.md`
6. `.agents/skills/high-end-visual-design/SKILL.md`
7. `.agents/skills/design-taste-frontend/SKILL.md`

All four design/output skills are mandatory; do not select a subset. Clay's layout and web-engineering directives are part of this skill (see below). Load them again for every independently executed UI task. In plans, list all seven files under that task's `Approach -> Documentation Reviewed`; plan-level evidence alone is insufficient.

Synthesize rather than blindly concatenate their aesthetics:

- User brief, existing Clay product identity, accessibility, security, authority boundaries, catalog compatibility, and typed theme/token ownership are hard constraints.
- `impeccable` owns product-context workflow, critique, bounded visual verification, and production craft.
- `full-output-enforcement` forbids placeholders, partial component states, or omitted deliverables.
- `high-end-visual-design` and `design-taste-frontend` provide anti-generic composition, typography, material, responsive, asset, and motion scrutiny. Adapt marketing-page rules to Clay's Operate-mode desktop UI; do not force AIDA, hero sections, decorative motion, hardcoded fonts/colors, or GSAP where they do not serve the task.
- The Layout and Spatial Engineering / Web Engineering Directives sections below supply rigid information hierarchy, mechanical precision, grid discipline, and data-density rules without forcing one palette or motif when the brief disagrees.
- Current native work translates applicable web guidance into Clay tokens, typography variants, Masonry primitives, and AccessKit semantics. Target React work uses the same semantic tokens through host-generated CSS custom properties and CodeMirror adapters.

Do not start source review or UI edits until this complete stack is loaded and reconciled against the task brief.

## Layout and Spatial Engineering

The layout must appear mathematically engineered. It rejects conventional web padding in favor of visible compartmentalization.

*   **The Blueprint Grid:** Strict adherence to CSS Grid architectures. Elements do not float; they are anchored precisely to grid tracks and intersections.
*   **Visible Compartmentalization:** Extensive utilization of solid borders (`1px` or `2px solid`) to delineate distinct zones of information. Horizontal rules (`<hr>`) frequently span the entire container width to segregate operational units.
*   **Bimodal Density:** Layouts oscillate between extreme data density (tightly packed monospace metadata clustered together) and vast expanses of calculated negative space framing macro-typography.
*   **Geometry:** Absolute rejection of `border-radius`. All corners must be exactly 90 degrees to enforce mechanical rigidity.

Clay adaptation: apply these as structure and density discipline through typed Clay tokens and cataloged primitives — never raw colors, concrete fonts, or off-catalog widgets. Where Clay's product identity requires rounded corners or softer surfaces, the user brief and existing catalog win over these defaults.

## Web Engineering Directives

1.  **Grid Determinism:** Utilize `display: grid; gap: 1px;` with contrasting parent/child background colors to generate mathematically perfect, razor-thin dividing lines without complex border declarations.
2.  **Semantic Rigidity:** Construct the DOM using precise semantic tags (`<data>`, `<samp>`, `<kbd>`, `<output>`, `<dl>`) to accurately reflect the technical nature of the telemetry.

Clay adaptation: directives apply to the Tauri/React target renderer; native Masonry equivalents use the matching token-driven primitive (dividing lines from tokens, semantic roles from AccessKit semantics, fluid type from `UiTextVariant` levels).

## Step 1 (mandatory): Inspect implemented UI

After every UI change, launch representative UI states and take screenshots. Inspect default and every changed interaction state; include empty, error/recovery, and narrow/wide layout states when applicable. Record screenshot paths and findings with completion evidence.

When `computer-use-linux` is available, call `get_app_state` first, then inspect the accessibility tree and test keyboard focus/order, names, roles, states, modal containment, and live announcements for changed controls. Re-check state after interaction. If GUI launch, screenshot capture, or computer use is blocked, record the exact blocker and leave visual/a11y acceptance unresolved; structural tests do not substitute for visual evidence.

## Golden Rules

1. **Primitives/components first.** Check [references/components.md](references/components.md) before building anything. Reuse an existing component or primitive. Never hand-roll a custom widget when a catalog entry covers the need. New components require explicit justification and must be added to the catalog in the same change.
2. **Token-only styling.** All colors, spacing, radii, typography, and opacity come from typed Clay theme tokens ([references/tokens.md](references/tokens.md)). Raw colors, raw CSS, concrete font families, and concrete point sizes are rejected by validation. Users configure theme and fonts; packages and components reference tokens.
3. **Font roles and variants only.** Text uses a semantic font role (`ui`, `monospace`, `proportional`) plus a `UiTextVariant` hierarchy level. Never hardcode families or sizes — both are user-configurable.
4. **Inert contributions.** Package UI emits inert command intents and versioned, bounded payloads. No package JavaScript in paint/layout/pointer/key handlers, no direct Masonry widget access.
5. **Do not break existing packages.** Component kinds, style variables, and token names are additive-only. Renames/removals need a decision log and migration path.
6. **Keep the catalog current.** Any change that adds, modifies, or removes a UI component, primitive, token, or layout rule must update `references/components.md` / `references/tokens.md` in the same commit or phase.

## Architecture Map

| Area | File |
|------|------|
| Component catalog + style-variable validation | `src/shell/components.rs` |
| Theme token resolver + core tokens | `src/shell/theme.rs` |
| Shell layout: working area, pane split tree, slots | `src/shell/layout.rs` |
| Package UI runtime: fixed panels, overlays, input routing | `src/shell/package_ui.rs` |
| Transient menu + inline completion pop-up | `src/shell/transient_menu.rs` |
| File browser surface | `src/shell/file_browser.rs` |
| Root shell widget | `src/masonry_shell.rs` |
| SDUI/package component paint | `src/masonry_sdui.rs` |
| Editor colors + style registry | `src/editor/theme.rs` |
| Font roles, typography variants, text metrics | `src/editor/typography.rs` |
| Package authoring guide | `docs/reference/packages/creating-packages.md` |

## Layout Model (summary)

- **Working area** owns a **pane split tree** (horizontal/vertical splits, ratio-clamped).
- Each leaf pane has a mandatory **main** slot plus optional **left, right, top, bottom** fixed panel slots.
- Fixed panel slots have `size`, `min_size`, `max_size`, `visible`, `collapsed`, `resized_by_user` — panel sizes stay user-configurable; never hardcode panel extents.
- Transient surfaces (menus, completion pop-up, overlays) render above slots with explicit anchor, dismissal, and focus policy.

Full component and token details: [references/components.md](references/components.md), [references/tokens.md](references/tokens.md).
