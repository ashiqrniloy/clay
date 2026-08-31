# Plan 103 — Visual Screenshot & Accessibility Review Findings

Review date: 2026-08-30. Real Linux client (`target/debug/clay` server + client),
private mode-700 config/data/socket roots, fixture-only documents. Capture:
xdg-desktop-portal screenshot + Python GI AT-SPI dump (`accessibility.txt`,
including status-bar text). Each state directory: `screenshot.png` (window crop),
`accessibility.txt`, `metadata.txt`, `instructions.md`, and `runtime-tree.txt` where applicable.

## States Reviewed (All PASS)

| State | Directory | Verified Surface / Interaction |
| --- | --- | --- |
| Default fallback shell | `default/` | Base shell fallback styles, no active design-system overrides, status `Connected`. |
| Loading SDUI delivery | `loading/` | Host-published "Loading review" panel with migrated `--clay-ds-panel-*` recipe variables; tree delivered cleanly via RuntimeStateSnapshot. |
| Explicit activation (dark) | `design-system/` | `setDesignSystem("@clay/core")` under `@clay/theme-gruvbox-material-dark`; panel, primary action button, enabled list row (host-owned action), disabled list row (dimmed, non-interactive, no action authority), and editor view. |
| Explicit activation (light) | `design-system-light/` | Same tree under `@clay/theme-gruvbox-material-light`; full palette recoloring across surfaces with zero stale dark color values, proving theme color authority over design-system geometry/materials. |
| Invalid-selection recovery | `error/` | Valid boot followed by reload-time invalid theme selection `setTheme("@clay/does-not-exist")`: server logs `clay server runtime reload failed [packages.not_installed]`, client stays connected on last valid generation, status bar displays sanitized `JavaScript runtime evaluation failed.` with zero leaked paths. |
| Disconnection recovery | `recovery/` | Server stopped after connection: alert role `Session lost` with `Reconnect session` button; no partial or broken visual layout. |

## Accessibility & Semantic Invariants

- **Full Semantic Ownership:** All accessibility roles and names (`document web`, `page tab list`, `landmark`, `button`, `list box`, `list item`, `entry`, `status bar`) remain host-authored and strictly preserved across design-system switches.
- **Theme Color Authority:** Normal-rendering colors for backgrounds, foregrounds, borders, selections, and focus rings resolve through active-theme CSS custom properties (`var(--clay-*)`). Design systems customize geometry, padding, border radii, border styles, and materials without polluting color authority.
- **Forced Colors & Reduced Motion:** Global fallback rules in `global.css` ensure that OS forced-colors and reduced-motion user preferences take precedence over custom animations and styling.
- **Interactive State Preservation:** Keyed text inputs, disclosure states, and editor contents persist across runtime design system activations and reconfigurations.

## Impeccable Detector Status

- Static scan across all migrated CSS modules and TypeScript components: 0 errors, 0 lint warnings, 100% type-checked and format-checked.
