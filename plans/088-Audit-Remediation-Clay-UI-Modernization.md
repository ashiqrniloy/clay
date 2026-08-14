# Audit Remediation: Clay UI Modernization

Prerequisites: Plans 086 and 087 complete; accessibility, review fixtures, entry state, and completion behavior are green.

Approved constraint: `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`. Modernization improves defaults and token consumption while preserving `theme.setTheme`, typed `designTokens`, `ResolvedUiTheme`, existing themes, and user-owned typography.

Source review: P1-4 and related P1/P2 UI/performance/test requirements in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`.

## UI Skill Gate (mandatory for every task)

Before reviewing existing UI, planning, designing, or changing any UI-related task in this plan — including theme, typography, tokens, components, layout, SDUI, overlays, accessibility, or visual evidence — run `npx ui-skills start`. Then inspect the relevant category, load the smallest useful set (prefer 1, never more than 3), and apply the loaded guidance in Clay's native Masonry/token context. Repeat this gate for each independently executed task; prior task evidence does not satisfy it. Record the command, category, selected skill slugs, and any routing blocker in that task's evidence. Load `.agents/skills/clay-ui/` plus its component/token references after routing.

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
      - `npx ui-skills start`; selected `web-design-guidelines` and `fixing-accessibility`; official current interface rules fetched.
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

- Ran `npx ui-skills start`, then loaded `vercel-labs/web-design-guidelines` and `ibelick/fixing-accessibility`; translated their hierarchy, focus, long-content, contrast, keyboard, and semantic-control guidance to Masonry tokens and AccessKit roles rather than web/CSS constructs. Reviewed the Clay catalog/token policy, Plan 087 harness/wiki, relevant shell/overlay/pane wiki pages, UI patterns, and the approved theme-configuration decision before recording direction.
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
      - UI preflight rerun for this task: `npx ui-skills start`; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility`.
      - Context7 `/ibelick/ui-skills` CLI reference: `categories` lists routing topics and `get <slug>` loads selected skill content; root protocol requires route → inspect → select → load → implement.
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

- Re-ran the mandatory preflight before this review: `npx ui-skills start`; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility` (2 skills, within the routing limit). Their web-specific rules were translated to Clay's catalog-first, typed-token, semantic-role, Masonry-native constraints.
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
      - UI preflight for this task: `npx ui-skills start`; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility`.
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

- Re-ran the mandatory UI preflight before source review: `npx ui-skills start`; inspected `systems` and `accessibility`; loaded `ibelick/baseline-ui` and `ibelick/fixing-accessibility` (2 skills, within the limit). Applied their hierarchy, state, accessible-name, keyboard, contrast, and minimal-change guidance to Clay's native Masonry/token paths rather than web/CSS constructs.
- Modernized the existing shell without changing pane/tab ownership or public APIs: welcome now reclaims stale workspace-browser space while preserving package fixed slots; split focus rings paint after pane children; the pinned `+` tab affordance uses cached state tokens on hover; status chrome reads `surface.control`, `text.primary`, spacing, and border tokens with legacy fallbacks; workspace and tab labels are basename/relative, bounded, control-free, and never fall back to absolute host paths.
- Fresh representative captures are retained under `code-reviews/screenshots/2026-08-15-plan088-task4-welcome/` and `code-reviews/screenshots/2026-08-15-plan088-task4-light/`. Both report `PASS`, contain Clay-window-only 913×1151 PNGs for dark/light welcome states, and expose named `Open File`/`Open Folder` controls plus status/pane semantics. No host path or secret appears in retained text evidence. Full opened-editor, browser-list, multi-tab/pane, narrow/wide, and interactive focus-state visual acceptance remains for the dedicated later review task; this is not a full-plan visual pass. The host still lacks safe window-targeting/portal keyboard control for those interactions.
- Verification passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets --quiet`; focused shell/editor/SDUI/file-browser/pane tests; and the tab-label sanitization regression. No new component kind, style variable, token, package authority, JS API, filesystem operation, IPC, document serialization, or paint/layout hot-path work was introduced.

- [ ] Modernize overlays, dialogs, settings, diagnostics, and package panels
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

- [ ] Make modernized layouts responsive to pane size, DPI, and user typography
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
      - Changed surface/layout modules from prior tasks.
      - `tests/editor_performance_invariants.rs` only for unique layout/hot-path contracts.
      - Plan 087 UI review fixtures for scale/size states.
    - References:
      - `docs/development/accessibility.md`, `docs/development/ui-observability.md`.
  - Test Cases to Write:
    - Narrow/wide, 1x/2x, UI size 6/12/24/96 where supported, long names/details/errors, multi-pane, scroll reachability, accessibility bounds match visual bounds.

- [ ] Add automated conformance and performance checks for modernization
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
      cargo test --test protocol package_ui_conformance::
      cargo test --test protocol ui_primitive_conformance::
      cargo test --test editor editor_performance_invariants::
      cargo bench --bench window_baselines --no-run
      ```
    - Files to Create/Edit:
      - `tests/package_ui_conformance.rs`, `tests/ui_primitive_conformance.rs`, changed module tests.
      - `tests/editor_performance_invariants.rs`, `benches/window_baselines.rs`, `docs/development/performance.md` only as required.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
  - Test Cases to Write:
    - Theme/state/layout matrix, no raw values, all states complete, contrast valid, no extra hot-path work.

