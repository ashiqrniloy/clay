# Audit Remediation: Clay UI Modernization

Prerequisites: Plans 086 and 087 complete; accessibility, review fixtures, entry state, and completion behavior are green.

Approved constraint: `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`. Modernization improves defaults and token consumption while preserving `theme.setTheme`, typed `designTokens`, `ResolvedUiTheme`, existing themes, and user-owned typography.

Source review: P1-4 and related P1/P2 UI/performance/test requirements in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`.

## UI Skill Gate (mandatory for every task)

Before reviewing existing UI, planning, designing, or changing any UI-related task in this plan — including theme, typography, tokens, components, layout, SDUI, overlays, accessibility, or visual evidence — load the complete mandatory project-local UI skill stack defined by `.agents/skills/clay-ui/SKILL.md`. Repeat this gate for each independently executed task; prior task evidence does not satisfy it.

## Objectives

- Create a coherent, restrained, editor-first native visual language across all core Clay surfaces.
- Preserve current theme and typography configurability while improving default hierarchy, spacing, states, and responsiveness.
- Reuse the existing Masonry component/token/chrome catalog; add only generic gaps proven by a full surface inventory.
- Validate every changed state through automated conformance plus screenshots, keyboard flow, and accessibility trees.

## Expected Outcome

- Shell, tabs, panes, file browser, status, menus/completion, Command Centre, dialogs, settings, diagnostics, and package panels share consistent hierarchy and state treatment.
- Dark/light themes and representative typed overrides remain functional, contrast-valid, and visually coherent.
- Narrow/wide windows, multi-pane/multi-tab layouts, high DPI, and user font scales remain usable without clipping or hidden focus.
- No raw colors/sizes/fonts, new UI framework, package-side native UI, or paint-time configuration work is introduced.

## Tasks

- [x] Establish visual baseline, state matrix, and measurable direction
  - Acceptance Criteria:
    - Functional: Capture every core surface/state before edits: light/dark, empty/editor, file browser, tabs/panes/status, completion/menu/Command Centre, dialogs/settings/diagnostics/package panels, loading/busy/error/recovery, narrow/wide, multi-tab/pane.
    - Performance: Record advisory typing, tab-switch, menu filter, and layout baseline commands; no implementation work yet.
    - Code Quality: Produce a concise visual direction: editor-first hierarchy, spacing/typography rhythm, active/inactive distinction, state treatment, icon policy, and content rules mapped to existing tokens/components.
    - Security: Use fixture data; verify screenshots/accessibility labels contain no host paths or secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`, `references/components.md`, `references/tokens.md`, `docs/reference/ui-components.md`.
      - the UI guidance current at execution time; selected `web-design-guidelines` and `fixing-accessibility`; official current interface rules fetched.
      - Project patterns `ui-modernization.md`, `ui-visual-review.md`, `package-ui-layout.md`, `typography-role-ownership.md`, `protocol-and-performance.md`.
      - Decision `2026-08-14-0331-ui-modernization-preserves-theme-configuration`.
    - Options Considered:
      - Copy a web/IDE design wholesale: rejected; Clay is native Masonry and has existing token/authority constraints.
      - Surface-by-surface ad hoc cleanup: rejected; preserves inconsistency.
      - Small documented direction mapped to existing system: chosen.
    - Chosen Approach:
      - Use Plan 087 fixtures to build a state matrix and specify deltas before code.
    - API Notes and Examples:
      ```text
      Visual hierarchy → typography.* + spacing.* + surface/text/border roles
      Interaction → Rest/Hover/Active/Focus/Disabled token states
      ```
    - Files to Create/Edit:
      - `plans/088-Audit-Remediation-Clay-UI-Modernization.md`: baseline matrix and execution evidence.
      - `code-reviews/screenshots/2026-08-14-plan088-baseline/*.png`: baseline artifacts.
    - References:
      - `code-reviews/screenshots/2026-08-14-clay-audit/`.
  - Test Cases to Write:
    - Baseline checklist covers every named surface/state/theme/layout before implementation.

### Task 1 Evidence (2026-08-15)

- Used the UI guidance current at execution time, then loaded `vercel-labs/web-design-guidelines` and `ibelick/fixing-accessibility`; translated their hierarchy, focus, long-content, contrast, keyboard, and semantic-control guidance to Masonry tokens and AccessKit roles rather than web/CSS constructs. Reviewed the Clay catalog/token policy, Plan 087 harness/wiki, relevant shell/overlay/pane wiki pages, UI patterns, and the approved theme-configuration decision before recording direction.
- Captured the current light-theme welcome state with an isolated mode-700 harness root and `theme.setTheme("@clay/theme-gruvbox-material-light")`. Baseline PNGs are retained under `code-reviews/screenshots/2026-08-14-plan088-baseline/`; existing Plan 087/086 live captures were privacy-cropped to the Clay window before reuse. New retained images contain no host paths or fixture secrets. An isolated `@clay/settings` fixture was also launched; `get_app_state` found Clay, but targeted `Ctrl+Alt+S` input was refused because this GNOME session has no window-list backend. No settings screenshot was retained or misreported as a pass.

  | Matrix state | Evidence | Baseline result |
  |---|---|---|
  | Light empty/welcome | `light-welcome.png` | Captured; theme resolves through the existing light package. The unused left column and low-contrast secondary copy are visible. |
  | Dark empty/welcome | `dark-default.png` | Captured; actionable entry state/status work, but the same unexplained left column consumes space. |
  | Open editor + status | `dark-opened-document.png` | Captured from the isolated document fixture; editor/status are readable, but document hierarchy is sparse. |
  | Loading/busy | `dark-loading.png` | Captured; Plan 087's host exposed the welcome shell rather than the published loading SDUI tree. This is an observability gap, not a loading visual pass. |
  | Runtime error/diagnostic | `dark-error.png` | Captured; diagnostic text and recovery guidance are present and sanitized. |
  | Disconnect/recovery | `dark-recovery.png` | Captured; recovery menu is visible, but its bottom-sheet treatment competes with editor space. |
  | Completion/menu | `dark-completion-overflow.png` | Captured; live rows escape the compact shell despite the scrollbar (`P1-087-UI-1`). |
  | Command Centre/dialog | `dark-command-centre-overflow.png` | Captured; centered modal hierarchy is clear, but long rows escape its shell (`P1-087-UI-1`). |
  | Multi-tab + active/inactive status | `dark-multi-tab-status.png` | Captured before welcome substitution; valid chrome baseline, but must be recaptured after tab-chrome edits. |
  | Multi-pane + focus | `dark-multi-pane.png` | Captured before welcome substitution; valid split/focus baseline, but must be recaptured after pane-chrome edits. |
  | File browser, settings, package panels, modal variants | No current isolated fixture/targetable keyboard sequence | No visual pass. Add fixture coverage only if later surface work needs it; do not invent a second UI path for baseline capture. |
  | Narrow/wide, high DPI, typography extremes | No safe window-list/resize backend on this host | No visual pass. Existing structural geometry checks remain the only evidence until a targetable desktop backend is available. |

- **Direction mapped to existing system:**

  | Need | Existing token/component mapping | Rule |
  |---|---|---|
  | Editor-first hierarchy | `typography.display`/`section`/`body`/`detail`/`caption`, `text.primary`/`text.muted` | Promote active document, active tab/pane, and recovery action; keep ancillary metadata quiet. Concrete font families and sizes remain user-owned. |
  | Surface rhythm | `surface.main`/`panel`/`overlay`, `border.hairline`/`subtle`/`strong`, `spacing.xs` through `xxl`, `radius.*`, density scale | One restrained background/panel/overlay ladder; remove unexplained empty gutters before adding cards or decoration. |
  | Interaction | Existing Rest/Hover/Active/Focus/Disabled palettes, `border.focus`, `focus.ring`, semantic diagnostic tokens | Active/inactive state needs contrast, focus ring, and text/shape/status support—never color alone. |
  | Commands, overlays, recovery | `TransientMenuSession`, `PackageOverlayHost`, `scroll`, `list`, `statusItem`, `paint_tooltip_shell` | Keep completion modeless/caret-adjacent and Command Centre modal/centered; fix shared clipping before cosmetic restyling. |
  | Icons and content | Existing `paint_icon_slot` only when a generic internal slot is already warranted; labels, `paint_kbd_hint`, bounded accessibility text | No icon library or unlabeled glyph controls. Decorative icons are presentation-only; actions retain visible text/accessibility names. |
  | Theme/responsiveness | Cached `ResolvedUiTheme`, `StyleRegistry`, semantic typography roles, Masonry constraints | No raw colors/sizes/fonts or per-frame resolution; treat light/dark, user typography, pane width, and DPI as first-class acceptance states. |

