# 15 — UI design systems (Plans 102, 103 & 104)

End-to-end activation, switching, recovery, component/surface recipe migration,
conformance testing, package security, and accessibility fallbacks for package-contributed UI design systems.

- **API reference:** `docs/reference/clay-js-api/theme/set-design-system.md`, `docs/reference/clay-js-api/settings/set-design-system.md`
- **Configuration semantics:** `docs/reference/clay-js-api/configuration.md`
- **Plan 110 artifacts:** `test-plan/artifacts/110-ui-design-systems/`
- **Public specification:** `docs/reference/ui-design-systems.md`
- **Recipe matrix:** `docs/development/ui-design-system-recipe-matrix.md`
- **CSS Audit:** `docs/development/ui-design-system-css-audit.md`
- **Conformance proof:** `docs/development/ui-design-system-conformance.md`
- **Canonical example:** `examples/init.js` (Theme + appearance section)
- **Visual & A11y Review:** `.impeccable/review/plan-104/`

---

## Steps

| # | Action | Expected |
|---|--------|----------|
| UI-DS-01 | Launch with `init.js` that sets only a theme (no `setDesignSystem` call) | Normal workspace UI in the selected theme; built-in `@clay/core` / `@clay/design-neobrutal` recipes active implicitly; no `runtime.*` diagnostics in server stderr |
| UI-DS-02 | Add `setDesignSystem("@clay/design-neobrutal")` and save; let the watcher reload (~2 s debounce) | Reload completes with no diagnostics; UI switches to the default restrained Neobrutal design system in one visible swap; no partial styles or lingering stale chrome |
| UI-DS-03 | Change the theme again (dark ↔ light) with the selection in place; repeat once | Each save produces exactly one atomic visible switch; no flicker, no unbounded repaint; layout stays stable during the swap |
| UI-DS-04 | Replace the selection with a never-installed specifier (`@vendor/never-installed-ds`) and save | Watcher reload fails: server stderr shows `runtime reload failed [theme.load_failed]`; client stays connected on the PREVIOUS generation; status bar shows the sanitized `JavaScript runtime evaluation failed.` diagnostic (no package names leaked) |
| UI-DS-05 | Select a real bundled package that contributes no design system (`@clay/markdown`) | Selection is rejected (`theme.invalid_design_system` semantics per the API doc); previous generation retained; no partial frontend variables |
| UI-DS-06 | Attempt to select an uninstalled third-party specifier (`@vendor/uninstalled-adopter`) | Startup fails fast: server stderr shows `configuration failed [theme.load_failed]`; nothing is installed or adopted (packages store unchanged — selection grants no package authority) |
| UI-DS-07 | Provenance/trust boundary (structural): select a specifier that would require adoption | Selection never installs, never expands package permissions, never promotes trust; adoption only through the package service's existing enable graph (fail-closed on replaced/revoked targets). Automated: `set_design_system_adopted_third_party_via_init_js`, `set_design_system_rejection_leaves_state_clean`, package loading/revocation suites |
| UI-DS-08 | Raw styling / color authority (structural): a `uiDesignSystem` declaration containing concrete colors or raw CSS | Rejected at package validation; recipes may only reference active-theme color roles. Automated: `plan104_design_system_packages_cover_all_25_components_and_enforce_color_authority`, `plan103_css_module_literal_deny_scan` |
| UI-DS-09 | Revocation/remove fallback (structural): enabled DS package is revoked or removed between generations | Next generation revalidates the selection against enabled records and falls back to default `@clay/design-neobrutal`; no stale recipes render |
| UI-DS-10 | App restart persistence: keep `setDesignSystem("@clay/design-neobrutal")` in `init.js`, fully stop server + client, relaunch | Identical baseline renders after restart with no diagnostics — `init.js` is the only persistence surface; no extra state was written |
| UI-DS-11 | **Plan 103 Component Migration Matrix:** Verify catalog controls (`button`, `textInput`, `dropdown`, `list`, `collapse`, `modal`, `panel`, chrome primitives) | Every control resolves styling exclusively through `--clay-ds-*` recipe properties and active theme color roles (`var(--clay-*)`) across all states (`rest`, `hover`, `active`, `focus-visible`, `disabled`, `invalid`, `selected`) with zero raw filter/color literals |
| UI-DS-12 | **Plan 103 Surface & Layout Migration:** Verify Shell, TabBar, PaneSplitTree, StatusBar, CommandCentre, Chat, and Settings surfaces | Surfaces consume host-owned `--clay-ds-*` variables for panel chrome, headers, separators, dialogs, and transcripts while layout geometry (Grid/Flex/split ratios) remains host-owned |
| UI-DS-13 | **Plan 103 Accessibility — Forced Colors Mode:** Run with OS `@media (forced-colors: active)` | System high-contrast colors (`Canvas`, `CanvasText`, `Highlight`, `HighlightText`, `GrayText`) override custom recipe styling for focus outlines, selections, borders, and disabled controls |
| UI-DS-14 | **Plan 103 Accessibility — Reduced Motion & Transparency:** Run with `@media (prefers-reduced-motion: reduce)` and `@media (prefers-reduced-transparency: reduce)` | Reduced motion collapses all transitions to `0.01ms !important` and `transform: none !important`; reduced transparency disables `backdrop-filter` and enforces solid opaque backgrounds (`1.0` opacity) falling back to `var(--clay-surface-overlay)` |
| UI-DS-15 | **Plan 103 Cross-Theme Recoloring Invariant:** Switch themes under an active design system (e.g. Gruvbox Dark ↔ Light) | All component and surface colors update instantaneously to active-theme color roles with zero geometry shift or stale color retention |
| UI-DS-16 | **Plan 104 Neobrutal Default Design System (`@clay/design-neobrutal`):** Verify 25 component recipes under dark and light themes | Strict 0px radii, 1px structural borders, 2px hard offset box-shadows with 0px blur, 100% solid opacity, and 100ms snappy transitions across all components |
| UI-DS-17 | **Plan 104 Glass Reference Design System (`@clay/design-glass`):** Verify 25 component recipes with solid fallbacks | Frosted translucency (0.7–0.85 opacity), 8px–24px backdrop blur, 1.2–1.4 saturation factor, 1px specular inner highlights, and solid editor canvas invariant (0.0 blur) |
| UI-DS-18 | **Plan 104 Package-Only Replacement & Source Independence:** Verify zero host branching on package specifiers | Host components and styles contain zero `if (ds === "@clay/design-glass")` checks; all styling is driven strictly via generic `--clay-ds-*` variables |
| UI-DS-19 | **Plan 104 DOM & State Continuity:** Switch between Neobrutal and Glass while typing in text field / editor | Zero React component unmounting; active focus, scroll position, and input buffers are preserved continuously across switches |
| UI-DS-20 | **Plan 104 Validation & Package Security Hardening:** Test out-of-bounds parameters and unauthorized capability requests | Rejected at parse/enable time for blur > 32px, duration > 1000ms, border-width > 8px, saturation > 2.0, or raw color injections; third-party packages request 0 permissions and 0 renderer capabilities |
| UI-DS-21 | **Plan 110 Settings-driven selection:** Open the Settings panel and switch the *Design system* dropdown (Core baseline ↔ bundled design package) | Dropdown enumerates the server-provided `ui_choices.design_systems` list (Core baseline always first, then bundled contributors); switching restyles the whole shell in one visible swap with zero component remounting, preserved focus, and an advanced `runtime_generation_id` |
| UI-DS-22 | **Plan 110 Command-centre selection:** Run `settings.setDesignSystem` through the command surface with a valid specifier | Same whole-shell restyle as the Settings dropdown; the command persists the `designSystem` preference and the next snapshot reports the new active design system |
| UI-DS-23 | **Plan 110 Invalid specifier surface:** Send `settings.setDesignSystem` with `@clay/design-unknown` (or any non-design specifier) | Command rejected with an `InvalidArguments` diagnostic before any persistence; no reload is triggered; previous generation retained; nothing installed or enabled |
| UI-DS-24 | **Plan 110 Server-enumerated theme list:** Open the Settings *Theme* dropdown | Lists installed `@clay/theme-*` packages enumerated from the server snapshot (`ui_choices.themes`, sorted) instead of any hardcoded client list |
| UI-DS-25 | **Plan 110 Appearance persistence:** Set *Appearance* to `dark` via Settings, fully quit Clay, relaunch | Appearance preference survives restart: persisted in `preferences.json`, re-applied at startup, snapshot reports the persisted variant |
| UI-DS-26 | **Plan 110 Visible DS × theme differences:** Compare Neobrutal vs Glass on dark and light themes | Clearly distinguishable geometry/materials per design system (0px radii + hard offset shadows vs frosted translucency + specular highlights) under both themes; theme owns colors, DS owns geometry |