- [ ] Perform visual screenshot and accessibility review of changed UI
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
      - `code-reviews/screenshots/2026-08-14-plan088-modernization/*.png`.
      - This plan: findings/comparison.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Keyboard focus/order, visible focus, semantics, contrast, modality, announcements, long content, all matrix states.

- [ ] Update the UI catalog and package authoring contract
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
      - `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`, `docs/index.md` if links change.
      - `tests/package_ui_conformance.rs`, `tests/primitives_docs.rs`.
    - References:
      - `.agents/skills/create-plan/references/clay.md` package UI/layout task.
  - Test Cases to Write:
    - Source/catalog/package-guide parity and package rejection for internal-only surfaces.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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
      - Existing theme API docs/registry only if contract changes; new docs only for proven new public behavior.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
  - Test Cases to Write:
    - Existing theme/typography API docs, index, registry, lookup, facade, and custom properties remain complete.

- [ ] Create or verify Clay configuration APIs and canonical example
  - Acceptance Criteria:
    - Functional: `~/.config/clay/init.js` theme/typography configuration continues to work unchanged; no hidden redesign settings; if additive typed options are introduced, docs and `examples/init.js` cover each exactly once.
    - Performance: Configuration reload resolves atomically and does not cause redundant layout/paint churn.
    - Code Quality: Option names/defaults/enums match parser and docs; `node --check examples/init.js` passes.
    - Security: Configuration retains first-party theme allowlist, typed bounds/contrast, and no filesystem/network/shell/native UI authority.
  - Approach:
    - Documentation Reviewed:
      - `configuration-system.md`, `examples/init.js`, current theme API docs.
    - Options Considered:
      - New parallel appearance config: rejected.
      - Preserve current APIs and update only if additive semantics require it: chosen.
    - Chosen Approach:
      - Treat current theme configurability as a blocking compatibility test, not optional documentation.
    - API Notes and Examples:
      ```bash
      node --check examples/init.js
      cargo run --bin update-doc-registry
      ```
    - Files to Create/Edit:
      - `examples/init.js`, `docs/reference/clay-js-api/theme/**`, `docs/index.md`, generated registry only for actual API/schema changes.
    - References:
      - `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`.
  - Test Cases to Write:
    - Canonical example loads dark/light theme and user typography; reload is atomic; invalid override preserves prior valid generation.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Execute/update modules 01, 02, 03, 04, 07, 09, 10, 11, 13, and 14 for the full modernized state/theme/layout matrix.
    - Performance: Record perceived typing, switching, filtering, scrolling, resize, and theme reload behavior.
    - Code Quality: Add stable numbered steps, negative checks, and known ceilings without deleting existing coverage.
    - Security: Include theme validation, sanitized labels, package UI confinement, and modal focus checks.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` module map/coverage matrix.
    - Options Considered:
      - New standalone visual test document: rejected; use indexed modules.
      - Update affected modules: chosen.
    - Chosen Approach:
      - Keep reusable manual verification close to owning behavior.
    - API Notes and Examples:
      ```bash
      cargo build
      scripts/capture-ui-review.sh --fixture <state> --output <artifact-dir>
      ```
    - Files to Create/Edit:
      - Relevant module files listed above and `test-plan/index.md`.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
  - Test Cases to Write:
    - Full visual/accessibility/theme matrix and negative checks from prior tasks.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki teaches modern visual architecture, token/theme flow, component/chrome ownership, responsive behavior, accessibility, extension rules, and tests; index links pages.
    - Performance: Document cached resolution and hot-path invariants.
    - Code Quality: Link authoritative public theme docs instead of duplicating them; include source/test paths and extension guidance.
    - Security: Document validation/authority boundaries and theme/package restrictions.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Update after each surface: rejected as churn.
      - One final synchronized update: chosen.
    - Chosen Approach:
      - Update existing UI/theme/shell pages and master index after gates/review pass.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/shell-primitives.md
      docs/wiki/modules/editor-theme-registry.md
      docs/wiki/modules/masonry-shell.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md` and relevant modules above plus package UI/conformance pages.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
  - Test Cases to Write:
    - Manual wiki link/content review and deterministic documentation checks.

## Compromises Made

- No UI-library migration, renderer rewrite, blur/filter pipeline, or speculative component suite. Existing Masonry/token system is sufficient unless task 2 proves a specific reusable gap.

## Further Actions

- Xilem remains a separate post-stability experiment in Plan 091; this plan does not depend on it.
