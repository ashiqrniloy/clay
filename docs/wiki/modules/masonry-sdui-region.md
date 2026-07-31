# SDUI / Package-UI Retained Masonry Reconciliation

## Source

- `src/masonry_sdui_region.rs` — SDUI tree reconciler + leaf widgets + scroll viewport
- `src/masonry_package_region.rs` — package component tree reconciler + package widgets + panel/overlay hosts
- `src/masonry_sdui.rs` — `SduiNativeState` (inert state + render inputs + observability)
- `src/masonry_editor.rs` — `EditorWidget` (hosts the three child widgets, editor canvas, status chrome)
- `src/shell/package_ui.rs` — `PackageUiRuntimeState`, `TransientPackageOverlay`, `MenuA11y`
- `src/shell/primitives.rs` — shared paint helpers (`paint_sdui_text`, `component_state_color`, `paint_scroll_chrome`, `paint_panel_chrome`, `paint_tooltip_shell`)
- Plan: `plans/070-SDUI-Retained-Masonry-Reconciliation.md`
- Decision: `decision-logs/2026-07-29-1451-stable-identity-sdui-reconciliation.md`

## Overview

Plan 070 replaced Clay's immediate-mode SDUI/package-UI paint path with a **retained reconciled Masonry subtree**. The server still owns declarative UI state and publishes inert `SduiSnapshot`/`SduiUpdate`/package-UI trees; the client now reconciles those into real Masonry widgets hosted as children of `EditorWidget`, so Masonry routes layout, paint, pointer, focus, scroll, and accessibility through the standard widget tree. There is no nested compositor, no god-object `paint_node` recursion, and no per-widget hand-built hit-testing. The earlier `SduiNativeState::paint` immediate-mode path and the `paint_package_component`/`paint_package_overlays` helpers are deleted.

Two parallel reconcilers exist because SDUI and package-UI have different data models:

- **`SduiRegionWidget`** reconciles the SDUI tree (flat `BTreeMap<SduiNodeId, SduiNode>` with op-based `ReplaceNode`/`RemoveNode`/`ReplaceRoot` updates) into `SduiLabel`/`SduiButton`/`SduiListRow`/`EditorViewWidget` under a Clay-owned `SduiScrollViewport`.
- **`PackageRegionWidget`** reconciles `PackageUiComponentTree` (recursive nested tree, re-provided wholesale per package-UI update) into `PackageLeaf`/`PackageButton`/`PackageListRow`/`PackageCollapse`/`PackageDropdown`/`PackageTextInput`/`PackageModal`. Fixed panels are hosted by `PackagePanelHost`; transient overlays + the active menu by `PackageOverlayHost`.

`SduiNativeState` (`src/masonry_sdui.rs`) is now an **inert state holder**: it stores typography, `ResolvedUiTheme`, `PackageUiRuntimeState`, the active menu, SDUI tree + versions, and dirty flags. It exposes render inputs (`region_render_input`, `panels_render_input`, `overlays_render_input`) consumed by `EditorWidget` sync methods, plus test/agent observability (`observable_snapshot`, `visible_texts`). It has no paint path and no interaction state.

## EditorWidget Multi-Child Hosting

`EditorWidget` is a container widget hosting three children in `children_ids` order `[panel_host, region, overlay_host]`:

| Child | Widget | Covers | Purpose |
|-------|--------|--------|---------|
| `panel_host` | `PackagePanelHost` | fixed-panel rects | hosts one `PackageRegionWidget` per visible fixed package panel |
| `region` | `SduiRegionWidget` | sidebar rect | reconciles the SDUI tree; paints sidebar chrome in its own `paint` |
| `overlay_host` | `PackageOverlayHost` | full working area (pointer-transparent) | hosts one `PackageRegionWidget` per transient overlay + active menu |

Masonry paints children in `children_ids` order and hit-tests in reverse, so `overlay_host` is topmost for hit-testing. `PackageOverlayHost` returns `false` from `accepts_pointer_interaction()`, so it is transparent to pointer events — clicks fall through to `region`/editor below via the sibling loop while its overlay-region children handle their own clicks.

**Z-order** (Masonry paint pass is `parent.paint()` → `children[first..last]` → `parent.post_paint()`):