- **Advisory performance baseline commands** (not CI thresholds):
  ```bash
  cargo bench --bench editor_baselines editor_render_adjacent -- --sample-size 10 --warm-up-time 1 --measurement-time 2
  cargo bench --bench window_baselines tab_switch_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
  cargo bench --bench window_baselines completion_filter_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
  cargo bench --bench window_baselines completion_layout_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
  ```
  Plan 087's latest local reference remains completion open `2.41/13.40/89.95/362.10 µs` for `1/8/60/256` items, filter `12.21/73.61/416.08 µs` for `16/60/256` candidates, and layout `0.98/0.88/0.89 µs`; compare only on the same machine.

- [x] Review catalog composition and approve only generic primitive/token gaps
  - Acceptance Criteria:
    - Functional: Map each redesign element to existing components/primitives; identify missing generic needs only after composition attempts. Evaluate actionable empty-state composition, compact metadata row, icon policy, toast/progress, badge/kbd/tooltip planned entries without assuming new kinds.
    - Performance: Any new primitive is allocation-free/deterministic in paint and cached-token-driven.
    - Code Quality: Prefer composition; every approved addition is generic, state-complete, additive, accessible, and cataloged in same change.
    - Security: Packages remain inert and cannot inject raw colors/CSS/fonts/native widgets/callbacks/client JS.
  - Approach:
    - Documentation Reviewed:
      - Component catalog “Rules for Adding Components” and planned components.
      - `src/shell/components.rs`, `src/shell/primitives.rs`, `src/masonry_package_region.rs`, conformance tests.
    - Options Considered:
      - Add a large design-system widget layer: rejected.
      - Compose existing kinds and add only a measured gap: chosen.
    - Chosen Approach:
      - Record an explicit reuse/add/reject table; implementation cannot add an unlisted custom component.
    - API Notes and Examples:
      ```text
      Actionable empty state = flex + label + button + kbd hint (no new kind)
      Compact metadata row = list title/detail unless tests prove insufficient
      ```
    - Files to Create/Edit:
      - This plan: approved primitive gap table.
      - `.agents/skills/clay-ui/references/components.md`, `references/tokens.md` only for approved additions.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay UI Primitives-First Task.
  - Test Cases to Write:
    - Catalog drift/state completeness/token-only conformance for each approved addition.

### Task 2 Evidence (2026-08-15)

- Traced the actual owners before approving anything: `ComponentKind`/style validation, `ResolvedUiTheme` core fallbacks and contrast gate, retained `PackageRegionWidget`, `WelcomeWidget`, file-browser SDUI, and `TransientMenuSession`/`PackageOverlayHost`. The catalog remains additive-only and package contributions remain inert.
- **Approved additions: none.** Every Task 088 redesign element either composes from an implemented owner or is a repair to an existing owner. No `ComponentKind`, style variable, primitive, token, public API, or package contract is approved by this task; consequently, the catalog/token references remain unchanged.

  | Review item | Existing composition / evidence | Decision |
  |---|---|---|
  | Actionable empty/recovery state | Clay-native `WelcomeWidget` already combines panel chrome, semantic UI typography, visible `Open File`/`Open Folder` buttons, existing client command routes, and a polite status. A declarative equivalent is `panel` + `flex` + `label` + `button` + `statusItem`. | Reuse; no `emptyState` kind or token. |
  | Compact metadata row | `PackageUiListItem` carries `label` + optional `detail`; file-browser rows and `TransientMenuSession` projections already consume it through `list`/`scroll`. | Reuse; no metadata-row, description, or divider kind. |
  | Menus, completion, dialogs, selectors, text entry | `TransientMenuSession` projects to existing `stack` + `scroll` + `list`/`statusItem`; `overlay`/`portal`, `modal`, `dropdown`, `collapse`, and `textInput` already own the required focus, dismissal, and typed validation paths. | Reuse; P1-087-UI-1 is shared scroll-host containment repair, not a component/token gap. |
  | Busy/error/recovery feedback | `WelcomeState` headline/detail and existing `statusItem`/semantic diagnostic tokens express current loading, error, and recovery states. There is no current Task 088 consumer needing a determinate progress model. | Reuse status treatment; do not add toast/progress state, timer, or token. |
  | Badge, `kbd`, icon, tooltip | Existing typed badge/`kbd`/tooltip/icon color, spacing, dimension, typography, z-level, and contrast entries cover Clay-native chrome. `paint_badge`, `paint_kbd_hint`, and `paint_icon_slot` have no production caller and intentionally leave text/glyph drawing deferred; `paint_tooltip_shell` already hosts overlays, but no Task 088 surface needs a hover-trigger API. | Keep catalog entries planned/internal; do not promote a half-used primitive or add an icon library. |
  | Table and directional package layout | `table` remains the lone reserved kind with no first-party consumer. `PackageRegionWidget` currently lays package `flex` and `stack` as vertical Masonry columns, so Task 088 must not assume a package-facing horizontal-flex contract. | No new primitive/token. If a real package needs horizontal composition, correct that existing catalog/runtime parity separately before using it. |

- The token inventory already supplies the required surface ladder, state/focus/disabled treatment, diagnostics, spacing/radius/density, semantic typography, overlay z-levels, and contrast enforcement. Adding visual aliases before a concrete reusable consumer would create token debt and violate the approved configurable-theme constraint.
- Verification passed: `cargo test --test editor package_ui_conformance` (10), `cargo test --test editor ui_primitive_conformance` (12), and `cargo test --lib shell::components` (7). No new test is needed because this task approves no addition; the existing catalog/status-partition and code↔catalog drift tests enforce the retained decision.

- [x] Modernize theme defaults and token consumption without breaking configurability
  - Acceptance Criteria:
    - Functional: Improve default surface hierarchy, text/border contrast, spacing, radii, focus, selection, density, and typography role usage; existing Gruvbox dark/light selection and typed overrides produce coherent results across every modernized surface.
    - Performance: Theme resolves once at install/reload; paint/layout reads cached `ResolvedUiTheme`/`StyleRegistry`; unchanged theme causes no invalidation churn.
    - Code Quality: No raw colors, dimensions, font families, or point sizes outside authoritative theme/token definitions; existing token names/meanings remain compatible, additions are additive.
    - Security: Preserve first-party theme resolver, typed value/bounds validation, WCAG contrast gates, and denial of raw CSS/renderer callbacks/client JS.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/theme/set-theme.md`, `set-typography.md`.
      - `.agents/skills/clay-ui/references/tokens.md`; `src/shell/theme.rs`, `src/editor/theme.rs`, `src/editor/typography.rs`.
      - UI preflight rerun for this task: the UI guidance current at execution time; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility`.
      - Project patterns `ui-modernization.md`, `typography-role-ownership.md`, `configuration-system.md`.
    - Options Considered:
      - Fixed redesigned palette: rejected by approved decision.
      - Replace current theme API: rejected; unnecessary migration.
      - Improve defaults and consume typed tokens consistently: chosen.
    - Chosen Approach:
      - Keep public theme/typography snapshots unchanged where possible; add typed tokens only where existing semantic roles cannot represent a reusable visual distinction.
    - API Notes and Examples:
      ```javascript
      import { setTheme, setTypography } from "clay:theme";
      setTheme("@clay/theme-gruvbox-material-dark");
      ```
    - Files to Create/Edit:
      - `src/shell/theme.rs`, `src/editor/theme.rs`, `src/editor/typography.rs`: default/token consumption.
      - First-party theme package manifests only if typed overrides need additive values.
      - `.agents/skills/clay-ui/references/tokens.md`: exact token changes.
      - `tests/theme_packages.rs`, `tests/package_ui_conformance.rs`, `tests/typography_protocol.rs`.
    - References:
      - `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`.
  - Test Cases to Write:
    - Default/dark/light/custom typed override, invalid override fallback/rejection, contrast, unchanged revision, large/small UI font, missing-font fallback.

### Task 3 Evidence (2026-08-15)

