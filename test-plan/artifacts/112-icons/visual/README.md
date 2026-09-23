# Plan 112 Task 11 — Visual Screenshot and Accessibility Review Index

Date: 2026-09-07 · Linux (Wayland/GNOME) · Real matched server+client (`target/debug/clay server` + `clay client`, private mode-700 config roots via `scripts/capture-ui-review.sh`). Review methodology per `.agents/skills/clay-execution/references/planning-checklist.md`; capture per `scripts/capture-ui-review.sh` (xdg-desktop-portal screenshots cropped to the Clay window + AT-SPI accessibility dumps).

## Capture matrix (all PASS, app-only screenshots)

| Artifact | Pack | Design system | Theme | Window | States visible |
| --- | --- | --- | --- | --- | --- |
| `regular-core-light/` | Regular | `@clay/core` (Neobrutal) | light (Modus Operandi) | 900×600 logical | SDUI panel (branch/status/eye/folder/file icons inline), editor action row icon buttons (Save primary+focus ring, Reload, Close), Open text button, tab strip, document bar |
| `duotone-core-dark/` | Duotone | `@clay/core` (Neobrutal) | dark (Gruvbox Material Dark) | 900×600 | same surfaces; duotone shade layers visible (soft-filled folder/file vs Regular pure outline) — pack difference confirmed visible, not just metadata |
| `fallback-core-dark-large/` | none (bundled fallback subset, zero init.js icon lines) | `@clay/core` | dark | 900×600 + UI typography 24px | icons render from fallback subset with zero config; glyphs track large typography after task-11 fix; generous targets, no clipping |
| `coding-agent-fallback/` | none (fallback) | core default | dark | 900×600 | file browser row `review.rs` with inline file icon (task 9 migration live) |

Each directory: `screenshot.png` (app-cropped), `accessibility.txt` (AT-SPI dump), `runtime-tree.txt` (delivered snapshot evidence), `metadata.txt`, `instructions.md`, `review.status` (PASS).

## Accessibility findings (from AT-SPI dumps + jsdom structural tests)

- PASS: icon-only controls expose correct roles/names — decorative icons are `aria-hidden`; buttons keep full accessible names ("Save", "Close Workspace tab", …) regardless of pack (AT-SPI dump: `button Toggle Preview`, `list item src/ folder row` with icon-free names).
- PASS (structural, jsdom `icons.test.tsx` 17/17): keyboard activation via Enter/Space, tooltip opens on keyboard focus (`user.tab()`), tooltip dismisses, disabled gating, DOM identity across pack switches, fallback label rendering when geometry missing.
- PASS: unknown icon keys render stable empty decorative slots; text labels always stand alone (fallback-core-dark-large uses zero-config fallback for every glyph).
- PASS (structural): glyph contrast rides `--clay-text-icon` (theme text role, WCAG-AA-validated themes); icons carry no colors of their own (task 10 "embeds no colors" test).

## UNRESOLVED (interactive legs) — exact blocker

- Keyboard tab-order, tooltip-on-focus, and Enter/Space activation could not be exercised in the live app: **Wayland session with no input backend — `wtype`, `xdotool`, and `ydotool` are not installed, no `ydotoold` socket, XDG RemoteDesktop portal input not enabled** (`get_app_state` readiness: `can_send_development_input: false`). Pointer synthesis is equally unavailable, so hover tooltips and pointer-driven pack swaps were not exercised live.
- Live pack-swap-while-focused: not pointer-verifiable; covered structurally instead (Task 5 reload persistence tests, Task 7 DOM-identity/focus-retention jsdom test, and the scripted captures proving `setIconPack` delivers through the live RuntimeStateSnapshot reload path).
- Third-party partial-pack live capture: `clay package add` requires pnpm, which is not installed — recorded blocker; fallback leg captured instead (unmatched-key fallback behavior proven by Task 7/10 tests).
- `setDesignSystem("@clay/design-glass")` live capture remains blocked by the documented plan-110 task-18 runtime deadlock (pre-existing, reproduced today: glass fixture UNRESOLVED, core fixture PASS). Icon×Glass visual leg covered structurally (Task 10 coexistence test).
- forced-colors / reduced-motion live states: no OS-level toggle available without input; structural `@media` rules exist in `icon.module.css` / `tooltip.module.css`.

## Defects found and fixed in this review (one batched pass)

1. **Icons missing in live app** — root cause: stale `clay-desktop` binary embedding a pre-Task-7 frontend dist (`cargo build --bin clay` does not rebuild the desktop binary). Fix: `npm --prefix frontend run build` + `cargo build --manifest-path src-tauri/Cargo.toml --bin clay-desktop` (+ `touch src-tauri/build.rs` to re-run tauri asset embedding). Not an icon-code defect; build-process hazard worth remembering for future UI plans.
2. **Icon glyphs stacked above their label** in SDUI labels/list rows — `.icon { display: block }` forced a line break. Fixed: `display: inline-block` (works in flex parents unchanged); confirmed inline rendering in all four captures.
3. **Glyphs did not track increased UI typography** — 16px `--clay-dimension-icon-size` next to 24px text looked undersized. Fixed: `inline-size: max(var(--clay-dimension-icon-size, 16px), 1em)` (token stays the floor; typography can grow the glyph). Confirmed in `fallback-core-dark-large/` confirmation capture.
4. **Capture tooling hardening** (`scripts/capture-ui-review.sh`): unresolved reviews now retain `server.partial.log` (failure evidence); icon fixtures registered (regular-light / duotone-dark / fallback-large) with document + tree wait patterns; `loading.txt` created for icon fixtures.
5. Review-fixture defect (not shipped code): SDUI actions referencing unregistered `markdown.togglePreview` command rejected the whole tree (`sdui.invalid_action`) — fixtures now use built-in `workspace.refresh`; behavior is correct-by-design (action authority validation).

## State-to-artifact index

| State | Artifact | Result |
| --- | --- | --- |
| Regular pack × light × Neobrutal, editor open | `regular-core-light/screenshot.png` + `accessibility.txt` | PASS |
| Duotone pack × dark × Neobrutal | `duotone-core-dark/screenshot.png` + `accessibility.txt` | PASS |
| Fallback (zero config) × dark × large typography | `fallback-core-dark-large/screenshot.png` + `accessibility.txt` | PASS (after fixes 2+3, confirmation capture) |
| Fallback file-browser row icons | `coding-agent-fallback/screenshot.png` + `accessibility.txt` | PASS |
| Glass DS × icons | UNRESOLVED — plan-110 task-18 deadlock (pre-existing) | recorded above |
| Keyboard/hover interactive legs | UNRESOLVED — no input backend (exact blocker above) | recorded above |

No unrelated desktop content retained; captures are app-window-cropped with fixture-only documents in scratch workspaces (no secrets). One earlier capture containing a terminal overlay was deleted and retaken.