# UI Design-System CSS Audit and Declaration Ownership Ledger

Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.  
Plan reference: `plans/103-UI-Design-System-Component-and-Surface-Migration.md` (Task 1).  
Recipe Matrix: `docs/development/ui-design-system-recipe-matrix.md`.  
Catalog reference: `.agents/skills/clay-ui/references/components.md` and `.agents/skills/clay-ui/references/tokens.md`.

---

## 1. Executive Summary and Scope

This audit provides a comprehensive, declaration-by-declaration inventory of all frontend visual, structural, color, typography, and state declarations across Clay's React client.

### Invariant Principles
1. **Content Themes as Sole Color Authority:** All normal-rendering colors for surfaces, text, borders, focus outlines, selections, status items, diagnostics, overlays, and solid fallbacks resolve exclusively from the active content theme via semantic theme color roles (e.g. `surface.control`, `text.primary`, `border.focus`). UI design-system packages may map a component slot/state to a theme color role, but may never introduce literal colors (`#hex`, `rgb()`, `hsl()`, named colors) or package-owned color palettes.
2. **Forced-Colors Accessibility Exemption:** OS/browser system colors (`Canvas`, `CanvasText`, `Highlight`, `HighlightText`, `ButtonFace`, `ButtonText`) are used solely under `@media (forced-colors: active)` media queries.
3. **UI Design Systems Own Non-Color Geometry, Materials, and Motion:** Radii, border widths, border styles, shadows, backdrop blur/saturation, inner highlights, opacity multipliers, transition timings/durations, and tactile transform presets resolve through host-owned recipe custom properties (`--clay-ds-*`) and fall back deterministically to Clay core defaults.
4. **Structural Layout Remains Host-Owned:** CSS Grid, Flexbox alignment, split ratios, panel min/max dimensions, overflow boundaries, landmarks, z-index layers, and React Aria accessibility semantics remain strictly host-owned in CSS Modules and TSX components.
5. **No Package Injection:** Packages cannot inject raw CSS strings, custom DOM selectors, arbitrary URLs, keyframe animations, filter pipelines, script callbacks, or Tauri API calls into the main webview.

### Scope Inventory
- **21 CSS Files:**
  - 17 Component & Surface CSS Modules (`button.module.css`, `chrome.module.css`, `controls.module.css`, `modal.module.css`, `text-field.module.css`, `text.module.css`, `shell.module.css`, `tab-bar.module.css`, `chat.module.css`, `command-centre.module.css`, `editor.module.css`, `package-workspace.module.css`, `fixture.module.css`, `workspace.module.css`, `registry.module.css`, `renderer.module.css`, `settings-panel.module.css`, `pane-tree.module.css`, `workspace-panes.module.css`)
  - 2 Global / Token Definition Files (`global.css`, `tokens.css`)
- **5 Inline Style Sites in TSX Files:**
  - `frontend/src/app/layout/tab-bar.tsx` (`TabList style={{ display: "contents" }}`)
  - `frontend/src/app/layout/working-area.tsx` (`Separator style={{ width: "var(--clay-dimension-border-hairline, 1px)" }}`)
  - `frontend/src/routes/fixture.tsx` (`SplitsFixture style={{ height: "100%" }}`)
  - `frontend/src/sdui/registry.tsx` (`componentStyle(node)` mapping typed SDUI `node.style` to host CSS properties)
  - `frontend/src/shell/PaneTree.tsx` (`Separator style={{ width/height: "var(--clay-dimension-border-thin, 2px)" }}`)

---

## 2. Classification Taxonomy

Every declaration is classified into exactly one of seven deterministic categories:

1. **`Structural Layout`**: Layout geometry, CSS Grid tracks, Flexbox directions, alignment, positioning, dimensional clamps, overflow clipping, and aspect ratios owned by the host application shell or component structure.
2. **`Active Content-Theme Color Role`**: Colors (background, text, border, outline, shadow color, selection, diagnostics) referencing semantic theme variables (`var(--clay-*)`).
3. **`User Typography`**: Font family stacks (`--clay-font-ui`, `--clay-font-monospace`, `--clay-font-proportional`), font sizes (`--clay-text-*-size`), line heights (`--clay-text-*-line-height`), and numeric tabular settings owned by the user-configured typography profile.
4. **`Non-Color UI Design-System Recipe`**: Replaceable geometry (radius, border width, border style), materials (shadow, blur, saturate, inner highlight, opacity), motion (duration, timing curve), and tactile transforms (`--clay-ds-*`).
5. **`Accessibility Override`**: Focus-visible indicators, high-contrast outlines, screen reader affordances, disabled opacity, and forced-colors system adaptations.
6. **`Browser Compatibility Rule`**: Standard reset rules, box-sizing, scrollbar styling primitives, and cross-browser containment.
7. **`Unjustified Literal`**: Raw hardcoded pixel dimensions, literal colors, or manual filter rules requiring refactoring or recipe mapping.

---

## 3. Expensive Effects, Paint Budgets, and Fallback Ownership

| Effect / Property | Paint & Composite Budget | Risk / Impact | Fallback Ownership & Condition |
| --- | --- | --- | --- |
| `backdrop-filter: blur(Npx)` | Max 32px; restricted to fixed dialogs/popovers (`modal`, `commandCentre`, `overlay`) | Heavy GPU raster cost on scrolling regions or large surfaces | Host disables blur and falls back to solid `surface.overlay` / `surface.scrim` under `prefers-reduced-transparency` or when unsupported |
| `backdrop-filter: saturate(N)` | Range `[1.0, 2.0]`; restricted to glass overlays | GPU color matrix calculation | Disabled under `prefers-reduced-transparency` |
| `box-shadow` | Max 3 layers; blur ≤ 64px, spread ≤ 16px, offset ≤ 32px | Complex blur convolution on redraw | Bounded by Rust validation; zero shadow on low-spec/fallback |
| `transition` | Max 1000ms duration; layout-neutral properties only (`background-color`, `border-color`, `box-shadow`, `color`, `opacity`, `transform`) | Reflow/jank if applied to layout properties (`width`, `height`, `margin`, `padding`) | Host enforces instant transitions (`0ms`) under `@media (prefers-reduced-motion: reduce)` |
| `filter: brightness()` | Replaced with semantic theme colors (`surface.hover`, `surface.active`) | Unbounded raster filter on hover/press | Eliminated from component CSS during Plan 103 migration |
| `overflow: auto / scroll` | Container clipping with `min-height: 0` / `min-width: 0` | Scroll chaining or layout overflow | Structural CSS retains explicit container bounds |

---

## 4. Master Declaration Ownership Ledger