- Kept public configuration unchanged: `theme.setTheme`, `theme.setAppearance`, typed `designTokens`, cached `ResolvedUiTheme`, `StyleRegistry`, and user-owned `setTypography`/`UiTypographyHierarchy` remain the authority. No new token, component kind, style variable, API, package permission, or fixed font value was added.
- Fixed legacy-theme consumption at the shared resolver instead of patching callers. `validate_active_theme_contrast` now validates the same `textStyles` → `ResolvedUiTheme::with_base_ui` projection used by the client. Existing text-style themes now feed surface/list/control/selection, focus/accent, border, diagnostic, badge/kbd, tooltip, scrollbar, and semantic text roles; typed design-token overrides still win. Low-contrast legacy placeholders fall back to base text for UI `text.muted`, while editor placeholder paint remains user/theme-owned.
- Fixed radius-domain consumption: focus rings, panel chrome, scrollbars, badges, kbd hints, tooltips, and tab cards now read `radius.*` through `scalar_f64`; panel resize grip geometry uses existing spacing/border tokens. No raw redesign palette or new token was introduced.
- Typography role usage was re-verified rather than duplicated: `TypographyRegistry` continues to resolve all UI variants from the user-selected `ui` profile and hierarchy, while editor/package roles remain semantic and cached. Existing invalid-hierarchy, large/small-size, missing-font, revision/no-churn, and geometry tests remain green.
- Updated `.agents/skills/clay-ui/references/tokens.md`, `docs/reference/packages/creating-packages.md`, `docs/reference/primitives/ui-chrome-primitives.md`, and `docs/wiki/modules/editor-theme-registry.md` with compatibility projection, contrast, and radius-domain rules. Visual screenshot/accessibility acceptance remains for the later dedicated review task; no visual pass is claimed here.
- Verification passed:
  - `cargo fmt --all -- --check`
  - `cargo check --all-targets`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --lib --quiet` — 1548 passed, 2 ignored
  - `cargo test --test editor --quiet` — 163 passed
  - `cargo test --test protocol --quiet` — 156 passed
  - `cargo test --test editor theme_packages --quiet` — 9 passed
  - `cargo test --test editor ui_primitive_conformance --quiet` — 12 passed

### Task 3 UI-skill Re-evaluation (2026-08-15)

- Re-ran the mandatory preflight before this review: the UI guidance current at execution time; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility` (2 skills, within the routing limit). Their web-specific rules were translated to Clay's catalog-first, typed-token, semantic-role, Masonry-native constraints.
- Review result: Task 3's implementation still uses existing component/chrome primitives, cached theme resolution, semantic typography roles, visible focus rings, keyboard-accessible named controls, and host-side contrast validation. No new component, token, raw style, native widget, renderer callback, or package authority is justified. Task 3 remains complete; no code changes were required by this re-evaluation.
- Fresh representative evidence is retained under `code-reviews/screenshots/2026-08-14-plan088-task3-reevaluation/`: `dark-default/` and `light-default/` both report `PASS`, contain Clay-window-only 913×1151 PNGs, and expose named `Open File`/`Open Folder` buttons plus status/panel semantics in AT-SPI dumps. Both themes remain readable; the known empty left column remains a Task 1/Task 4 shell-layout finding, not a Task 3 token-resolution failure. Full changed-state visual/accessibility acceptance remains delegated to the dedicated later review task; these captures are not a full-plan visual pass.
- Recheck passed: `cargo test --test editor theme_packages --quiet` (9), `cargo test --test editor ui_primitive_conformance --quiet` (12), `cargo test --lib shell::theme --quiet` (21), `cargo test --lib editor::typography --quiet` (17), and `git diff --check`.