1. `EditorWidget::paint()` — background fill + editor canvas (`EditorSurface::paint_in_rect`).
2. Children pass — `panel_host`, `region` (sidebar chrome + reconciled tree), `overlay_host` (overlay chrome + reconciled overlay trees).
3. `EditorWidget::post_paint()` — status line only.

This gives `chrome + editor < sidebar/panels/overlays < status`. Moving editor content from `post_paint` to `paint` was the fix for the completion-overlay regression (overlays must render above editor text). Widgets that paint in `post_paint` must ensure all repaint call sites use `request_render` (not `request_paint_only`, which skips `post_paint`).

## Stable-Identity Reconciliation

Both reconcilers diff the new tree against the live widget subtree and **reuse surviving `WidgetPod`s in place**, preserving Masonry `WidgetId`s and internal widget state (focus, scroll position, expanded/collapsed, dropdown selection) across updates. This is the prerequisite for interactive widgets — wholesale rebuild would destroy widget identity and reset state on every server update.

- **SDUI** keys diff by `SduiNodeId`; `ChildKey` discriminates `Node(id)`, `PanelTitle`, `ListRow(item_id)`. `PodRecord` maps `SduiNodeId → (WidgetId, Discriminant<SduiNodeKind>)`. Same-order children reconcile in place via `Flex::child_mut`; removals are reverse-order; reorders lose identity (Masonry has no move-child API) but SDUI trees rarely reorder. Kind-change forces a fresh rebuild; prop-only changes mutate fields via `try_downcast` + direct assignment + `request_layout`.
- **Package** keys diff by `stable_package_source_id(component.id)`; `PackageChildKey::Component` carries `(u64 hash, String kind)` so same-id different-kind components read as remove+add. Package containers (`flex`/`stack`/`overlay`/`scroll`/`portal`) all map to `Flex::column` with `gap=0` (legacy trees assume zero inter-row gap; Masonry `Flex` defaults to 10px, so `with_gap(Length::ZERO)` is explicit).

The reconcile entry points are `reconcile_snapshot_live`/`apply_update_live`/`reconcile_tree_live` (take `&mut MutateCtx`, mutate the persistent region in place). `sync_region`/`sync_panels`/`sync_overlays` in `EditorWidget` call these via `edit_widget` → `get_mut` → `try_downcast`. The no-ctx `reconcile_snapshot`/`apply_update`/`reconcile_tree` variants do wholesale rebuild and exist for standalone tests.

**Lifecycle constraint**: Masonry 0.4.0 `RenderRoot` registers children at construction; post-construction `edit_base_layer` mutations that create new `WidgetPod`s leave them unregistered, causing panics. The reconcilers build fully-populated subtrees before `RenderRoot::new` in tests; production builds mutate the live region in place via `MutateCtx::get_mut` (the safe in-place mutation API).

## Widget Inventory

### SDUI widgets (`masonry_sdui_region.rs`)

| Widget | SDUI kind | Behavior |
|--------|-----------|----------|
| `SduiLabel` | `Label` / panel title | text via `paint_sdui_text`; title vs body color distinction |
| `SduiButton` | `Button` | paint via `component_state_color("surface.control", state)`; emits `SduiButtonPress(SduiActionIntent)` on click/Enter/Space |
| `SduiListRow` | `List` item | row fill via `list_row_fill_color`; emits `SduiListRowPress(SduiActionIntent)` |
| `EditorViewWidget` | `EditorView` | zero-width binding/slot component; the real editor canvas is painted by `EditorWidget::paint` on top |
| `SduiScrollViewport` | (wraps the tree) | Clay-owned scroll viewport: owns scroll state, clips via `set_clip_path` in layout, paints themed scrollbar via `paint_scroll_chrome` in `post_paint`, handles wheel/trackpad scroll; no drag-to-scroll (matches editor) |
| `SduiRegionWidget` | (region root) | hosts `SduiScrollViewport` child at `(0, padding)`; paints sidebar chrome in `paint`; `Role::Group` |

### Package widgets (`masonry_package_region.rs`)