---

## Plan 104 Visual & Accessibility Review Record (2026-08-31)

Full review artifacts saved under `.impeccable/review/plan-104/` (generated with `scripts/capture-ui-review.sh`):

| Review State | Capture Directory | Status | Verified Surface & Invariants |
| --- | --- | --- | --- |
| `default` | `.impeccable/review/plan-104/default/` | PASS | Baseline shell fallback without active overrides, status `Connected`, AT-SPI window tree intact. |
| `design-neobrutal` | `.impeccable/review/plan-104/design-neobrutal/` | PASS | Default `@clay/design-neobrutal` package: 0px radii, 1px structural borders, 2px hard offset box-shadows, 0px blur, and snappy 100ms transitions. |
| `design-glass` | `.impeccable/review/plan-104/design-glass/` | PASS | Luminous `@clay/design-glass` reference package: 6px–14px radii, 8px–24px backdrop blur, 1.2–1.4 saturation factor, 1px specular inner highlights, and solid editor canvas invariant (0.0 blur). |
| `design-system-light` | `.impeccable/review/plan-104/design-system-light/` | PASS | Full palette recoloring under `@clay/theme-gruvbox-material-light` with zero stale dark color values, proving theme color authority over design system geometry/materials. |
| `large-typography` | `.impeccable/review/plan-104/large-typography/` | PASS | User-configured large typography profile (`ui: 15.0px`, `editor: 16.0px`); proportional layout scaling and hit targets preserved. |
| `loading` | `.impeccable/review/plan-104/loading/` | PASS | Host-published "Loading review" panel with migrated `--clay-ds-panel-*` variables delivered via RuntimeStateSnapshot. |
| `error` | `.impeccable/review/plan-104/error/` | PASS | Valid startup followed by reload-time invalid theme selection: client stays connected on last valid generation, status bar displays sanitized `JavaScript runtime evaluation failed.` with zero leaked paths. |
| `recovery` | `.impeccable/review/plan-104/recovery/` | PASS | Server stopped after connection: alert role `Session lost` with `Reconnect session` button, clean non-broken layout. |