- [x] Modernize shell, tab, pane, browser, and status chrome
  - Acceptance Criteria:
    - Functional: Active/inactive tabs and panes are unmistakable; overflow/close/add/focus states remain complete; browser hierarchy and status information are compact/readable; dirty/recovery/connection state never relies on color alone.
    - Performance: Tab/pane geometry remains O(visible tabs/panes), no document serialization on switch, no JS/IPC/filesystem work in paint/layout.
    - Code Quality: Route chrome through existing primitives/tokens; preserve pane split, slot sizing, focus, persistence, and tab accessibility contracts.
    - Security: Workspace/path labels remain sanitized; shell actions keep current server/client authority and grant checks.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/masonry-shell.md`, `tabs-and-clients.md`, `pane-document-views.md`, `shell-primitives.md`.
      - `docs/reference/ui-components.md`, `docs/development/launch-and-gui-smoke.md`.
      - UI preflight for this task: the UI guidance current at execution time; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility`.
      - `test-plan/13-window-splits.md`, `14-tabs.md`.
    - Options Considered:
      - Restructure pane/tab model during visual pass: rejected.
      - Paint/layout-only modernization over current retained state: chosen.
    - Chosen Approach:
      - Keep interaction/state ownership unchanged and update token-driven geometry/chrome incrementally by surface.
    - API Notes and Examples:
      ```text
      tab_card_chrome(active, focused, hovered, dirty) → typed tokens only
      pane host focus → existing Masonry focus + Role::Pane
      ```
    - Files to Create/Edit:
      - `src/masonry_shell.rs`, `src/masonry_editor.rs`, `src/masonry_pane_document.rs`, `src/masonry_sdui.rs`, `src/driver/mod.rs`, `src/driver/reconcile.rs`.
      - `src/shell/primitives.rs`, `src/shell/file_browser.rs` only where visual geometry/labels belong; pane topology remains unchanged.
      - `docs/wiki/modules/masonry-shell.md`, `pane-document-views.md`, `shell-primitives.md`, `workspace-file-browser.md`, and the file-browser smoke contract.
      - Tests/benchmarks adjacent to changed modules.
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`.
  - Test Cases to Write:
    - Single/multi tab, overflow, active/inactive/dirty/focus/disabled, split focus, browser hidden/visible, connection/recovery status, narrow/wide and font-scale layouts.

### Task 4 Evidence (2026-08-15)

- Re-ran the mandatory UI preflight before source review: the UI guidance current at execution time; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility` (2 skills, within the limit). Applied their hierarchy, state, accessible-name, keyboard, contrast, and minimal-change guidance to Clay's native Masonry/token paths rather than web/CSS constructs.
- Modernized the existing shell without changing pane/tab ownership or public APIs: welcome now reclaims stale workspace-browser space while preserving package fixed slots; split focus rings paint after pane children; the pinned `+` tab affordance uses cached state tokens on hover; status chrome reads `surface.control`, `text.primary`, spacing, and border tokens with legacy fallbacks; workspace and tab labels are basename/relative, bounded, control-free, and never fall back to absolute host paths.
- Fresh representative captures are retained under `code-reviews/screenshots/2026-08-15-plan088-task4-welcome/` and `code-reviews/screenshots/2026-08-15-plan088-task4-light/`. Both report `PASS`, contain Clay-window-only 913×1151 PNGs for dark/light welcome states, and expose named `Open File`/`Open Folder` controls plus status/pane semantics. No host path or secret appears in retained text evidence. Full opened-editor, browser-list, multi-tab/pane, narrow/wide, and interactive focus-state visual acceptance remains for the dedicated later review task; this is not a full-plan visual pass. The host still lacks safe window-targeting/portal keyboard control for those interactions.
- Verification passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets --quiet`; focused shell/editor/SDUI/file-browser/pane tests; and the tab-label sanitization regression. No new component kind, style variable, token, package authority, JS API, filesystem operation, IPC, document serialization, or paint/layout hot-path work was introduced.

- [x] Modernize overlays, dialogs, settings, diagnostics, and package panels
  - Acceptance Criteria:
    - Functional: Menus/completion/Command Centre/dialogs/settings/diagnostics/package panels share spacing, hierarchy, focus, selected/disabled/error states, and clear recovery actions; modality and focus restoration remain correct.
    - Performance: Visible rows are bounded/scrollable; no blur/offscreen filter or per-frame theme resolution; package JS remains absent from input/paint/layout.
    - Code Quality: Use shared overlay/portal/list/input/panel primitives; no duplicate overlay system or per-surface bespoke style constants.
    - Security: Package UI stays inert/validated; modal containment and action routing preserve provenance and current authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/transient-menu-session.md`, `centered-command-centre-surface.md`, `slot-aware-package-ui.md`, `phase20.5-overlay-menu-input-components.md`.
      - Clay component/state catalog and accessibility guidelines.
    - Options Considered:
      - Custom visuals per surface: rejected.
      - Shared primitive/token uplift: chosen.
    - Chosen Approach:
      - Modernize generic primitives first, then consume them without changing package component schema unless approved gap task requires additive change.
    - API Notes and Examples:
      ```text
      overlay/modal/menu → paint_tooltip_shell + list/input/focus primitives + z.* tokens
      diagnostic → semantic status + action, never color-only
      ```
    - Files to Create/Edit:
      - `src/masonry_package_region.rs`, `src/masonry_sdui_region.rs`, `src/masonry_editor.rs`.
      - `src/shell/package_ui.rs`, `src/shell/transient_menu.rs`, `src/shell/primitives.rs`, `src/shell/components.rs`.
      - Settings/diagnostic owners identified in primitive review (tentative).
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`.
  - Test Cases to Write:
    - Rest/hover/active/focus/disabled, empty/loading/error/recovery, long content, modal tab loop/Escape/focus restore, package action provenance, narrow pane/window.

### Task 5 Evidence (2026-08-15)

- Re-ran mandatory UI preflight before review: the UI guidance current at execution time; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility` (2 skills, within the routing limit), then translated their focus, disabled-state, long-content, containment, and minimal-change guidance to Clay's retained Masonry/token path.
- Fixed shared host behavior instead of adding surface-specific UI: `PackageRegionWidget` clips each retained package subtree to its owning panel/overlay bounds; `SduiRegionWidget` applies the same boundary; package panel/overlay/modal/region accessibility nodes expose clipped-child semantics. Nested `scroll` children receive flex sizing during build and live reconcile, so bounded fixed panels can scroll long content instead of painting rows below their shell (closing P1-087-UI-1).
- Completed generic state/accessibility plumbing: `statusItem` maps to `Role::Status` and primary status text; disabled package controls expose AccessKit disabled state; package text-input padding and border width read cached tokens and focused fields use `paint_focus_ring`; modal Escape carries its optional declared inert command intent through `PackageModalDismiss` and `main.rs` routes it through the existing server-first path.
- Updated `@clay/settings` to compose `panel` + `scroll` without changing the package schema, permissions, or public API. Manifest dependency now names `ui.serverRegisterPanelContribution`; bundled trust fingerprint was regenerated after the manifest edit. Catalog/reference/wiki docs now describe the real token names, scroll composition, containment, status semantics, and modal routing.
- Evidence: isolated dark welcome capture is retained at `code-reviews/screenshots/2026-08-15-plan088-task5-dark-default/` with `review.status=PASS`, a Clay-window-only 913×1151 PNG, and named welcome/status AT-SPI nodes. A live TTY Command Centre attempt was recorded as `UNRESOLVED` because this GNOME session's portal input did not produce the menu; no interactive visual pass is claimed. Changed overlay/settings states remain part of the dedicated visual/accessibility review task.
- Added/updated focused checks: package-region containment and modal-intent tests, settings scroll-kind assertion, and package inventory fingerprint validation.
- Verification passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --lib --quiet` (1549 passed, 2 ignored); `cargo test --test editor --quiet` (163); `cargo test --test protocol --quiet` (156); `cargo test --all-targets --quiet`; focused package/UI primitive/docs tests.

- [x] Make modernized layouts responsive to pane size, DPI, and user typography
  - Acceptance Criteria:
    - Functional: Core states remain legible/operable at narrow/wide windows, 1x/2x representative scale, and min/max supported UI typography; long localized/user text truncates/wraps intentionally.
    - Performance: Layout uses Masonry constraints/token metrics, not repeated global measurement or full-tree invalidation.
    - Code Quality: Centralize reusable clamp/breakpoint logic only when at least two surfaces share it; no speculative responsive framework.
    - Security: Scaling/truncation never reveals hidden paths or bypasses focus/accessibility bounds.
  - Approach:
    - Documentation Reviewed:
      - Masonry 0.4 local layout rustdocs/source; `typography-role-ownership.md`.
      - Current interface rules for long content, layout, focus, and reduced motion translated to native Masonry.
    - Options Considered:
      - Fixed dimensions: rejected.
      - Token/constraint-driven clamping with minimal shared helpers: chosen.
    - Chosen Approach:
      - Add state fixtures and layout assertions for representative extremes; fix actual clipping rather than inventing broad breakpoints.
    - API Notes and Examples:
      ```rust
      let width = preferred.min(constraints.max().width).max(minimum);
      ```
    - Files to Create/Edit:
      - `src/masonry_shell.rs`, `src/main.rs`, `src/masonry_editor.rs`, `src/masonry_sdui.rs`, `src/shell/package_ui.rs` for cached typography propagation and constraint-driven shell/sidebar/overlay geometry.
      - `tests/rust_visibility_api_mapping.rs` for the internal `#[doc(hidden)]` binary bridge allowlist; adjacent module tests cover unique layout/hot-path contracts.
      - `.agents/skills/clay-ui/references/components.md`, `docs/reference/packages/creating-packages.md`, and `docs/wiki/modules/{masonry-shell,masonry-sdui-region,masonry-editor,typography-registry-and-font-roles}.md` for the responsive shell/layout contract.
      - Plan 087 UI review harness output under `code-reviews/screenshots/` for default and large-typography states.
    - References:
      - `docs/development/accessibility.md`, `docs/development/ui-observability.md`.
  - Test Cases to Write:
    - Narrow/wide, 1x/2x, UI size 6/12/24/96 where supported, long names/details/errors, multi-pane, scroll reachability, accessibility bounds match visual bounds.

### Task 6 Evidence (2026-08-15)

- Re-ran the mandatory UI preflight before Task 6 review/implementation: the UI guidance current at execution time; inspected systems/typography/accessibility guidance and loaded `pbakaus/layout` plus `jakubkrehel/better-typography` (2 skills, within the prefer-1/max-3 rule). Applied the guidance to native Masonry constraints and Clay semantic typography rather than introducing web/CSS breakpoints. Re-read the Clay UI component/token catalog; no new component kind, style variable, token, primitive, package permission, or JS API was needed.
- Installed `ActiveTypography` into each `TabChrome` and synchronized the active tab's cached registry to the window-level shell. Tab-bar height, card padding, close affordances, `+` geometry, hit-testing, and pane placement now follow `UiTextVariant::Status` metrics and remain clamped to logical window bounds; duplicate/stale revisions do not request reflow. Added the 2x logical-size layout assertion.
- Replaced the fixed sidebar usability guard with a bounded UI-body em heuristic shared by the editor-region and SDUI left-slot decisions. Narrow panes and large UI text yield the sidebar before the editor becomes unusable. Long SDUI labels clip to typography-derived row bounds while retaining full accessibility text. Bottom transient overlays remain inside short main regions.
- Updated the internal hidden Rust bridge allowlist for shell typography synchronization; this remains binary plumbing only and is not a Clay JS/public programmatic API. Updated catalog, package authoring, and implementation wiki coverage for the responsive layout contract.
- Visual/accessibility evidence: `code-reviews/2026-08-15-plan088-task6-default/` and `code-reviews/2026-08-15-plan088-task6-large-typography/` each contain `review.status=PASS`, a cropped Clay-window-only 913×1151 PNG, metadata, and AT-SPI evidence. Default and UI-size-24 captures showed readable welcome/status text, bounded controls, and named `Open File`/`Open Folder` buttons; accessibility dumps retained `Role::Status`/button names and no clipped-path disclosure. Safe compositor window resize/focus remains unavailable on this GNOME Wayland host (`can_query_windows=false`, `can_focus_windows=false`), so narrow/wide interactive screenshot acceptance remains explicitly unresolved for Task 8; structural narrow/large-font/2x checks are the strongest available evidence.
- Verification passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets --quiet` (1556 passed, 2 ignored in lib; all bins/tests/benches green); focused `masonry_shell` (85), `masonry_sdui` (55), `shell::package_ui` (12), `editor_performance_invariants` (32), `typography_protocol` (11), package conformance (10), primitive conformance (12), docs (25), security visibility (12), and package-doc (5) tests; `git diff --check` clean.

- [x] Add automated conformance and performance checks for modernization
  - Acceptance Criteria:
    - Functional: Structural/state tests cover every changed component/surface and theme; package docs/catalog drift remains blocking.
    - Performance: Existing typing/tab/menu/pane budgets do not regress; benchmark additions measure only missing representative paths and remain advisory until promotion policy is met.
    - Code Quality: Prefer behavioral/typed checks over prose/source needles; keep one focused test per unique contract.
    - Security: Contrast, payload, provenance, stale-state, and raw-style denial tests remain green.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md`, `ui-observability.md`; existing conformance/benchmark suites.
    - Options Considered:
      - Screenshot goldens as hard CI: deferred.
      - Structural/behavioral gates plus live screenshot acceptance: chosen.
    - Chosen Approach:
      - Extend existing tests/benchmarks surgically and record before/after evidence.
    - API Notes and Examples:
      ```bash
      cargo test --test editor package_ui_conformance::
      cargo test --test editor ui_primitive_conformance::
      cargo test --test editor editor_performance_invariants::
      cargo bench --bench window_baselines responsive_layout_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
      ```
    - Files to Create/Edit:
      - `tests/package_ui_conformance.rs`, `tests/ui_primitive_conformance.rs`, changed module tests.
      - `tests/editor_performance_invariants.rs`, `tests/performance_budgets.rs` for typed responsive bounds, hot-path, chrome, and benchmark-documentation gates.
      - `src/perf/baselines.rs`, `benches/window_baselines.rs` for the advisory responsive-layout benchmark group.
      - `docs/development/performance.md`, `docs/wiki/modules/performance-fixtures.md` for the blocking/advisory split and runnable commands.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
  - Test Cases to Write:
    - Theme/state/layout matrix, no raw values, all states complete, contrast valid, no extra hot-path work.

