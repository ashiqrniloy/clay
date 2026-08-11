# Phase 20.4 Core Component Uplift on the Existing Catalog

## Source

- `src/masonry_sdui.rs`
- `src/masonry_editor.rs`
- `src/editor/surface.rs`
- `src/shell/primitives.rs`
- `src/shell/theme.rs`
- `src/shell/components.rs`
- `src/shell/package_ui.rs`
- `src/shell/mod.rs`
- `tests/ui_primitive_conformance.rs`
- `tests/editor_performance_invariants.rs`
- `tests/primitives_docs.rs`
- `tests/rust_visibility_api_mapping.rs`
- `tests/clay_js_doc_registry.rs`
- `docs/reference/packages/creating-packages.md`
- `docs/reference/primitives/ui-chrome-primitives.md`
- `.agents/skills/clay-ui/references/components.md`
- `.agents/skills/clay-ui/references/tokens.md`
- `plans/065-Phase20.4-Core-Component-Uplift-on-the-Existing-Catalog.md`

## Overview

Phase 20.4 is a **restyle-only** phase: every implemented catalog component is restyled to a minimalist aesthetic using the Phase 20.1 design tokens and Phase 20.2 chrome primitives, **without changing component kinds, style-variable schemas, token names, or the public Clay JS API**. It adds no `ComponentKind`, no `ThemeTokenType`, no core/package token name, no style variable, and no package-facing API. All new paint/interaction-state helpers are `pub(crate)` or test-local. First-party `@clay/*` packages render unchanged (zero manifest/style/token edits).

The phase closes the gap between Phase 20.1/20.2 token/primitive foundations and the actual component paint path: before Phase 20.4, SDUI component paint resolved colors and spacing from `SduiThemeStyle::default()` (core fallbacks only), ignored the active `ResolvedUiTheme`, and had no interaction states (buttons/lists were stateless fills). Phase 20.4 routes SDUI and editor chrome paint through the active theme and makes every interactive component render all five `InteractionState`s from state tokens.

## Reusable Capability Before New Code

Phase 20.4 inherited a complete token + primitive substrate and added **no new primitive, kind, token, or variable**. The reuse inventory (Plan 065 task 2) confirmed:

| Capability | Owner | Phase 20.4 reuse |
| --- | --- | --- |
| `ResolvedUiTheme` | `src/shell/theme.rs` (Phase 20.1) | Active-theme typed reads via `color`/`scalar_f64`/`opacity`/`dimension`/`typography` accessors |
| `SduiThemeStyle` | `src/shell/theme.rs` (Phase 20.1) | SDUI paint geometry/colors; now resolved from the active theme |
| `ThemeTokenResolver` | `src/shell/theme.rs` (Phase 20.1) | Used **only** at install time (core fallbacks); never in the paint path |
| `StyleRegistry` | `src/editor/theme.rs` (Phase 18.15) | Editor caret/selection/diagnostics color — unchanged |
| `TypographyRegistry` / `UiTextVariant` | `src/editor/typography.rs` (Phase 18.16.5/20.1) | UI text metrics — unchanged |
| Chrome primitives (`paint_panel_chrome`, `paint_scroll_chrome`, `paint_tooltip_shell`, `paint_focus_ring`, …) | `src/shell/primitives.rs` (Phase 20.2) | Panel/overlay/scrollbar/focus chrome — unchanged surface, now driven by threaded state |
| `InteractionState` enum | `src/shell/primitives.rs` (Phase 20.2) | 5-state derivation for every interactive component |
| State tokens (`surface.hover`/`active`/`disabled`, `text.disabled`, `border.focus`, `opacity.disabled`) | `src/shell/theme.rs` (Phase 20.1) | Drives component state colors |
| `ComponentKind` catalog (11 implemented + 4 reserved) | `src/shell/components.rs` (Phase 18.3) | Reused unchanged |
| 14 typed style variables | `src/shell/components.rs` | Reused unchanged |

Conclusion: reuse-first. No new kind, token, variable, or primitive was needed; the work was routing and state threading, not new capability.

## Locked Generic Phase 20.4 Gaps (closed)