### 4.1 `frontend/src/components/button.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.button` | `display` | `inline-flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.button` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.button` | `gap` | `var(--clay-ds-button-default-root-rest-gap, var(--clay-spacing-inline, 6px))` | Non-Color UI Design-System Recipe | `button.default.root.rest.gap` | `spacing.inline` (6px) | rest | UI Design System | Retained / verified |
| `.button` | `border` | `var(--clay-ds-button-default-root-rest-border-width, 1px) solid var(--clay-ds-button-default-root-rest-border-color, transparent)` | Non-Color UI Design-System Recipe | `button.default.root.rest.borderWidth` / `borderStyle` | `dimension.border.hairline` (1px) | rest | UI Design System | Retained / verified |
| `.button` | `border-radius` | `var(--clay-ds-button-default-root-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `button.default.root.rest.borderRadius` | `0` (Neobrutal) | rest | UI Design System | Retained / verified |
| `.button` | `padding` | `var(--clay-ds-button-default-root-rest-padding, var(--clay-spacing-xs) var(--clay-spacing-sm))` | Non-Color UI Design-System Recipe | `button.default.root.rest.padding` | `spacing.xs` `spacing.sm` | rest | UI Design System | Retained / verified |
| `.button` | `font-family` | `var(--clay-font-ui)` | User Typography | `font.ui` | User Profile | rest | User Typography | Retained (user typography) |
| `.button` | `font-size` | `var(--clay-text-body-size)` | User Typography | `text.body.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.button` | `cursor` | `pointer` | Structural Layout | n/a | Host | rest | React Host | Retained (interaction) |
| `.button` | `box-shadow` | `var(--clay-ds-button-default-root-rest-shadow, none)` | Non-Color UI Design-System Recipe | `button.default.root.rest.shadow` | `none` | rest | UI Design System | Retained / verified |
| `.button` | `transform` | `var(--clay-ds-button-default-root-rest-transform-preset, none)` | Non-Color UI Design-System Recipe | `button.default.root.rest.transformPreset` | `none` | rest | UI Design System | Retained / verified |
| `.button` | `transition` | `background-color/border-color/box-shadow/transform var(--clay-ds-button-default-root-rest-transition-duration, 100ms) linear` | Non-Color UI Design-System Recipe | `button.default.root.rest.transitionDuration` / `transitionTiming` | `motion.fast` (100ms) | rest | UI Design System | Retained / verified |
| `.default` (rest) | `background` | `var(--clay-ds-button-default-root-rest-background-color, var(--clay-surface-control))` | Active Content-Theme Color Role | `surface.control` | Active Theme | rest | Content Theme | Retained / verified |
| `.default` (rest) | `color` | `var(--clay-ds-button-default-root-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Retained / verified |
| `.default` (rest) | `border-color` | `var(--clay-ds-button-default-root-rest-border-color, transparent)` | Active Content-Theme Color Role | `transparent` | Active Theme | rest | Content Theme | Retained / verified |
| `.muted` (rest) | `background` | `var(--clay-ds-button-muted-root-rest-background-color, transparent)` | Active Content-Theme Color Role | `transparent` | Active Theme | rest | Content Theme | Retained / verified |
| `.muted` (rest) | `color` | `var(--clay-ds-button-muted-root-rest-text-color, var(--clay-text-muted))` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Retained / verified |
| `.muted` (rest) | `border-color` | `var(--clay-ds-button-muted-root-rest-border-color, transparent)` | Active Content-Theme Color Role | `transparent` | Active Theme | rest | Content Theme | Retained / verified |
| `.primary` (rest) | `background` | `var(--clay-ds-button-primary-root-rest-background-color, var(--clay-accent-primary))` | Active Content-Theme Color Role | `accent.primary` | Active Theme | rest | Content Theme | Retained / verified |
| `.primary` (rest) | `color` | `var(--clay-ds-button-primary-root-rest-text-color, var(--clay-surface-main))` | Active Content-Theme Color Role | `surface.main` | Active Theme | rest | Content Theme | Retained / verified |
| `.primary` (rest) | `border-color` | `var(--clay-ds-button-primary-root-rest-border-color, transparent)` | Active Content-Theme Color Role | `transparent` | Active Theme | rest | Content Theme | Retained / verified |
| `.danger` (rest) | `background` | `var(--clay-ds-button-danger-root-rest-background-color, var(--clay-surface-control))` | Active Content-Theme Color Role | `surface.control` | Active Theme | rest | Content Theme | Retained / verified |
| `.danger` (rest) | `color` | `var(--clay-ds-button-danger-root-rest-text-color, var(--clay-diagnostic-error))` | Active Content-Theme Color Role | `diagnostic.error` | Active Theme | rest | Content Theme | Retained / verified |
| `.danger` (rest) | `border-color` | `var(--clay-ds-button-danger-root-rest-border-color, transparent)` | Active Content-Theme Color Role | `transparent` | Active Theme | rest | Content Theme | Retained / verified |
| `.default[data-hovered]` | `background` | `var(--clay-ds-button-default-root-hover-background-color, var(--clay-surface-hover))` | Active Content-Theme Color Role | `surface.hover` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.default[data-hovered]` | `color` | `var(--clay-ds-button-default-root-hover-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.muted[data-hovered]` | `background` | `var(--clay-ds-button-muted-root-hover-background-color, transparent)` | Active Content-Theme Color Role | `transparent` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.muted[data-hovered]` | `color` | `var(--clay-ds-button-muted-root-hover-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.primary[data-hovered]` | `background` | `var(--clay-ds-button-primary-root-hover-background-color, var(--clay-accent-primary))` | Active Content-Theme Color Role | `accent.primary` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.primary[data-hovered]` | `color` | `var(--clay-ds-button-primary-root-hover-text-color, var(--clay-surface-main))` | Active Content-Theme Color Role | `surface.main` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.primary[data-hovered]` | `filter` | `brightness(1.1)` | Unjustified Literal | `button.primary.root.hover.backgroundColor` role | Theme Role | `data-hovered` | Host Component | Replace with recipe color role in Task 3 |
| `.danger[data-hovered]` | `background` | `var(--clay-ds-button-danger-root-hover-background-color, var(--clay-surface-hover))` | Active Content-Theme Color Role | `surface.hover` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.danger[data-hovered]` | `color` | `var(--clay-ds-button-danger-root-hover-text-color, var(--clay-diagnostic-error))` | Active Content-Theme Color Role | `diagnostic.error` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.default[data-pressed]` | `background` | `var(--clay-ds-button-default-root-active-background-color, var(--clay-surface-active))` | Active Content-Theme Color Role | `surface.active` | Active Theme | `data-pressed` | Content Theme | Retained / verified |
| `.primary[data-pressed]` | `background` | `var(--clay-ds-button-primary-root-active-background-color, var(--clay-accent-primary))` | Active Content-Theme Color Role | `accent.primary` | Active Theme | `data-pressed` | Content Theme | Retained / verified |
| `.primary[data-pressed]` | `filter` | `brightness(0.95)` | Unjustified Literal | `button.primary.root.active.backgroundColor` role | Theme Role | `data-pressed` | Host Component | Replace with recipe color role in Task 3 |
| `.danger[data-pressed]` | `background` | `var(--clay-ds-button-danger-root-active-background-color, var(--clay-surface-active))` | Active Content-Theme Color Role | `surface.active` | Active Theme | `data-pressed` | Content Theme | Retained / verified |
| `.button[data-focused]` | `outline` | `var(--clay-ds-button-default-root-focus-outline-width, 2px) solid var(--clay-ds-button-default-root-focus-outline-color, var(--clay-focus-ring))` | Accessibility Override | `focus.ring` / `outlineWidth` | `focus.ring` (2px solid) | `data-focused` | React Aria / DS | Retained / verified |
| `.button[data-focused]` | `outline-offset` | `var(--clay-ds-button-default-root-focus-outline-offset, 1px)` | Non-Color UI Design-System Recipe | `button.default.root.focus.outlineOffset` | `1px` | `data-focused` | UI Design System | Retained / verified |
| `.button[data-disabled]` | `background` | `var(--clay-ds-button-default-root-disabled-background-color, var(--clay-surface-disabled))` | Active Content-Theme Color Role | `surface.disabled` | Active Theme | `data-disabled` | Content Theme | Retained / verified |
| `.button[data-disabled]` | `color` | `var(--clay-ds-button-default-root-disabled-text-color, var(--clay-text-disabled))` | Active Content-Theme Color Role | `text.disabled` | Active Theme | `data-disabled` | Content Theme | Retained / verified |
| `.button[data-disabled]` | `opacity` | `var(--clay-ds-button-default-root-disabled-opacity, var(--clay-opacity-disabled))` | Non-Color UI Design-System Recipe | `button.default.root.disabled.opacity` | `opacity.disabled` (0.55) | `data-disabled` | UI Design System | Retained / verified |
| `.button[data-disabled]` | `border-color` | `var(--clay-ds-button-default-root-disabled-border-color, transparent)` | Active Content-Theme Color Role | `transparent` | Active Theme | `data-disabled` | Content Theme | Retained / verified |
| `.button[data-disabled]` | `cursor` | `default` | Structural Layout | n/a | Host | `data-disabled` | React Host | Retained (interaction) |

---

### 4.2 `frontend/src/components/chrome.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.badge` | `display` | `inline-flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.badge` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.badge` | `background` | `var(--clay-surface-badge)` | Active Content-Theme Color Role | `surface.badge` | Active Theme | rest | Content Theme | Map to `badge.default.root.rest.backgroundColor` |
| `.badge` | `color` | `var(--clay-text-badge)` | Active Content-Theme Color Role | `text.badge` | Active Theme | rest | Content Theme | Map to `badge.default.root.rest.textColor` |
| `.badge` | `border-radius` | `var(--clay-radius-xs, 2px)` | Non-Color UI Design-System Recipe | `badge.default.root.rest.borderRadius` | `radius.xs` (2px) | rest | UI Design System | Map to `badge.default.root.rest.borderRadius` |
| `.badge` | `padding` | `0 var(--clay-spacing-badge, 4px)` | Non-Color UI Design-System Recipe | `badge.default.root.rest.padding` | `spacing.badge` (4px) | rest | UI Design System | Map to `badge.default.root.rest.padding` |
| `.badge` | `font-size` | `var(--clay-text-caption-size)` | User Typography | `text.caption.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.kbd` | `display` | `inline-flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.kbd` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.kbd` | `background` | `var(--clay-surface-kbd)` | Active Content-Theme Color Role | `surface.kbd` | Active Theme | rest | Content Theme | Map to `kbd.default.root.rest.backgroundColor` |
| `.kbd` | `color` | `var(--clay-text-kbd)` | Active Content-Theme Color Role | `text.kbd` | Active Theme | rest | Content Theme | Map to `kbd.default.root.rest.textColor` |
| `.kbd` | `border` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-kbd)` | Non-Color UI Design-System Recipe | `kbd.default.root.rest.borderWidth` / `border.kbd` | `dimension.border.hairline` (1px) | rest | UI Design System / Theme | Map to `kbd.default.root.rest.border*` |
| `.kbd` | `border-radius` | `var(--clay-radius-xs, 2px)` | Non-Color UI Design-System Recipe | `kbd.default.root.rest.borderRadius` | `radius.xs` (2px) | rest | UI Design System | Map to `kbd.default.root.rest.borderRadius` |
| `.kbd` | `min-height` | `var(--clay-dimension-kbd-height, 20px)` | Non-Color UI Design-System Recipe | `kbd.default.root.rest.minHeight` | `dimension.kbd.height` (20px) | rest | UI Design System | Retained / verified |
| `.kbd` | `padding` | `0 var(--clay-spacing-xxs, 4px)` | Non-Color UI Design-System Recipe | `kbd.default.root.rest.padding` | `spacing.xxs` (4px) | rest | UI Design System | Map to `kbd.default.root.rest.padding` |
| `.kbd` | `font-family` | `var(--clay-font-monospace)` | User Typography | `font.monospace` | User Profile | rest | User Typography | Retained (user typography) |
| `.kbd` | `font-size` | `var(--clay-text-caption-size)` | User Typography | `text.caption.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.divider` | `border` | `none` | Structural Layout | n/a | Host | rest | React Host | Retained (reset) |
| `.divider` | `border-top` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-hairline)` | Non-Color UI Design-System Recipe | `divider.default.root.rest.borderWidth` / `border.hairline` | `border.hairline` (1px) | rest | UI Design System / Theme | Map to `divider.default.root.rest.border*` |
| `.divider` | `margin` | `0` | Structural Layout | n/a | Host | rest | React Host | Retained (reset) |
| `.divider` | `width` | `100%` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |

---

### 4.3 `frontend/src/components/controls.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.selectTrigger` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.selectTrigger` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.selectTrigger` | `justify-content`| `space-between` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.selectTrigger` | `gap` | `var(--clay-ds-dropdown-default-trigger-rest-gap, var(--clay-spacing-xs, 8px))` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.rest.gap` | `spacing.xs` (8px) | rest | UI Design System | Retained / verified |
| `.selectTrigger` | `width` | `100%` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.selectTrigger` | `background` | `var(--clay-ds-dropdown-default-trigger-rest-background-color, var(--clay-surface-control))` | Active Content-Theme Color Role | `surface.control` | Active Theme | rest | Content Theme | Retained / verified |
| `.selectTrigger` | `color` | `var(--clay-ds-dropdown-default-trigger-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Retained / verified |
| `.selectTrigger` | `border` | `var(--clay-ds-dropdown-default-trigger-rest-border-width, 1px) solid var(--clay-ds-dropdown-default-trigger-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Retained / verified |
| `.selectTrigger` | `border-radius` | `var(--clay-ds-dropdown-default-trigger-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.rest.borderRadius` | `0` | rest | UI Design System | Retained / verified |
| `.selectTrigger` | `padding` | `var(--clay-ds-dropdown-default-trigger-rest-padding, var(--clay-spacing-xs) var(--clay-spacing-sm))` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.rest.padding` | `spacing.xs` `spacing.sm` | rest | UI Design System | Retained / verified |
| `.selectTrigger` | `cursor` | `pointer` | Structural Layout | n/a | Host | rest | React Host | Retained (interaction) |
| `.selectTrigger` | `font-family` | `inherit` | User Typography | `font.ui` | User Profile | rest | User Typography | Retained (user typography) |
| `.selectTrigger` | `font-size` | `var(--clay-text-body-size)` | User Typography | `text.body.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.selectTrigger` | `box-shadow` | `var(--clay-ds-dropdown-default-trigger-rest-shadow, none)` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.rest.shadow` | `none` | rest | UI Design System | Retained / verified |
| `.selectTrigger` | `transition` | `background-color/border-color var(--clay-ds-dropdown-default-trigger-rest-transition-duration, 100ms) linear` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.rest.transitionDuration` | `motion.fast` (100ms) | rest | UI Design System | Retained / verified |
| `.selectTrigger[data-hovered]` | `background` | `var(--clay-ds-dropdown-default-trigger-hover-background-color, var(--clay-surface-hover))` | Active Content-Theme Color Role | `surface.hover` | Active Theme | `data-hovered` | Content Theme | Retained / verified |
| `.selectTrigger[data-pressed]` | `background` | `var(--clay-ds-dropdown-default-trigger-active-background-color, var(--clay-surface-active))` | Active Content-Theme Color Role | `surface.active` | Active Theme | `data-pressed` | Content Theme | Retained / verified |
| `.selectTrigger[data-focused]` | `outline` | `var(--clay-ds-dropdown-default-trigger-focus-outline-width, 2px) solid var(--clay-ds-dropdown-default-trigger-focus-outline-color, var(--clay-focus-ring))` | Accessibility Override | `focus.ring` | `focus.ring` (2px solid) | `data-focused` | React Aria / DS | Retained / verified |
| `.selectTrigger[data-focused]` | `outline-offset` | `var(--clay-ds-dropdown-default-trigger-focus-outline-offset, 1px)` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.focus.outlineOffset` | `1px` | `data-focused` | UI Design System | Retained / verified |
| `.selectTrigger[data-disabled]` | `background` | `var(--clay-ds-dropdown-default-trigger-disabled-background-color, var(--clay-surface-disabled))` | Active Content-Theme Color Role | `surface.disabled` | Active Theme | `data-disabled` | Content Theme | Retained / verified |
| `.selectTrigger[data-disabled]` | `color` | `var(--clay-ds-dropdown-default-trigger-disabled-text-color, var(--clay-text-disabled))` | Active Content-Theme Color Role | `text.disabled` | Active Theme | `data-disabled` | Content Theme | Retained / verified |
| `.selectTrigger[data-disabled]` | `opacity` | `var(--clay-ds-dropdown-default-trigger-disabled-opacity, var(--clay-opacity-disabled))` | Non-Color UI Design-System Recipe | `dropdown.default.trigger.disabled.opacity` | `opacity.disabled` (0.55) | `data-disabled` | UI Design System | Retained / verified |
| `.popover` | `background` | `var(--clay-ds-dropdown-default-surface-rest-background-color, var(--clay-surface-overlay))` | Active Content-Theme Color Role | `surface.overlay` | Active Theme | rest | Content Theme | Align variable name to `dropdown.default.popover.*` in Task 3 |
| `.popover` | `border` | `var(--clay-ds-dropdown-default-surface-rest-border-width, 1px) solid var(--clay-ds-dropdown-default-surface-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `dropdown.default.popover.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Align variable name to `dropdown.default.popover.*` in Task 3 |
| `.popover` | `border-radius` | `var(--clay-ds-dropdown-default-surface-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `dropdown.default.popover.rest.borderRadius` | `0` | rest | UI Design System | Align variable name in Task 3 |
| `.popover` | `box-shadow` | `var(--clay-ds-dropdown-default-surface-rest-shadow, none)` | Non-Color UI Design-System Recipe | `dropdown.default.popover.rest.shadow` | `none` | rest | UI Design System | Align variable name in Task 3 |
| `.popover` | `min-width` | `160px` | Structural Layout | n/a | Host | rest | React Host | Retained (structural constraint) |
| `.popover` | `max-height` | `320px` | Structural Layout | n/a | Host | rest | React Host | Retained (structural constraint) |
| `.popover` | `overflow-y` | `auto` | Structural Layout | n/a | Host | rest | React Host | Retained (scrolling containment) |
| `.listBox` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.listBox` | `flex-direction` | `column` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.listBox` | `margin` / `padding` | `0` | Structural Layout | n/a | Host | rest | React Host | Retained (reset) |
| `.listBox` | `list-style` | `none` | Structural Layout | n/a | Host | rest | React Host | Retained (reset) |
| `.listRow` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.listRow` | `flex-direction` | `column` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.listRow` | `gap` | `var(--clay-ds-list-default-row-rest-gap, 2px)` | Non-Color UI Design-System Recipe | `list.default.row.rest.gap` | `2px` | rest | UI Design System | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow` | `background` | `var(--clay-ds-list-default-row-rest-background-color, var(--clay-surface-list))` | Active Content-Theme Color Role | `surface.list` | Active Theme | rest | Content Theme | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow` | `color` | `var(--clay-ds-list-default-row-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow` | `border-bottom` | `var(--clay-ds-list-default-row-rest-border-width, 1px) solid var(--clay-ds-list-default-row-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `list.default.row.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow` | `padding` | `var(--clay-ds-list-default-row-rest-padding, var(--clay-spacing-xs) var(--clay-spacing-sm))` | Non-Color UI Design-System Recipe | `list.default.row.rest.padding` | `spacing.xs` `spacing.sm` | rest | UI Design System | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow` | `cursor` | `pointer` | Structural Layout | n/a | Host | rest | React Host | Retained (interaction) |
| `.listRow[data-hovered]` | `background` | `var(--clay-ds-list-default-row-hover-background-color, var(--clay-surface-hover))` | Active Content-Theme Color Role | `surface.hover` | Active Theme | `data-hovered` | Content Theme | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow[data-pressed]` | `background` | `var(--clay-ds-list-default-row-active-background-color, var(--clay-surface-active))` | Active Content-Theme Color Role | `surface.active` | Active Theme | `data-pressed` | Content Theme | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow[data-focused]` | `outline` | `var(--clay-ds-list-default-row-focus-outline-width, 2px) solid var(--clay-ds-list-default-row-focus-outline-color, var(--clay-focus-ring))` | Accessibility Override | `focus.ring` | `focus.ring` (2px solid) | `data-focused` | React Aria / DS | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow[data-focused]` | `outline-offset` | `var(--clay-ds-list-default-row-focus-outline-offset, -2px)` | Non-Color UI Design-System Recipe | `list.default.row.focus.outlineOffset` | `-2px` | `data-focused` | UI Design System | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow[data-selected]` | `background` | `var(--clay-ds-list-default-row-selected-background-color, var(--clay-surface-selected))` | Active Content-Theme Color Role | `surface.selected` | Active Theme | `data-selected` | Content Theme | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow[data-disabled]` | `color` | `var(--clay-ds-list-default-row-disabled-text-color, var(--clay-text-disabled))` | Active Content-Theme Color Role | `text.disabled` | Active Theme | `data-disabled` | Content Theme | Aligned to canonical `list.default.row.*` in Task 3 |
| `.listRow[data-disabled]` | `opacity` | `var(--clay-ds-list-default-row-disabled-opacity, var(--clay-opacity-disabled))` | Non-Color UI Design-System Recipe | `list.default.row.disabled.opacity` | `opacity.disabled` (0.55) | `data-disabled` | UI Design System | Aligned to canonical `list.default.row.*` in Task 3 |
| `.rowDetail` | `color` | `var(--clay-text-muted)` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Retained / verified |
| `.rowDetail` | `font-size` | `var(--clay-text-detail-size)` | User Typography | `text.detail.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.collapseHeader` | `width` | `100%` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.collapseHeader` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.collapseHeader` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.collapseHeader` | `justify-content` | `space-between` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.collapseHeader` | `background` | `transparent` | Active Content-Theme Color Role | `transparent` | Active Theme | rest | Content Theme | Map to `collapse.default.header.rest.backgroundColor` in Task 3 |
| `.collapseHeader` | `color` | `var(--clay-text-primary)` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Map to `collapse.default.header.rest.textColor` in Task 3 |
| `.collapseHeader` | `border-bottom` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-hairline)` | Non-Color UI Design-System Recipe | `collapse.default.header.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Map to `collapse.default.header.rest.border*` in Task 3 |
| `.collapseHeader` | `border-radius` | `0` | Non-Color UI Design-System Recipe | `collapse.default.header.rest.borderRadius` | `0` | rest | UI Design System | Map to `collapse.default.header.rest.borderRadius` in Task 3 |
| `.collapseHeader` | `padding` | `var(--clay-spacing-xs, 8px) var(--clay-spacing-sm, 12px)` | Non-Color UI Design-System Recipe | `collapse.default.header.rest.padding` | `spacing.xs` `spacing.sm` | rest | UI Design System | Map to `collapse.default.header.rest.padding` in Task 3 |
| `.collapseHeader` | `font-size` | `var(--clay-text-section-size)` | User Typography | `text.section.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.collapseHeader[data-focused]` | `outline` | `2px solid var(--clay-focus-ring)` | Accessibility Override | `focus.ring` | `focus.ring` (2px solid) | `data-focused` | React Aria | Retained / verified |
| `.collapseHeader[data-focused]` | `outline-offset` | `-2px` | Non-Color UI Design-System Recipe | `collapse.default.header.focus.outlineOffset` | `-2px` | `data-focused` | UI Design System | Retained / verified |
| `.collapseChevron` | `transition` | `transform var(--clay-motion-fast, 100ms) linear` | Non-Color UI Design-System Recipe | `collapse.default.chevron.rest.transitionDuration` | `motion.fast` (100ms) | rest | UI Design System | Retained / verified |
| `.collapseChevronExpanded` | `transform` | `rotate(90deg)` | Non-Color UI Design-System Recipe | `collapse.default.chevron.expanded.transform` | `rotate(90deg)` | `expanded` | UI Design System | Retained / verified |
| `.collapseBody` | `border-bottom` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-hairline)` | Non-Color UI Design-System Recipe | `collapse.default.body.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Map to `collapse.default.body.rest.border*` in Task 3 |
| `.collapseBody` | `padding` | `var(--clay-spacing-sm, 12px)` | Non-Color UI Design-System Recipe | `collapse.default.body.rest.padding` | `spacing.sm` (12px) | rest | UI Design System | Map to `collapse.default.body.rest.padding` in Task 3 |

---

### 4.4 `frontend/src/components/modal.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.scrim` | `position` | `fixed` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.scrim` | `inset` | `0` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.scrim` | `background` | `var(--clay-ds-modal-default-scrim-rest-background-color, color-mix(in srgb, var(--clay-surface-scrim) calc(var(--clay-opacity-scrim) * 100%), transparent))` | Active Content-Theme Color Role | `surface.scrim` | Active Theme | rest | Content Theme | Retained / verified |
| `.scrim` | `backdrop-filter` | `blur(var(--clay-ds-modal-default-scrim-rest-backdrop-blur, 0px))` | Non-Color UI Design-System Recipe | `modal.default.scrim.rest.backdropBlur` | `0px` | rest | UI Design System | Retained / verified (budget ≤ 32px) |
| `.scrim` | `z-index` | `var(--clay-z-modal, 40)` | Structural Layout | `z.modal` | Host (40) | rest | React Host | Retained (structural layering) |
| `.scrim` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.scrim` | `place-items` | `center` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.dialog` | `background` | `var(--clay-ds-modal-default-dialog-rest-background-color, var(--clay-surface-overlay))` | Active Content-Theme Color Role | `surface.overlay` | Active Theme | rest | Content Theme | Aligned to canonical `modal.default.dialog.*` in Task 3 |
| `.dialog` | `color` | `var(--clay-ds-modal-default-dialog-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Aligned to canonical `modal.default.dialog.*` in Task 3 |
| `.dialog` | `border` | `var(--clay-ds-modal-default-dialog-rest-border-width, 1px) solid var(--clay-ds-modal-default-dialog-rest-border-color, var(--clay-border-strong))` | Non-Color UI Design-System Recipe | `modal.default.dialog.rest.border*` | `border.strong` (1px) | rest | UI Design System / Theme | Aligned to canonical `modal.default.dialog.*` in Task 3 |
| `.dialog` | `border-radius` | `var(--clay-ds-modal-default-dialog-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `modal.default.dialog.rest.borderRadius` | `0` | rest | UI Design System | Aligned to canonical `modal.default.dialog.*` in Task 3 |
| `.dialog` | `box-shadow` | `var(--clay-ds-modal-default-dialog-rest-shadow, none)` | Non-Color UI Design-System Recipe | `modal.default.dialog.rest.shadow` | `none` | rest | UI Design System | Aligned to canonical `modal.default.dialog.*` in Task 3 |
| `.dialog` | `backdrop-filter` | `blur(var(--clay-ds-modal-default-dialog-rest-backdrop-blur, 0px)) saturate(var(--clay-ds-modal-default-dialog-rest-backdrop-saturate, 1))` | Non-Color UI Design-System Recipe | `modal.default.dialog.rest.backdropBlur` / `backdropSaturate` | `0px` / `1.0` | rest | UI Design System | Aligned to canonical `modal.default.dialog.*` in Task 3 |
| `.dialog` | `min-width` | `320px` | Structural Layout | n/a | Host | rest | React Host | Retained (structural constraint) |
| `.dialog` | `max-width` | `min(var(--clay-dimension-overlay-centered-width, 640px), 90vw)` | Structural Layout | `dimension.overlay.centered.width` | Host (640px) | rest | React Host | Retained (structural constraint) |
| `.dialog` | `max-height` | `80vh` | Structural Layout | n/a | Host | rest | React Host | Retained (structural constraint) |
| `.dialog` | `overflow-y` | `auto` | Structural Layout | n/a | Host | rest | React Host | Retained (scrolling containment) |
| `.dialog` | `padding` | `var(--clay-ds-modal-default-dialog-rest-padding, var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `modal.default.dialog.rest.padding` | `spacing.md` (16px) | rest | UI Design System | Aligned to canonical `modal.default.dialog.*` in Task 3 |
| `.title` | `margin` | `0 0 var(--clay-spacing-sm, 12px)` | Structural Layout | `spacing.sm` | `spacing.sm` (12px) | rest | React Host | Retained (layout spacing) |
| `.title` | `font-size` | `var(--clay-text-title-size)` | User Typography | `text.title.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.title` | `line-height` | `var(--clay-text-title-line-height)` | User Typography | `text.title.line-height` | User Profile | rest | User Typography | Retained (user typography) |
| `.close` | `display` / `place-items` | `grid` / `center` | Structural Layout | n/a | Host | rest | React Host | Retained (layout reset) |
| `.close` | `width` / `height` | `var(--clay-dimension-target-min, 32px)` | Structural Layout | `dimension.target.min` | Host (32px) | rest | React Host | Retained (touch target) |
| `.close` | `background` / `border` | `transparent` / `none` | Structural Layout | n/a | Host | rest | React Host | Retained (button reset) |
| `.close` | `border-radius` | `var(--clay-ds-modal-default-close-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `modal.default.close.rest.borderRadius` | `0` | rest | UI Design System | Retained / verified |
| `.close` | `color` | `var(--clay-ds-modal-default-close-rest-text-color, var(--clay-text-muted))` | Active Content-Theme Color Role | `modal.default.close.rest.textColor` | `text.muted` | rest | Content Theme | Fixed undefined `--clay-text-secondary` to `--clay-text-muted` in Task 4 |

---

### 4.5 `frontend/src/components/text-field.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.field` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.field` | `flex-direction` | `column` | Structural Layout | n/a | Host | rest | React Host | Retained (structural) |
| `.field` | `gap` | `var(--clay-ds-text-input-default-field-rest-gap, var(--clay-spacing-xxs, 4px))` | Non-Color UI Design-System Recipe | `textInput.default.field.rest.gap` | `spacing.xxs` (4px) | rest | UI Design System | Retained / verified |
| `.label` | `color` | `var(--clay-text-muted)` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Map to `textInput.default.label.rest.textColor` in Task 3 |
| `.label` | `font-size` | `var(--clay-text-detail-size)` | User Typography | `text.detail.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.input` | `background` | `var(--clay-ds-text-input-default-input-rest-background-color, var(--clay-surface-control))` | Active Content-Theme Color Role | `surface.control` | Active Theme | rest | Content Theme | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input` | `color` | `var(--clay-ds-text-input-default-input-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input` | `border` | `var(--clay-ds-text-input-default-input-rest-border-width, 2px) solid var(--clay-ds-text-input-default-input-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `textInput.default.input.rest.border*` | `border.subtle` (2px) | rest | UI Design System / Theme | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input` | `border-radius` | `var(--clay-ds-text-input-default-input-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `textInput.default.input.rest.borderRadius` | `0` | rest | UI Design System | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input` | `padding` | `var(--clay-ds-text-input-default-input-rest-padding, var(--clay-spacing-xs, 8px))` | Non-Color UI Design-System Recipe | `textInput.default.input.rest.padding` | `spacing.xs` (8px) | rest | UI Design System | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input` | `font-family` | `inherit` | User Typography | `font.ui` | User Profile | rest | User Typography | Retained (user typography) |
| `.input` | `font-size` | `var(--clay-text-body-size)` | User Typography | `text.body.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.input` | `box-shadow` | `var(--clay-ds-text-input-default-input-rest-shadow, none)` | Non-Color UI Design-System Recipe | `textInput.default.input.rest.shadow` | `none` | rest | UI Design System | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input` | `transition` | `border-color/background-color var(--clay-ds-text-input-default-input-rest-transition-duration, 100ms) linear` | Non-Color UI Design-System Recipe | `textInput.default.input.rest.transitionDuration` | `motion.fast` (100ms) | rest | UI Design System | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input::placeholder` | `color` | `var(--clay-text-muted)` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Retained / verified |
| `.input[data-focused], .input:focus-visible` | `border-color` | `var(--clay-ds-text-input-default-input-focus-border-color, var(--clay-border-focus))` | Active Content-Theme Color Role | `border.focus` | Active Theme | `data-focused` | Content Theme | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input[data-focused], .input:focus-visible` | `outline` | `var(--clay-ds-text-input-default-input-focus-outline-width, 2px) var(--clay-ds-text-input-default-input-focus-outline-style, solid) var(--clay-ds-text-input-default-input-focus-outline-color, var(--clay-focus-ring))` | Accessibility / Recipe Focus Ring | `textInput.default.input.focus.outline*` | `focus.ring` (2px solid) | `data-focused` | UI Design System / React Aria | Replaced `outline: none` with DS recipe focus outline in Task 4 |
| `.input[data-focused], .input:focus-visible` | `outline-offset` | `var(--clay-ds-text-input-default-input-focus-outline-offset, 1px)` | Non-Color UI Design-System Recipe | `textInput.default.input.focus.outlineOffset` | `1px` | `data-focused` | UI Design System | Migrated in Task 4 |
| `.error` | `border-color` | `var(--clay-ds-text-input-default-input-invalid-border-color, var(--clay-diagnostic-error))` | Active Content-Theme Color Role | `diagnostic.error` | Active Theme | `data-invalid` | Content Theme | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.warning` | `border-color` | `var(--clay-diagnostic-warning)` | Active Content-Theme Color Role | `diagnostic.warning` | Active Theme | state | Content Theme | Retained / verified |
| `.success` | `border-color` | `var(--clay-diagnostic-success)` | Active Content-Theme Color Role | `diagnostic.success` | Active Theme | state | Content Theme | Retained / verified |
| `.input[data-disabled]` | `background` | `var(--clay-ds-text-input-default-input-disabled-background-color, var(--clay-surface-disabled))` | Active Content-Theme Color Role | `surface.disabled` | Active Theme | `data-disabled` | Content Theme | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input[data-disabled]` | `color` | `var(--clay-ds-text-input-default-input-disabled-text-color, var(--clay-text-disabled))` | Active Content-Theme Color Role | `text.disabled` | Active Theme | `data-disabled` | Content Theme | Aligned to canonical `textInput.default.input.*` in Task 3 |
| `.input[data-disabled]` | `opacity` | `var(--clay-ds-text-input-default-input-disabled-opacity, var(--clay-opacity-disabled))` | Non-Color UI Design-System Recipe | `textInput.default.input.disabled.opacity` | `opacity.disabled` (0.55) | `data-disabled` | UI Design System | Aligned to canonical `textInput.default.input.*` in Task 3 |

---

### 4.6 `frontend/src/components/text.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.text` | `margin` | `0` | Structural Layout | n/a | Host | rest | React Host | Retained (reset) |
| `.text` | `color` | `var(--clay-text-primary)` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Retained / verified |
| `.text` | `font-family` | `var(--clay-font-ui)` | User Typography | `font.ui` | User Profile | rest | User Typography | Retained (user typography) |
| `.muted` | `color` | `var(--clay-text-muted)` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Retained / verified |
| `.disabled` | `color` | `var(--clay-text-disabled)` | Active Content-Theme Color Role | `text.disabled` | Active Theme | `disabled` | Content Theme | Retained / verified |
| `.disabled` | `opacity` | `var(--clay-opacity-disabled)` | Non-Color UI Design-System Recipe | `label.default.root.disabled.opacity` | `opacity.disabled` (0.55) | `disabled` | UI Design System | Retained / verified |
| `.display` | `font-size` / `line-height` | `var(--clay-text-display-size)` / `line-height` | User Typography | `text.display.*` | User Profile | rest | User Typography | Retained (user typography) |
| `.title` | `font-size` / `line-height` | `var(--clay-text-title-size)` / `line-height` | User Typography | `text.title.*` | User Profile | rest | User Typography | Retained (user typography) |
| `.section` | `font-size` / `line-height` | `var(--clay-text-section-size)` / `line-height` | User Typography | `text.section.*` | User Profile | rest | User Typography | Retained (user typography) |
| `.body` | `font-size` / `line-height` | `var(--clay-text-body-size)` / `line-height` | User Typography | `text.body.*` | User Profile | rest | User Typography | Retained (user typography) |
| `.status` | `font-size` / `line-height` | `var(--clay-text-status-size)` / `line-height` | User Typography | `text.status.*` | User Profile | rest | User Typography | Retained (user typography) |
| `.detail` | `font-size` / `line-height` | `var(--clay-text-detail-size)` / `line-height` | User Typography | `text.detail.*` | User Profile | rest | User Typography | Retained (user typography) |
| `.caption` | `font-size` / `line-height` | `var(--clay-text-caption-size)` / `line-height` | User Typography | `text.caption.*` | User Profile | rest | User Typography | Retained (user typography) |
| `.monospace` | `font-family` | `var(--clay-font-monospace)` | User Typography | `font.monospace` | User Profile | rest | User Typography | Retained (user typography) |
| `.proportional` | `font-family` | `var(--clay-font-proportional)` | User Typography | `font.proportional` | User Profile | rest | User Typography | Retained (user typography) |

---

### 4.7 `frontend/src/app/layout/shell.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.shell` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Shell | Retained (shell grid layout) |
| `.shell` | `grid-template-rows` | `auto 1fr auto` | Structural Layout | n/a | Host | rest | React Shell | Retained (shell grid layout) |
| `.shell` | `height` | `100%` | Structural Layout | n/a | Host | rest | React Shell | Retained (viewport constraint) |
| `.shell` | `background` | `var(--clay-surface-main)` | Active Content-Theme Color Role | `surface.main` | Active Theme | rest | Content Theme | Retained / verified |
| `.header` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Shell | Retained (header layout) |
| `.header` | `align-items` | `stretch` | Structural Layout | n/a | Host | rest | React Shell | Retained (header layout) |
| `.header` | `gap` | `1px` | Structural Layout | n/a | Host | rest | React Shell | Retained (compartmentalization) |
| `.header` | `background` | `var(--clay-border-hairline)` | Active Content-Theme Color Role | `border.hairline` | Active Theme | rest | Content Theme | Retained / verified |
| `.header` | `border-bottom` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-hairline)` | Non-Color UI Design-System Recipe | `tabBar.default.bar.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Map to `tabBar.default.bar.*` in Task 4 |
| `.brand` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Shell | Retained (header brand layout) |
| `.brand` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Shell | Retained (header brand layout) |
| `.brand` | `padding` | `0 var(--clay-spacing-sm, 12px)` | Non-Color UI Design-System Recipe | `tabBar.default.bar.rest.padding` | `spacing.sm` (12px) | rest | UI Design System | Retained / verified |
| `.brand` | `background` | `var(--clay-surface-panel)` | Active Content-Theme Color Role | `surface.panel` | Active Theme | rest | Content Theme | Retained / verified |
| `.brand` | `color` | `var(--clay-text-muted)` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Retained / verified |
| `.brand` | `font-size` | `var(--clay-text-section-size)` | User Typography | `text.section.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.brand` | `letter-spacing` | `0.08em` | User Typography | n/a | User Profile | rest | User Typography | Retained (typography styling) |
| `.brand` | `user-select` | `none` | Structural Layout | n/a | Host | rest | React Shell | Retained (interaction) |
| `.workingArea` | `height` / `min-height` | `100%` / `0` | Structural Layout | n/a | Host | rest | React Shell | Retained (viewport constraint) |
| `.workingArea` | `background` | `var(--clay-surface-main)` | Active Content-Theme Color Role | `surface.main` | Active Theme | rest | Content Theme | Retained / verified |
| `.footer` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Shell | Retained (footer layout) |
| `.footer` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Shell | Retained (footer layout) |
| `.footer` | `gap` | `var(--clay-ds-status-bar-default-root-rest-gap, var(--clay-ds-shell-default-footer-rest-gap, var(--clay-spacing-sm, 12px)))` | Non-Color UI Design-System Recipe | `statusBar.default.root.rest.gap` | `spacing.sm` (12px) | rest | UI Design System | Migrated to `statusBar.default.root.rest.gap` with shell fallback in Task 6 |
| `.footer` | `border-top` | `var(--clay-ds-status-bar-default-root-rest-border-width, var(--clay-ds-shell-default-footer-rest-border-width, var(--clay-dimension-border-hairline, 1px))) var(--clay-ds-status-bar-default-root-rest-border-style, var(--clay-ds-shell-default-footer-rest-border-style, solid)) var(--clay-ds-status-bar-default-root-rest-border-color, var(--clay-ds-shell-default-footer-rest-border-color, var(--clay-border-hairline)))` | Non-Color UI Design-System Recipe | `statusBar.default.root.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Migrated to `statusBar.default.root.rest.border*` with shell fallback in Task 6 |
| `.footer` | `background` | `var(--clay-ds-status-bar-default-root-rest-background-color, var(--clay-ds-shell-default-footer-rest-background-color, var(--clay-surface-panel)))` | Active Content-Theme Color Role | `surface.panel` | Active Theme | rest | Content Theme | Migrated to `statusBar.default.root.rest.backgroundColor` with shell fallback in Task 6 |
| `.footer` | `color` | `var(--clay-ds-status-bar-default-root-rest-text-color, var(--clay-ds-shell-default-footer-rest-text-color, var(--clay-text-muted)))` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Migrated to `statusBar.default.root.rest.textColor` with shell fallback in Task 6 |
| `.footer` | `padding` | `var(--clay-ds-status-bar-default-root-rest-padding, var(--clay-ds-shell-default-footer-rest-padding, var(--clay-spacing-xxs, 4px) var(--clay-spacing-sm, 12px)))` | Non-Color UI Design-System Recipe | `statusBar.default.root.rest.padding` | `spacing.xxs` `spacing.sm` | rest | UI Design System | Migrated to `statusBar.default.root.rest.padding` with shell fallback in Task 6 |
| `.footer` | `font-size` | `var(--clay-text-status-size)` | User Typography | `text.status.size` | User Profile | rest | User Typography | Retained (user typography) |

---

### 4.8 `frontend/src/components/tab-strip.module.css` (Unified `ClayTabStrip` primitive; formerly `tab-bar.module.css`)

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.tabList` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Shell | Retained (tab strip layout) |
| `.tabList` | `align-items` | `stretch` | Structural Layout | n/a | Host | rest | React Shell | Retained (tab strip layout) |
| `.tabList` | `gap` | `1px` | Structural Layout | n/a | Host | rest | React Shell | Retained (compartmentalization) |
| `.tabList` | `flex` | `1` | Structural Layout | n/a | Host | rest | React Shell | Retained (tab strip flex) |
| `.tabList` | `overflow-x` | `auto` | Structural Layout | n/a | Host | rest | React Shell | Retained (scrolling strip) |
| `.tabList` | `background` | `var(--clay-ds-tab-bar-default-root-rest-background-color, var(--clay-surface-main))` | Active Content-Theme Color Role / Recipe | `tabBar.default.root.rest.backgroundColor` | `surface.main` | rest | UI Design System / Theme | Migrated to DS recipe with core fallback in Task 4 |
| `.tab` | `display` | `inline-flex` | Structural Layout | n/a | Host | rest | React Shell | Retained (tab card layout) |
| `.tab` | `align-items` | `center` | Structural Layout | n/a | Host | rest | React Shell | Retained (tab card layout) |
| `.tab` | `gap` | `var(--clay-ds-tab-default-item-rest-gap, var(--clay-spacing-xs, 8px))` | Non-Color UI Design-System Recipe | `tabBar.default.card.rest.gap` | `spacing.xs` (8px) | rest | UI Design System | Align variable to `tabBar.default.card.*` in Task 4 |
| `.tab` | `background` | `var(--clay-ds-tab-default-item-rest-background-color, var(--clay-surface-panel))` | Active Content-Theme Color Role | `surface.panel` | Active Theme | rest | Content Theme | Align variable in Task 4 |
| `.tab` | `color` | `var(--clay-ds-tab-default-item-rest-text-color, var(--clay-text-muted))` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Align variable in Task 4 |
| `.tab` | `border-right` | `var(--clay-ds-tab-default-item-rest-border-width, 1px) solid var(--clay-ds-tab-default-item-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `tabBar.default.card.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Align variable in Task 4 |
| `.tab` | `border-radius` | `var(--clay-ds-tab-default-item-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `tabBar.default.card.rest.borderRadius` | `0` | rest | UI Design System | Align variable in Task 4 |
| `.tab` | `padding` | `var(--clay-ds-tab-default-item-rest-padding, var(--clay-spacing-xs) var(--clay-spacing-sm))` | Non-Color UI Design-System Recipe | `tabBar.default.card.rest.padding` | `spacing.xs` `spacing.sm` | rest | UI Design System | Align variable in Task 4 |
| `.tab` | `min-width` | `fit-content` | Structural Layout | n/a | Host | rest | React Shell | Retained (tab sizing) |
| `.tab` | `cursor` | `pointer` | Structural Layout | n/a | Host | rest | React Shell | Retained (interaction) |
| `.tab` | `font-size` | `var(--clay-text-detail-size)` | User Typography | `text.detail.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.tab` | `box-shadow` | `var(--clay-ds-tab-default-item-rest-shadow, none)` | Non-Color UI Design-System Recipe | `tabBar.default.card.rest.shadow` | `none` | rest | UI Design System | Align variable in Task 4 |
| `.tab` | `transition` | `background-color/color var(--clay-ds-tab-default-item-rest-transition-duration, 100ms) linear` | Non-Color UI Design-System Recipe | `tabBar.default.card.rest.transitionDuration` | `motion.fast` (100ms) | rest | UI Design System | Align variable in Task 4 |
| `.tab[data-hovered]` | `background` | `var(--clay-ds-tab-default-item-hover-background-color, var(--clay-surface-hover))` | Active Content-Theme Color Role | `surface.hover` | Active Theme | `data-hovered` | Content Theme | Align variable in Task 4 |
| `.tab[data-hovered]` | `color` | `var(--clay-ds-tab-default-item-hover-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | `data-hovered` | Content Theme | Align variable in Task 4 |
| `.tab[data-selected]` | `background` | `var(--clay-ds-tab-default-item-selected-background-color, var(--clay-surface-selected))` | Active Content-Theme Color Role | `surface.selected` | Active Theme | `data-selected` | Content Theme | Align variable in Task 4 |
| `.tab[data-selected]` | `color` | `var(--clay-ds-tab-default-item-selected-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | `data-selected` | Content Theme | Align variable in Task 4 |
| `.tab[data-focused]` | `outline` | `var(--clay-ds-tab-default-item-focus-outline-width, 2px) solid var(--clay-ds-tab-default-item-focus-outline-color, var(--clay-focus-ring))` | Accessibility Override | `focus.ring` | `focus.ring` (2px solid) | `data-focused` | React Aria / DS | Align variable in Task 4 |
| `.tab[data-focused]` | `outline-offset` | `var(--clay-ds-tab-default-item-focus-outline-offset, -2px)` | Non-Color UI Design-System Recipe | `tabBar.default.card.focus.outlineOffset` | `-2px` | `data-focused` | UI Design System | Align variable in Task 4 |
| `.close` | `background` / `border` | `transparent` / `none` | Structural Layout | n/a | Host | rest | React Shell | Retained (reset) |
| `.close` | `color` | `inherit` | Active Content-Theme Color Role | `text.muted` | Active Theme | rest | Content Theme | Map to `tabBar.default.closeButton.rest.textColor` in Task 4 |
| `.close` | `padding` | `0 var(--clay-spacing-xxs, 4px)` | Non-Color UI Design-System Recipe | `tabBar.default.closeButton.rest.padding` | `spacing.xxs` (4px) | rest | UI Design System | Map to `tabBar.default.closeButton.*` in Task 4 |
| `.close` | `font-size` | `var(--clay-text-caption-size)` | User Typography | `text.caption.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.dirty` | `width` / `height` | `0.45rem` / `0.45rem` | Non-Color UI Design-System Recipe | `tabBar.default.dirtyIndicator.rest.size` | `0.45rem` | rest | UI Design System | Map to `tabBar.default.dirtyIndicator.*` in Task 4 |
| `.dirty` | `background` | `var(--clay-accent-primary)` | Active Content-Theme Color Role | `accent.primary` | Active Theme | rest | Content Theme | Map to `tabBar.default.dirtyIndicator.rest.backgroundColor` in Task 4 |

---

### 4.9 `frontend/src/chat/chat.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.chat` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (chat layout) |
| `.chat` | `grid-template-rows` | `auto auto minmax(0, 1fr) auto auto auto` | Structural Layout | n/a | Host | rest | React Host | Retained (chat layout) |
| `.chat` | `height` / `min-height` | `100%` / `0` | Structural Layout | n/a | Host | rest | React Host | Retained (containment) |
| `.chat` | `background` | `var(--clay-ds-chat-default-root-rest-background-color, var(--clay-surface-main))` | Active Content-Theme Color Role | `chat.default.root.rest.backgroundColor` | `surface.main` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.chat` | `color` | `var(--clay-ds-chat-default-root-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `chat.default.root.rest.textColor` | `text.primary` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.chat` | `border` | `var(--clay-ds-chat-default-root-rest-border-width, 1px) var(--clay-ds-chat-default-root-rest-border-style, solid) var(--clay-ds-chat-default-root-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `chat.default.root.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.chat` | `gap` | `var(--clay-ds-chat-default-root-rest-gap, 0px)` | Non-Color UI Design-System Recipe | `chat.default.root.rest.gap` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.chat` | `padding` | `var(--clay-ds-chat-default-root-rest-padding, 0px)` | Non-Color UI Design-System Recipe | `chat.default.root.rest.padding` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.chat` | `border-radius` | `var(--clay-ds-chat-default-root-rest-border-radius, 0px)` | Non-Color UI Design-System Recipe | `chat.default.root.rest.borderRadius` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.header` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (header layout) |
| `.header` | `gap` | `var(--clay-ds-chat-default-header-rest-gap, var(--clay-spacing-xs, 8px))` | Non-Color UI Design-System Recipe | `chat.default.header.rest.gap` | `spacing.xs` (8px) | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.header` | `padding` | `var(--clay-ds-chat-default-header-rest-padding, var(--clay-spacing-lg, 24px) var(--clay-spacing-lg, 24px) var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `chat.default.header.rest.padding` | `spacing.lg` `spacing.md` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.header` | `border-bottom` | `var(--clay-ds-chat-default-header-rest-border-width, 2px) var(--clay-ds-chat-default-header-rest-border-style, solid) var(--clay-ds-chat-default-header-rest-border-color, var(--clay-border-strong))` | Non-Color UI Design-System Recipe | `chat.default.header.rest.border*` | `border.strong` (2px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.actions` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Host | Retained (action bar layout) |
| `.actions` | `gap` | `1px` | Structural Layout | n/a | Host | rest | React Host | Retained (compartmentalization) |
| `.actions` | `background` | `var(--clay-border-hairline)` | Active Content-Theme Color Role | `border.hairline` | Active Theme | rest | Content Theme | Retained / verified |
| `.actions` | `border-bottom` | `1px solid var(--clay-border-hairline)` | Non-Color UI Design-System Recipe | `border.hairline` (1px) | `border.hairline` (1px) | rest | UI Design System / Theme | Retained |
| `.transcriptScroll` | `overflow-y` | `auto` | Structural Layout | n/a | Host | rest | React Host | Retained (scrolling containment) |
| `.transcriptScroll` | `padding` | `var(--clay-spacing-md) var(--clay-spacing-lg)` | Non-Color UI Design-System Recipe | `spacing.md` `spacing.lg` | `spacing.md` `spacing.lg` | rest | UI Design System | Retained |
| `.transcriptScroll` | `gap` | `var(--clay-spacing-sm)` | Non-Color UI Design-System Recipe | `spacing.sm` | `spacing.sm` | rest | UI Design System | Retained |
| `.transcriptScroll` | `background` | `var(--clay-surface-panel)` | Active Content-Theme Color Role | `surface.panel` | Active Theme | rest | Content Theme | Retained / verified |
| `.user` | `justify-self` | `end` | Structural Layout | n/a | Host | rest | React Host | Retained (message alignment) |
| `.user` | `padding` | `var(--clay-ds-chat-default-user-rest-padding, var(--clay-spacing-sm, 12px) var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `chat.default.user.rest.padding` | `spacing.sm` `spacing.md` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.user` | `border` | `var(--clay-ds-chat-default-user-rest-border-width, 1px) var(--clay-ds-chat-default-user-rest-border-style, solid) var(--clay-ds-chat-default-user-rest-border-color, var(--clay-border-strong))` | Non-Color UI Design-System Recipe | `chat.default.user.rest.border*` | `border.strong` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.user` | `border-radius` | `var(--clay-ds-chat-default-user-rest-border-radius, 0px)` | Non-Color UI Design-System Recipe | `chat.default.user.rest.borderRadius` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.user` | `background` | `var(--clay-ds-chat-default-user-rest-background-color, var(--clay-surface-badge))` | Active Content-Theme Color Role | `chat.default.user.rest.backgroundColor` | `surface.badge` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.user` | `color` | `var(--clay-ds-chat-default-user-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `chat.default.user.rest.textColor` | `text.primary` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.assistant` | `justify-self` | `start` | Structural Layout | n/a | Host | rest | React Host | Retained (message alignment) |
| `.assistant` | `padding` | `var(--clay-ds-chat-default-assistant-rest-padding, var(--clay-spacing-sm, 12px) var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `chat.default.assistant.rest.padding` | `spacing.sm` `spacing.md` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.assistant` | `border-left` | `var(--clay-ds-chat-default-assistant-rest-border-width, 2px) var(--clay-ds-chat-default-assistant-rest-border-style, solid) var(--clay-ds-chat-default-assistant-rest-border-color, var(--clay-accent-primary))` | Non-Color UI Design-System Recipe | `chat.default.assistant.rest.border*` | `accent.primary` (2px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.assistant` | `background` | `var(--clay-ds-chat-default-assistant-rest-background-color, transparent)` | Active Content-Theme Color Role | `chat.default.assistant.rest.backgroundColor` | `transparent` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.assistant` | `color` | `var(--clay-ds-chat-default-assistant-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `chat.default.assistant.rest.textColor` | `text.primary` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.thinking` | `font-family` | `var(--clay-font-monospace)` | User Typography | `font.monospace` | User Profile | rest | User Typography | Retained (user typography) |
| `.error` | `border-left` | `2px solid var(--clay-diagnostic-error)` | Active Content-Theme Color Role | `diagnostic.error` (2px) | Active Theme | state | Content Theme | Retained / verified |
| `.composerArea` | `border-top` | `2px solid var(--clay-border-strong)` | Unjustified Literal | `chatPanel.default.composer.rest.border*` | `border.strong` (2px) | rest | UI Design System / Theme | Migrate to recipe in Task 4 |
| `.statusLine` | `background` | `var(--clay-surface-panel)` | Active Content-Theme Color Role | `surface.panel` | Active Theme | rest | Content Theme | Map to `chatPanel.default.statusLine.rest.backgroundColor` in Task 4 |
| `.statusLine` | `border-bottom` | `1px solid var(--clay-border-hairline)` | Non-Color UI Design-System Recipe | `chatPanel.default.statusLine.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Migrate to recipe in Task 4 |
| `.composer` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (composer layout) |
| `.composer` | `gap` | `var(--clay-spacing-sm)` | Non-Color UI Design-System Recipe | `chatPanel.default.composer.rest.gap` | `spacing.sm` | rest | UI Design System | Map to `chatPanel.default.composer.rest.gap` in Task 4 |
| `.composer` | `padding` | `var(--clay-spacing-md) var(--clay-spacing-lg)` | Non-Color UI Design-System Recipe | `chatPanel.default.composer.rest.padding` | `spacing.md` `spacing.lg` | rest | UI Design System | Map to `chatPanel.default.composer.rest.padding` in Task 4 |
| `.sessions` | `border-top` | `var(--clay-ds-chat-default-sessions-rest-border-width, 1px) solid var(--clay-ds-chat-default-sessions-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `chatPanel.default.sessions.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Retained / verified |
| `.sessionList` | `background` | `var(--clay-ds-chat-default-session-list-rest-background-color, var(--clay-border-hairline))` | Active Content-Theme Color Role / Recipe | `chatPanel.default.sessionList.rest.backgroundColor` | `border.hairline` | rest | UI Design System / Theme | Migrated to DS recipe with core fallback in Task 4 |
| `.sessionRow` | `background` | `var(--clay-ds-chat-default-session-row-rest-background-color, var(--clay-surface-main))` | Active Content-Theme Color Role / Recipe | `chatPanel.default.sessionRow.rest.backgroundColor` | `surface.main` | rest | UI Design System / Theme | Retained / verified |
| `.footerCommands` | `background` | `var(--clay-ds-chat-default-footer-commands-rest-background-color, var(--clay-border-hairline))` | Active Content-Theme Color Role / Recipe | `chatPanel.default.footerCommands.rest.backgroundColor` | `border.hairline` | rest | UI Design System / Theme | Migrated to DS recipe with core fallback in Task 4 |
| `.footerCommands` | `border-top` | `var(--clay-ds-chat-default-footer-commands-rest-border-width, 1px) solid var(--clay-ds-chat-default-footer-commands-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `chatPanel.default.footerCommands.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Migrated to DS recipe with core fallback in Task 4 |

---

### 4.10 `frontend/src/command-centre/command-centre.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.surface` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (command centre layout) |
| `.surface` | `gap` | `var(--clay-spacing-sm, 12px)` | Non-Color UI Design-System Recipe | `spacing.sm` (12px) | `spacing.sm` (12px) | rest | UI Design System | Retained |
| `.surface` | `width` | `min(calc(var(--clay-dimension-overlay-centered-width) - 2 * var(--clay-ds-command-centre-default-root-rest-padding, var(--clay-spacing-md, 16px))), calc(90vw - 2 * var(--clay-ds-command-centre-default-root-rest-padding, var(--clay-spacing-md, 16px))))` | Structural Layout | `dimension.overlay.centered.width` | Host (640px) | rest | React Host | Retained (centered overlay width constraint) |
| `.surface` | `padding` | `var(--clay-ds-command-centre-default-root-rest-padding, var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `commandCentre.default.root.rest.padding` | `spacing.md` (16px) | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.surface` | `border` | `var(--clay-ds-command-centre-default-root-rest-border-width, 1px) var(--clay-ds-command-centre-default-root-rest-border-style, solid) var(--clay-ds-command-centre-default-root-rest-border-color, var(--clay-border-strong))` | Non-Color UI Design-System Recipe | `commandCentre.default.root.rest.border*` | `border.strong` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.surface` | `border-radius` | `var(--clay-ds-command-centre-default-root-rest-border-radius, 0px)` | Non-Color UI Design-System Recipe | `commandCentre.default.root.rest.borderRadius` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.surface` | `background` | `var(--clay-ds-command-centre-default-root-rest-background-color, var(--clay-surface-overlay))` | Active Content-Theme Color Role | `commandCentre.default.root.rest.backgroundColor` | `surface.overlay` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.surface` | `box-shadow` | `var(--clay-ds-command-centre-default-root-rest-shadow, none)` | Non-Color UI Design-System Recipe | `commandCentre.default.root.rest.shadow` | `none` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.surface [role="listbox"]` | `max-height` | `min(50vh, 420px)` | Structural Layout | n/a | Host | rest | React Host | Retained (result list cap) |
| `.surface [role="listbox"]` | `overflow` | `auto` | Structural Layout | n/a | Host | rest | React Host | Retained (scrolling containment) |
| `.empty` | `min-height` | `calc(var(--clay-spacing-xxl, 48px) * 2)` | Structural Layout | n/a | Host (96px) | rest | React Host | Retained (empty view constraint) |
| `.empty` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (empty view layout) |
| `.empty` | `border-top` | `var(--clay-ds-command-centre-default-empty-rest-border-width, var(--clay-dimension-border-hairline, 1px)) var(--clay-ds-command-centre-default-empty-rest-border-style, solid) var(--clay-ds-command-centre-default-empty-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `commandCentre.default.empty.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.empty` | `padding` | `var(--clay-ds-command-centre-default-empty-rest-padding, var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `commandCentre.default.empty.rest.padding` | `spacing.md` (16px) | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.empty` | `background` | `var(--clay-ds-command-centre-default-empty-rest-background-color, transparent)` | Active Content-Theme Color Role | `commandCentre.default.empty.rest.backgroundColor` | `transparent` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.empty` | `color` | `var(--clay-ds-command-centre-default-empty-rest-text-color, var(--clay-text-muted))` | Active Content-Theme Color Role | `commandCentre.default.empty.rest.textColor` | `text.muted` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.status` | `color` | `var(--clay-ds-command-centre-default-status-rest-text-color, var(--clay-text-muted))` | Active Content-Theme Color Role | `commandCentre.default.status.rest.textColor` | `text.muted` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.status` | `border-top` | `var(--clay-ds-command-centre-default-status-rest-border-width, var(--clay-dimension-border-hairline, 1px)) var(--clay-ds-command-centre-default-status-rest-border-style, solid) var(--clay-ds-command-centre-default-status-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `commandCentre.default.status.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.status` | `padding` | `var(--clay-ds-command-centre-default-status-rest-padding, var(--clay-spacing-xxs, 4px) var(--clay-spacing-sm, 12px))` | Non-Color UI Design-System Recipe | `commandCentre.default.status.rest.padding` | `spacing.xxs` `spacing.sm` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.status` | `background` | `var(--clay-ds-command-centre-default-status-rest-background-color, transparent)` | Active Content-Theme Color Role | `commandCentre.default.status.rest.backgroundColor` | `transparent` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.status` | `font-family` | `var(--clay-font-ui)` | User Typography | `font.ui` | User Profile | rest | User Typography | Retained (user typography) |
| `.status` | `font-size` | `var(--clay-text-status-size)` | User Typography | `text.status.size` | User Profile | rest | User Typography | Retained (user typography) |
| `.status` | `font-variant-numeric` | `tabular-nums` | User Typography | n/a | Host | rest | User Typography | Retained (tabular number formatting) |

---

### 4.11 `frontend/src/editor/editor.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.host` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (editor host layout) |
| `.host` | `grid-template-rows` | `auto 1fr` | Structural Layout | n/a | Host | rest | React Host | Retained (editor host layout) |
| `.host` | `height` / `min-height` | `100%` / `0` | Structural Layout | n/a | Host | rest | React Host | Retained (pane main slot containment) |
| `.host` | `background` | `var(--clay-ds-editor-default-container-rest-background-color, var(--clay-ds-editor-default-root-rest-background-color, var(--clay-surface-main)))` | Active Content-Theme Color Role | `editor.default.container.rest.backgroundColor` | `surface.main` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.host` | `color` | `var(--clay-ds-editor-default-container-rest-text-color, var(--clay-ds-editor-default-root-rest-text-color, var(--clay-text-primary)))` | Active Content-Theme Color Role | `editor.default.container.rest.textColor` | `text.primary` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.host` | `border` | `var(--clay-ds-editor-default-container-rest-border-width, var(--clay-ds-editor-default-root-rest-border-width, 0px)) var(--clay-ds-editor-default-container-rest-border-style, var(--clay-ds-editor-default-root-rest-border-style, solid)) var(--clay-ds-editor-default-container-rest-border-color, var(--clay-ds-editor-default-root-rest-border-color, transparent))` | Non-Color UI Design-System Recipe | `editor.default.container.rest.border*` | `transparent` (0px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.host` | `border-radius` | `var(--clay-ds-editor-default-container-rest-border-radius, var(--clay-ds-editor-default-root-rest-border-radius, 0px))` | Non-Color UI Design-System Recipe | `editor.default.container.rest.borderRadius` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.chrome` | `display` | `flex` | Structural Layout | n/a | Host | rest | React Host | Retained (editor chrome bar layout) |
| `.chrome` | `background` | `var(--clay-surface-panel)` | Active Content-Theme Color Role | `surface.panel` | Active Theme | rest | Content Theme | Retained |
| `.chrome` | `border-bottom` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-hairline)` | Non-Color UI Design-System Recipe | `border.hairline` (1px) | `border.hairline` (1px) | rest | UI Design System / Theme | Retained |
| `.path` | `background` | `var(--clay-surface-control)` | Active Content-Theme Color Role | `surface.control` | Active Theme | rest | Content Theme | Retained / verified |
| `.path` | `color` | `var(--clay-text-primary)` | Active Content-Theme Color Role | `text.primary` | Active Theme | rest | Content Theme | Retained / verified |
| `.path` | `border` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-subtle)` | Non-Color UI Design-System Recipe | `textInput.default.input.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Retained / verified |
| `.path:focus-visible` | `outline` | `2px solid var(--clay-focus-ring)` | Accessibility Override | `focus.ring` | `focus.ring` (2px solid) | `focus-visible` | React Aria | Retained / verified |
| `.canvas` | `min-height` / `overflow` | `0` / `hidden` | Structural Layout | n/a | Host | rest | CodeMirror Host | Retained (CodeMirror mount containment) |
| `.canvas :global(.cm-gutters)` | `background` | `var(--clay-ds-editor-default-gutter-rest-background-color, var(--clay-surface-panel))` | Active Content-Theme Color Role | `editor.default.gutter.rest.backgroundColor` | `surface.panel` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-gutters)` | `color` | `var(--clay-ds-editor-default-gutter-rest-text-color, var(--clay-text-muted))` | Active Content-Theme Color Role | `editor.default.gutter.rest.textColor` | `text.muted` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-gutters)` | `border-right` | `var(--clay-ds-editor-default-gutter-rest-border-width, var(--clay-dimension-border-hairline, 1px)) var(--clay-ds-editor-default-gutter-rest-border-style, solid) var(--clay-ds-editor-default-gutter-rest-border-color, var(--clay-border-hairline))` | Non-Color UI Design-System Recipe | `editor.default.gutter.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-activeLine)` | `background` | `var(--clay-ds-editor-default-active-line-rest-background-color, var(--clay-surface-hover))` | Active Content-Theme Color Role | `editor.default.activeLine.rest.backgroundColor` | `surface.hover` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-selectionBackground)` | `background` | `var(--clay-ds-editor-default-selection-rest-background-color, var(--clay-surface-selected))` | Active Content-Theme Color Role | `editor.default.selection.rest.backgroundColor` | `surface.selected` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-matchingBracket)` | `background` | `var(--clay-ds-editor-default-matching-bracket-rest-background-color, var(--clay-surface-control))` | Active Content-Theme Color Role | `editor.default.matchingBracket.rest.backgroundColor` | `surface.control` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-matchingBracket)` | `outline` | `var(--clay-ds-editor-default-matching-bracket-rest-border-width, 1px) var(--clay-ds-editor-default-matching-bracket-rest-border-style, solid) var(--clay-ds-editor-default-matching-bracket-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `editor.default.matchingBracket.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-searchMatch)` | `background` | `var(--clay-ds-editor-default-find-match-rest-background-color, var(--clay-surface-selected))` | Active Content-Theme Color Role | `editor.default.findMatch.rest.backgroundColor` | `surface.selected` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-searchMatch)` | `border` | `var(--clay-ds-editor-default-find-match-rest-border-width, 1px) var(--clay-ds-editor-default-find-match-rest-border-style, solid) var(--clay-ds-editor-default-find-match-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `editor.default.findMatch.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.canvas :global(.cm-tooltip)` | `border` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-strong)` | Non-Color UI Design-System Recipe | `completion.default.root.rest.border*` | `border.strong` (1px) | rest | UI Design System / Theme | Map to `completion.default.root.*` in Task 4 |
| `.canvas :global(.cm-tooltip-autocomplete > ul > li[aria-selected])` | `background` | `var(--clay-surface-selected)` | Active Content-Theme Color Role | `surface.selected` | Active Theme | `aria-selected` | Content Theme | Retained / verified |
| `.canvas :global(.cm-clay-t-*)` (37 syntax tokens) | `color` / `background` / `weight` / `style` | `var(--clay-editor-*-color)` etc. | Active Content-Theme Color Role | StyleRegistry / CodeMirror syntax token | Theme Snapshot | rest | Content Theme / CM | Retained (internal CM syntax mapping) |
| `.canvas :global(.cm-diagnostic-error)` | `border-left-color` | `var(--clay-diagnostic-error)` | Active Content-Theme Color Role | `diagnostic.error` | Active Theme | state | Content Theme | Retained / verified |
| `.canvas :global(.cm-diagnostic-warning)` | `border-left-color` | `var(--clay-diagnostic-warning)` | Active Content-Theme Color Role | `diagnostic.warning` | Active Theme | state | Content Theme | Retained / verified |
| `.canvas :global(.cm-diagnostic-info)` | `border-left-color` | `var(--clay-diagnostic-info)` | Active Content-Theme Color Role | `diagnostic.info` | Active Theme | state | Content Theme | Retained / verified |

---

### 4.12 `frontend/src/packages/package-workspace.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.workspace` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (package slot grid) |
| `.workspace` | `grid-template-rows` | `auto minmax(0, 1fr) auto auto` | Structural Layout | n/a | Host | rest | React Host | Retained (package slot grid) |
| `.workspace` | `background` | `var(--clay-surface-main)` | Active Content-Theme Color Role | `surface.main` | Active Theme | rest | Content Theme | Retained / verified |
| `.middle` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (split grid) |
| `.middle` | `grid-template-columns` | `auto minmax(0, 1fr) auto` | Structural Layout | n/a | Host | rest | React Host | Retained (split grid) |
| `.left`, `.right` | `width` | `clamp(var(--clay-dimension-panel-side-min, 48px), var(--clay-dimension-panel-side-default, 240px), var(--clay-dimension-panel-side-max, 480px))` | Structural Layout | `dimension.panel.side.*` | Host bounds (48px-480px) | rest | React Host | Retained (user-resizable panel bounds) |
| `.left` | `border-right` | `var(--clay-ds-file-browser-default-root-rest-border-width, var(--clay-ds-panel-default-root-rest-border-width, var(--clay-dimension-border-hairline, 1px))) var(--clay-ds-file-browser-default-root-rest-border-style, var(--clay-ds-panel-default-root-rest-border-style, solid)) var(--clay-ds-file-browser-default-root-rest-border-color, var(--clay-ds-panel-default-root-rest-border-color, var(--clay-border-subtle)))` | Non-Color UI Design-System Recipe | `fileBrowser.default.root.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Migrated to fileBrowser recipe with panel fallback in Task 6 |
| `.left` | `background` | `var(--clay-ds-file-browser-default-root-rest-background-color, var(--clay-ds-panel-default-root-rest-background-color, var(--clay-surface-panel)))` | Active Content-Theme Color Role | `fileBrowser.default.root.rest.backgroundColor` | `surface.panel` | rest | UI Design System / Theme | Migrated to fileBrowser recipe with panel fallback in Task 6 |
| `.left` | `color` | `var(--clay-ds-file-browser-default-root-rest-text-color, var(--clay-ds-panel-default-root-rest-text-color, var(--clay-text-primary)))` | Active Content-Theme Color Role | `fileBrowser.default.root.rest.textColor` | `text.primary` | rest | UI Design System / Theme | Migrated to fileBrowser recipe with panel fallback in Task 6 |
| `.left` | `padding` | `var(--clay-ds-file-browser-default-root-rest-padding, var(--clay-ds-panel-default-root-rest-padding, var(--clay-spacing-xs, 8px)))` | Non-Color UI Design-System Recipe | `fileBrowser.default.root.rest.padding` | `spacing.xs` (8px) | rest | UI Design System | Migrated to fileBrowser recipe with panel fallback in Task 6 |
| `.left` | `gap` | `var(--clay-ds-file-browser-default-root-rest-gap, var(--clay-ds-panel-default-root-rest-gap, var(--clay-spacing-xs, 8px)))` | Non-Color UI Design-System Recipe | `fileBrowser.default.root.rest.gap` | `spacing.xs` (8px) | rest | UI Design System | Migrated to fileBrowser recipe with panel fallback in Task 6 |
| `.right` | `border-left` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-subtle)` | Non-Color UI Design-System Recipe | `panel.fixed.root.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Map to `panel.fixed.root.rest.border*` in Task 4 |
| `.top`, `.bottom` | `max-height` | `var(--clay-dimension-panel-vertical-max, 240px)` | Structural Layout | `dimension.panel.vertical.max` | Host bounds (240px) | rest | React Host | Retained (user-resizable vertical bounds) |
| `.top` | `border-bottom` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-subtle)` | Non-Color UI Design-System Recipe | `panel.fixed.root.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Map to `panel.fixed.root.rest.border*` in Task 4 |
| `.bottom` | `border-top` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-subtle)` | Non-Color UI Design-System Recipe | `panel.fixed.root.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Map to `panel.fixed.root.rest.border*` in Task 4 |
| `.status` | `background` | `var(--clay-ds-status-bar-default-root-rest-background-color, var(--clay-ds-panel-default-root-rest-background-color, var(--clay-surface-panel)))` | Active Content-Theme Color Role | `statusBar.default.root.rest.backgroundColor` | `surface.panel` | rest | UI Design System / Theme | Migrated to statusBar recipe with panel fallback in Task 6 |
| `.status` | `color` | `var(--clay-ds-status-bar-default-root-rest-text-color, var(--clay-ds-panel-default-root-rest-text-color, var(--clay-text-muted)))` | Active Content-Theme Color Role | `statusBar.default.root.rest.textColor` | `text.muted` | rest | UI Design System / Theme | Migrated to statusBar recipe with panel fallback in Task 6 |
| `.status` | `border-top` | `var(--clay-ds-status-bar-default-root-rest-border-width, var(--clay-ds-panel-default-root-rest-border-width, var(--clay-dimension-border-hairline, 1px))) var(--clay-ds-status-bar-default-root-rest-border-style, var(--clay-ds-panel-default-root-rest-border-style, solid)) var(--clay-ds-status-bar-default-root-rest-border-color, var(--clay-ds-panel-default-root-rest-border-color, var(--clay-border-hairline)))` | Non-Color UI Design-System Recipe | `statusBar.default.root.rest.border*` | `border.hairline` (1px) | rest | UI Design System / Theme | Migrated to statusBar recipe with panel fallback in Task 6 |
| `.status` | `padding` | `var(--clay-ds-status-bar-default-root-rest-padding, var(--clay-ds-panel-default-root-rest-padding, var(--clay-spacing-xxs, 4px) var(--clay-spacing-sm, 12px)))` | Non-Color UI Design-System Recipe | `statusBar.default.root.rest.padding` | `spacing.xxs` `spacing.sm` | rest | UI Design System | Migrated to statusBar recipe with panel fallback in Task 6 |
| `.status` | `gap` | `var(--clay-ds-status-bar-default-root-rest-gap, var(--clay-ds-panel-default-root-rest-gap, var(--clay-spacing-sm, 12px)))` | Non-Color UI Design-System Recipe | `statusBar.default.root.rest.gap` | `spacing.sm` (12px) | rest | UI Design System | Migrated to statusBar recipe with panel fallback in Task 6 |
| `.overlay` | `position` | `absolute` | Structural Layout | n/a | Host | rest | React Host | Retained (overlay positioning) |
| `.overlay` | `inset` | `0` | Structural Layout | n/a | Host | rest | React Host | Retained (overlay positioning) |
| `.overlay` | `z-index` | `var(--clay-z-overlay, 20)` | Structural Layout | `z.overlay` | Host (20) | rest | React Host | Retained (structural layering) |
| `.overlay` | `pointer-events` | `none` | Structural Layout | n/a | Host | rest | React Host | Retained (pass-through clicks to host) |
| `.overlay > *` | `pointer-events` | `auto` | Structural Layout | n/a | Host | rest | React Host | Retained (interactive target) |
| `.overlay > *` | `width` | `min(100%, var(--clay-dimension-overlay-centered-width, 640px))` | Structural Layout | `dimension.overlay.centered.width` | Host (640px) | rest | React Host | Retained (overlay width cap) |

---

### 4.13 `frontend/src/routes/fixture.module.css` and `routes/workspace.module.css`

| File | Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fixture.module.css` | `.fixture` | `display: flex; flex-direction: column` | Structural Layout | n/a | Host | React Host | Retained (fixture route) |
| `fixture.module.css` | `.fixture` | `padding` | `var(--clay-spacing-lg, 24px)` | Non-Color UI Design-System Recipe | `spacing.lg` (24px) | UI Design System | Retained |
| `fixture.module.css` | `.documentFixture` | `background` | `var(--clay-surface-main)` | Active Content-Theme Color Role | `surface.main` | Active Theme | Content Theme | Retained |
| `fixture.module.css` | `.reviewStatus` | `background` | `var(--clay-surface-panel)` | Active Content-Theme Color Role | `surface.panel` | Active Theme | Content Theme | Retained |
| `fixture.module.css` | `.panel` | `background` / `border` | `var(--clay-surface-panel)` / hairline | Active Content-Theme Color Role | `surface.panel` / hairline | Active Theme | Content Theme | Retained |
| `fixture.module.css` | `.emptyState` | `background` / `border` | `var(--clay-surface-list)` / hairline | Active Content-Theme Color Role | `surface.list` / hairline | Active Theme | Content Theme | Retained |
| `workspace.module.css` | `.workspace` | `display: grid; place-items: center` | Structural Layout | n/a | Host | React Host | Retained (workspace route) |
| `workspace.module.css` | `.workspace` | `padding` | `var(--clay-spacing-lg, 24px)` | Non-Color UI Design-System Recipe | `spacing.lg` (24px) | UI Design System | Retained |

---

### 4.14 `frontend/src/sdui/registry.module.css` and `sdui/renderer.module.css`

| File | Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `registry.module.css` | `.surface` | `background` | `var(--clay-ds-panel-default-root-rest-background-color, var(--clay-surface-panel))` | Active Content-Theme Color Role | `surface.panel` | Active Theme | Content Theme | Retained / verified |
| `registry.module.css` | `.surface` | `color` | `var(--clay-ds-panel-default-root-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `text.primary` | Active Theme | Content Theme | Retained / verified |
| `registry.module.css` | `.surface` | `padding` | `var(--clay-ds-panel-default-root-rest-padding, var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `panel.default.root.rest.padding` | `spacing.md` (16px) | UI Design System | Retained / verified |
| `registry.module.css` | `.surface` | `border-radius` | `var(--clay-ds-panel-default-root-rest-border-radius, 0)` | Non-Color UI Design-System Recipe | `panel.default.root.rest.borderRadius` | `0` | UI Design System | Retained / verified |
| `registry.module.css` | `.surface` | `box-shadow` | `var(--clay-ds-panel-default-root-rest-shadow, none)` | Non-Color UI Design-System Recipe | `panel.default.root.rest.shadow` | `none` | UI Design System | Retained / verified |
| `registry.module.css` | `.scroll` | `scrollbar-color` | `var(--clay-surface-scrollbar) var(--clay-surface-scrollbar-track)` | Active Content-Theme Color Role | `surface.scrollbar` / `track` | Active Theme | Content Theme | Retained / verified |
| `registry.module.css` | `.scroll:focus-visible` | `outline` | `var(--clay-dimension-border-thin, 2px) solid var(--clay-focus-ring)` | Accessibility Override | `focus.ring` (2px) | React Aria | Retained / verified |
| `renderer.module.css` | `.panel` | `background` | `var(--clay-surface-panel)` | Active Content-Theme Color Role | `surface.panel` | Active Theme | Content Theme | Retained / verified |
| `renderer.module.css` | `.panel` | `border-right` | `var(--clay-dimension-border-hairline, 1px) solid var(--clay-border-subtle)` | Non-Color UI Design-System Recipe | `panel.fixed.root.rest.border*` | `border.subtle` (1px) | UI Design System / Theme | Retained / verified |

---

### 4.15 `frontend/src/settings/settings-panel.module.css`

| Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | State Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `.panel` | `min-width` / `width` / `max-width` | `clamp(48px, 240px, 480px)` | Structural Layout | `dimension.panel.side.*` | Host bounds (48px-480px) | rest | React Host | Retained (panel width constraint) |
| `.panel` | `display` | `grid` | Structural Layout | n/a | Host | rest | React Host | Retained (panel layout) |
| `.panel` | `background` | `var(--clay-ds-settings-panel-default-panel-rest-background-color, var(--clay-surface-panel))` | Active Content-Theme Color Role | `settingsPanel.default.panel.rest.backgroundColor` | `surface.panel` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.panel` | `color` | `var(--clay-ds-settings-panel-default-panel-rest-text-color, var(--clay-text-primary))` | Active Content-Theme Color Role | `settingsPanel.default.panel.rest.textColor` | `text.primary` | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.panel` | `border-left` | `var(--clay-ds-settings-panel-default-panel-rest-border-width, var(--clay-dimension-border-hairline, 1px)) var(--clay-ds-settings-panel-default-panel-rest-border-style, solid) var(--clay-ds-settings-panel-default-panel-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `settingsPanel.default.panel.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.panel` | `padding` | `var(--clay-ds-settings-panel-default-panel-rest-padding, 0px)` | Non-Color UI Design-System Recipe | `settingsPanel.default.panel.rest.padding` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.panel` | `gap` | `var(--clay-ds-settings-panel-default-panel-rest-gap, 0px)` | Non-Color UI Design-System Recipe | `settingsPanel.default.panel.rest.gap` | `0px` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.panel` | `box-shadow` | `var(--clay-ds-settings-panel-default-panel-rest-shadow, none)` | Non-Color UI Design-System Recipe | `settingsPanel.default.panel.rest.shadow` | `none` | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.heading` | `border-bottom` | `var(--clay-ds-settings-panel-default-heading-rest-border-width, var(--clay-dimension-border-hairline, 1px)) var(--clay-ds-settings-panel-default-heading-rest-border-style, solid) var(--clay-ds-settings-panel-default-heading-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `settingsPanel.default.heading.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.heading` | `padding` | `var(--clay-ds-settings-panel-default-heading-rest-padding, var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `settingsPanel.default.heading.rest.padding` | `spacing.md` (16px) | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.actions` | `border-top` | `var(--clay-ds-settings-panel-default-actions-rest-border-width, var(--clay-dimension-border-hairline, 1px)) var(--clay-ds-settings-panel-default-actions-rest-border-style, solid) var(--clay-ds-settings-panel-default-actions-rest-border-color, var(--clay-border-subtle))` | Non-Color UI Design-System Recipe | `settingsPanel.default.actions.rest.border*` | `border.subtle` (1px) | rest | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `.actions` | `padding` | `var(--clay-ds-settings-panel-default-actions-rest-padding, var(--clay-spacing-md, 16px))` | Non-Color UI Design-System Recipe | `settingsPanel.default.actions.rest.padding` | `spacing.md` (16px) | rest | UI Design System | Migrated to DS recipe in Task 6 |
| `.error` | `color` / `border-top` | `var(--clay-diagnostic-error)` | Active Content-Theme Color Role | `diagnostic.error` | Active Theme | state | Content Theme | Retained / verified |

---

### 4.16 `frontend/src/shell/pane-tree.module.css` and `shell/workspace-panes.module.css`

| File | Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pane-tree.module.css` | `.group` | `background` | `var(--clay-ds-pane-split-tree-default-group-rest-background-color, var(--clay-border-hairline))` | Active Content-Theme Color Role | `paneSplitTree.default.group.rest.backgroundColor` | `border.hairline` | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.group` | `gap` | `var(--clay-ds-pane-split-tree-default-group-rest-gap, 0px)` | Non-Color UI Design-System Recipe | `paneSplitTree.default.group.rest.gap` | `0px` | UI Design System | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.group` | `padding` | `var(--clay-ds-pane-split-tree-default-group-rest-padding, 0px)` | Non-Color UI Design-System Recipe | `paneSplitTree.default.group.rest.padding` | `0px` | UI Design System | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.pane` | `background` | `var(--clay-ds-pane-split-tree-default-pane-rest-background-color, var(--clay-surface-main))` | Active Content-Theme Color Role | `paneSplitTree.default.pane.rest.backgroundColor` | `surface.main` | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.pane` | `border` | `var(--clay-ds-pane-split-tree-default-pane-rest-border-width, 0px) var(--clay-ds-pane-split-tree-default-pane-rest-border-style, solid) var(--clay-ds-pane-split-tree-default-pane-rest-border-color, transparent)` | Non-Color UI Design-System Recipe | `paneSplitTree.default.pane.rest.border*` | `transparent` (0px) | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.pane` | `border-radius` | `var(--clay-ds-pane-split-tree-default-pane-rest-border-radius, 0px)` | Non-Color UI Design-System Recipe | `paneSplitTree.default.pane.rest.borderRadius` | `0px` | UI Design System | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.pane.active` | `outline` | `var(--clay-ds-pane-split-tree-default-pane-active-outline-width, 1px) var(--clay-ds-pane-split-tree-default-pane-active-outline-style, solid) var(--clay-ds-pane-split-tree-default-pane-active-outline-color, var(--clay-border-focus))` | Accessibility / Recipe Outline | `paneSplitTree.default.pane.active.outline*` | `border.focus` (1px) | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.pane.active` | `outline-offset` | `var(--clay-ds-pane-split-tree-default-pane-active-outline-offset, -1px)` | Non-Color UI Design-System Recipe | `paneSplitTree.default.pane.active.outlineOffset` | `-1px` | UI Design System | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.separator` | `background` | `var(--clay-ds-pane-split-tree-default-handle-rest-background-color, var(--clay-border-strong))` | Active Content-Theme Color Role | `paneSplitTree.default.handle.rest.backgroundColor` | `border.strong` | UI Design System / Theme | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.separator` | `flex` | `0 0 var(--clay-ds-pane-split-tree-default-handle-rest-border-width, var(--clay-dimension-border-thin, 2px))` | Non-Color UI Design-System Recipe | `paneSplitTree.default.handle.rest.borderWidth` | `dimension.border.thin` (2px) | UI Design System | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.separator:focus-visible` | `outline` | `var(--clay-ds-pane-split-tree-default-handle-focus-outline-width, 2px) var(--clay-ds-pane-split-tree-default-handle-focus-outline-style, solid) var(--clay-ds-pane-split-tree-default-handle-focus-outline-color, var(--clay-focus-ring))` | Accessibility Override | `paneSplitTree.default.handle.focus.outline*` | `focus.ring` (2px) | UI Design System / React Aria | Migrated to DS recipe in Task 6 |
| `pane-tree.module.css` | `.separator:focus-visible` | `outline-offset` | `var(--clay-ds-pane-split-tree-default-handle-focus-outline-offset, -1px)` | Non-Color UI Design-System Recipe | `paneSplitTree.default.handle.focus.outlineOffset` | `-1px` | UI Design System | Migrated to DS recipe in Task 6 |

---

### 4.17 `frontend/src/styles/global.css` and `styles/tokens.css`

| File | Selector / State | Property | Current Value | Classification | Target Recipe Key / Theme Role | Fallback Source | Owner | Migration Plan |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `global.css` | `*` | `box-sizing` | `border-box` | Browser Compatibility Rule | n/a | Global Reset | React Host | Retained (global reset) |
| `global.css` | `body` | `background` | `var(--clay-surface-main)` | Active Content-Theme Color Role | `surface.main` | Active Theme | Content Theme | Retained / verified |
| `global.css` | `body` | `color` | `var(--clay-text-primary)` | Active Content-Theme Color Role | `text.primary` | Active Theme | Content Theme | Retained / verified |
| `global.css` | `body` | `font-family` | `var(--clay-font-ui)` | User Typography | `font.ui` | User Profile | User Typography | Retained (user typography) |
| `global.css` | `:focus-visible` | `outline` | `2px solid var(--clay-focus-ring)` | Accessibility Override | `focus.ring` (2px solid) | React Aria | Retained (global focus ring) |
| `global.css` | `::selection` | `background` / `color` | `var(--clay-surface-selected)` / `var(--clay-text-primary)` | Active Content-Theme Color Role | `surface.selected` / `text.primary` | Active Theme | Content Theme | Retained / verified |
| `global.css` | `*` | `scrollbar-color` | `var(--clay-surface-scrollbar) var(--clay-surface-scrollbar-track)` | Active Content-Theme Color Role | `surface.scrollbar` / `track` | Active Theme | Content Theme | Retained / verified |
| `tokens.css` | `:root` | `--clay-*` (45 core tokens) | Hex / px fallback values | Active Content-Theme Color Role / Tokens | Core fallback | Bootstrap snapshot | Rust Theme Resolver | Retained as instant bootstrap fallback |
| `tokens.css` | `:root` | `--clay-ds-*` (221 recipe variables) | Default recipe fallbacks | Non-Color UI Design-System Recipe | Baseline fallbacks | Bootstrap snapshot | Rust Design System Resolver | Retained as instant bootstrap fallback |

---

### 4.18 Inline Style Sites in TSX Files

| File | Component / Line | Inline Style | Classification | Owner | Migration Plan & Rationale |
| --- | --- | --- | --- | --- | --- |
| `frontend/src/app/layout/tab-bar.tsx` | `TabList` (L54) | `style={{ display: "contents" }}` | Structural Layout | React Shell | Retained: React Aria TabList wrapper requires display: contents to inherit flex container sizing without adding DOM layout box. |
| `frontend/src/app/layout/working-area.tsx` | `Separator` (L31) | `style={{ width: "var(--clay-dimension-border-hairline, 1px)" }}` | Structural Layout | React Shell | Retained: react-resizable-panels horizontal separator width constraint. |
| `frontend/src/routes/fixture.tsx` | `SplitsFixture` (L186) | `style={{ height: "100%" }}` | Structural Layout | Fixture Host | Retained: Test/review fixture container height. |
| `frontend/src/sdui/registry.tsx` | `PackageComponent` (L90, L107, L119, L135, L169, L187, L196, L204) | `style={componentStyle(node)}` | Non-Color UI Design-System Recipe / Color | SDUI Bridge | Retained: Host maps typed, validated SDUI style variables (`node.style`) to CSS custom-property references (`var(--clay-*)`). Denies raw values. |
| `frontend/src/shell/PaneTree.tsx` | `Separator` (L173) | `style={orientation === "horizontal" ? { width: "var(--clay-dimension-border-thin, 2px)" } : { height: "var(--clay-dimension-border-thin, 2px)" }}` | Structural Layout | React Shell | Retained: react-resizable-panels dynamic horizontal/vertical separator thickness constraint. |

---

## 5. Security & Boundary Audit

The audit confirms the following security and isolation invariants:

1. **No Raw Package CSS:** No package contribution can supply CSS strings, `@import`, `@keyframes`, or class rules. All rendering occurs through host-authored CSS Modules consuming closed, host-generated `--clay-ds-*` and `--clay-*` custom properties.
2. **No Arbitrary DOM Selectors:** Package design systems supply structured data (`values`, `recipes`) keyed by closed 4-tuple identifiers (`component.variant.slot.state`).
3. **No Literal Color Injections:** Color fields in design-system recipes must reference active-theme color roles. Any literal hex, rgb, hsl, or named color causes immediate validation rejection in Rust (`ThemeColorRef::parse`) and TypeScript (`formatColorRole`).
4. **No Asset or Remote URL Fetching:** No property family accepts `url()`, network paths, fonts, or image URLs.
5. **No Event Handler / Script Injection:** Design systems declare static visual values only; no callbacks, script execution, or DOM mutation authority is granted.
6. **No Stacking / Z-Index Bypass:** Z-index levels are fixed host constants (`z.base: 0`, `z.panel: 10`, `z.overlay: 20`, `z.modal: 40`, `z.tooltip: 50`).

---

## 6. Migration Status & Implementation Evidence

1. **Task 2 (Shared host recipe selectors and state mapping):** Completed.
   - Standardized host slot attributes (`data-clay-component`, `data-clay-slot`, `data-variant`) across primitives in `frontend/src/components/recipe-attributes.ts` and React SDUI registry (`frontend/src/sdui/registry.tsx`).
2. **Task 3 (Migrate controls and package-facing components):** Completed.
   - Migrated `button.module.css`, `controls.module.css`, `modal.module.css`, `text-field.module.css`, `chrome.module.css`, and `registry.module.css` to closed `--clay-ds-*` recipe properties with core baseline fallbacks.
   - Prohibited raw filters (`brightness()`) and color leaks.
3. **Task 4 (Migrate shell, layout, transient, product, and editor-chrome surfaces):** Completed.
   - Migrated `shell.module.css`, `pane-tree.module.css`, `package-workspace.module.css`, `command-centre.module.css`, `chat.module.css`, `settings-panel.module.css`, `editor.module.css`, `fixture.module.css`, and `renderer.module.css` to closed `--clay-ds-*` recipe variables.
   - Preserved CodeMirror syntax highlighting tokens under content-theme ownership.
4. **Task 5 (Accessibility and fallback layers):** Completed.
   - Centralized `@media (forced-colors: active)`, `@media (prefers-reduced-motion: reduce)`, `@media (prefers-reduced-transparency: reduce)`, and `@supports not (backdrop-filter: blur(1px))` in `frontend/src/styles/global.css`.
   - Verified server-side and adapter effect bounds (max blur 32px, max duration 1000ms, max border width 8px).
5. **Task 6 (Automated migration and non-regression checks):** Completed.
   - Added `plan103_css_module_literal_deny_scan` and `plan103_fallback_recipes_have_tokens_css_definitions` to `tests/package_ui_conformance.rs`.
   - Verified full Linux test suite and budget passes.
6. **Task 7 (Visual screenshot & accessibility review):** Completed.
   - Generated AT-SPI accessibility dumps and screenshots for 6 representative desktop states in `.impeccable/review/plan-103/` with findings documented in `findings.md`.