### Task 7 Evidence (2026-08-15)

- Re-ran the mandatory UI preflight before Task 7 review/implementation: the UI guidance current at execution time; inspected `testing` and `performance`, selected `pbakaus/audit` from the testing category (1 skill, within the prefer-1/max-3 rule), and translated its measurable-audit guidance to native Masonry, typed tokens, AccessKit, and host-authority tests. Re-read the Clay component/token catalog and relevant performance/observability patterns; no new package-facing component, token, API, permission, or screenshot-golden policy was introduced.
- Added `responsive_layout_work` in `src/perf/baselines.rs` and the `responsive_layout_baselines` Criterion group in `benches/window_baselines.rs`. It exercises production SDUI sidebar/editor slot geometry at 320/900/1200 logical pixels and UI sizes 12/24/96; timing remains advisory while typed bounds stay blocking.
- Added the blocking typed matrix `responsive_layout_work_preserves_sidebar_and_editor_bounds`, extended hot-path checks across modernized shell/package/SDUI files, and extended primitive conformance to retained package chrome. Existing theme, state-completeness, catalog/token drift, contrast, payload, provenance, and accessibility tests remain in the blocking suite.
- Updated `docs/development/performance.md` and `docs/wiki/modules/performance-fixtures.md` with the deterministic/advisory split, benchmark command, sanitized layout-helper contract, and no-screenshot-golden policy. No new Clay JS or configuration surface was required.
- Verification passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets --quiet` (lib 1556 passed/2 ignored; bin 63; editor 164; protocol 157; remaining suites and all benches green); focused editor performance (33), package conformance (10), primitive conformance (12), and performance-doc (20) tests; `cargo bench --bench window_baselines --no-run`; responsive benchmark run with 10 samples/1 s warm-up/2 s measurement; `git diff --check` clean.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: Review light/dark and representative override themes across empty/busy/error/recovery, editor, file browser, tabs/panes/status, menus/completion/Command Centre, dialogs/settings/diagnostics/package panels, multi-tab/pane, narrow/wide, and typography extremes.
    - Performance: Interactions remain responsive with no flashing, clipping, duplicate overlays, focus loss, or visible layout churn.
    - Code Quality: Retain before/after screenshots and concise findings under one artifact path; unresolved defects block completion or receive explicit priority.
    - Security: Inspect labels/screenshots for path/secret leakage and package-origin confusion.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/ui-visual-review.md`; Plan 087 harness.
    - Options Considered:
      - Representative subset only: rejected for broad multi-surface redesign.
      - State matrix with computer-use semantic checks: chosen.
    - Chosen Approach:
      - Call `get_app_state` first, run keyboard-only interactions, verify roles/names/states/modal containment/live announcements, and capture each matrix state.
    - API Notes and Examples:
      ```text
      dark/default → light → typed override → typography extremes
      get_app_state before and after each interaction
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-14-plan088-modernization/{default,light-default,large-typography,error,recovery}/` Clay-window-only PNG/AT-SPI artifacts.
      - `code-reviews/screenshots/2026-08-14-plan088-modernization/review-log.md` and `retained-plan087/` comparison evidence.
      - This plan: findings, screenshot matrix, blockers, and prioritized follow-ups.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Keyboard focus/order, visible focus, semantics, contrast, modality, announcements, long content, all matrix states.

### Task 8 Evidence (2026-08-15)

- Re-ran the mandatory UI preflight for this independent review: the UI guidance current at execution time; inspected `visual` and `accessibility`; loaded `vercel-labs/web-design-guidelines` and `ibelick/fixing-accessibility` (2 skills, within the prefer-1/max-3 rule). Fetched the current Web Interface Guidelines source and applied both selected skills to native Masonry/AccessKit semantics and Clay token-driven chrome.
- Called `computer_2d_use_2d_linux_get_app_state` before desktop interaction, then `computer_2d_use_2d_linux_doctor`. AT-SPI and portal capture are available, but GNOME Wayland window introspection remains unavailable (`can_query_windows=false`, `can_focus_windows=false`). Targeted focus/click failed with the exact compositor backend blocker; global `Ctrl+Space` and `Ctrl+Alt+P` attempts did not reach Clay. No interactive screenshot was claimed as PASS.
- Captured and inspected current Clay-window-only screenshots/AT-SPI dumps under `code-reviews/screenshots/2026-08-14-plan088-modernization/`: dark default, light Gruvbox override, UI-size-24 typography, runtime error, disconnect/recovery, and loading fixture. Default/light/large passed visual bounds, named controls, status/panel semantics, and path-leak inspection. Recovery exposed a named selected `Dismiss` action but revealed a P1 stale welcome connection state; loading reached the shell but did not expose its intended loading SDUI state.
- Inspected retained opened-document, completion, Command Centre, multi-tab, and multi-pane evidence copied under `retained-plan087/`. Completion/Command Centre references expose menu/dialog/list/selected/provenance semantics but predate Task 5 containment; current post-containment visual confirmation remains unresolved. Narrow/wide, file-browser, settings/package-panel, and interactive focus states remain unresolved for the host/harness reasons recorded in `review-log.md`.
- Security review found no absolute workspace path in current Clay labels or cropped current screenshots. Accessibility review found named welcome actions, status/panel landmarks, error/recovery announcements, selected recovery dismissal, and retained menu/dialog semantics. The review log records P1 recovery-state consistency and fresh post-containment interactive recapture, plus P2 fixture/harness observability follow-ups; structural tests are not substituted for those live passes.
- Verification corroboration remains green from Task 7: `cargo test --all-targets --quiet`, `cargo clippy --all-targets -- -D warnings`, formatting, conformance, containment, contrast, payload, provenance, and responsive-layout checks. Task 8 is complete as an evidence-producing review with explicit unresolved priorities; next task is catalog/authoring-contract maintenance.

