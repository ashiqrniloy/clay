# Plan 102 — Visual screenshot & accessibility review findings

Review date: 2026-08-30. Real Linux client (`target/debug/clay` server + client),
private mode-700 config/data/socket roots, fixture-only documents. Capture:
xdg-desktop-portal screenshot + Python GI AT-SPI dump (`accessibility.txt`,
including status-bar text). Each state directory: `screenshot.png` (window crop
1280×1151), `accessibility.txt`, `metadata.txt`, `instructions.md`,
`runtime-tree.txt` where applicable.

## States reviewed (all PASS)

| State | Directory | Verified |
| --- | --- | --- |
| Default fallback shell | `default/` | Welcome shell, no design-system contribution, status `Connected`. |
| Loading SDUI delivery | `loading/` | Host-published "Loading review" panel replaces raw theme overrides; tree delivered via RuntimeStateSnapshot. |
| Explicit activation (dark) | `design-system/` | `setDesignSystem("@clay/core")` with `@clay/theme-gruvbox-material-dark`; panel, primary action, enabled list row (host-owned action), disabled list row (no action authority), editor view. Disabled row renders dimmed and non-interactive. |
| Explicit activation (light) | `design-system-light/` | Same tree under `@clay/theme-gruvbox-material-light`; full palette switch with no stale dark values — two materially different active themes over one design system. |
| Invalid-selection recovery | `error/` | Valid boot, then reload-time `setTheme("@clay/does-not-exist")`: server logs `clay server runtime reload failed [packages.not_installed]`, client stays connected on the last valid generation, status bar shows sanitized `JavaScript runtime evaluation failed.` — no paths, no package internals, shell remains usable. |
| Disconnection recovery | `recovery/` | Server stopped after connect: `Session lost` / `connection closed` alert (role alert) with `Reconnect session` button; no partial or stale package-controlled content. |

## Accessibility observations

- Roles/names present per state: frame `Clay`, page tab `Workspace`, workspace
  landmark, SDUI panel/button/list box/list items with host-authored labels,
  editor landmark, status bar. Alert and status roles correct in error/recovery
  states.
- Package-controlled values are limited to theme variables (colors/typography);
  semantics (roles, names, actions) remain host-authored — no
  package-controlled semantics observed.
- Focus order/visibility could not be driven (keyboard input synthesis
  unavailable in this environment); component-level focus/disabled/invalid
  states remain covered by frontend conformance/Vitest suites. The disabled
  list row is verified visually in both design-system captures.
- Modal and completion captures (`ui-review-command-centre`, `ui-review-completion`,
  `ui-review-rust`) require interactive input and remain interactive-capture
  fixtures; not exercised here. Layout captured at the harness's fixed 900×600
  logical size only (no narrow/wide sweep).

## Performance observations

- Design-system activation and theme switching settle to a fully applied
  snapshot in captured end states (no partial variable application visible);
  switching exercised through the live RuntimeStateSnapshot reload path.
- No editor interaction stalls observed during capture windows.

## Harness changes made for this review

- `scripts/capture-ui-review.sh`: loading wait fixed to the delivered tree
  marker; error fixture now boots valid and breaks on reload (exercises the
  sanitized diagnostic + fallback generation instead of a failed first boot);
  AT-SPI probe skips stale registrations whose owning process is gone
  (orphaned windows previously shadowed the live window); new
  `ui-review-design-system` / `ui-review-design-system-light` fixtures.
- New fixtures: `tests/fixtures/configuration/ui-review-design-system/init.js`,
  `tests/fixtures/configuration/ui-review-design-system-light/init.js`;
  `tests/fixtures/configuration/ui-review-error/init.js` (valid baseline).

## Impeccable detector

- One pass over changed frontend targets (components, sdui, layout,
  use-clay-session, design-system store/adapter/types, tokens.css):
  `detector.json` = 0 findings (`detector.status`, `detector.stderr`).