---

## Plan 110 manual-test-plan execution record (2026-09-06, task 14)

Executed against a freshly rebuilt `target/debug/clay` + `clay-desktop` (the run initially reproduced a client `ArchivedSduiTree` rkyv deserialization failure caused by a stale mixed build — rebuilding both binaries fixed it; keep `cargo build --bins -p clay-desktop` before captures).

Artifacts: `test-plan/artifacts/110-ui-design-systems/` (real-app portal screenshots cropped to the Clay window — the capture script now waits for fixture SDUI trees and never retains full-desktop captures with unrelated host windows).

| Fixture | Result | Evidence/notes |
| --- | --- | --- |
| ui-review-default | PASS | Clean workspace shell, welcome actions, status bar |
| ui-review-design-system | PASS | Explicit `@clay/core` activation (dark): SDUI panel, primary/enabled/disabled states render through core baseline recipes |
| ui-review-design-system-light | PASS | Explicit `@clay/core` activation (light): same recipes under the light theme, theme-owned colors |
| ui-review-error | PASS | Reload-time invalid selection: client stays connected on the previous generation; sanitized `JavaScript runtime evaluation failed.` diagnostic |
| ui-review-design-neobrutal | UNRESOLVED | Blocked by the plan-110 task-18 pre-existing defect: `setDesignSystem("@clay/design-neobrutal")` inside `init.js` evaluation during a live reload deadlocks the JS runtime, so the fixture SDUI tree never appears (`@clay/core` applies instantly — same root cause as the task-10 bisection). Retry after task 18 lands |
| ui-review-design-glass | UNRESOLVED | Same task-18 deadlock |
| Settings/command-centre interactive switching | UNRESOLVED | Documented host ceiling (no `/dev/uinput`, no xdotool/ydotool, no Wayland portal input path); switching logic is pinned by the automated tests listed under UI-DS-21/22 |

Task 14 also added the `ui-review-design-neobrutal-light` / `ui-review-design-glass-light` capture fixtures plus SDUI-tree waits and window cropping to `scripts/capture-ui-review.sh` so the full DS × theme matrix can be captured once task 18 unblocks package activation. No existing step was weakened.

---

## Linux Execution Record (Plans 102, 103 & 104)