- [x] Update the UI catalog and package authoring contract
  - Acceptance Criteria:
    - Functional: Catalogs, token tables, package guide, UI navigation, and tests describe every changed/additive primitive/token/layout rule and current package limits.
    - Performance: Document cached token resolution and paint/layout ceilings.
    - Code Quality: Implemented/planned/internal markers and package-facing enums match source exactly.
    - Security: Reaffirm no raw CSS/colors/fonts/native widgets/client JS and no package control of Clay-owned shell geometry.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md`, `docs/reference/ui-components.md`, conformance drift tests.
    - Options Considered:
      - Duplicate catalog prose in navigation: rejected.
      - Keep references authoritative and navigation linked: chosen.
    - Chosen Approach:
      - Update authoritative catalogs once implementation stabilizes and make drift tests actionable.
    - API Notes and Examples:
      ```text
      Component kind/token additions are additive-only.
      ```
    - Files to Create/Edit:
      - `.agents/skills/clay-ui/references/components.md`, `references/tokens.md`.
      - `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`, and `docs/reference/primitives/shell-layout-strategy.md`.
      - `docs/wiki/modules/slot-aware-package-ui.md`: internal package-runtime explanation and test path.
      - `docs/index.md` verified as the master link for the UI navigation and package guide.
      - `tests/primitives_docs.rs`: new catalog/package-guide parity gate.
      - `tests/package_ui_conformance.rs`, `tests/rust_visibility_api_mapping.rs`: existing package-boundary gates verified unchanged.
    - References:
      - `.agents/skills/create-plan/references/clay.md` package UI/layout task.
      - `.agents/skills/project-patterns/references/package-ui-layout.md`, `package-runtime-trust-domains.md`, and `maintenance-validation.md`.
  - Test Cases to Write:
    - `primitives_docs::plan088_ui_catalog_and_package_authoring_contract_are_consistent`: source/catalog/package-guide/navigation parity, token no-addition note, and exact package anchor/internal-surface limits.
    - Existing `src/shell/package_ui.rs` centered-origin/internal-anchor test, `src/server/ui.rs` four-anchor validator, reserved-kind/raw-style trust-boundary tests, and `tests/rust_visibility_api_mapping.rs` anchor allowlist remain green.

### Task 9 Evidence (2026-08-15)

- Re-ran the mandatory UI preflight before reviewing/editing the catalog and package contract: the UI guidance current at execution time; inspected `systems` (the CLI has no `components` category), selected `ibelick/baseline-ui` (1 skill, within the prefer-1/max-3 rule), and translated its existing-primitives/accessibility/minimal-change guidance to Clay's native Masonry and typed-token model. Loaded `.agents/skills/clay-ui/` plus `components.md` and `tokens.md` before edits.
- Updated the authoritative component catalog with the Plan 088 no-additions contract: retained host clipping and clipped-child accessibility semantics, nested `scroll` flex sizing, modal Escape `PackageModalDismiss`, `statusItem`/disabled semantics, responsive shell ownership, sanitized labels, and the package-facing anchor/kind limits. Updated the token catalog with an explicit Plan 088 consumption-only note: existing cached surface/state/spacing/dimension/typography tokens are reused; no core or package token domain was added and no package breakpoint/concrete font/size contract exists.
- Updated `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`, and `docs/reference/primitives/shell-layout-strategy.md` so navigation and authoring guidance agree on Clay-owned shell/layout, `panel` + `scroll` composition, fixed/transient slots, internal completion/centered origins, status/disabled/accessibility semantics, cached token hot paths, path sanitization, and raw-style/client-JS/native-widget denial. Updated the internal `slot-aware-package-ui` wiki page; `docs/index.md` links were verified unchanged.
- Added `tests/primitives_docs.rs::plan088_ui_catalog_and_package_authoring_contract_are_consistent` as an actionable documentation parity gate. Existing component/token drift, raw-style/oversize trust-boundary, reserved `table`, exact four package overlay anchors, and centered internal-origin tests remain the enforcement layer; no conformance API or package authority was added.
- Verification passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets --quiet`; focused `cargo test --test protocol primitives_docs` (26), `cargo test --test editor package_ui_conformance` (10), `cargo test --test editor ui_primitive_conformance` (12), `cargo test --lib shell::package_ui` (12); and `git diff --check`. Next task is Clay JS API verification.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory changed server-side public functions and commands; visual-only internals remain private; existing `theme.setTheme`/`theme.setTypography` remain documented and discoverable.
    - Performance: JS APIs affect cached install/reload state only, never paint/input/layout.
    - Code Quality: Any new public behavior uses bare domain IDs, explicit op/facade/docs/index/registry, and complete metadata.
    - Security: No raw style/native widget/renderer callback API is exposed.
  - Approach:
    - Documentation Reviewed:
      - `clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `documentation-as-code.md`, `doc-registry-tests.md`.
    - Options Considered:
      - Expose every visual knob: rejected; typed themes already own configuration.
      - Reuse existing theme/typography APIs and add only necessary semantic options: chosen.
    - Chosen Approach:
      - Verify compatibility and no-new-API default.
    - API Notes and Examples:
      ```javascript
      setTheme("@clay/theme-gruvbox-material-light");
      setTypography({ monospace, proportional, ui, hierarchy });
      ```
    - Files to Create/Edit:
      - `tests/clay_js_facade_layout.rs`: keep the facade export matrix aligned with `runtime/js/theme.js` and `theme.d.ts` (including `setAppearance`).
      - `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/rust_visibility_api_mapping.rs`: existing inventory, registry, metadata, lookup, and internal-visibility gates verified unchanged.
      - `docs/wiki/modules/phase20.6-theme-segregation-settings-ui.md`: API-to-op/facade/Rust ownership inventory and Plan 088 no-new-API result.
      - `docs/wiki/modules/embedded-js-runtime.md`: current 22-facade compiled inventory count.
      - `docs/reference/clay-js-api/**`, `docs/index.md`, `docs/generated/clay-js-api-registry.json`: verify authoritative API docs, index, and generated registry; edit/regenerate only if drift is found.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `documentation-as-code.md`, and `doc-registry-tests.md`.
  - Test Cases to Write:
    - Theme trio (`theme.setTheme`, `theme.setAppearance`, `theme.setTypography`) has complete op/facade/declaration/docs/index/registry/lookup/custom-property metadata.
    - `settings.*` remains package-owned and `pub(crate)` `apply_*` helpers plus client-only visual bridges do not become Clay JS APIs.

### Task 10 Evidence (2026-08-15)

- Re-ran the mandatory Plan 088 UI preflight before reviewing theme/typography API consumers: the UI guidance current at execution time; inspected the `architecture` category and loaded `mattpocock/engineering` (1 skill, within the prefer-1/max-3 rule). Loaded `.agents/skills/clay-ui/SKILL.md` plus `references/components.md` and `references/tokens.md`; translated the selected web/UI engineering context to Clay's native token/cache and server-facade boundaries.
- Inventory found no new public programmatic capability in Plan 088. The complete public theme surface is the existing trio: `theme.setTheme` → `src/server/ops/theme.rs::op_clay_theme_set_theme` / `apply_theme`; `theme.setAppearance` → `op_clay_theme_set_appearance` / `apply_appearance`; and `theme.setTypography` → `src/server/ops/typography.rs::op_clay_theme_set_typography` / `apply_typography`. All use `clay:theme` flat exports, bare stable IDs, explicit `deno_core` wrappers, cached install/reload state, and authoritative Markdown/inventory/index/generated-registry entries with complete custom properties, empty default key bindings, lookup tags, and authority metadata.
- The shared `apply_theme`, `apply_appearance`, and `apply_typography` functions remain `pub(crate)` server primitives. `settings.*` commands remain owned by `@clay/settings` (`apiPrefix: settings`) and are inert server-first intents, not duplicate core Clay APIs. Plan 088 client-only `ClayShellWidget::set_active_typography`, `PackageModalDismiss`, and benchmark/layout helpers remain outside server ops, JS facades, API inventory, and configuration authority; the existing hidden Rust bridge allowlist covers the visual shell method.
- Found and fixed one verification gap: `tests/clay_js_facade_layout.rs` listed only `setTheme` and `setTypography`, so it did not assert the already-implemented `setAppearance` export. The expected facade matrix now covers all three JavaScript and declaration exports; no runtime/API behavior changed.
- Updated the implementation wiki with the API ownership matrix and no-new-API boundary, and corrected the embedded-runtime facade count from 21 to the current 22 compiled facade files. Existing `docs/wiki/index.md` links both updated pages. `docs/reference/clay-js-api/**`, `docs/index.md`, `api-inventory.toml`, and `docs/generated/clay-js-api-registry.json` were checked and already current; no registry regeneration was needed. The generated-registry repair command remains `cargo run --bin update-doc-registry`.
- Verification passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; focused `cargo test --test protocol clay_js_facade_layout` (5), `clay_js_api_inventory` (11), and `clay_js_doc_registry` (40); `cargo test --all-targets --quiet` (1556 lib, 63 bin, 164 editor, 158 protocol, 198 runtime, 130 security passed with 2 lib/1 security ignored). The first all-targets run hit a transient security fixture `Text file busy (os error 26)`; the targeted test and full security/runtime reruns passed. `git diff --check` is clean. Next task is configuration API/canonical-example verification.

- [x] Create or verify Clay configuration APIs and canonical example
  - Acceptance Criteria:
    - Functional: `~/.config/clay/init.js` theme/typography configuration continues to work unchanged; no hidden redesign settings; if additive typed options are introduced, docs and `examples/init.js` cover each exactly once.
    - Performance: Configuration reload resolves atomically and does not cause redundant layout/paint churn.
    - Code Quality: Option names/defaults/enums match parser and docs; `node --check examples/init.js` passes.
    - Security: Configuration retains first-party theme allowlist, typed bounds/contrast, and no filesystem/network/shell/native UI authority.
  - Approach:
    - Documentation Reviewed:
      - `configuration-system.md`, `examples/init.js`, current theme API docs, and `docs/wiki/modules/configuration-runtime.md`.
    - Options Considered:
      - New parallel appearance config: rejected; appearance already belongs to `clay:theme` and is persisted through the existing settings path.
      - Preserve current APIs and update only if additive semantics require it: chosen.
    - Chosen Approach:
      - Treat current theme/typography configurability as a blocking compatibility test, keep `clay:configuration` closed to its documented surface, and make the canonical example executable with optional alternatives only in comments.
    - API Notes and Examples:
      ```js
      // Comment out explicit setTheme when using appearance-driven defaults.
      setAppearance("system");
      setTypography({ monospace, proportional, ui, hierarchy });
      ```
    - Files to Create/Edit:
      - `examples/init.js`: document appearance alternatives and the complete optional seven-field typography hierarchy while preserving safe active defaults.
      - `src/server/mod.rs`: assert the real canonical tree installs the explicit theme, all profiles, and hierarchy.
      - `tests/clay_js_doc_registry.rs`: add canonical-example/docs parity coverage.
      - `docs/wiki/modules/configuration-runtime.md`: synchronize runtime flow, validation, canonical-example, and test documentation.
      - `docs/reference/clay-js-api/theme/**`, `docs/reference/clay-js-api/configuration.md`, `docs/index.md`, `docs/generated/clay-js-api-registry.json`: verify current API docs/index/registry; edit or regenerate only if schema drift is found.
    - References:
      - `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`.
      - `.agents/skills/project-patterns/references/configuration-system.md`, `documentation-as-code.md`, `doc-registry-tests.md`, and `typography-role-ownership.md`.
  - Test Cases to Write:
    - Canonical example loads the explicit dark theme and all three typography profiles with default hierarchy; documented light/dark/system alternatives remain available.
    - Reload remains atomic; invalid typography/preferences preserve prior valid state and hidden authority/configuration keys remain rejected.

### Task 11 Evidence (2026-08-15)

- Ran the mandatory UI preflight for this theme/typography configuration task: the UI guidance current at execution time; inspected `typography`; loaded `jakubkrehel/better-typography` (1 skill, within the prefer-1/max-3 rule). Re-read `.agents/skills/clay-ui/` and its component/token references, translating the web typography guidance to Clay's cached `TypographyRegistry`, logical-pixel metrics, and server-side configuration path.
- Verified the existing configuration API boundary: `clay:configuration` retains its exact six inventory rows (three runtime-backed and three planned/unavailable), while theme/appearance/typography remain the existing `clay:theme` trio. No new hidden config key, public API, permission, renderer hook, or layout API was added.
- Updated `examples/init.js` to cover the additive `setTypography` hierarchy option exactly once with all seven bounded default ratios, and to show all `setAppearance` enum alternatives with the explicit-theme precedence caveat. Active configuration remains safe to copy: explicit dark Gruvbox theme, complete three-profile typography, caret styling, bindings, and optional fault-isolated package modules. `authorizeLanguageServer` ordering and third-party template remain unchanged.
- Strengthened the real canonical-tree runtime test in `src/server/mod.rs` to assert explicit dark theme installation, all three profile sizes/fallback families, and the documented default hierarchy. Added `tests/clay_js_doc_registry.rs::canonical_example_covers_theme_typography_and_modular_configuration` to keep facade imports, active calls, alternatives, hierarchy fields, module paths, and configuration documentation markers synchronized.
- Updated `docs/wiki/modules/configuration-runtime.md`; its master-index link already exists. Authoritative theme/configuration Markdown, `docs/index.md`, `api-inventory.toml`, and `docs/generated/clay-js-api-registry.json` were checked and current, so `cargo run --bin update-doc-registry` was not needed. Configuration remains atomic/cached and grants no filesystem, network, shell, package-install, workspace, raw-op, native-widget, or client-JavaScript authority.
- Verification passed: `node --check` for `examples/init.js`, `examples/packages/first-party.js`, and `examples/packages/third-party.js`; `cargo fmt --all -- --check`; focused `cargo test --test protocol clay_js_doc_registry` (41), `cargo test --lib example_configuration_loads_cleanly_and_applies_effects -- --test-threads=1` (1), `cargo test --lib js_runtime:: -- --test-threads=1` (186 passed/1 ignored), and `cargo test --lib configuration:: -- --test-threads=1` (17); `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets --quiet` (lib 1556 passed/2 ignored, bin 63, editor 164, protocol 159, runtime 198, security 130 passed/1 ignored, all bench suites green); and `git diff --check`. Next task is the manual test plan.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Execute/update modules 01, 02, 03, 04, 07, 09, 10, 11, 13, and 14 for the full modernized state/theme/layout matrix.
    - Performance: Record perceived typing, switching, filtering, scrolling, resize, and theme reload behavior.
    - Code Quality: Add stable numbered steps, negative checks, and known ceilings without deleting existing coverage.
    - Security: Include theme validation, sanitized labels, package UI confinement, and modal focus checks.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` module map/coverage matrix and modules `01`, `02`, `03`, `04`, `07`, `09`, `10`, `11`, `13`, `14`.
      - `docs/development/launch-and-gui-smoke.md`, `file-open-save-reload-workflow.md`, `accessibility.md`, `performance.md`, and the referenced theme/package/shell docs.
      - `.agents/skills/project-patterns/references/ui-visual-review.md`, `ui-modernization.md`, `package-ui-layout.md`, `typography-role-ownership.md`, `maintenance-validation.md`, and `documentation-as-code.md`.
    - Options Considered:
      - New standalone visual test document: rejected; use indexed modules and module-local step IDs.
      - Claim structural tests as visual passes when window targeting is unavailable: rejected; record `BLOCKED`/`UNRESOLVED` with exact evidence and retain automated coverage.
      - Update affected modules and index: chosen.
    - Chosen Approach:
      - Add Plan 088 step ranges, expected results, negative checks, known ceilings, and Linux execution records to the existing modules without deleting or weakening prior coverage. Use current-build Clay-window-only capture plus retained same-build comparison artifacts; treat host limitations and product findings explicitly.
    - API Notes and Examples:
      ```bash
      cargo build
      scripts/capture-ui-review.sh --fixture ui-review-default --output <artifact-dir>
      cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
      ```
    - Files to Create/Edit:
      - `test-plan/index.md`: add Plan 088 coverage row and Task 12 execution summary.
      - `test-plan/01-launch-and-connection.md`, `02-configuration-init-js.md`, `03-files-and-workspace.md`, `04-core-editing.md`, `07-caret-and-typography.md`, `09-packages-and-modes.md`, `10-keybindings-and-commands.md`, `11-performance.md`, `13-window-splits.md`, `14-tabs.md`: add numbered Plan 088 steps and execution records; correct stale path/persistence expectations.
      - `code-reviews/screenshots/2026-08-15-plan088-task12-manual/`: retain current Clay-window-only review evidence (ignored artifact directory).
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
      - `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`.
  - Test Cases to Write:
    - Step ranges `L15–L19`, `C20–C24`, `F38–F41`, `E22–E24`, `T14–T17`, `P16–P21`, `K73–K77`, `Q15–Q19`, `S36–S40`, and `T71–T76`: expected visual/accessibility behavior plus negative checks.
    - Verify every unresolved state records a reason and artifact, never a false pass; verify no prior manual step is removed or weakened.

### Task 12 Evidence (2026-08-15)

- Ran the mandatory per-task UI preflight: the UI guidance current at execution time; inspected `accessibility` and `testing`; loaded `wshobson/wcag-audit-patterns` and `vercel-labs/web-design-guidelines` (2 skills, within prefer-1/max-3). Re-read `.agents/skills/clay-ui/SKILL.md`, `references/components.md`, and `references/tokens.md`; translated the web guidance to Clay's native Masonry, typed-token, logical-window, and AT-SPI workflow.
- Called `computer_2d_use_2d_linux_get_app_state` before desktop work, then `computer_2d_use_2d_linux_doctor`. AT-SPI tree/screenshot and portal capture are available, but GNOME Wayland has `can_query_windows=false`/`can_focus_windows=false`; targeted key delivery, resize, and native-dialog selection are therefore recorded as blocked, not passed.
- Built the current Linux binary and ran `scripts/capture-ui-review.sh --fixture ui-review-default`. `review.status` is `PASS`; `metadata.txt` records a 900×600 logical window, private mode-700 roots, portal PNG, and GI/AT-SPI dump. The screenshot was gold-border-cropped to a Clay-only 913×1152 PNG at `code-reviews/screenshots/2026-08-15-plan088-task12-manual/default/screenshot.png`; the tree exposes Welcome to Clay, named Open File/Open Folder buttons, Connected/Editable status, and no absolute path.
- Updated the ten affected modules and `test-plan/index.md` with stable Plan 088 step IDs, expected results, security/negative checks, known ceilings, and pass/fail/blocked records. Corrected the file-browser expectation to exclude full authorized paths and corrected the stale tab-persistence ceiling to distinguish in-memory server registry from client-owned `layout.json` v2.
- Recorded retained dark/light/error/recovery/large-typography evidence from the same current implementation under `code-reviews/screenshots/2026-08-14-plan088-modernization/`. Results: dark/default, light, large typography, and runtime error are PASS; recovery has a P1 stale WelcomeWidget `Connected` status mismatch; loading is UNRESOLVED because its fixture exposed welcome instead of the intended loading tree; interactive completion, Command Centre, settings, file-browser, multi-tab, narrow/wide, and live DPI states remain explicitly blocked/unresolved by host targeting rather than misreported.
- Headless/structural verification passed: canonical example runtime test, `clay_js_doc_registry` (41), package UI conformance (10), UI primitive conformance (12), editor performance invariants (33), and Rust visibility mapping (12). `cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2` completed all groups; advisory Criterion comparisons reported some local-baseline regressions but remained below blocking budgets and are not CI pass/fail thresholds.
- No existing test-plan steps were removed or weakened. The final code-wiki task records the unresolved UI findings as explicit follow-ups; no unchecked Plan 088 task remains.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki teaches modern visual architecture, token/theme flow, component/chrome ownership, responsive behavior, accessibility, extension rules, and tests; index links pages.
    - Performance: Document cached resolution and hot-path invariants.
    - Code Quality: Link authoritative public theme docs instead of duplicating them; include source/test paths and extension guidance.
    - Security: Document validation/authority boundaries and theme/package restrictions.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md` and `references/page-template.md`.
      - Existing wiki pages for theme/typography, shell/tabs/panes, retained SDUI/package UI, overlays, workspace/file browser, performance, configuration, and the review harness.
      - `.agents/skills/project-patterns/references/ui-modernization.md`, `ui-visual-review.md`, `maintenance-validation.md`, `documentation-as-code.md`, plus current implementation/source and authoritative UI/package references.
    - Options Considered:
      - New standalone Plan 088 wiki page: rejected; existing module ownership pages are already indexed and avoid duplicating implementation explanations.
      - Duplicate public JS/package API reference prose: rejected; link `docs/reference/` and keep `docs/wiki/` implementation-focused.
      - One final synchronized update with a master-index map and deterministic content guard: chosen.
    - Chosen Approach:
      - Update the existing owner pages and `docs/wiki/index.md` after implementation/verification, record retained visual-review findings and host limitations, and add one non-mutating `primitives_docs` guard for the cross-page contract.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol primitives_docs -- --test-threads=1
      cargo test --all-targets --quiet
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Plan 088 implementation map and corrected module navigation.
      - `docs/wiki/modules/{shell-primitives,editor-theme-registry,masonry-shell,pane-document-views,tabs-and-clients,masonry-sdui-region,server-driven-ui,phase20.5-overlay-menu-input-components,slot-aware-package-ui,workspace-file-browser,performance-fixtures,ui-review-harness,maintenance-validation,typography-registry-and-font-roles,phase20.6-theme-segregation-settings-ui,configuration-runtime}.md`: synchronized implementation, ownership, accessibility, performance, security, and review notes.
      - `tests/primitives_docs.rs`: deterministic Plan 088 wiki-index/content guard.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
      - `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`, `.agents/skills/clay-ui/references/components.md`, `.agents/skills/clay-ui/references/tokens.md`.
  - Test Cases to Write:
    - `wiki_index_links_every_wiki_page` and `plan088_code_wiki_documents_modernization_contract`: master-index navigation and required implementation markers.
    - `cargo test --test protocol primitives_docs`, `cargo test --test editor package_ui_conformance`, `cargo test --test editor ui_primitive_conformance`, and `cargo test --test editor editor_performance_invariants`: deterministic docs/catalog/conformance/hot-path coverage.
    - `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets --quiet`: Linux blocking verification.

### Task 13 Evidence (2026-08-15)

- Ran the per-task UI preflight before reviewing UI implementation for documentation: the UI guidance current at execution time; inspected `accessibility`; loaded `ibelick/fixing-accessibility` (1 skill, within prefer-1/max-3). Re-read `.agents/skills/clay-ui/SKILL.md`, `references/components.md`, and `references/tokens.md`; translated accessible-name, keyboard/focus, modal, state, contrast, and tool-boundary guidance to native Masonry/AccessKit rather than web ARIA.
- Read the project-wiki workflow/template, current owner pages, Plan 088 implementation paths, package/catalog/reference contracts, and project patterns. Updated the master wiki map and synchronized shell primitives, theme/typography, shell/tabs/panes, retained SDUI/package UI, overlay, file-browser, performance, maintenance, and visual-review documentation. Existing authoritative JS/package docs remain linked rather than duplicated.
- Added `tests/primitives_docs.rs::plan088_code_wiki_documents_modernization_contract`, which checks the Plan 088 index map and required markers across implementation pages without mutating files. The broader `wiki_index_links_every_wiki_page` guard remains green.
- Verification passed: `cargo fmt --all -- --check`; `cargo test --test protocol primitives_docs -- --test-threads=1` (27 passed); package UI conformance (10), UI primitive conformance (12), editor performance invariants (33); `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; and `cargo test --all-targets --quiet` (1556 library, 63 binary, 164 editor, 160 protocol, 198 runtime, 130 security; 2 library/1 security ignored). All window benchmark harnesses completed during the all-target run.
- No JS API, package permission, component kind, token, style variable, generated registry entry, or runtime behavior changed in this task, so `cargo run --bin update-doc-registry` was not needed. The wiki records the existing public API/reference boundary, cached theme/typography hot-path policy, package trust/conformance rules, path sanitization, responsive constraints, and visual-review blockers.
- Plan 088 is complete. Remaining follow-ups are intentionally explicit: stale WelcomeWidget recovery status, loading-fixture observability, live completion/Command Centre/other window-targeted captures, and renderer-level containment recapture.

## Compromises Made

- No UI-library migration, renderer rewrite, blur/filter pipeline, or speculative component suite. Existing Masonry/token system is sufficient unless task 2 proves a specific reusable gap.
- Kept one cross-cutting Plan 088 map in `docs/wiki/index.md` and extended existing owner pages instead of adding a duplicate phase page.
- Live GUI coverage remains evidence-producing rather than fully interactive on this host; no screenshot/AT-SPI blocker was converted into a false pass.
- Criterion window baselines remain advisory; only deterministic responsive/hot-path bounds are blocking.

## Further Actions

- **P1 — Plan 089 task `Close Plan 088 welcome recovery/status and loading-fixture follow-ups`:** Fix WelcomeWidget connection/status propagation and publish a loading fixture that actually exposes its intended SDUI state; recapture recovery/loading.
- **P1 — Plan 089 tasks `Add real Linux multi-window, DPI, font-scale, and Wayland validation` and `Perform visual screenshot and accessibility review of validation fixtures`:** Enable safe GNOME window targeting or add a no-focus harness action path, then recapture completion, Command Centre, settings, file browser, multi-tab/multi-pane, narrow/wide, and DPI states; verify retained clipping/containment visually.
- **P2 — Plan 089 task `Triage Plan 088 advisory Criterion regressions before policy promotion`:** Investigate local Criterion regressions before promoting advisory window numbers to any cross-machine policy.
- Xilem remains a separate post-stability experiment in Plan 091; this plan does not depend on it.
