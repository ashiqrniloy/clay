# Clay UI Component and Primitive Catalog

Single source of truth for reusable UI components and primitives. Update this file in the same change that adds/modifies/removes any entry.

Status legend: **implemented** (usable now), **reserved** (name locked, validation rejects use until its phase), **planned** (approved for a future UI revamp phase), **internal** (Clay-native surface, not package-facing).

## Package-Facing Component Kinds

Declared in `src/shell/components.rs` (`ComponentKind`). Packages compose these; Clay renders them. All emit inert command intents. Phase 20.4 made every implemented kind state-complete (`Rest`/`Hover`/`Active`/`Focus`/`Disabled` derived from state tokens) and routed SDUI paint through the active `ResolvedUiTheme`; see the per-kind Interaction / spacing notes below the table.

| Kind | Status | Purpose | Notes |
|------|--------|---------|-------|
| `editorView` | implemented | Editor surface placed in a pane `main` slot | One content host per pane leaf; Phase 22.1: panes are generic content hosts, not just editor views; Phase 22.2: content hosts serve live per-pane document views (`PaneDocumentView` — independent `EditorSurface`, caret/selection/viewport, status line; pane↔document mapping is client-local, duplicate opens focus the existing pane); chrome editor-`StyleRegistry`-driven |
| `panel` | implemented | Container for slot content (`left`/`right`/`top`/`bottom`) | Fixed or transient; chrome via `paint_panel_chrome`; size user-configurable via slot state |
| `label` | implemented | Static text | Supports text font role; disabled → `text.disabled` × `opacity.disabled` |
| `button` | implemented | Action trigger | Variants: `default`, `muted`, `primary`, `danger`; fill via `component_state_color("surface.control", state)`; focus ring on `Focus`; disabled gates action |
| `list` | implemented | Row collection | Row items can carry title + detail text; row fill via `list_row_fill_color(state, selected)`; disabled rows gate action |
| `flex` | implemented | 1D layout container | Row/column with `gap` token; container, no chrome of its own |
| `stack` | implemented | Z-stacked container | Base for overlay compositions; container, no chrome of its own |
| `overlay` | implemented | Anchored floating layer | Anchor + dismissal + focus policy; chrome via `paint_tooltip_shell` |
| `scroll` | implemented | Scrollable region | Scrollbar chrome from `paint_scroll_chrome`; container, no body chrome |
| `portal` | implemented | Renders outside normal slot flow | For transient surfaces; container, no chrome of its own |
| `statusItem` | implemented | Status bar entry | Supports text font role; disabled → `text.disabled` × `opacity.disabled` |
| `dropdown` | implemented | Single-select drop-down | Phase 20.5: button-like trigger row; `Role::ComboBox`; keyboard nav (ArrowUp/Down cycles the widget's `selected_index`, Enter/Space confirms); open list painted by `PackageDropdown`; fill via `component_state_color("surface.control", state)` |
| `collapse` | implemented | Expand/collapse section | Phase 20.5: title row with `clay.ui.collapseToggle` action; `Role::Group`; Enter/Space toggles `PackageCollapse.expanded`; content shown/hidden via a layout clip |
| `modal` | implemented | Blocking dialog | Phase 20.5: `paint_tooltip_shell` chrome (painted by the overlay host) + title + children; `Role::Dialog`; Tab focus-trap cycles the modal's widget-local focusable descendants; `z.modal` stacking |
| `textInput` | implemented | Single-line editable text field | Phase 20.5: bordered field, placeholder in `text.muted`, focus ring, validation-state border (`diagnostic.error`/`warning`/`success` or `border.subtle`); `Role::TextInput`; `style.validationState` and `style.placeholderColor` style variables |
| `table` | reserved | Tabular data | Deferred; no first-party package need identified as of Phase 20.5 |

### Phase 20.4 interaction-state and spacing rhythm notes

Phase 20.4 restyled the implemented kinds to the minimalist design language using Phase 20.1 tokens and Phase 20.2 primitives, with no kind/style-variable/token-name change. Plan 070 later moved every kind onto a retained reconciled Masonry widget (see below); the interaction-state contract below is unchanged — it is now derived per-widget from Masonry `EventCtx`/`QueryCtx` state (`is_disabled`/`is_active`/`is_focus_target`/`is_hovered`) rather than a god-object `interaction_state` field.

**Interaction states** (derived per-widget from Masonry pointer/focus state; precedence `Disabled` > `Active` > `Hover` > `Focus` > `Rest`):
- `button`: all five states; `Rest`=`surface.control`, `Hover`=`surface.hover`, `Active`=`surface.active`, `Focus`=`accent.primary` + `paint_focus_ring` (`border.focus`), `Disabled`=`surface.disabled`×`opacity.disabled` with `text.disabled` text and action gated.
- `list`: per-row `Rest`/`Hover`/`Active`/`Focus` honor `selected` (`surface.selected` vs `surface.list`); `Hover`/`Active` override selection; `Disabled` dims and gates the action.
- `label` / `statusItem`: text `text.muted` at `Rest`; `Disabled` → `text.disabled`×`opacity.disabled`; `Focus` paints a focus ring. No fill.
- `panel` / `overlay`: chrome via `paint_panel_chrome` / `paint_tooltip_shell` (state-independent chrome; collapse/resize affordances route through the primitive, currently `Rest`).
- `editorView`: chrome is editor-`StyleRegistry`-driven (caret/selection/diagnostics); the scrollbar reflects `Hover`/`Active` from pointer state via `paint_scroll_chrome`. No SDUI state-token fill.
- `flex` / `stack` / `scroll` / `portal`: containers — recurse children, no chrome of their own; `InteractionState` is not applicable.

**Spacing rhythm**: SDUI panel padding reads `spacing.md` (16) × `spacing_scale()` (`compact`=0.875 / `default`=1.0 / `spacious`=1.125). The status bar uses `spacing.sm` × `spacing_scale()` insets with a `border.hairline` top divider. Per-element `spacing.xs`/`sm`/`lg` differentiation is deferred to a later spacing pass.

**Active-theme routing**: component paint reads the active `ResolvedUiTheme` via `SduiThemeStyle::from_ui_theme` (SDUI widgets) / the package widget's `ui_theme` field; theme `clay.contributions.designTokens` overrides flow through to component paint automatically.

### Plan 070 retained reconciliation notes

Plan 070 replaced the immediate-mode `SduiNativeState::paint` compatibility bridge with a retained reconciled Masonry subtree. Each package-facing kind now maps to a real Masonry widget that owns its paint, interaction state, focus, scroll, and a11y:

- **SDUI tree** (`SduiRegionWidget`, `src/masonry_sdui_region.rs`): `label`→`SduiLabel`, `button`→`SduiButton`, `list`→`SduiListRow` (column of rows), `editorView`→`EditorViewWidget` (binding/slot component; the editor canvas itself stays bespoke-painted by `EditorWidget`), under a Clay-owned `SduiScrollViewport`. The region is a real child of `EditorWidget`; sidebar chrome is painted by the region widget.
- **Package component trees** (`PackageRegionWidget`, `src/masonry_package_region.rs`): `label`/`statusItem`→`PackageLeaf`, `button`→`PackageButton`, `list` rows→`PackageListRow`, `collapse`→`PackageCollapse`, `dropdown`→`PackageDropdown`, `textInput`→`PackageTextInput` (wraps Masonry `TextArea<true>`), `modal`→`PackageModal` (Dialog + Tab focus-trap). Fixed panels are hosted by `PackagePanelHost`; transient overlays + the active menu by `PackageOverlayHost`; both are children of `EditorWidget`.
- **State moved out of the god-object**: `dropdown_selected`, `collapse_expanded`, `modal_focusable_intents`, `focused_action`, and pointer hit-test state are deleted from `SduiNativeState`; each is now widget-local (`PackageDropdown.selected_index`, `PackageCollapse.expanded`, `PackageModal` focusable list, etc.). The legacy `paint_package_component`/`paint_package_overlays` immediate-mode paths are deleted.
- **Compatibility guarantee**: no `ComponentKind`, style-variable, token-name, or package-facing contract change — the cutover is a client-internal substrate change. Packages require no manifest or style edit.

### Phase 20.5 overlay, menu, and input component notes

Phase 20.5 promoted `dropdown`, `collapse`, `modal` from reserved to implemented, added `textInput`, and uplifted all transient surfaces onto the shared overlay primitive.

**New kind interaction states**:
- `dropdown`: `Rest`/`Hover`/`Active`/`Focus`/`Disabled` via `component_state_color("surface.control", state)`; focus ring on `Focus`; the open item list is painted by `PackageDropdown` from its `items` (single source of truth for trigger label + selectable rows); selected item label from the widget's `selected_index`; ArrowUp/Down cycles, Enter/Space confirms and closes.
- `collapse`: title row with `clay.ui.collapseToggle` action intent; focus ring on `Focus`; content children are shown/hidden by `PackageCollapse` via a layout clip (expanded/collapsed is widget-local state); Enter/Space toggles.
- `modal`: `paint_tooltip_shell` chrome (state-independent, painted by the overlay host behind children); title + children; `Role::Dialog`; Tab/Shift+Tab cycles the modal's focusable descendants (focus trap, widget-local list); `z.modal` stacking order.
- `textInput`: bordered field with `surface.control` fill; validation-state border color (`diagnostic.error`/`warning`/`success` or `border.subtle`); focus ring on `Focus`; placeholder text in `text.muted`; `Role::TextInput`; `style.validationState` (`none`/`error`/`warning`/`success`) and `style.placeholderColor` (color-role token) style variables.

**Z-level stacking**: `TransientPackageOverlay` carries a `z_level_token: &'static str`; `PackageOverlayHost` sorts children by z-order: `z.overlay` (0) < `z.modal` (1) < `z.tooltip` (2). `from_menu_session` sets `"z.overlay"`; modal overlays set `"z.modal"`; tooltip-anchored overlays set `"z.tooltip"`.

**Surface origin**: `TransientMenuOrigin` (`CommandPalette`/`ContextMenu`/`MenuBar`) on `TransientMenuSession` selects the overlay anchor: `Bottom`/`Pointer`/`Main` respectively. `TransientPackageOverlay::from_menu_session` reads `session.origin()` instead of hardcoding `Bottom`.

**Overlay cursor inset**: `PackageOverlayHost` reads `ui_theme.scalar_f64("spacing.panel")` for overlay padding, consistent with `paint_tooltip_shell` which reads `ResolvedUiTheme` tokens directly.

## Typed Style Variables

Validated in `src/shell/components.rs`. Token-backed variables must reference a known token of the matching type; raw colors/CSS are rejected.

| Variable | Token type / enum | Applies to |
|----------|-------------------|------------|
| `background` | color-role token | Surfaces |
| `contentColor` | color-role token | Foreground content |
| `borderColor` | color-role token | Borders/dividers |
| `accentColor` | color-role token | Accents, focus |
| `padding` | spacing token | Inner spacing |
| `gap` | spacing token | Sibling spacing in `flex` |
| `rowHeight` | spacing token | `list` rows |
| `inset` | spacing token | Overlay offset |
| `radius` | radius token | Corner radius |
| `typography` | typography token | Text hierarchy level (`typography.body`, `typography.title`, `typography.status`, `typography.display`, `typography.section`, `typography.detail`, `typography.caption`) |
| `opacity` | opacity token | Disabled/muted states |
| `fontRole` | enum: `ui`, `monospace`, `proportional` | Text components |
| `variant` | enum: `default`, `muted`, `primary`, `danger` | `button`, emphasis |
| `placeholderColor` | color-role token | `textInput` placeholder text (Phase 20.5) |
| `validationState` | enum: `none`, `error`, `warning`, `success` | `textInput` border state (Phase 20.5) |

## Clay-Native Surfaces (internal)

| Surface | Status | File | Purpose |
|---------|--------|------|---------|
| Shell root widget | internal | `src/masonry_shell.rs` | `ClayShellWidget`, owns working area above editor |
| Pane split tree | internal | `src/shell/layout.rs` | Horizontal/vertical splits, ratio 0.05–0.95 |
| Fixed panel slots | internal | `src/shell/layout.rs` | `left`/`right`/`top`/`bottom` with size/min/max/visible/collapsed/resized_by_user; the Clay workspace browser may be absent entirely when its per-tab visibility flag is off |
| Status bar | internal | editor/shell paint | Uses `statusBg`/`statusText` theme keys |
| Transient menu | internal | `src/shell/transient_menu.rs` | Bottom-pane prompt + filtered item list, focus policy, package provenance; Phase 20.5: `TransientMenuOrigin` (`CommandPalette`/`ContextMenu`/`MenuBar`) selects overlay anchor |
| Inline completion pop-up | internal | `src/shell/transient_menu.rs` | Completion results rendered through the transient menu session (`completion_result_to_menu_session`) |
| Fixed package panels | internal | `src/shell/package_ui.rs` | Slot-bound package panels with visibility |
| Transient package overlays | internal | `src/shell/package_ui.rs` | Anchored overlays (`PackageOverlayAnchor`) |
| File browser | internal | `src/shell/file_browser.rs` | Workspace/selected-file browsing surface; Phase 22.8 starts with an editor-only hidden snapshot, toggles a per-tab left panel, and shows workspace name + full location when visible |
| Editor chrome | internal | `src/editor/surface.rs` | Caret, selection, scrollbar, diagnostics paint |

## Clay-Native Chrome Primitives (internal)

Phase 20.2 introduced a native chrome primitive layer in `src/shell/primitives.rs`. These are `pub(crate)` inert paint helpers that read from `ResolvedUiTheme` tokens. Packages cannot call primitives directly; package-declared `ComponentKind` components map onto primitives by construction (the SDUI paint path calls primitive helpers for chrome).

| Primitive | Status | File | Purpose | Token mapping | Accessibility role |
|-----------|--------|------|---------|---------------|-------------------|
| `paint_divider` | internal | `src/shell/primitives.rs` | Horizontal/vertical separator | `color.border`, `dimension.border.width` | `separator` |
| `paint_focus_ring` | internal | `src/shell/primitives.rs` | Focus indicator ring | `color.focus.ring`, `dimension.focus.ring.width`, `dimension.focus.ring.offset` | Applied to focused element |
| `paint_panel_chrome` | internal | `src/shell/primitives.rs` | Panel background/border with optional title/collapse/resize | `color.surface.panel`, `color.border`, `dimension.border.width`, `dimension.radius.panel`, `spacing.panel.padding` | `region` or `complementary` |
| `paint_scroll_chrome` | internal | `src/shell/primitives.rs` | Scrollbar track/thumb with interaction states | `color.scrollbar.track`, `color.scrollbar.thumb`, `dimension.scrollbar.width`, `dimension.scrollbar.margin`, `dimension.scrollbar.min.thumb`, `dimension.radius.scrollbar` | `scrollbar` |
| `paint_badge` | internal | `src/shell/primitives.rs` | Badge/tag with label and interaction states | `color.surface.badge`, `color.text.badge`, `dimension.radius.badge`, `spacing.badge.padding.x`, `spacing.badge.padding.y`, `typography.badge` | `status` or `note` |
| `paint_kbd_hint` | internal | `src/shell/primitives.rs` | Keyboard shortcut hint | `color.surface.kbd`, `color.text.kbd`, `dimension.radius.kbd`, `spacing.kbd.padding.x`, `spacing.kbd.padding.y`, `typography.kbd` | `kbd` (via label) |
| `paint_icon_slot` | internal | `src/shell/primitives.rs` | Standardized icon placeholder | `dimension.icon.size`, `dimension.icon.slot.size`, `color.text.muted`, `dimension.radius.icon` | `img` or `presentation` |
| `paint_tooltip_shell` | internal | `src/shell/primitives.rs` | Tooltip background/border | `color.surface.overlay`, `color.border`, `dimension.border.width`, `dimension.radius.tooltip`, `spacing.tooltip.padding` | `tooltip` |

All primitives:
- Read color/dimension/opacity/typography from `ResolvedUiTheme` tokens (no hardcoded values).
- Handle `InteractionState` variants (`Rest`, `Hover`, `Active`, `Focus`, `Disabled`) where applicable.
- Apply `opacity.disabled` for `InteractionState::Disabled`.
- Are panic-free on zero-size rects.
- Are deterministic and allocation-free in paint paths.

Conformance contract (enforced by `tests/ui_primitive_conformance.rs` and `tests/package_ui_conformance.rs`):
- Shell/SDUI chrome paint files contain no `Color::from_rgb8`/`Color::from_rgba8` literals outside `primitives.rs` and `theme.rs`.
- Shell/SDUI chrome paint files contain no hardcoded chrome-size constants outside `primitives.rs` and `theme.rs`.
- Package components map onto primitives by construction (SDUI paint routes chrome through primitive helpers).
- Each primitive is token-driven and renders all declared interaction states.

**Phase 20.7 enforced checks** (host authority; see `docs/reference/packages/creating-packages.md` § "Phase 20.7 authoring contract: UI conformance guardrails"):
- **Contrast / legibility:** active-theme status-chrome pairs must meet `TEXT_CONTRAST_MIN` (4.5) and `UI_CONTRAST_MIN` (3.0); a below-AA theme is not activated (`validate_active_theme_contrast`, `src/shell/theme.rs`; `enforce_contrast`, `src/server/ops/theme.rs`).
- **State-completeness:** `applicable_states(kind)` (`src/shell/components.rs`) is the per-kind interaction-state contract; pinned against the documented per-kind notes and `component_state_palette` in `tests/masonry_sdui.rs`.
- **Payload budgets:** SDUI snapshot ≤ 4096 B, update ≤ 1024 B; runtime tree ≤ 16 KiB / ≤ 128 nodes / ≤ 16 depth / ≤ 4096-char text node (`src/packages/record.rs`, `src/server/ui.rs`, `src/server/ops/sdui.rs`).
- **Code-vs-catalog drift:** `ComponentKind` enum ↔ `component_state_palette` match arms ↔ this catalog's `Package-Facing Component Kinds` table; typed-style-variable match arms ↔ `Typed Style Variables` table; `core_theme_value` arms ↔ `tokens.md` Core Tokens. All four drift guards live in `tests/package_ui_conformance.rs`.
- **Author diagnostics:** rejection messages name the rejected value, expected type, and field via `ComponentCatalogError::reject` (`{field} = `{value}` rejected: expected {expected}; {reason}`).
- **Trust domains:** no `clay.ui.validate*` op or `clay:*` facade exposes conformance; third-party raw values and oversized payloads are rejected at `assemble_package_record` without reaching the trusted runtime.

Conformance is host authority, not package-facing: validation runs inside Clay's Rust host validator at parse/install/theme-apply time.

## Planned Components (UI Revamp Phases 20.2/20.5)

Reuse-first: before adding any of these, confirm no implemented kind composes to the same result. Each planned entry must ship token-driven, state-complete (hover/active/focus/disabled), accessible, and cataloged here.

| Component | Status | Purpose | Composition notes |
|-----------|--------|---------|-------------------|
| Pop-up / dialog | implemented | Non-blocking anchored pop-up and blocking dialog | Phase 20.5: `overlay`+`portal` composition; `modal` kind for blocking; z-level stacking (`z.overlay`<`z.modal`<`z.tooltip`) |
| Dropdown / select | implemented | Single-choice selection | Phase 20.5: `dropdown` kind; keyboard nav via `dropdown_selected` state |
| Multi-select | implemented | Multi-choice selection with tags | Phase 20.5: `list`+`checkbox` composition; no new kind |
| Text input field | implemented | Single-line editable field | Phase 20.5: `textInput` kind; focus, placeholder, validation states |
| Menu (context / menu bar) | implemented | Command menus | Phase 20.5: `TransientMenuOrigin::ContextMenu`/`MenuBar`; anchor selected by origin |
| Completion pop-up (uplift) | implemented | Inline completion restyle | Phase 20.5: routes through shared `paint_package_overlays` + `paint_tooltip_shell`; cursor inset from `ui_theme` |
| Command palette | implemented | Command Centre surface | Phase 20.5: `TransientMenuOrigin::CommandPalette` (default); `Modal` focus policy + `Bottom` anchor |
| Tooltip | planned | Hover hint | `overlay` anchored, `detail` typography |
| Tabs | implemented | Tab strip (window tab bar) | Phase 22.3: shell-owned tab bar row above the working area; token-state cards (idle/hover/active/focus/disabled) via `tab_card_chrome`; the close glyph, switch-on-click, and server-registry reconciliation. The close glyph remains bespoke two-stroke internal chrome, not a package-facing icon primitive. Phase 22.7: cards shrink-to-fit until `TAB_BAR_CARD_MIN_WIDTH` (100px) binds, then the strip scrolls — wheel over the bar scrolls (one `f64` offset, clamped to the last card's right edge at the "+" slot), activation auto-scrolls the active card into view, cards clip to the strip, and hit-testing honors the offset. Scroll is bespoke internal chrome (the `scroll` component stays rejected for the strip — recorded ceiling). Generic paint contract (cards with labels + close affordance) so later phases can reuse it for panel/pane tabs; shell-level, not package-facing |
| Split divider | implemented | Draggable pane/slot separator | Phase 20.3: `paint_divider` + drag interaction on `PaneSplitTree`; resize handles via `paint_panel_chrome` |
| Badge / tag | planned | Status/count marker | `label` + muted pastel tokens |
| Toast / notification | planned | Transient feedback | `overlay` + portal, auto-dismiss |
| `kbd` hint | planned | Shortcut rendering | `label` with monospace role + bordered token style |
| Icon slot | planned | Standardized icon placeholder | Token-sized glyph slot; no package image assets initially |

## Typography Variants (Phase 20.1)

The `typography` style variable references one of seven semantic `UiTextVariant` tokens (see [tokens.md](tokens.md#typography)): `body`, `title`, `status`, `display`, `section`, `detail`, `caption`. Variants are scale ratios over the selected role base, resolved through the user-owned `UiTypographyHierarchy` — never absolute point sizes. Phase 20.1 added `display`, `section`, `detail`, and `caption` additively; existing `body`/`title`/`status` usage is unchanged. No new style variable was added in Phase 20.1.

## Rules for Adding Components

1. Prefer composing existing kinds (`flex`, `stack`, `overlay`, `scroll`, `list`, `label`, `button`) before adding a kind.
2. New kinds are additive; never rename or remove an implemented kind.
3. New style variables must be token-typed or closed enums — no raw values.
4. Every component ships with all interaction states styled from tokens.
5. Update this catalog, `docs/reference/packages/creating-packages.md`, and the component validation tests together.