| Widget | Component kind | Behavior |
|--------|---------------|----------|
| `PackageLeaf` | `label`/`statusItem`/`editorView`/panel-title | text rendering; explicit `disabled` field |
| `PackageButton` | `button` | disabled derived from `intent.is_none()`; emits `PackageButtonPress(SduiActionIntent)` |
| `PackageListRow` | `list` item | `selected`/disabled-from-`intent.is_none()`; emits `PackageListRowPress`; action source_id `{component_id}.{item_id}` |
| `PackageCollapse` | `collapse` | clip-based content show/hide (layout `set_clip_path`); widget-local `expanded`; Enter/Space toggles |
| `PackageDropdown` | `dropdown` | single-widget (painted trigger + open list); widget-local `selected_index`/`open`; ArrowUp/Down cycles, Enter/Space confirms; emits `PackageDropdownSelect` |
| `PackageTextInput` | `textInput` | wraps Masonry `TextArea<true>` (editable substrate); Clay-owned chrome paint (bg, validation border, placeholder, focus ring); optimistic local edit + server-authority adopt via `reset_text` when unfocused; commit via `TextAction::Entered` routed through `text_input_intents` map |
| `PackageModal` | `modal` | `Role::Dialog`; Tab/Shift+Tab focus trap cycles widget-local focusable descendants via `ctx.set_focus`; `set_handled` prevents global Tab traversal; Escape emits `PackageModalDismiss` |
| `PackagePanelHost` | (host) | hosts `PackageRegionWidget` per visible fixed panel; reconciles by `FixedSlotId`; paints panel chrome |
| `PackageOverlayHost` | (host) | hosts `PackageRegionWidget` per transient overlay + active menu; reconciles by overlay id; sorts by z-order; `accepts_pointer_interaction()=false` (pointer-transparent); paints `paint_tooltip_shell` chrome behind children |

## Action Routing

Custom Masonry action types carry `SduiActionIntent` payloads; `main.rs` downcasts via `ErasedAction::downcast` in order: `SduiButtonPress` → `SduiListRowPress` → `PackageButtonPress` → `PackageListRowPress` → `PackageDropdownSelect` → `TextAction` → `EditorAction`, then routes to `enqueue_sdui_intent`. Stock Masonry `ButtonPress` carries only `PointerButton` (no payload), so the custom-action-type pattern is necessary for all SDUI/package interactive kinds.

`PackageTextInput` commit is special: `TextArea<true>` submits `TextAction::Entered(value)` directly to the global action handler with the TextArea's `WidgetId` (it does not bubble through parent `on_text_event`). `PackageRegionWidget` stores `text_input_intents: HashMap<WidgetId, SduiActionIntent>` mapping the TextArea widget id to the component's base commit intent; `main.rs` navigates `EditorWidget` → `PackagePanelHost` → `PackageRegionWidget` → map lookup to recover component context and append the value argument.

## Active Menu (Completion / Command Palette / Context Menu)

The active menu (`TransientMenuSession`) is the only real runtime overlay — zero packages declare `modal`/`overlay`/`portal` component kinds at runtime. It renders through the same hosted overlay pipeline:

- `SduiNativeState::transient_overlays()` collects `package_ui.overlays()` + `TransientPackageOverlay::from_menu_session`, sorted by `overlay_z_order` (`z.overlay`=0 < `z.modal`=1 < `z.tooltip`=2).
- `PackageOverlayHost` reconciles the menu overlay like any other; `from_menu_session` builds a `PackageUiComponentTree` (root `kind='stack'`, children=[prompt label, query label, statusItem or list]).
- Menu keyboard navigation stays with `route_menu_key` (client-side); `EditorAction::MenuStateChanged` is submitted after `route_menu_key` returns true so `main.rs on_action` re-runs `sync_overlays` (keyboard events don't provide `MutateCtx`, so the action path is the only way to reconcile after keyboard-driven menu mutations).
- Menu dismissal stays with `route_menu_key` (Escape → `menu_cancel`), not `PackageModalDismiss`.

## Menu Accessibility

The legacy `collect_active_menu_accessibility_entries` (Menu > MenuItem with custom accessibility labels + `' selected'` suffix) is deleted. Menu a11y now flows through the hosted `PackageRegionWidget` subtree via `MenuA11y`:

- `TransientPackageOverlay.menu_a11y: Option<MenuA11y>` carries `prompt`, `items: Vec<MenuA11yItem>` (resolved `accessibility_label` + `selected`), and optional `status`.
- `PackageRegionWidget.set_menu_a11y(Some(...))` switches `accessibility_role` to `Role::Menu` and builds synthetic Menu/MenuItem/Status accesskit nodes (unstable `NodeId`s via `WidgetId::next()`) with the `' selected'` suffix on the selected item — parity with the legacy contract.
- `EditorWidget::accessibility` includes `overlay_host.id()` in its children (gated on sidebar/overlay presence); the legacy `append_accessibility_children` path is deleted.

## Sync Wiring

`EditorWidget` exposes three sync methods, each gated on a `take_*_dirty` flag from `SduiNativeState`:

| Method | Dirty flag | Called from |
|--------|-----------|-------------|
| `sync_region` | `region_dirty` | `apply_connection_event` (SduiSnapshot/SduiUpdate), `set_typography`, `set_ui_theme`, init |
| `sync_panels` | `panels_dirty` | same sites + `apply_package_ui_update`/`install_package_ui_snapshot` |
| `sync_overlays` | `overlays_dirty` | same sites + `MenuStateChanged` action |

`apply_package_ui_update` and `install_package_ui_snapshot` set both `panels_dirty` and `overlays_dirty` on any package-UI mutation (previously they cleared actions but did not set dirty flags, a latent bug fixed in Step 13e).

## Invariants and Constraints

- One widget tree, one event tree, one render pass. No nested `RenderRoot` compositor, no `SduiNativeState::paint` path, no per-widget hand-built hit-testing.
- `SduiNativeState` is inert state + render inputs + observability only. It has no paint path, no pointer/focus interaction state (those deleted in Steps 13e/14).
- All reconciliation widgets/containers are `pub(crate)`; the two region modules are `pub(crate) mod`. The 5 `pub` action types are the legitimate bin-crate `ErasedAction::downcast` surface (re-exported via `pub mod masonry_editor`), not a JS API.
- Editor canvas paint stays in `EditorWidget::paint` (hot path unchanged); `EditorViewWidget` is a zero-width binding/slot component, not a re-hosted editor.
- Stable-identity reconcile preserves `WidgetId` + internal state across updates; wholesale rebuild is test-only.
- SDUI/package actions are `SduiActionIntent` payloads (inert command intents); they never grant filesystem/network/shell/extension/AI/workspace/client-JS authority.
- No new Clay JS API, op, facade, config key, or env var was introduced (`CLAY_SDUI_RETAINED` kill-switch deleted in Step 8). Package-facing contract (kinds, style variables, tokens) unchanged.
- `SduiScrollViewport`/`PackageCollapse` use `set_clip_path` in their own layout (not parent clipping) — Masonry 0.4.0 `set_clip_path` clips the widget's own paint + own pointer hit-testing + children, so a parent cannot clip a child without disabling its own hit-testing.

## Tests

- `src/masonry_sdui_region.rs`: `stable_identity_nodes_keep_widget_ids_across_inplace_update`, `stable_identity_preserves_focus_across_unrelated_update`, `container_child_list_add_remove_reorder_reconciles_correctly`, `prop_update_changes_label_text_without_recreating_the_widget`, `retained_layout_matches_legacy_row_geometry`, `reconciled_containers_use_zero_gap_for_scroll_parity`, scroll-viewport + lifecycle spike tests.
- `src/masonry_package_region.rs`: `overlay_host_reconcile_updates_menu_selection`, `hosted_menu_overlay_exposes_menu_role_and_item_accessibility_labels`, package widget interaction + collapse/dropdown/textInput/modal tests, panel-host/overlay-host reconcile tests.
- `src/masonry_sdui.rs`: SDUI snapshot/update application + observability tests (`sdui_snapshot_replaces_native_tree_state`, `slot_ui_observation_omits_document_text_native_handles_and_raw_authority`, layout regression tests).
- `src/masonry_editor.rs`: `sync_region`/`sync_panels`/`sync_overlays` wiring tests, `MenuStateChanged` re-sync, completion-overlay z-order.
- Conformance: `tests/ui_primitive_conformance.rs`, `tests/package_ui_conformance.rs` (run via `cargo test --test editor`).
- Commands: `cargo test --lib`, `cargo test --test editor`, `cargo test --test runtime`, `cargo test --test protocol` (doc/catalog drift gates).

## Related

- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Masonry Shell](masonry-shell.md)
- [Rendering Primitives](rendering-primitives.md)
- [Phase 20.5 Overlay/Menu/Input Components](phase20.5-overlay-menu-input-components.md)
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `plans/070-SDUI-Retained-Masonry-Reconciliation.md`