1. **Active-theme routing for SDUI paint.** SDUI paint resolved from `SduiThemeStyle::default()` (core fallbacks), ignoring user/theme-package design-token overrides. Closed by `SduiThemeStyle::from_ui_theme(&ResolvedUiTheme)` + `SduiNativeState::theme_style()`; the free `sdui_theme_style()` fallback was deleted.

2. **`ResolvedUiTheme::typography()` accessor.** `SduiThemeStyle` needed typography resolution from the active theme; the resolver's `resolved()` is private. Closed by a public-to-crate `typography()` accessor matching the existing accessor pattern.

3. **Spacing rhythm + density.** `panel_padding` read legacy `spacing.panel` (14, off-grid). Closed by reading `spacing.md` (16, 4pt grid) × `spacing_scale` (the `spacing_scale()` density scale 0.875/1.0/1.125). Default panel padding shifted 14→16px.

4. **Interaction-state completeness.** Buttons, lists, labels, status items were stateless fills. Closed by `component_state_color`, `list_row_fill_color`, `disabled_text_color` helpers + per-component `interaction_state` derivation (Disabled > Active > Hover > Focus > Rest).

5. **Editor scrollbar state.** `paint_vertical_scrollbar` hardcoded `InteractionState::Rest`. Closed by `EditorSurface::scrollbar_interaction_state` (O(1) pointer hit-test) + `paint_scroll_chrome` Rest/Disabled → `opacity.disabled`, Hover/Active/Focus → `opacity.full`.

6. **Status bar token-driven insets.** `paint_status_line` hardcoded `12.0`/`24.0` insets. Closed by `spacing.sm` × `spacing_scale()` + `paint_divider` hairline at the top.

7. **Conformance latent gap.** `non_test_body` in both `tests/ui_primitive_conformance.rs` and `tests/editor_performance_invariants.rs` truncated `masonry_sdui.rs` at its first `#[cfg(test)] use` (line 39), so the color/size/hot-path guards were scanning only 38 lines and silently missing the real paint code. Closed by preferring the `mod tests` boundary (strict improvement).

## How It Works

### Active-theme routing

`SduiNativeState` already held a `ui_theme: ResolvedUiTheme` field (Phase 20.1) but the paint path never read it. Phase 20.4 adds:

- `ResolvedUiTheme::typography(token) -> Option<UiTextVariant>` — matches `ResolvedThemeValue::Typography`.
- `SduiThemeStyle::from_ui_theme(&ResolvedUiTheme)` — resolves `panel_padding` (`spacing.md` × `spacing_scale()`), backgrounds (`surface.panel`/`control`/`list`/`selected`), text (`text.primary`/`muted`), and typography (`typography.title`/`body`/`status`) from the **active** theme, falling back to core tokens only when an override is absent.
- `SduiNativeState::theme_style(&self) -> SduiThemeStyle` — delegates to `from_ui_theme(&self.ui_theme)`; the cached resolved style is recomputed per paint call from already-resolved typed values (no string parsing, no `ThemeTokenResolver`).

All `&self` SDUI paint methods (`paint_node`, `paint_package_component`, `paint_text`, accessibility/action-region collectors, `row_rect`) read `self.theme_style()` (the `theme_style` helper) instead of the deleted free `sdui_theme_style()`. `row_rect` moved from a free function into `impl SduiNativeState` so it can read `self.theme_style().panel_padding`.

### Interaction-state derivation

`SduiNativeState` gains three client-local fields (rebuilt-aware, not persisted across snapshots): `pointer_pos: Option<Point>`, `pointer_pressed: bool`, `focused_action: Option<SduiActionIntent>`. `interaction_state(rect, action, disabled) -> InteractionState` derives the state with precedence **Disabled → Active → Hover → Focus → Rest**:

- **Disabled**: `disabled` flag true (package-declared `PackageUiComponentTree.disabled`/`PackageUiListItem.disabled`; typed sidebar path is always non-disabled).
- **Active**: `pointer_pressed && pointer_pos` inside `rect`.
- **Hover**: `pointer_pos` inside `rect`.
- **Focus**: `focused_action == action`.
- **Rest**: otherwise.

