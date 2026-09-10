# @clay/design-neobrutal

The official, default visual UI design system for Clay.

---

## 1. Overview and Design Philosophy

`@clay/design-neobrutal` provides a restrained, utilitarian Neobrutal design system tailored for intensive, long-session desktop software development.

### Key Visual Characteristics
- **Mathematical 90° Corners:** Strict `0px` border radius across buttons, inputs, tabs, panels, popovers, dialogs, and toolbars.
- **Crisp Structural Borders:** 1px solid structural framing borders; 2px prominent borders on active tabs, modal dialogs, and focused controls.
- **Hard Offset Shadows:** Physical hard-edge shadows (e.g. `2px 2px 0px var(--clay-border-strong)`) with `0px` diffuse blur for tactile spatial separation.
- **Mechanical Tactile Feedback:** Controls translate `-1px, -1px` on hover and `+1px, +1px` on active press, collapsing the offset shadow for tactile physical confirmation.
- **Bimodal Density:** Compact density for editor chrome, toolbars, and tab bars; comfortable generous density for modals and configuration forms.
- **Snappy Motion:** 100ms transitions for rapid visual feedback without perception lag.

---

## 2. Content-Theme Color Authority Invariant

In adherence to Clay's core visual architecture:
- **Zero Color Palettes:** This package contains no hardcoded hex, RGB, HSL, or named CSS colors.
- **Semantic Theme Roles:** All surface, text, border, focus, accent, and diagnostic colors resolve dynamically from whichever content theme the user has activated (`@clay/theme-gruvbox-material-dark`, `@clay/theme-modus-operandi`, custom themes, etc.).
- **User-Owned Typography:** Typography scales and font families are managed by the user and editor configuration, completely independent of the design system.

---

## 3. Activation and Configuration

To activate `@clay/design-neobrutal` in your `~/.clay/init.js`:

```javascript
import { theme } from "clay:theme";

// Set Neobrutal as the active UI design system
theme.setDesignSystem("@clay/design-neobrutal");
```

---

## 4. Accessibility and Platform Invariants

- **Forced Colors Mode (`forced-colors: active`):** Structural borders and outlines remain 100% visible while background fills yield to OS system colors (`Canvas`, `ButtonFace`, `Highlight`).
- **Reduced Motion (`prefers-reduced-motion: reduce`):** All tactile translations and transitions collapse to `0ms` (instant state updates).
- **Reduced Transparency (`prefers-reduced-transparency: reduce`):** Surfaces are fully opaque by default, requiring no fallback degradation.
- **Focus Rings:** 2px high-contrast focus rings conform to WCAG 2.1 AA requirements.

---

## 5. Recipe Inventory

This package provides complete recipe declarations for all 25 Clay component and surface kinds:
1. `button` (variants: `default`, `muted`, `primary`, `danger`; states: `rest`, `hover`, `active`, `focus`, `disabled`)
2. `textInput` (slots: `field`, `input`, `label`, `description`, `error`)
3. `dropdown` (slots: `trigger`, `triggerLabel`, `popover`, `list`, `item`)
4. `list` (slots: `root`, `row`)
5. `collapse` (slots: `root`, `header`, `body`)
6. `modal` (slots: `root`, `scrim`, `dialog`)
7. `panel` (variants: `default`, `fixed`, `transient`)
8. `label` (variants: `body`, `title`, `status`, `display`, `section`, `detail`, `caption`)
9. `statusItem` (slots: `root`)
10. `flex` (variants: `row`, `column`)
11. `stack` (slots: `root`)
12. `overlay` (slots: `root`)
13. `portal` (slots: `root`)
14. `scroll` (slots: `root`, `scrollbarTrack`, `scrollbarThumb`)
15. `tab` (slots: `root`)
16. `tabBar` (slots: `root`)
17. `card` (slots: `root`)
18. `badge` (variants: `default`, `accent`, `muted`, `error`, `warning`, `success`)
19. `kbd` (slots: `root`)
20. `tooltip` (slots: `root`)
21. `popover` (slots: `root`)
22. `menu` (slots: `root`, `item`)
23. `commandCentre` (slots: `root`)
24. `chat` (slots: `root`)
25. `editor` (slots: `container`, `gutter`, `activeLine`, `selection`, `matchingBracket`, `findMatch`)
