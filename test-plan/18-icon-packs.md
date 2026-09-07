# 18 — Icon packs (Plan 112)

Configurable UI icon packs: bounded host-validated vector geometry contributed
by first-party packages (`@clay/icons-phosphor-regular`,
`@clay/icons-phosphor-duotone`), zero-config bundled fallback subset,
user-global selection via `setIconPack` (load ≠ select), semantic icon
references on SDUI/package surfaces, and the icon-only control contract
(ClayIconButton + ClayTooltip). Icons are inert data: Clay owns rendering,
active-theme color roles are the only color authority, and packs can never
ship colors, CSS, scripts, or URLs.

- **API reference:** `docs/reference/clay-js-api/theme/set-icon-pack.md`
- **Authoring guide:** `docs/reference/icon-packs.md`
- **UI components:** `docs/reference/ui-components.md` (Plan 112 section)
- **Primitive review:** `docs/development/icon-pack-primitive-review.md`
- **Canonical example:** `examples/config/init.js` (section 2 icon block)
- **Plan 112 artifacts:** `test-plan/artifacts/112-icons/` (`visual/` task 11
  captures, `example-launch/` task 15 isolated launches, `automated/` gates)
- **Contract:** recipes reuse existing `button.icon`, `dropdown.indicator`,
  `collapse.chevron`, `iconSlot` slots; core tokens `dimension.icon.size` /
  `text.icon`; icon-slot CSS consumes core tokens only (no `--clay-ds-*`).

---

## Setup

Icons render from the active pack only; no extra setup beyond a scratch
config. For isolation use the task 15 recipe (mode-700 scratch roots,
`$HOME/.config/clay` copy of `examples/`), or run the developer profile with
the canonical example. Never adopt packages on a real profile for these steps.

---

## Steps

| # | Action | Expected |
|---|--------|----------|
| ICON-01 | Launch with **no** `setIconPack` call anywhere in config (zero-config default) | Workspace renders normally with the bundled Regular fallback subset active; editor action row and SDUI icons render from fallback geometry; no `configuration failed` diagnostics; `init.js` line count does not matter |
| ICON-02 | Launch with the canonical example (`setIconPack("@clay/icons-phosphor-regular")`) | Connected ~1.3s; file browser shows folder/file kind icons before names, parent row is an up-arrow; editor action row shows Save/Reload/Close as icon-only buttons with `Open` still a text button beside the path input; rust mode + theme unchanged |
| ICON-03 | Swap the selection to `@clay/icons-phosphor-duotone` (edit the copy, relaunch) | Same layout; duotone shade layers visible on glyphs (heavier silhouettes); rendered pixels differ from the Regular run on identical state; theme, design system, and appearance unchanged |
| ICON-04 | While running, edit the selection in the copied config (good → broken → restored; watcher reload) | Broken third-party selection: reload rejected fail-closed — one bounded sanitized diagnostic (`clay server runtime reload failed [theme.load_failed]`), previous working generation preserved, UI stays usable; restore: reload succeeds with no new failures, shell + open document recover |
| ICON-05 | Select a never-installed third-party specifier (`@vendor/unloaded-icons`) at startup | Startup fails fast: bounded `configuration failed [theme.load_failed]`; nothing installed or adopted (selection grants no package authority); app still launches into the usable fallback-rendering shell |
| ICON-06 | Third-party pack with own-prefixed keys (adoption path) | Third-party packs may only contribute own-prefixed keys and render through the identical host controls; core-key impersonation and raw `svg` fields are rejected at record validation (hostile fixture). **Live adoption leg UNRESOLVED on this host:** `pnpm` is absent, so `clay package add` cannot run; covered structurally by the task 5/13 suites (valid-partial + hostile fixtures) |
| ICON-07 | Git/markdown semantic status icons: git branch label, dirty/refresh status, markdown Toggle Preview button and enabled-preview status label | Glyphs match semantics (`git.branch`, `status.success`/`status.warning`/`status.error`, `preview.toggle`); text labels always carry full meaning independently — a label whose key resolves to no geometry keeps its text |
| ICON-08 | Icon-only control contract: hover the Save/Reload/Close composer and editor action buttons, then Tab to them | Accessible names carry meaning (`Save`, `Reload`, `Close`, `Send`, `Stop`); tooltip opens on hover and on keyboard focus with label + shortcut; visible keyboard focus ring; hit target ≥ 24px. **Interactive hover/focus legs UNRESOLVED on this host** (no input-synthesis backend); names/roles verified in AT-SPI dumps, tooltip + focus in jsdom suites |
| ICON-09 | Retained text labels: agent tab strip, approval Allow/Deny, setup/model choices, destructive confirmations, empty-state discovery actions (`Open file`, `Resume session`, `Search sessions…`) | These controls keep visible text and have no icon glyph; icon-only conversion never applies to text-bearing safety or discovery surfaces |
| ICON-10 | Screen-reader pass on a live capture: inspect the AT-SPI tree | Decorative icons are `aria-hidden`/presentation (not in the a11y tree as images); icon-only buttons expose their label as the accessible name (e.g. `Save`, listed list-item names like `src/ folder row`); status labels read as text |
| ICON-11 | No-network rendering check: run all captures/launches with the workspace offline (no network beyond localhost socket) | Icons always render — geometry is inlined in package manifests / compiled inventory / wire snapshots; packs cannot reference URLs (denied at validation), so no remote asset path exists |
| ICON-12 | Responsive + performance feel: resize window narrow, enable large UI typography (24px), type/scroll during a pack swap | Icons scale with typography (floor `--clay-dimension-icon-size`, `max(…, 1em)`); no reflow clipping at 780px width; pack swap does not unmount controls or move focus; launch/switch latency stays ~1s-scale with bounded server logs (no reload loops) |