`MasonyEditor::on_pointer_event` feeds the editor's SDUI state: `Down` sets `pointer_pos` + `pointer_pressed` + `focused_action` on an action hit (and requests repaint); `Move` always sets `pointer_pos` (passive hover, not only during drag) then extends selection if active; `Up` clears `pointer_pressed`; `Cancel`/`Leave` call `clear_pointer_state`. The `Up`/`Cancel` arms were split out of the original `ctx.is_active()`-guarded combined arm — a latent bug where a `Down` hitting an SDUI action (no `capture_pointer`) left `is_active()=false`, so `Up` never matched and `pointer_pressed` stayed true.

### Per-kind state paint

`paint_package_component` (string-kind package path) and `paint_node` (typed sidebar path) apply state colors through the new `src/shell/primitives.rs` helpers:

- **button**: fill via `component_state_color(theme, "surface.control", state)` (Rest→`surface.control`, Hover→`surface.hover`, Active→`surface.active`, Focus→`accent.primary` + `paint_focus_ring`, Disabled→`surface.disabled`); label uses `disabled_text_color` when disabled.
- **list**: rows fill via `list_row_fill_color(theme, state, selected)` (selected→`surface.selected`, Hover→`surface.hover`, Active→`surface.active`, Disabled→`surface.disabled` × `opacity.disabled`); disabled rows are gated out of the action-region collector.
- **label / statusItem**: `disabled_text_color(theme)` (`text.disabled` × `opacity.disabled`) when disabled.
- **panel / overlay / scroll / portal / flex / stack / editorView**: route chrome through existing state-complete primitives (`paint_panel_chrome`, `paint_tooltip_shell`); containers have no per-component fill.

Disabled gating also runs in `collect_package_action_regions`: a disabled button or list item does not push a `SduiVisibleAction`, so it is not hit-testable and cannot emit a `ClientMessage::SduiAction`. Accessibility entries still include disabled components (visible to assistive tools), only action eligibility is gated.

### Editor chrome

`EditorSurface` gains `pointer_pos`/`pointer_pressed` (chrome-local, separate from SDUI state) + `scrollbar_interaction_state(rect, available_height)` (O(1) hit-test against `scrollbar_thumb_rect`). `paint_vertical_scrollbar` passes the derived state to `paint_scroll_chrome`, which now maps Rest/Disabled → `opacity.disabled` (dim, near-invisible) and Hover/Active/Focus → `opacity.full`. `MasonyEditor::on_pointer_event` feeds editor chrome state on Down/Move/Up/Cancel/Leave; a press inside `scrollbar_thumb_rect` skips caret placement and pointer capture (returns `(false, true)` + repaint) so the thumb press does not start a text selection. Caret/selection/diagnostics stay on `StyleRegistry` (`base.caret`/`base.selection`/`diagnostic_style`); no new `BaseUiColorKey`.

### Status bar

`paint_status_line` reads `inset = editor.ui_theme().scalar_f64("spacing.sm") * spacing_scale()` (symmetric, preserves prior layout at default density) and paints a `paint_divider` hairline at the top. `EditorSurface::ui_theme()` is a new `pub(crate)` read-only accessor.

### Structural observability

Plan 065 task 7 added a test-local `ComponentStatePalette { fill, border, text }` + `component_state_palette(theme, kind, state)` helper that pins the resolved fill/border/text color for each of the 11 kinds × 5 states (55 cells) against exact core token values, plus the full `cargo test --all-targets` package regression gate (packages/ unmodified).

## State-Color Helpers

`src/shell/primitives.rs` adds three `pub(crate)` helpers (re-exported from `src/shell/mod.rs`):

| Helper | Signature | Token mapping |
| --- | --- | --- |
| `component_state_color` | `(theme, rest_token, state) -> Color` | Rest→`rest_token`, Hover→`surface.hover`, Active→`surface.active`, Focus→`accent.primary`, Disabled→`surface.disabled` × `opacity.disabled` |
| `list_row_fill_color` | `(theme, state, selected) -> Color` | selected & Rest/Focus→`surface.selected`, Hover→`surface.hover`, Active→`surface.active`, Disabled→`surface.disabled` × `opacity.disabled` |
| `disabled_text_color` | `(theme) -> Color` | `text.disabled` × `opacity.disabled` |

