# @clay/design-glass

The official luminous frosted glass reference UI design system for Clay with solid active-theme fallbacks.

---

## 1. Overview and Design Philosophy

`@clay/design-glass` provides a modern, translucent frosted-glass aesthetic for Clay desktop surfaces while strictly preserving developer productivity, text legibility, and high-framerate scrolling.

### Key Visual Characteristics
- **Frosted Translucency:** Subtle translucent background fills (`0.7`–`0.85` opacity) combined with hardware-accelerated `backdrop-filter: blur(...)` and saturation boosting (`1.2`–`1.4`).
- **Inner Specular Highlights:** Crisp 1px top edge inner highlights (`box-shadow: inset 0 1px 0 ...`) simulating real glass refraction.
- **Refined Geometry:** Harmonious rounded corners (4px for status items/tags, 6px for buttons/inputs/tabs, 8px–10px for panels/overlays, 14px for modals, 999px for pill badges).
- **Multi-Layer Diffuse Shadows:** Deep, soft-focus ambient shadows for natural optical depth.
- **Fluid Micro-Interactions:** Smooth 150ms transitions and gentle hover lifts (`hover-lift`).

---

## 2. Performance & Scrolling Discipline

To maintain Clay's strict performance guarantees (60fps scrolling and sub-millisecond typing latencies):
- **Solid Editor Canvas:** The main document text editor, line number gutter, and scroll track remain 100% opaque (`backgroundOpacity: 1.0`, `backdropBlur: 0.0`). No GPU-intensive backdrop filters are applied over active document typing lines or large scrolling viewports.
- **Bounded Glass Surface Footprint:** Backdrop blur is strictly localized to elevated overlays, floating dropdown menus, modal dialogs, and workspace shell chrome.

---

## 3. Solid Active-Theme Fallbacks & Color Authority

In adherence to Clay's core visual architecture:
- **Zero Hardcoded Colors:** This package contains zero hex, RGB, HSL, or named CSS color literals. All background, border, text, outline, and highlight values resolve dynamically from the active content theme (`@clay/theme-gruvbox-material-dark`, `@clay/theme-modus-operandi`, custom themes, etc.).
- **Automatic Solid Fallback:** If `backdrop-filter` is unsupported or disabled, or when `prefers-reduced-transparency: reduce` is enabled at the OS level, all surfaces cleanly fall back to solid active-theme background colors with WCAG 2.1 AA contrast intact.

---

## 4. Activation and Configuration

To activate `@clay/design-glass` in your `~/.clay/init.js`:

```javascript
import { theme } from "clay:theme";

// Set Glass as the active UI design system
theme.setDesignSystem("@clay/design-glass");
```

---

## 5. Accessibility and Platform Invariants

- **Reduced Transparency (`prefers-reduced-transparency: reduce`):** Translucency collapses to solid theme-role fills without visual artifacts.
- **Reduced Motion (`prefers-reduced-motion: reduce`):** Tactile lift and hover transitions collapse to `0ms` (instant state updates).
- **Forced Colors (`forced-colors: active`):** Structural borders and high-contrast focus rings conform to system palette contracts.
- **Focus Rings:** 2px high-contrast focus rings conform to WCAG 2.1 AA requirements.

---

## 6. Recipe Inventory

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