---

## Execution record (Linux, 2026-09-07, current `clay-desktop` build)

Executed on the task 15/16 build (frontend dist re-embedded) via the isolated
launch harness of task 15 and the task 11 capture tooling. Artifacts:
`test-plan/artifacts/112-icons/` (`visual/`, `example-launch/`, `automated/`).

| Step | Result | Evidence |
|---|---|---|
| ICON-01 | PASS | `visual/fallback-core-dark-large/` (zero-config fallback renders at 24px typography); `automated/` gates |
| ICON-02 | PASS | `example-launch/default/` (screenshot + AT-SPI + 25-line bounded server log, zero diagnostics); `visual/regular-core-light/` |
| ICON-03 | PASS | `example-launch/duotone/` + `visual/duotone-core-dark/` (shade layers visible; 36 pixel rows differ vs default on identical state) |
| ICON-04 | PASS | `example-launch/recovery/` (fail-closed break + clean restore, `dump-broken.txt`/`dump-restored.txt` show the open document across the swap) |
| ICON-05 | PASS | `example-launch/unloaded/` (bounded sanitized diagnostic at startup AND reload, usable UI, fallback icons) |
| ICON-06 | UNRESOLVED (live) / PASS (structural) | No `pnpm` on host → adoption cannot run live; task 5/13 suites cover valid-partial + hostile fixtures |
| ICON-07 | PASS | `visual/regular-core-light/` (branch/check/eye glyphs), package suites (git `status.js`, markdown `sdui.js` semantics) |
| ICON-08 | UNRESOLVED (interactive) / PASS (structural) | No keyboard/pointer synthesis backend (no wtype/xdotool/ydotool; Wayland); AT-SPI names verified in `example-launch/*/accessibility.txt`; tooltip/focus/hit-target in jsdom icon suites (task 7/8) |
| ICON-09 | PASS | Task 8 jsdom suites (tabs/approvals/empty-state keep text); visible in `example-launch/default/` screenshot |
| ICON-10 | PASS | `example-launch/*/accessibility.txt` + `dump-*.txt` (decorative icons absent from tree; named controls present) |
| ICON-11 | PASS | All task 11/15 captures ran on localhost-only sockets; URL-bearing pack data is denied at validation (record suites) |
| ICON-12 | PASS | `visual/fallback-core-dark-large/` (icons scale at 24px), `example-launch/` metadata (socket ≈ 0.11s, connected ≈ 1.3s, bounded logs) |

## Known ceilings (not bugs)

- **No input synthesis on this host** (no `wtype`/`xdotool`/`ydotool`;
  Wayland; `/dev/uinput` denied): hover/tooltip-on-focus/keyboard-only legs
  stay UNRESOLVED and are covered by jsdom suites and AT-SPI structure.
- **No `pnpm` on this host**: third-party adoption cannot run live; the
  unloaded-specifier legs exercise the identical fail-closed selection path.
- **Transient first-reload flake (1 of 4 task 15 runs)**: one no-op reload
  recorded a `theme.load_failed` evaluation failure with an unchanged valid
  selection; the next watcher event re-evaluated cleanly and the app stayed
  usable. Not reproduced in other runs or by the isolated production-path
  probe — investigate on recurrence (server reload/worker cache path).
- `setDesignSystem("@clay/design-*")` inside `init.js` still deadlocks the
  runtime (Plan 110 task 18): pack captures use `@clay/core` design system.