`apply_alpha(color, factor)` multiplies the alpha channel (matching the existing `to_rgba8`/`from_rgba8` pattern; `peniko`'s `Brush::with_alpha` *sets* rather than multiplies).

## Additive Compatibility Contract

- **Zero breaking changes**: No new component kind, style variable, token, or Clay JS API was added; no `ComponentKind`, typed style variable, `ThemeTokenType`, core/package token name, `fontRole` value, `UiTextVariant`, or Clay JS API was renamed, removed, or changed. The 11 implemented + 4 reserved kinds still parse in `components.rs` and are cataloged in `components.md` (asserted by `no_component_kind_or_token_renamed`).
- **Packages render unchanged**: `git status --porcelain packages/` is empty across all Phase 20.4 tasks. First-party packages cannot reach `pub(crate)` paint/state helpers (asserted by `existing_packages_render_unchanged`).
- **Token consumption is additive-only**: Phase 20.4 consumes *existing* Phase 20.1 state/spacing/opacity tokens; no new token was added.
- **`StyleRegistry` remains the editor color authority**; `ResolvedUiTheme` remains the SDUI/shell chrome authority. The two registries stay separate.
- **`panel_padding` shift (14→16px)** is the only user-visible geometry change and follows the 4pt grid documented in `tokens.md`; the typed sidebar tree's hit-test geometry moved with it (tests click action-region centers, not hardcoded pixels).

## Hot-Path Boundary

Phase 20.4 paint paths read **cached** resolved values only. Forbidden in `masonry_sdui.rs`/`surface.rs`/`masonry_editor.rs`/`primitives.rs` non-test paint code (asserted by `hot_path_no_theme_resolution_or_package_js`):

- `ThemeTokenResolver::new()` / `from_resolver()` / `core_theme_value` (theme re-resolution happens once at install time into `ResolvedUiTheme`).
- `Deno.core`, `op_clay_theme_set_*`, package JavaScript, IPC waits, `reqwest`/`ureq`/`TcpStream`/`Command::new`/`std::fs::read`.

`SduiThemeStyle::from_ui_theme` reads already-resolved typed values; `interaction_state` and `scrollbar_interaction_state` are O(1) pointer-rect tests. No layout mutation during the Masonry layout pass (paint paths push scenes only).

## Security Boundary

- All new helpers are `pub(crate)` or private; `PackageUiComponentTree.disabled`/`PackageUiListItem.disabled` are `pub(crate)`. The Phase 20.4 visibility audit (`phase20_4_introduces_no_unexposed_public_rust_function`) asserts no new bare `pub fn` and that none appear in `src/server/ops/ui.rs` deno_core ops or `runtime/js/*` facades.
- No new Clay JS API (`ui.componentStateColor`, `theme.spacingRhythm`, etc.) was added (`clay_js_api_inventory_unchanged_or_documented`). Density/state-token/spacing control stays on the existing `theme.setTheme` + `designTokens` path.
- Disabled gating removes action authority (a disabled button cannot emit a `SduiAction`); it does not change SDUI validation, command execution, or server-side authority.
- Observability types stay `pub(crate)`/test-local; `SduiObservableSnapshot` is unchanged; `ComponentStatePalette` lives in `mod tests`.

## Deferred (Compromises)

- **Per-element spacing differentiation** (`spacing.xs`/`sm`/`lg` per kind) deferred to a later phase; `panel_padding` uses a single `spacing.md` × scale value.
- **Focus is click-to-focus only** — no Tab traversal. The `focused_action` mechanism is ready; input-wiring for keyboard focus traversal is deferred.
- **Container collapse/resize affordance state** stays `Rest` (shell drag state is not visible to SDUI paint).
- **Button Focus fill `accent.primary`** is strong; Phase 20.6 can tone it down.
- **Scrollbar thumb-drag scrolling** deferred: a press inside the thumb sets the Active visual only; the drag math is not implemented (scrolling stays wheel/keyboard).
- **Rest scrollbar reuses `opacity.disabled`** (no dedicated `opacity.scrollbar.rest` token); the upgrade path is a dedicated token if Rest needs to differ from Disabled.
- **Editor chrome size constants** (`SCROLLBAR_WIDTH`/`MARGIN`/`MIN_THUMB`, `TEXT_INSET`) are not yet routed through `dimension.*` tokens (a bigger refactor outside Phase 20.4 scope); `surface.rs` is intentionally excluded from the conformance size guard.
- **Full manual accessibility audit** deferred to Phase 20.7 (structural role/state checks cover this phase).

## Tests

- `src/shell/primitives.rs`: `component_state_color_maps_all_five_states_to_tokens`, `list_row_fill_color_honors_selected_and_state` pin exact core token colors per state.
- `src/masonry_sdui.rs`: `sdui_paint_uses_active_theme_not_core_fallbacks` (design-token overrides reach `theme_style()`), `sdui_spacing_rhythm_scales_with_density` (`panel_padding = spacing.md × spacing_scale()` for compact/default/spacious), `disabled_component_applies_opacity_disabled_and_gates_actions`, `focused_component_tracks_focus_and_derives_interaction_state`, `each_component_kind_renders_all_five_states` (55-cell matrix), `component_state_colors_are_token_derived`.
- `src/editor/surface.rs`: `editor_scrollbar_reflects_hover_and_active_state`, `editor_caret_selection_diagnostics_use_base_ui_colors`.
- `tests/ui_primitive_conformance.rs`: `sdui_paint_resolves_from_active_theme_not_core_fallback_resolver`, `sdui_paint_wires_focus_ring_and_state_colors_for_interactive_components` (source guards); `masonry_editor.rs` is in both the color and size conformance file lists; `surface.rs` is in the color list.
- `tests/editor_performance_invariants.rs`: `hot_path_no_theme_resolution_or_package_js`; `non_test_body` now scans the full `masonry_sdui.rs`/`masonry_editor.rs` paint bodies.
- `tests/primitives_docs.rs`: `package_guide_documents_phase20_4_uplift`, `clay_ui_catalog_notes_state_completeness`, `primitives_reference_documents_component_state_color`, `no_component_kind_or_token_renamed`, `existing_packages_render_unchanged`.
- `tests/rust_visibility_api_mapping.rs`: `phase20_4_introduces_no_unexposed_public_rust_function`.
- `tests/clay_js_doc_registry.rs`: `configuration_api_covers_phase20_4_needs_or_defers`, `configuration_api_no_authority_grant`, `clay_js_api_inventory_unchanged_or_documented`.

```bash
cargo test --test editor editor_performance_invariants::hot_path_no_theme_resolution_or_package_js
cargo test --test protocol primitives_docs::package_guide_documents_phase20_4_uplift
cargo test --test protocol primitives_docs::no_component_kind_or_token_renamed
cargo test --test protocol primitives_docs::existing_packages_render_unchanged
cargo test --all-targets
```

## Phase Boundary

- **Phase 20.1:** typed design tokens, `ResolvedUiTheme`, typography hierarchy, panel/density defaults. (Complete.)
- **Phase 20.2:** native chrome primitive library (divider, focus ring, panel chrome, scroll chrome, badge, kbd, icon, tooltip shell). (Complete.)
- **Phase 20.3:** user-facing split/panel resizing, collapse/restore, persistence, inert package layout intents. (Complete.)
- **Phase 20.4:** component visual uplift + interaction-state rendering on the existing catalog, active-theme routing, spacing rhythm, status bar tokens. (This phase. Restyle-only.)
- **Phase 20.5:** reserved overlay/menu/input component kinds (`dropdown`, `collapse`, `modal`, tooltip content/trigger wiring, tabs, toast).
- **Phase 20.6:** Modus Operandi/Vivendi packages, light/dark selection semantics, theme/font settings UI, accent tone-down.
- **Phase 20.7–20.8:** conformance enforcement, full accessibility audit, continuing reference/catalog maintenance.

Phase 20.4 must not add a `ComponentKind`, style variable, token, primitive, Clay JS API, or package manifest change. It must not implement deferred component kinds, thumb-drag scrolling, Tab focus traversal, or the Modus themes.

## Related

- [Phase 20.2 UI Primitive Library Primitive Review](phase20.2-ui-primitive-library-primitive-review.md)
- [Phase 20.1 UI Design Language Primitive Review](phase20.1-ui-design-language-primitive-review.md)
- [Shell Primitives](shell-primitives.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md)
- [UI Chrome Primitives Reference](../../reference/primitives/ui-chrome-primitives.md)
- [Package Authoring Guide](../../reference/packages/creating-packages.md)
- [Plan 065](../../../plans/065-Phase20.4-Core-Component-Uplift-on-the-Existing-Catalog.md)