# Plan 104 — Visual Screenshot, Accessibility & Design-System Review Findings

Review date: 2026-08-31.
Environment: Real Linux client (`target/debug/clay` server + desktop client on WebKitGTK / Wayland), private mode-700 config/data/socket roots, fixture-only documents.
Capture tooling: `xdg-desktop-portal` screenshot + Python GI AT-SPI tree dumps (`accessibility.txt`), including status-bar text and landmark structure.

---

## 1. States Reviewed (All PASS)

| State | Directory | Verified Surface / Interaction & Design-System Mode | Status |
| --- | --- | --- | --- |
| **Default Shell** | `default/` | Base shell fallback styles, status `Connected`, clean editor canvas. | **PASS** |
| **Neobrutal Design System (Dark)** | `design-neobrutal/` | Default Neobrutal design system (`@clay/design-neobrutal`) under `@clay/theme-gruvbox-material-dark`. Sharp 0px radii, 1px structural borders, 2px hard offset box-shadows, 0px backdrop blur, and snappy 100ms transitions. | **PASS** |
| **Glass Reference System (Dark)** | `design-glass/` | Luminous Glass design system (`@clay/design-glass`) under `@clay/theme-gruvbox-material-dark`. Smooth 6px–14px radii, translucent control/panel opacities, 8px–24px backdrop blurs, 1.2–1.4 backdrop saturation, 1px specular inner highlights, and solid editor/scroll canvas (0px blur). | **PASS** |
| **Light Theme Orthogonality** | `design-system-light/` | UI under `@clay/theme-gruvbox-material-light`; full palette recoloring across surfaces with zero stale dark color values, proving theme color authority over design-system geometry/materials. | **PASS** |
| **Large Typography** | `large-typography/` | User-configured large typography profile (`ui: 15.0px`, `editor: 16.0px`); scaled proportions, preserved hit targets, and layout stability across panes. | **PASS** |
| **Loading SDUI Delivery** | `loading/` | Host-published "Loading review" panel with `--clay-ds-panel-*` recipe variables; tree delivered cleanly via `RuntimeStateSnapshot`. | **PASS** |
| **Error Recovery** | `error/` | Valid boot followed by reload-time invalid theme selection `setTheme("@clay/does-not-exist")`: server logs `clay server runtime reload failed [packages.not_installed]`, client stays connected on last valid generation, status bar displays sanitized `JavaScript runtime evaluation failed.` with zero leaked paths. | **PASS** |
| **Disconnection Recovery** | `recovery/` | Server stopped after connection: alert role `Session lost` with `Reconnect session` button; no partial or broken visual layout. | **PASS** |

---

## 2. Accessibility & Semantic Invariants

- **Host Semantic Ownership**: All accessibility roles and names (`document web`, `page tab list`, `landmark`, `button`, `list box`, `list item`, `entry`, `status bar`, `dialog`) remain host-authored and strictly preserved across design-system switches.
- **Strict Color Authority**: Normal-rendering colors for backgrounds, foregrounds, borders, selections, and focus rings resolve through active-theme CSS custom properties (`var(--clay-*)`). Design systems contribute non-color geometry, padding, border radii, border styles, and materials without polluting color authority.
- **Forced Colors & Reduced Motion**: Global rules in `global.css` ensure that OS forced-colors and `prefers-reduced-motion` user preferences take precedence over custom shadows, blurs, and animations.
- **Interactive State Continuity**: Keyed text inputs, disclosure states, and editor contents persist across runtime design system activations and reconfigurations without DOM remounting.
- **Focus Rings**: 2px solid focus rings with `var(--clay-focus-ring)` outline color remain active and visible across all interactive components in both design systems.

---

## 3. Impeccable Quality & Conformance Status

- Static scan across all migrated CSS modules, TypeScript components, Rust structs, and package manifests: **0 errors, 0 lint warnings, 100% type-checked and format-checked**.
- Conformance test suite (`frontend/src/test/design-system-conformance.test.tsx` and `tests/package_ui_conformance.rs`): **Passed**.