| Check | Result | Evidence |
|---|---|---|
| UI-DS-01 | PASS | `default/screenshot.png`: dark Gruvbox workspace, tab bar, status bar; clean server logs |
| UI-DS-02 | PASS | `design-neobrutal/screenshot.png`: default `@clay/design-neobrutal` selection applied cleanly; status `Connected` |
| UI-DS-03 | PASS | `design-system-light/screenshot.png`: atomic switch to light theme under active design system; clean recoloring |
| UI-DS-04 | PASS | `error/screenshot.png`: server stderr shows `runtime reload failed`; status bar shows sanitized diagnostic |
| UI-DS-05 | PASS automated | `reload_with_missing_design_system_preserves_previous_generation_and_reports_diagnostic` pins specified semantics |
| UI-DS-06 | PASS | `configuration failed [theme.load_failed]` at startup; nothing installed or adopted |
| UI-DS-07 | PASS automated | Package loading/authorization/revocation suites green (134 passed) |
| UI-DS-08 | PASS automated | `plan104_design_system_packages_cover_all_25_components_and_enforce_color_authority` passed |
| UI-DS-09 | PASS automated | `reload_with_missing_design_system_preserves_previous_generation_and_reports_diagnostic` passed |
| UI-DS-10 | PASS | Restart persistence through `init.js` verified; both startup logs clean |
| UI-DS-11 | PASS | Catalog controls migrated to recipe properties; validated by `components.test.tsx` (all 31 tests passed) |
| UI-DS-12 | PASS | Surfaces migrated to recipe properties; validated by `shell.test.tsx` and `registry.test.tsx` |
| UI-DS-13 | PASS | Global CSS forced-colors rules centralized in `global.css` |
| UI-DS-14 | PASS | Reduced-motion/transparency rules centralized in `global.css`; unit tested in `design-system-adapter.test.ts` |
| UI-DS-15 | PASS | Visual review captures prove complete palette swap without geometry disruption |
| UI-DS-16 | PASS | `design-neobrutal/screenshot.png` + `theme_packages.rs` validation of Neobrutal package |
| UI-DS-17 | PASS | `design-glass/screenshot.png` + `theme_packages.rs` validation of Glass package |
| UI-DS-18 | PASS automated | `plan104_source_independence_guard_rejects_package_name_branching` passed |
| UI-DS-19 | PASS automated | `frontend/src/test/design-system-conformance.test.tsx` proves DOM/state continuity across switches |
| UI-DS-20 | PASS automated | `plan104_malicious_and_out_of_bounds_design_system_values_are_rejected` and `plan104_third_party_design_system_security_and_authority_isolation` passed |
| UI-DS-21 | PASS automated | `settings-panel-choices.test.tsx` proves dropdown renders from server snapshot, switch re-renders without remount, focus preserved; `settings_set_design_system_persists_and_snapshot_lists_choices` pins persist/reload/active_design_system. Interactive keyboard leg UNRESOLVED (host input ceiling below) |
| UI-DS-22 | PASS automated | Validator allowlist tests (`settings_set_design_system_accepts_core_and_bundled_contributors`, `_rejects_unknown_and_non_design_specifiers`) + persistence e2e; command-centre interactive leg UNRESOLVED (host input ceiling) |
| UI-DS-23 | PASS automated | Rejection e2e: invalid specifier rejected without reload; real-app sanitized-diagnostic surface captured in `ui-review-error` |
| UI-DS-24 | PASS automated | `enumerate_ui_choices` e2e asserts themes enumerated from enabled package records (sorted, snapshot-delivered); settings-panel test asserts the dropdown renders from the snapshot |
| UI-DS-25 | PASS automated | Persistence e2e asserts appearance survives restart: `persist_settings_change` writes the pref, startup `apply_persisted_preferences` re-applies it; real-app init.js restart persistence in UI-DS-10 |
| UI-DS-26 | PARTIAL | Fixture-layer: `.impeccable/reviews/110-final/` CDP captures show clearly visible Neobrutal/Glass differences on dark+light themes. Real-app package activation (`ui-review-design-neobrutal`, `ui-review-design-glass`): **UNRESOLVED — blocked by the task-18 pre-existing reload deadlock** (see record below) |

## Plan 112 cross-reference (2026-09-07)

Icon packs are independent of theme, appearance, and design-system selection:
all four resolve concurrently, pack swaps preserve the other three axes, and
icons render solely with active-theme color roles (`fill=currentColor`, no
package palettes). Icon-slot CSS consumes core tokens only. Independence
steps: [18 — Icon packs](18-icon-packs.md) (ICON-03, ICON-04, ICON-12,
executed 2026-09-07).
