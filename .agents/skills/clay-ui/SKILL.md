---
name: clay-ui
description: Clay app UI conventions, reusable primitives, component catalog, theme tokens, typography hierarchy, and shell layout model. Use for ANY Clay UI work — revamping or adding components, panels, pop-ups, dropdowns, menus, text inputs, multi-selects, completion pop-ups, dialogs, tooltips, split layouts, theme/token/typography changes, or package UI contributions. Enforces primitives-first and visual proof: agents must reuse the documented component catalog, route through `npx ui-skills start`, then inspect screenshots and accessibility state of implemented UI before calling UI work complete.
---

# Clay UI

Clay is a native Rust GUI app (Masonry/winit + Vello/Parley). All UI visuals are owned by Clay core; packages declare inert components and typed tokens only. This skill is the single source of truth for reusable UI primitives and components.

## Step 0 (mandatory): Route through ui-skills

Before reviewing existing UI, planning, designing, or implementing any UI work, run:

```bash
npx ui-skills start
```

Then inspect the relevant category (`npx ui-skills list --category <category>`) and load the smallest useful skill set (`npx ui-skills get <slug>`; prefer 1, max 3). This is a per-task gate; do not reuse routing evidence from an earlier task. Record the selected category/slugs when the work is plan-driven. Apply the loaded guidance to Clay's native context — Clay has no CSS/Tailwind; translate web guidance into Clay theme tokens, typography variants, and Masonry primitives. Do not start source review or UI edits until this routing step is complete.

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
