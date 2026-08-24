# Masonry Shell Runtime

> **Historical implementation — removed by Plan 097 Phase 12.** See
> [Tauri/React Desktop Cutover](tauri-react-cutover.md) and
> [React Tabs, Splits, and Layout Persistence](react-tabs-and-splits.md).

## Source

- `src/masonry_shell/mod.rs`
- `src/shell/mod.rs`
- `src/shell/layout.rs`
- `src/lib.rs`
- `src/launch.rs`
- `src/app_driver.rs`
- `src/masonry_editor.rs`

## Module layout (Plan 090)

`src/masonry_shell.rs` was split into a directory module in Plan 090 (task 6). `ClayShellWidget` (state owner + paint/layout hot paths) stays in `mod.rs`; the tab/window data vocabulary and the accessibility virtual-node builder each own one private submodule:

| File | Contents |
|------|----------|
| `src/masonry_shell/mod.rs` | `ClayShellWidget` + `impl ClayShellWidget` + `impl Widget`, `ShellObservableSnapshot`, paint helpers, and the collocated test module |
| `src/masonry_shell/window_tabs.rs` | `PaneFocusPolicy`, `ShellClientCommand` + catalogue + `TAB_BAR_*` consts, `TabCard`, `TabBarGeometry`/`TabCardGeometry`, `TabChrome` + impl |
| `src/masonry_shell/accessibility.rs` | `node_window_size`, `accesskit_rect`, `AnnouncementKind`, `ANNOUNCEMENT_MAX_CHARS`, `compose_announcement` |

## Overview

The Masonry shell runtime is Clay's native root container for a window working area. It keeps `EditorWidget` as an editor component below a Clay-owned shell root instead of treating the editor widget as the whole application layout.

Phase 18.2 implements the `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` runtime foundations needed for a default one-editor pane, generic horizontal/vertical split topology, and leaf-pane slot geometry. Phase 18.3 adds crate-internal slot-aware package UI runtime state in `src/shell/package_ui.rs`: accepted fixed panel contributions are installed as versioned `FixedPackagePanel` records for explicit `left`, `right`, `top`, or `bottom` slots, and accepted transient overlays are installed separately so overlays do not consume fixed slot geometry. User-visible shell configuration remains planned in later tasks/phases. `ClayShellWidget` is `#[doc(hidden)]` and Rust-public only because the Cargo package's `clay` binary target constructs the library-owned native root; this binary-boundary visibility is not a Clay JS API, package extension point, op, facade, or registry entry.

## Plan 088 shell chrome modernization

The welcome entry state now uses the full pane width and suppresses the empty workspace-browser slot until a real document is mounted; package-owned fixed panels still retain their slots. Split-pane focus rings paint in `post_paint`, after pane hosts, so the active pane remains visibly distinct without relying on fill color. The pinned `+` tab action uses the shared state palette on hover, while tab labels use the driver-sanitized final workspace segment (or `Workspace` fallback) and tab overflow keeps its existing bounded strip geometry; absolute host paths never enter tab chrome.

Plan 088 Task 6 also installs each tab's `ActiveTypography` into shell chrome. The active tab drives the window-level tab bar registry; status metrics size the bar, cards, close affordances, and `+` slot, all clamped to logical window bounds. The SDUI left-slot decision uses a bounded UI-body em heuristic with the actual pane width, so large user typography yields the sidebar before the editor becomes unusable. Bottom transient overlays clamp to short main regions, and long SDUI labels paint inside typography-derived row clips while retaining their full accessible value.

## Responsibilities

- Own the internal Rust layout state for a working area, pane split tree, per-pane slot layout, and slot-aware package UI runtime state.
- Apply inert, version-checked shell layout updates and package UI runtime updates that replace local pane/slot/panel/overlay state only after bounded validation.
- Expose internal structural shell observations for tests and agent inspection without exposing document text or native handles.
- Register the existing `EditorWidget` as a Masonry child of `ClayShellWidget`.
- Keep native widget IDs, Masonry handles, raw callbacks, raw CSS, raw ops, and client-side JavaScript out of package-facing state.
- Compute editor child geometry from installed Clay shell state during Masonry layout without adding or removing children in the layout pass.
- Preserve existing editor input, rendering, SDUI, status, focus fallback, and action routing responsibilities inside `EditorWidget` while routing shell-root events to the editor child.

## How It Works

`src/launch.rs::run_editor` constructs an `EditorWidget`, wraps it in `ClayShellWidget::single_editor(...)`, records the child editor `WidgetId`, and starts the Masonry window with the shell as the root widget. The driver sets Masonry focus fallback to the editor child and routes existing `EditorAction` handling back to that child ID, so connection events, file-open UI command results, SDUI snapshots, edit acknowledgements, and resync snapshots continue to mutate `EditorWidget` rather than the shell container.

`src/shell/layout.rs` owns the reusable shell state:

1. `WorkingAreaLayout` records the shell layout version, working-area ID, editor component binding, and installed `PaneSplitTree`.
2. `PaneSplitTree` stores a validated tree of `PaneSplitNode::Leaf` and `PaneSplitNode::Split` values with a stable active `PaneId`.
3. `SplitOrientation::Horizontal` divides a rectangle into left/right regions. `SplitOrientation::Vertical` divides a rectangle into top/bottom regions. `SplitRatio` is the first region's fraction.
4. `PaneSlotLayout` stores the mandatory `main` slot plus optional fixed `left`, `right`, `top`, and `bottom` slots for each leaf pane. `FixedSlotState` records size, min/max clamps, visibility, collapse state, and whether the slot was user-resized.
5. Validation rejects zero pane IDs, duplicate pane IDs, active panes that are not leaves in the tree, invalid/non-finite split ratios, invalid fixed-slot sizes/bounds, and trees over the internal node limit.
6. Geometry helpers traverse the installed tree to return deterministic `PaneGeometry` values, then subtract visible fixed slots from a leaf pane to produce the editor component's `main` rectangle.

`src/masonry_shell/mod.rs` owns the Masonry container behavior. `register_children` registers the already-created editor child. During layout, the shell computes its working-area size, asks `WorkingAreaLayout` for the editor component rectangle, runs the child layout with tight constraints for that rectangle, and places the child at the computed origin. The shell's `children_ids` remains the already-registered editor child list; split geometry does not mutate the Masonry child tree.

Inside the editor child, `src/masonry_editor.rs` reuses the same SDUI/package `PaneSlotLayout` main-region calculation for paint and pointer hit-testing. `EditorWidget::editor_main_rect` asks `editor_region_for_document` for the current `main` rect, paints `EditorSurface::paint_in_rect` clipped to that rect, and translates pointer positions into editor-local coordinates before caret placement or selection extension. `editor_region_for_document` reserves the Clay-owned left file-browser slot when the installed SDUI tree contains a sidebar panel, not merely because an editor binding/root exists. This keeps the main region away from a visible browser after opening a workspace file under a new document ID, while an editor-only hidden snapshot reclaims the full width. Accepted visible fixed panels therefore consume `left`/`right`/`top`/`bottom` geometry instead of covering text, while transient overlays still paint after fixed panels and may cover content by design.

`WorkingAreaLayoutUpdate` is the Phase 18.2 inert update path. It carries the current `base_version`, target `WorkingAreaId`, a prevalidated `PaneSplitTree`, the editor pane, and bounded per-pane `PaneSlotLayout` assignments. `WorkingAreaLayout::apply_update` rejects stale base versions, wrong working-area IDs, editor panes not in the tree, slot assignments for missing panes, duplicate pane-slot assignments, and more slot layouts than the tree can contain. Phase 18.3 `PackageUiRuntimeUpdate` carries the current package UI runtime version plus bounded fixed panels and transient overlays; `PackageUiRuntimeState::apply_update` rejects stale versions, more than four fixed panels, more than sixteen overlays, duplicate contribution IDs, and duplicate exclusive fixed-slot claims. Successful updates increment local runtime versions and replace only local pane/slot/component-binding/panel/overlay state; no package JavaScript, IPC, protocol decoding, widget-tree mutation, or document work runs in Masonry paint/layout/text-event handlers.

`ShellObservableSnapshot`, `WorkingAreaLayoutObservation`, and the package UI observations used by `SduiNativeState` provide structural observability. They record the shell layout version, working-area ID, pane tree shape, pane/split counts, active/root pane IDs, editor component binding, slot rectangles/visibility/user-resize flags, package fixed panel IDs/slots/component roots, transient overlay IDs/anchors/focus/dismissal policy, whether the editor region is non-empty, and whether the shell still has editor SDUI/status state. They deliberately omit Masonry `WidgetId` values, native handles, document text, source snippets, raw filesystem paths, raw action payload authority, raw CSS, raw ops, Vello/Parley callbacks, and executable package code.

## Code Examples

```rust
let tree = PaneSplitTree::new(
    PaneSplitNode::split(
        SplitOrientation::Horizontal,
        SplitRatio::new(0.5)?,
        PaneSplitNode::leaf(PaneId(1)),
        PaneSplitNode::leaf(PaneId(2)),
    ),
    PaneId(1),
)?;

let pane_rects = tree.compute_geometry(Rect::new(0.0, 0.0, 1000.0, 600.0));

let slots = PaneSlotLayout::main_only()
    .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 240.0, 120.0, 320.0)?);
let geometry = slots.compute_geometry(Rect::new(0.0, 0.0, 900.0, 600.0));
assert_eq!(geometry.main_rect.x0, 240.0);
```

The examples are internal Rust-only. They are not public `clay:ui` JavaScript APIs.

## Primitive Coverage

- `WorkingAreaLayout`
  - Owner/source: `src/shell/layout.rs`.
  - Runtime status: internal Rust state installed by the shell root for the default one-editor working area.
  - Public API status: planned `ui.serverRegisterWorkingAreaLayout` inventory stub only; no callable `clay:ui` facade/op is exposed in Phase 18.2.
- `PaneSplitTree`
  - Owner/source: `src/shell/layout.rs`.
  - Runtime status: internal Rust state with leaf/split nodes, active pane metadata, bounded ratios, duplicate-ID rejection, node-count bound, and deterministic geometry helpers.
  - Public API status: planned `ui.serverRegisterPaneSplitTree` inventory stub only; packages cannot mutate Masonry children or provide split callbacks.
- `PaneSlotLayout`
  - Owner/source: `src/shell/layout.rs`.
  - Runtime status: internal Rust state with mandatory `main`, optional fixed `left`/`right`/`top`/`bottom` slots, finite size validation, min/max clamps, visibility, collapse state, user-resize metadata, and deterministic main/fixed-slot geometry.
  - Public API status: planned `ui.serverSetPaneSlotLayout` inventory stub only; package-facing panel contributions and user layout overrides remain deferred.

Current and future modes/packages should reuse these generic shell primitives through documented Clay APIs when those APIs are implemented. They must not add Markdown-specific Rust shell branches or depend on Masonry widget IDs/types.

## Public API, Configuration, and Package Authoring Boundary

Phase 18.2 is an internal runtime foundation, not a public package UI API release. The public/reference docs and package guide deliberately distinguish three layers:

1. Implemented internal Rust runtime: `ClayShellWidget`, `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, the SDUI left-slot bridge, inert local layout updates, and structural shell observability.
2. Runtime-backed package-facing `clay:ui` APIs for package panel/component/overlay/theme-token declarations now validate and store accepted inert declarations; internal package UI runtime state composes accepted fixed panels and transient overlays. Working-area registration, pane split-tree registration, direct pane slot layout declarations, UI state scopes, and layout overrides remain planned/unavailable inventory rows.
3. User-visible configuration: shell layout defaults and overrides remain planned until they have Clay JS API Markdown docs, facade/op support, generated registry coverage, validation, and tests. There are no hidden JSON/TOML/ad hoc split, slot, panel, preview-position, or shell style keys in Phase 18.2.

`docs/reference/packages/creating-packages.md` records the package-authoring contract for this status: packages can register inert fixed panels and transient overlays only through the documented Phase 18.3 `clay:ui` contribution APIs, but they still cannot create panes, mutate split ratios, directly claim arbitrary pane slots, change shell configuration through hidden keys, create Masonry widgets, provide raw CSS, run client-side JavaScript in the native client, call raw `Deno.core.ops`, or bypass registered action validation. Future package or user layout inputs must enter through documented Clay JS APIs, validate package prefixes/permissions/provenance/budgets, and remain inert data before affecting the shell.

## Invariants and Constraints

- One-leaf pane state remains the default launch topology.
- Split topology, slot state updates, and shell update validation are startup/update/test setup work, not keypress, paint, text-event, pointer, or scroll work.
- Masonry layout consumes installed pane/slot/package UI state and places already-registered children; it does not parse packages, run package JavaScript, validate package metadata, wait on IPC, deserialize full documents, or mutate the child tree.
- Shell state and structural observations contain no document text, native handles, raw filesystem authority, raw action callbacks, raw CSS, raw ops, Vello/Parley callbacks, or executable package code.
- `EditorWidget` continues to own editor hot-path input/rendering, SDUI content rendering, status chrome, edit queue behavior, viewport, caret, and selection; paint and pointer hit-testing use the same slot-computed editor `main` rect.

## Tests

- `src/shell/layout.rs::tests::pane_split_tree_default_has_one_leaf`: validates default one-pane topology and full-area geometry.
- `src/shell/layout.rs::tests::pane_split_tree_rejects_duplicate_pane_ids`: validates duplicate leaf ID rejection.
- `src/shell/layout.rs::tests::pane_split_tree_rejects_invalid_ratios`: validates ratio bounds and non-finite rejection.
- `src/shell/layout.rs::tests::pane_split_tree_rejects_oversize_tree_payloads`: validates the internal split-tree node-count ceiling.
- `src/shell/layout.rs::tests::pane_split_tree_computes_horizontal_and_vertical_geometry`: validates generic split rectangle calculation.
- `src/shell/layout.rs::tests::pane_slot_layout_requires_main_slot`: validates that `main` is mandatory and main-only geometry fills the pane.
- `src/shell/layout.rs::tests::pane_slot_layout_computes_main_with_left_right_top_bottom_slots`: validates fixed-slot geometry for every side.
- `src/shell/layout.rs::tests::pane_slot_layout_clamps_fixed_panel_sizes`: validates min/max clamps, collapse, and visibility.
- `src/shell/layout.rs::tests::working_area_editor_component_uses_main_slot_rect`: validates editor component placement uses the pane's `main` slot.
- `src/shell/layout.rs::tests::working_area_layout_applies_inert_validated_update`: validates successful bounded shell state updates increment the local layout version and preserve split/slot observations.
- `src/shell/layout.rs::tests::shell_layout_update_rejects_stale_or_oversize_payload`: validates stale base versions and oversize slot-layout payloads are rejected.
- `src/shell/layout.rs::tests::shell_layout_update_rejects_malformed_slot_and_editor_targets`: validates missing editor panes and duplicate slot assignments are rejected.
- `src/masonry_shell/mod.rs::tests::shell_observable_snapshot_captures_default_working_area`: validates the default structural shell observation without exposing widget handles.
- `src/masonry_shell/mod.rs::tests::shell_observable_snapshot_captures_split_and_slots`: validates split/slot structural observation after an inert update.
- `src/masonry_shell/mod.rs::tests::shell_observation_does_not_expose_document_text_or_native_handles`: validates the snapshot debug surface omits document/native/raw authority markers.
- `src/masonry_shell/mod.rs::tests::shell_layout_update_rejects_stale_or_oversize_payload`: validates the shell widget update wrapper preserves layout validation.
- `src/masonry_shell/mod.rs::tests::shell_places_editor_child_in_main_slot_rect`: validates the shell places the editor child in the slot-computed main rectangle without changing child identity.
- `src/masonry_shell/mod.rs::tests::pane_split_tree_layout_does_not_mutate_children`: validates split geometry reads preserve registered editor child IDs.
- `src/masonry_shell/mod.rs::tests::shell_editor_text_input_remains_client_first`: validates IME text input reaches the editor child through a `RenderRoot` with the shell as root, updates local text immediately, and enqueues a delta edit.
- `src/masonry_shell/mod.rs::tests::shell_editor_keyboard_routing_uses_installed_behavior_manifest`: validates keyboard behavior-manifest routing, using Enter/newline, still reaches the editor child under the shell.
- `src/masonry_shell/mod.rs::tests::shell_editor_read_only_observer_blocks_local_edit_queue`: validates read-only observer state blocks local mutation and edit queue emission through the shell root.
- `src/shell/package_ui.rs::tests::slot_panel_contribution_places_panel_in_requested_slot_and_preserves_main_editor`: validates package fixed panel slot composition through `PaneSlotLayout`.
- `src/shell/package_ui.rs::tests::slot_panel_contribution_rejects_duplicate_exclusive_slot_claims`: validates duplicate exclusive fixed-slot claims fail deterministically.
- `src/shell/package_ui.rs::tests::transient_overlay_renders_without_consuming_fixed_slot_geometry`: validates transient overlay geometry stays separate from fixed slot geometry.
- `src/masonry_shell/mod.rs::tests::shell_routes_edit_ack_and_resync_to_editor`: validates edit acknowledgements and resync snapshots still update editor text/status state through the shell child route.
- `src/masonry_shell/mod.rs::tests::shell_routes_sdui_snapshots_to_editor_component`: validates SDUI connection events still land in the editor component under the shell.
- `src/masonry_editor.rs::tests::fixed_package_panel_shrinks_editor_hit_region`: validates visible package fixed panels offset the editor hit region instead of letting panel coordinates place the caret under the panel.
- `src/driver/mod.rs::tests::connection_event_action_is_dispatched_to_shell_editor_child`: validates GUI connection-event user events target the shell-owned editor child.
- `src/driver/mod.rs::tests::driver_routes_editor_actions_to_shell_editor_child`: validates `AppDriver` editor action routing uses the shell editor child ID even if a shell/root source is reported.
- Command: `cargo test --lib shell --quiet`
- Command: `cargo test --lib masonry_shell --quiet`
- Command: `cargo test --lib main --quiet`

## Phase 20.3: Layout Primitives — Splits, Panel Resizing, and Screen Division

Phase 20.3 (2026-07-25) adds user-facing layout interaction on top of the Phase 18.2 structural model.

### Split Divider Drag

`ClayShellWidget::on_pointer_event` hit-tests pointer-down events against split divider rects (`hit_test_split_divider`, 4px slop). During drag, `compute_drag_ratio` maps pointer position to a clamped `SplitRatio` (0.05–0.95) and `pane_tree_mut().update_split_ratio()` applies it live. On pointer-up, `commit_split_drag()` bumps the layout version. Escape cancels via `cancel_split_drag()`. Dividers paint through `paint_divider` (Phase 20.2 primitive).

### Fixed Slot Resize and Collapse

Slot resize handles are hit-tested via `hit_test_slot_handle` (4px slop). Drag computes a clamped size via `compute_slot_resize_size` and applies it live through `resize_slot_live()`. On pointer-up, `commit_slot_resize()` sets `resized_by_user = true` and bumps the version. Double-click (< 300ms) on a handle toggles collapse/restore via `toggle_slot_collapse()`. Handles paint through `paint_panel_chrome` with `InteractionState::Resize`/`Collapse`.

### Layout Persistence

`src/shell/layout_persist.rs` persisted user-modified state (split ratios
≠ 0.5, slots with `resized_by_user` or `collapsed`) to `~/.config/clay/layout.json`
via `serde_json` (Phase 20.3). Phase 22.5 replaced the single-tab v1 writer:
`save_layout` was deleted and the v1 serializers survive only as frozen
round-trip test fixtures; the file is now the **v2 window state**
(`save_window_state`/`load_window_state`, see the Phase 22.5 section below),
and `ClayShellWidget::persist_debounced()` no longer writes files — it
submits `EditorAction::PersistenceDue` through a ≥ 500 ms debounce so the
driver persists once per mutation burst. Legacy v1 files still apply to the
single bootstrap tab exactly as before. Corrupt/missing files fall back to
defaults.

### Focus and Input Routing

`PaneSplitTree::next_pane()`/`prev_pane()` traverse panes in reading order (in-order, wrapping). Tab/Shift+Tab in `on_text_event` moves focus; `set_focus_pane()` validates membership. A focus ring paints on the active pane when multiple panes exist (`paint_focus_ring`). `focused_pane_rect()` provides the transient surface anchor for overlays/menus.

### Focus removal and AT-SPI ingress (Plan 089)

Pane/tab removal uses the shared Masonry focus seam rather than a frame-by-frame
scan. `ClayShellWidget` transfers focus to a surviving same-tab target before
`reconcile_pane_hosts` detaches the old host; active-tab removal clears focus
while the replacement remains stashed, and `driver/reconcile.rs` focuses that
replacement after layout. The exact pinned Masonry 0.4.0 source under
`vendor/masonry_core/` invalidates focused/next/fallback IDs in
`MutateCtx::remove_child`; Masonry's normal focus rewrite rebuilds the path
and clears ancestor flags, preventing stale `accesskit_consumer` focused IDs.
Its `RenderRoot::handle_access_event` ignores actions addressed to the
synthetic top-level Window node, which is exposed by AT-SPI but has no Masonry
widget. Valid editor Entry focus remains unchanged. Tests:
`cargo test --lib masonry_shell -- --test-threads=1`.

### Layout Intent API

`serverRequestLayoutIntent` (`clay:ui` facade → `op_clay_ui_request_layout_intent` → `PackageUiRegistry::request_layout_intent`) accepts inert versioned layout intents from packages. Validation: package-prefixed ID, orientation (horizontal/vertical), ratio (0.05–0.95), position (first/second), payload size. Intents are composed into `WorkingAreaLayoutUpdate` via `PaneSplitTree::split_pane()` at Clay's discretion. Packages cannot mutate native layout directly.

### Key Invariants

- `compute_geometry()` is a pure `&self` read — no mutation during Masonry layout pass.
- Drag/resize handlers call only layout methods — no package JS, no ops, no theme resolution.
- All layout types are `pub(crate)`; only `serverRequestLayoutIntent` crosses the public op/facade boundary.
- Persistence writes only non-default state; defaults are never persisted.

### Source Paths

- `src/shell/layout.rs`: Split divider hit-test, drag ratio, slot resize/collapse, focus traversal, `split_pane` composition.
- `src/shell/layout_persist.rs`: Serialization, I/O, apply/restore.
- `src/masonry_shell/mod.rs`: Pointer/keyboard event handlers, paint, persistence debounce.
- `src/server/ui.rs`: `RegisteredLayoutIntent`, `request_layout_intent` validation.
- `src/server/ops/ui.rs`: `op_clay_ui_request_layout_intent`.
- `runtime/js/ui.js`: `serverRequestLayoutIntent` facade.

### Tests

- `src/shell/layout.rs`: 52 tests (geometry invariants, clamping, collapse, focus traversal, split composition, drag interaction).
- `src/shell/layout_persist.rs`: 6 tests (round-trip, corrupt fallback, selective persistence).
- `src/server/ui.rs`: 6 tests (intent validation: ratio, orientation, provenance, duplicate, default position).
- Command: `cargo test --lib shell --quiet`

## Phase 22.1: Equal-Area Window Splits

Phase 22.1 (2026-08-05) turns the single-pane shell into a real multi-pane working area with user-rebindable commands, a generic pane content host, and a configurable pane-focus policy.

### PaneSplitTree Lifecycle Operations

`src/shell/layout.rs` extends the Phase 18.2 tree with four immutable lifecycle operations (each returns a new tree or `None`):

- `split_pane(...)` — now capped: returns `None` once the tree reaches `MAX_PANES_PER_TAB` (4).
- `close_pane(pane_id)` — removes the leaf and promotes its sibling subtree; returns `None` for the last pane or a missing ID. Focus hands off to the sibling subtree's first leaf.
- `add_equal_pane()` — redivides the whole working area into N+1 EQUAL areas: a right-leaning comb along the root split orientation with ratios `1/(N+1), 1/N, ..., 1/2` (leaf k gets exactly `1/(N+1)` of the parent area). Existing panes keep reading order; the new pane is appended last. `None` at the cap.
- `move_pane(pane_id, SplitChild)` — swaps leaf IDs in reading order (tree shape and ratios preserved); active pane follows the moved ID. `None` at the reading-order ends.
- `keyboard_resize(pane_id, PaneResizeDirection)` — finds the deepest ancestor split whose orientation matches the direction axis AND whose child side borders the direction (e.g. `Left` requires the pane to sit in the `Second` subtree of a `Horizontal` split), then nudges that divider by `KEYBOARD_RESIZE_STEP` (0.05), clamped. Returns the `(SplitPath, SplitRatio)` for the caller to apply via `update_split_ratio` + version bump; `None` when no bordering divider exists.

### Multi-Pane Hosting (`PaneContentHost`)

`ClayShellWidget` no longer registers the `EditorWidget` directly. It holds `pane_hosts: BTreeMap<PaneId, WidgetPod<PaneContentHost>>` — one stable-identity content host per tree leaf (`src/masonry_pane_host.rs`). `PaneContent` is an enum: `Placeholder` (fills the pane with the `surface.panel` token color) or `Editor(WidgetPod<EditorWidget>)`. Panes are generic workspace-bound content hosts, not file views — a future terminal emulator or preview plugs into the same host shape (Phase 22.2+).

Tree changes reconcile through `reconcile_pane_hosts(ctx: &mut MutateCtx)`: surviving pane hosts keep their `WidgetPod` identity (focus/scroll state preserved), new leaves get placeholder hosts, and removed hosts go through `ctx.remove_child()` before drop, then `ctx.children_changed()` re-registers. `apply_layout_update` (no-MutateCtx path) pre-syncs state and parks removed pods in `pending_orphans` until the next reconcile. Masonry layout places each host at its pane's `main_slot_rect` from `pane_slot_geometry`.

### Pane Focus Policy

`PaneFocusPolicy` (`ClickToFocus` default | `FollowsCursor`) lives on `ClayShellWidget` and is set via `set_pane_focus_policy`. Activation uses a dual path because `EditorWidget` consumes every pointer-down (`request_focus` + handled):

- Placeholder panes don't consume pointer events, so `PointerEvent::Down` bubbles to the shell and activates the pane (click-to-focus).
- Editor panes activate via `Update::ChildFocusChanged(true)` — the shell maps its editor child's focus gain to the editor pane. This path is policy-independent.
- `FollowsCursor` additionally activates the pane under the pointer on `PointerEvent::Move` (editor Move events bubble when the editor is not pressed), skipped while a divider/slot drag is active and when only one pane exists.

Tab/Shift+Tab pane cycling (Phase 20.3) remains: with more than one pane, `Tab` cycles pane focus instead of inserting indentation.

### Shell Commands and Key Routing

`ShellClientCommand` (20 variants, `src/masonry_shell/window_tabs.rs`) parses the `shell.client*` command IDs: the 12 pane commands (`clientSplitPaneVertical/Horizontal`, `clientAddEqualPane`, `clientClosePane`, `clientFocusPaneNext/Prev`, `clientResizePaneLeft/Right/Up/Down`, `clientMovePaneNext/Prev`) plus the 8 Phase 22.4 tab commands (`clientTabNext/Prev/New/Close/MoveLeft/MoveRight`, `clientTabActivate(u32)`, `clientTabMoveTo(u32)` — 1-based card positions). Command names follow the vim convention: a "vertical" split places panes side by side (`SplitOrientation::Horizontal`), a "horizontal" split stacks them.

Dispatch path: `main.rs` resolves `ClientUiCommandResult::ShellCommand`; tab commands are intercepted by the driver's `apply_tab_command` **before** the widget (the widget's tab arms stay inert) — the driver resolves card positions from its `tab_order` policy and routes through the shared execution paths the tab bar also uses (activate/close/new/move), so chords and clicks share one code path. Pane commands reach `render_root.edit_widget(shell_widget_id).apply_shell_client_command(...)` — tree ops go through `WorkingAreaLayout::replace_pane_tree` (new in 22.1; bumps the layout version) + `reconcile_pane_hosts`; resize goes through `keyboard_resize` + `commit_split_drag`. No server round-trip, no JS runtime, no IPC in the dispatch path (guarded by `shell_command_dispatch_requires_no_server_or_js_runtime`).

Defaults ship in `default_keymaps()` (`src/protocol/mod.rs`) as `Global`-context `ClientUiCommand` rules: `Ctrl+\` / `Ctrl+-` / `Ctrl+Shift+\` / `Ctrl+Alt+W`, `Ctrl+Alt+Left|Right` (focus), `Ctrl+Alt+Shift+arrows` (resize), `Ctrl+Alt+[|]` (move), and the 22.4 tab chords (`Ctrl+Tab`, `Ctrl+Shift+Tab`, `Ctrl+T`, `Ctrl+Shift+W`, `Ctrl+<N>` 1..=9, `Ctrl+Shift+[|]`, `Ctrl+Shift+<N>` 1..=9). All IDs are registered bindable in `is_runtime_bindable_command` with `ClientUiCommand` routing (22.4 numbered families via `tab_family_variant`, 1..=9 only), so `bindKey(key, id, { scope: "global" })` in init.js overrides them. `ClientBehaviorState::route_key` matches in two passes — `EditorTextFocus` rules first, then `Global` — so an editor-context binding for the same chord always wins over a shell default.

Known 22.1 ceiling: `Global` rules route through the editor's key path, so the chords don't fire while a placeholder pane (no editor focus) is active.

### Shell Preferences Transport

`setPaneFocusPolicy({ paneFocusPolicy: "click" | "cursor" })` (`clay:shell` facade, `runtime/js/shell.js`) calls `op_clay_shell_set_pane_focus_policy` (admin-only op), which validates the closed enum and publishes through `ClayOpState::publish_shell_preferences`. The value travels the same broadcast-lane pattern as `CaretStyleOverride`: service-level channel + store (`ClayJsRuntimeService`), connection handler forwards it as `ServerMessage::ShellPreferences` (protocol v10; also sent during the initial handshake), and the client applies it as `ClientConnectionEvent::ShellPreferences` → `ClayShellWidget::set_pane_focus_policy` live. Unknown values fail evaluation with a `shell.invalid_pane_focus_policy` diagnostic.

## Phase 22.2: Pane Document Views and Focus Routing

Phase 22.2 (2026-08-05) wires document views into the pane hosts and makes the shell's focus state drive document routing (see [Pane Document Views](pane-document-views.md) for the full model):

- `PaneContent` (`src/masonry_pane_host.rs`) gained `Document(PaneDocumentView)`; `PaneContentHost::set_document_view` mounts a view into a host and `clear_content` unmounts it (via `std::mem::replace` so the child leaves the Masonry context). Placeholder panes remain until a document is opened in them.
- `ClayShellWidget` tracks `pane_targets: BTreeMap<PaneId, WidgetId>` — the routing target (document view) per pane — so the driver can deliver document-scoped `ClientConnectionEvent`s to the owning pane and move Masonry focus to the active pane.
- `EditorAction::PaneFocused(pane_id)` is submitted on Tab/Shift+Tab cycling AND pointer-driven activation; the driver uses it to keep Masonry focus in sync with the shell's active pane. `focus_fallback_widget_id()` returns the ACTIVE pane's routing target (was the single editor child).
- The shell's `Action` type changed from `NoAction` to `EditorAction` to carry `PaneFocused`.
- Pane close (`ClosePane` shell command) is now document-aware in the driver: a dirty active document blocks the close (save-conflict menu), clean panes release their document through `PaneDocumentView::close_pane` before the tree op, and the pane's routing target + pending-open entries are removed. Topology mutation itself stays a pure client-side `PaneSplitTree` rebuild; the only server round-trip is the capability-gated document release.
- Hot-path guard unchanged in spirit: keystroke handling in a 4-pane shell touches ONLY the focused pane's view — no IPC, no JS runtime (`pane_document_typing_requires_no_server_or_js`).

### Key Invariants

- Pane host `WidgetPod` identity survives topology changes; only added/removed leaves touch the Masonry arena.
- All tree ops are immutable rebuilds bounded by `MAX_PANES_PER_TAB` = 4 and `MAX_PANE_SPLIT_TREE_NODES` = 64.
- Split command dispatch is pure client work: no server IPC, JS runtime, or package code in the keypress path.
- The focus policy grants no authority; it only selects which pointer interaction activates a pane.
- Topology persistence (split trees across restarts) is Phase 22.5, which
  persists the whole per-tab window state in `layout.json` v2 (see the
  Phase 22.5 section); legacy v1 files still restore ratio/slot state only.

### Source Paths

- `src/shell/layout.rs`: lifecycle ops, `MAX_PANES_PER_TAB`, `PaneResizeDirection`, `replace_pane_tree`.
- `src/masonry_pane_host.rs`: `PaneContentHost` widget (placeholder/document content, `set_document_view`/`clear_content`).
- `src/masonry_shell/mod.rs`: `pane_hosts` map, `pane_targets` routing map, reconcile, focus policy, `PaneFocused` actions, `ShellClientCommand` dispatch.
- `src/app_driver.rs`: shell action target + `ShellCommand`/`ShellPreferences` dispatch.
- `src/server/ops/shell.rs`: `op_clay_shell_set_pane_focus_policy`.
- `src/client/behavior.rs`: two-pass `EditorTextFocus`-before-`Global` route_key matching.
- `src/protocol/mod.rs`: `ShellPreferences`, shell default keymaps/commands, protocol v10.
- `runtime/js/shell.js` / `shell.d.ts`: command-ID helpers + `setPaneFocusPolicy`.
- `docs/reference/clay-js-api/shell/`: per-command and configuration API docs (authoritative public surface).

### Tests

- `src/shell/layout.rs`: cap rejection, equal-area redivision (2/3/4 panes, area equality), close/merge/focus-handoff, move swaps + end no-ops + ratio preservation, keyboard resize bordering/deepest/clamp/no-divider. Command: `cargo test --lib shell::layout --quiet`.
- `src/masonry_shell/mod.rs`: host identity stability across tree mutations, orphan detachment, placeholder hosting, per-pane placement, click-to-focus on placeholders, focus-policy behavior (default, follows-cursor, drag-skip), all 12 command dispatches, 4-pane cap enforcement, the no-server/no-JS hot-path guard, and (22.2) independent per-pane document views with document-scoped routing, typing-isolation hot-path guard, routing-target cleanup on pane close, and concurrent per-pane major modes isolated across behavior manifests. Phase 22.8 verification re-runs this matrix with per-tab server roots/document sets, per-document lease/version reservations, retained-session switching, duplicate-open ownership, and the four-pane cap; no split or hot-path implementation changes were needed. Command: `cargo test --lib masonry_shell --quiet`.
- `src/server/js_runtime/mod.rs`: policy publish/reject/default-unset through real init.js evaluation.
- `src/server/ops/keybindings.rs`: all 12 shell IDs bindable + `ClientUiCommand`-routed; unknown `shell.*` rejected.
- `src/protocol/mod.rs`: defaults present with `Global` context and `ClientUi` authority.

## Phase 22.3: Tabs as Independent Client Views (multi-connection model)

Phase 22.3 (2026-08-06) makes each tab an independent client view with its own server connection. The server side (protocol v11, `src/server/tab_registry.rs`) is covered in the tab-registry task; this section covers the client-side multi-connection model:

- `ClayShellWidget` now owns `tabs: BTreeMap<ClientId, TabChrome>` + `active_tab: ClientId`. `TabChrome` (new, `src/masonry_shell/window_tabs.rs`) bundles one tab's `WorkingAreaLayout`, `pane_hosts`, `pane_targets`, `pane_focus_policy`, and `pending_orphans` — everything that was previously shell-level single-tab state. The shell hosts **every** tab's hosts as registered children; only the active tab's hosts are laid out at their pane rects, inactive tabs' hosts remain registered but are stashed during layout, so they keep connection/reconnect continuity without painting, hit-testing, or accessibility emission.
- Tabs are keyed by the connection's `ClientId` (the client-known identity at mount time; the server-assigned `TabId` arrives asynchronously via the registry snapshot and is tracked by the app driver). `install_tab` mounts a tab's chrome (first tab becomes active; later tabs are retained until `set_active_tab`), `set_active_tab` switches with one layout pass and resets in-flight drag sessions, `tab_for_chrome` resolves an event's tab from its chrome id, and per-tab queries (`pane_targets_for`, `pane_host_id_for`, `editor_widget_id_for`, `set_pane_focus_policy_for`, …) scope routing to one tab. The no-arg routing methods delegate to the active tab, so single-tab behavior is the pre-22.3 experience.
- `src/driver/mod.rs` `Driver` owns `tabs: BTreeMap<ClientId, TabState>`
  (`TabState` = the connection's `ClientEditQueue` clone, per-tab
  `pending_opens`, and the server-assigned `tab_id`), `active_tab`, and the
  latest `registry` snapshot (Phase 22.7: the whole driver moved here from
  `src/main.rs` — see the [driver module map](driver.md)). The session
  itself is consumed at mount (initial state → chrome, events → per-tab
  bridge, edit queue → chrome + `TabState`). `mount_tab` (open-tab
  mechanism: receives a session already bound with `TabCommand::New`, builds
  the chrome + `TabChrome`, installs + activates, and spawns the per-tab event bridge) and `switch_tab`
  are wired by the lifecycle task's affordance; `apply_tab_registry` fills
  each tab's `tab_id` by matching `client_id`.
- Event routing: each tab's bridge tags its events with that tab's chrome `WidgetId`; the driver resolves the tab via `tab_for_chrome` and routes document-scoped events, fan-outs, runtime snapshots, and editor commands to **that tab's** targets only. `ShellPreferences` applies to the sending tab's focus policy; `TabRegistry` snapshots are driver-level state. Menus display in the tab's own chrome overlay. Focus follows only for the active tab (an inactive tab's open never steals focus).
- `ClientEditQueue::enqueue_tab_command` sends `ClientMessage::TabCommand` on a connection.

- The tab bar (Phase 22.3): a shell-owned window-level row painted only when more than one card is present. It is carved in `ClayShellWidget::layout()` (the plan's "equivalent shell-level row" option — `PaneSlotLayout::compute_geometry` is per-pane, so a window-level row cannot live there without changing package slot semantics). The bar is token-driven end to end: `tab_card_chrome` (src/shell/primitives.rs) resolves `surface.list`/`surface.selected` rests, `surface.hover`, `surface.active`, `text.primary`/`text.muted`/`text.disabled`, the `accent.primary` focus ring (painted for the `Focus` state; keyboard focus traversal between cards is a 22.6 ceiling — the 22.4 chords activate/close/move tabs without moving card focus), and the dimmed disabled close. The label uses the UI `Status` typography variant (`UiTextVariant::Status` via the active tab's cached `TypographyRegistry`) painted through the shared `paint_sdui_text` with a clip layer. `ActiveTypography` events update the matching tab and request layout/render/accessibility only when the revision changes; the active tab mirrors that registry to the window-level bar. Bar/card affordance geometry follows the resulting status metrics and is clamped to the logical window. The close glyph is two inline strokes. Cards are registry-driven: the driver pushes `TabCard`s from the server snapshot (order + names = root display path's final segment); mounted tabs awaiting their registry entry are appended with close disabled. Card order is the numbering authority for the 22.4 numbered commands — `clientTabActivate.<N>`/`clientTabMoveTo.<N>` resolve 1-based against this exact card list (registry order, entry-less mounted tabs appended; beyond-count = silent no-op). Clicking a card submits `EditorAction::TabBar(Activate)`; the driver switches optimistically and the server registry reconciles — the server now pushes a `TabRegistry` snapshot even for rejected `Activate`/`Close`/`OpenWorkspace`/move commands, so a rejected optimistic switch reverts. Clicking the close glyph submits `Close`; the driver refuses closing the last mounted tab; a dirty tab gets the 22.4 driver-owned tab-confirm menu (`tab_close_confirm_session`) instead of closing; a clean (or confirmed) close sends `TabCommand::Close` on that tab's own connection, and the server's pushed snapshot (entry removed) drives removal (`apply_registry_reconcile` uninstalls the chrome, switches + focuses the remaining tab, refreshes cards). Removals are skipped against an empty registry (server restart; the lifecycle task re-registers via `Reclaim`/`New`).

### Lifecycle (22.3 task 7: open / close / switch / reconnect)

- **Open tab**: the tab bar's `+` affordance (`TabBarAction::NewTab`, right edge of the bar, present whenever the bar is) opens the native folder picker; the picked folder becomes the new tab's workspace root. The driver connects a fresh session with `TabCommand::New(workspace_root)` during the handshake; the server returns that tab's initial document/browser state, then `mount_tab` mounts a new chrome + default single-pane split tree, spawns its event bridge, and switches to it. Connection failures (`OpenTabFailed` — e.g. the `MAX_ACTIVE_CONNECTIONS` cap) surface a `tabs.open_failed` diagnostic on the active tab's chrome and never mount a tab. `Ctrl+T` (22.4) routes to the same `open_new_tab_dialog` path as `+` — same flow, same in-flight guard (a second request while the picker is open is ignored).
- **Close tab**: the driver's `close_tab` (shared by card `✕` and `Ctrl+Shift+W`) first passes `tab_close_allowed` (never the last tab — the window never goes to zero tabs) and then the 22.4 dirty inventory `dirty_documents_in_tab` (replaces the 22.3 `guard_tab_close` first-dirty-view walk): a clean tab enqueues `TabCommand::Close` directly; a dirty tab gets the driver-owned confirm session (`clay::shell::tab_close_confirm_session` — Save all and close / Discard and close / Cancel) hosted on the active pane view + chrome overlay, with `pending_close_after_saves`/`advance_pending_close_after_saves` counting `DocumentSaved` acks before the close enqueues (cancelled on `FileOperationFailed`/disconnect). A clean close is server-authoritative: the server removes the registry entry and ends the connection, so the permit + leases release through the existing disconnect cleanup; the removal snapshot (observed on the other tabs' connections — the closing connection never reads its own broadcast update) drives `apply_registry_reconcile` uninstall.
- **Switch tab**: card click → optimistic `switch_tab` + `TabCommand::Activate`; the server registry reconciles rejections.
- **Reconnect**: on `Disconnected`/`ConnectionError` for a tab, `start_tab_reconnect` spawns a per-tab task that retries `client::connect_for_reclaim_or_new(tab_id, workspace_root)` with the existing backoff until it succeeds or the tab is removed (per-tab `Arc<AtomicBool>` cancellation flag set by `apply_registry_reconcile`). The fresh session already owns the reclaimed tab's initial document/browser state when `Reclaim` succeeds; after a server reset or TTL eviction, the same persisted root is bound through `New`. The driver swaps its queue into the chrome and every pane view (`reconnect`), re-keys the tab, and re-opens every document it holds through plain `OpenDocument` because the fresh connection has no selected-file capability. It spawns a new event bridge to the same chrome and restores focus when the tab was active. In-flight `pending_opens` are cleared. Split trees and per-pane document state restore from retained `TabChrome`/sessions; a full client process restart selects each persisted root before handshake-bound `New`. Multi-client reclamation remains a 21 ceiling.

### Phase 24.4: centered modal surface accessibility

The active Command Centre centered menu is mounted as one window-level Masonry
layer above the shell. Its `PackageOverlayHost` reports a named modal Dialog,
paints the scrim, and swallows scrim pointer events. The menu region reports
Menu/MenuItem/Status semantics; the result-count Status is stable and polite.
Masonry focus remains on the originating pane so `PaneDocumentView` remains the
single keyboard route and close restores the original focus target.

### Key Invariants

- One connection per tab: separate `ClientEditQueue`/sync state, chrome (SDUI region, panels, overlays, runtime generation), split tree, pane targets, focus policy, and pending-open attribution. Editing in one tab never mutates another tab's state.
- Inactive tabs remain registered for connection/reconnect continuity but are stashed: no paint, hit-test, or accessibility walk occurs until activation unstashes and re-lays out the hosts.
- The client-side tab map is view/routing state and grants nothing; the server registry is authoritative for tab order/active/ids.
- `layout.json` v2 persistence runs at any tab count (Phase 22.5) — the
  shell signals `PersistenceDue` on committed layout mutations; the driver
  writes the whole window state (see the Phase 22.5 section below).
- Multi-client tab reclamation is out of scope (Phase 21); disk persistence
  is shipped (22.5), unsaved buffers/caret/viewport are not restored.


### Source Paths

- `src/masonry_shell/mod.rs`: `tabs`/`active_tab`, `install_tab`/`set_active_tab`/`tab_for_chrome`, per-tab routing queries, inactive-host stashing; tab bar paint + pointer handling.
- `src/masonry_shell/window_tabs.rs`: `TabChrome`, `TabCard`, `set_tab_cards`, `remove_tab`, `tab_bar_geometry`/`tab_bar_hit_test`, `TAB_BAR_*` constants.
- `src/shell/primitives.rs`: `tab_card_chrome` state resolver (`TabCardChrome`).
- `src/masonry_pane_document.rs`: `reconnect` (queue swap + reinstall re-arm + menu clear), `documents_for_reopen`, per-session `workspace_root_id`/`path` retention.
- `src/editor/document_session.rs`: `RetainedDocumentSession` open identity; `reopen_documents`.
- `src/masonry_editor.rs`: `EditorAction::TabBar` + `TabBarAction`.
- `src/driver/mod.rs` + `src/driver/reconcile.rs`: `TabRegistryReconcile` (pure `apply_tab_registry` + shell-side `apply_registry_reconcile`), `TabBar` action handler (optimistic activate / dirty-guarded close / `NewTab` affordance), handshake-bound `TabCommand::New`, reconnect (`start_tab_reconnect`, `reconnect_tab` handler: queue swap, rekey, reclaimed initial state, `OpenDocument` re-opens, new bridge), `guard_tab_close`, `tab_close_allowed`, `tab_card_display_name`.
- `src/driver/mod.rs`: `TabState`, `mount_tab`/`switch_tab`/`apply_tab_registry`, per-tab event routing, per-tab bridges, `with_shell`/`with_editor`/`with_view` typed access helpers.
- `src/client/mod.rs`: `ClientEditQueue::enqueue_tab_command`.
- `src/protocol/mod.rs`: `TabId`, `TabEntry`, `TabRegistrySnapshot`, `TabCommand`, `ClientMessage::TabCommand`, `ServerMessage::TabRegistry`, protocol v15 deferred-binding pin.
- `src/server/tab_registry.rs`: server-authoritative in-memory registry.

### Tests

- `src/masonry_shell/mod.rs`: install-then-activate, stable chrome/host ids across switches, inactive hosts stashed and hidden from accessibility, single-tab shape unchanged, per-tab routing-target and focus-policy isolation; tab bar: hidden below two cards (single-tab geometry unchanged), geometry/carve with two cards, activate/close/no-op click actions, hover tracking, remove-tab uninstall + active fallback. Command: `cargo test --lib masonry_shell --quiet`.
- `src/shell/primitives.rs`: `tab_card_chrome` resolves every state (Rest/Hover/Active/Focus/Disabled × selected). Command: `cargo test --lib shell::primitives --quiet`.
- `src/client/mod.rs`: real-server tab command end-to-end — handshake-bound `New` registers the selected root, a rejected `Activate` pushes a reconciling snapshot, `Close` ends the connection + removes the registry entry (observed on an unbound fresh replay), a dropped connection is reclaimed by `connect_for_reclaim` (`Reclaim` keeps `TabId`, rebinds `ClientId`), server restart falls back to root-scoped `New`, and the connection cap refuses excess connections. Command: `cargo test --lib client::tests::real_server_tab --quiet`, `cargo test --lib real_server_ --quiet`.
- `src/driver/reconcile.rs`: registry reconciliation — fills tab ids +
  builds cards, removes closed tabs + activates the remaining one, skips
  removals on empty (restart) snapshots. Command:
  `cargo test --bin clay --quiet`.
- `src/driver/mod.rs`: registry application fills server-assigned tab ids;
  per-tab edit queues are isolated channels. Command:
  `cargo test --bin clay --quiet` (driver tests).
- Server-side registry tests: `cargo test --lib server::tab_registry --quiet`.

## Phase 22.5: Window-State Persistence (per-tab layouts and documents)

Phase 22.5 (2026-08-08) persists the whole window to `layout.json` v2 and
restores it at startup: tab order, active tab, per-tab workspace + split
tree, and per-pane open documents. The shell side of the design:

- **Persistence signal**: `persist_debounced` (no longer a file writer)
  emits `EditorAction::PersistenceDue` — the driver's single v2 writer
  (`persist_window_state` → `clay::shell::save_window_state`) fires from
  the signal, registry snapshots, `DocumentOpened`, and the quit flush. The
  ≥ 500 ms debounce (`mark_persistence_due`, `last_persist`) keeps the
  pointer hot path free of disk I/O; keyboard pane commands
  (`apply_keyboard_resize`, `apply_tree_change`) also signal.
- **Collection**: `tab_layout_data()` → `Vec<(ClientId, PersistedTabLayout)>`
  (owned clones: active pane, `PaneSplitNode` tree root, v1-style slot
  entries) + per-pane document identity via `PaneDocumentView::
  active_document_identity()` / the `EditorWidget` wrapper (active document
  only, retained sessions excluded). Driver orders tabs by registry order
  with entry-less mounted tabs appended.
- **Restore constructors**: `restored_single_editor(client_id, editor,
  persisted)` builds the bootstrap tab's chrome pre-event-loop from
  `layout_from_persisted_tab` (production primitives: `single_editor` +
  `PaneSplitTree::new` + `replace_pane_tree` + slot apply; any structural
  failure degrades to the default single pane, never partial state);
  `install_restored_tab(ctx, client_id, editor, persisted) -> WidgetId`
  installs tabs 2..N without switching active (returns the chrome's editor
  widget id for the event bridge). Both route through the shared
  `TabChrome::with_layout` (formerly test-only), which now also serves the
  restore path. A v2 file is a silent no-op for the legacy
  `apply_persisted_state` (no top-level `splits`/`slots` keys).
- **TabChrome rebuild path**: restored chromes are built once from the
  persisted tree (placeholder panes become real targets as documents open),
  then live exactly like mounted tabs — rekey/reconnect/switch mechanics
  are unchanged.

### Key Invariants

- The shell never writes files: all disk I/O is the driver's
  `save_window_state`, off the hot path and debounced.
- Restore never switches active mid-mount (the driver activates the
  persisted active tab once at the end; see the tabs-and-clients page for
  the sequencing state machine).

### Source Paths

- `src/shell/layout_persist.rs`: v2 schema (`PersistedWindowState`/
  `PersistedTabState`/`PersistedTabLayout`), `serialize_window_state`/
  `parse_window_state`/`layout_from_persisted_tab`, `save_window_state`/
  `load_window_state`.
- `src/masonry_shell/mod.rs`: `persist_debounced`/`mark_persistence_due`,
  `tab_layout_data`, `restored_single_editor`, `install_restored_tab`,
  `TabChrome::with_layout`.
- `src/driver/restore.rs`: restore state machine (`advance_restore`,
  `mount_restored_tab`, `reopen_restored_documents`, `finish_restore`,
  `abandon_restore`, `RESTORE_CONFIRM_TIMEOUT`) + `persist_window_state`.
- `src/masonry_editor.rs`, `src/masonry_pane_document.rs`:
  `active_document_identity` accessors.

### Tests

- `src/shell/layout_persist.rs`: v2 round-trip, bounds, corrupt/legacy
  fallback, panic-free hostile input. Command: `cargo test --lib
  layout_persist --quiet`.
- `src/masonry_shell/mod.rs`: persistence-signal emission (pointer mutation +
  keyboard resize, multi-tab), `tab_layout_data_returns_every_mounted_tab_layout`,
  `restored_single_editor_mounts_persisted_split_tree`,
  `install_restored_tab_mounts_persisted_tree_without_switching`. Command:
  `cargo test --lib masonry_shell --quiet`.
- Driver restore suite + real-server E2E: see the tabs-and-clients page.
## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.2 Shell Runtime Primitive Review](phase18.2-shell-runtime-primitive-review.md)
- [Pane Document Views](pane-document-views.md) — per-pane document hosting and routing (22.2)
- [Tabs and Independent Client Views](tabs-and-clients.md) — the multi-connection model, server registry, lifecycle, reconnect (22.3)
- [Phase 18.1 Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Shell Primitives](shell-primitives.md)
- `plans/025-Phase18.2-Masonry-Clay-Shell-and-Pane-Runtime-Foundation.md`
- `plans/064-Phase20.3-Layout-Primitives-Splits-Panel-Resizing-and-Screen-Division.md`
- `plans/072-Phase22.1-Equal-Area-Window-Splits.md`

## Phase 22.6: Window-Model Accessibility and Announcements

Phase 22.6 (2026-08-08) adds the window-model accessibility contract and
its budgets. Two surfaces: a **structural tree** (roles/names for the tab
bar, tab cards, and panes) and **polite live announcements** for tab and
split changes. The public contract (roles table, exact announcement
strings, sanitization rules) lives in `docs/development/accessibility.md`;
this section documents the implementation.

### Structural tree (`ClayShellWidget::accessibility`)

- **Virtual-node pattern**: synthetic nodes use
  `crate::editor::accessibility::virtual_a11y_node_id(owner, slot)`, not
  `WidgetId::next()` per pass. The helper uses the `0xD000…` virtual prefix,
  retained owner ID, and a bounded 9-bit slot; shell slots are TabList `1`,
  announcement `2`, and Tab `3 + client_id`. The retained owner keeps IDs
  stable across redraws while replacing the owner intentionally retires its
  namespace. AccessKit 0.21 has no `insert_children`, so the shell still
  builds semantic nodes in `ctx.tree_update().nodes` and attaches their IDs
  with `node.set_children(...)`.
- **Reachable-child invariant**: Masonry walks every `children_ids()` child
  during accessibility. The shell keeps all registered pane hosts and
  pending orphans there for the arena contract, while `layout()` calls
  `ctx.set_stashed(host, true)` for inactive-tab hosts and pending orphans.
  Active hosts are unstashed and laid out normally. Stashed subtrees are not
  painted or emitted by the accessibility walk, so inactive panes cannot
  become orphaned nodes.
- **Registration truth**: `TabChrome.registered_panes`
  (`BTreeSet<PaneId>`) records which hosts a register pass inserted. The
  reconcile path updates registered hosts through `MutateCtx::get_mut` only;
  newly synced pods receive their pane count at creation and are updated on a
  later register pass. This avoids the Masonry arena panic found when
  `apply_layout_update` synced hosts before reconciliation.
- **TabList/Tab**: when `tab_cards.len() >= 2`, one `Role::TabList`
  (`Workspace tabs`) precedes the pane hosts with one `Role::Tab` per card
  in card order — sanitized workspace basename, `selected` on
  `card.client_id == self.active_tab` (`set_selected`, `is_selected` →
  `Option<bool>`). Single-tab windows keep the pre-22.6 tree shape (no
  TabList noise). Cards stay painted chrome; the nodes are informational
  (pane hosts remain the focusable units; keyboard switching is the 22.4
  tab commands).
- **Pane labels**: `PaneContentHost` gained `pane_count` (default 1,
  `with_pane_count` builder, `set_pane_count` during
  `reconcile_pane_hosts`) and `document_display_name`
  (`set_document_display_name`). Labels: `Empty pane N of M` /
  `Pane N of M: editor` / `Pane N of M: {name}` / `Pane N of M: document`
  (name unknown). Names arrive from the driver via
  `set_pane_document_name(path)` which runs `sanitize_document_display_name`
  (pub(crate) in `src/editor/accessibility.rs`, invisible to the bin crate,
  so sanitization happens at the shell boundary).
- **Geometry**: `node_window_size` uses the root node bounds;
  `accesskit_rect` builds AccessKit `Rect` literals (AccessKit has no
  `Rect::new`).

### Announcements (task 4)

- One persistent shell-owned `Role::Status` node with
  `set_live(Live::Polite)` is ALWAYS the last child; its label is
  `self.announcement: Option<String>` (empty until the first action).
  Being persistent keeps AT live-region registration stable across tree
  rebuilds. `announce()` sets the label and calls
  `ctx.request_accessibility_update()` — the tree is invalidated only when
  the announcement changes.
- `compose_announcement(kind, name, position, count)` is an O(1) builder
  over `AnnouncementKind` (9 variants: TabActivated/TabCreated/TabClosed/
  SplitPaneVertical/SplitPaneHorizontal/PaneAdded/PaneClosed/
  PaneMovedForward/PaneMovedBackward); names are sanitized inside, strings
  are char-truncated to `TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS`
  (256, the shared transient-menu budget; the cap is defense-in-depth —
  the 64-char name cap (`ACCESSIBILITY_DISPLAY_NAME_MAX_CHARS`) binds
  first).
- **User-initiated paths only** (driver, `src/driver/`): `switch_tab`
  returns `bool` and `apply_tree_change` returns `bool` so no-op
  operations never announce; `activate_tab` announces TabActivated after a
  real switch, `mount_tab` (its single call site is the new-tab dialog)
  TabCreated, `remove_tab`/registry reconcile TabClosed, and the
  `apply_shell_client_command` pane arms Split/Pane announcements only when
  the tree actually changed. Focus moves, repaints, startup/restore, and
  no-ops stay silent. Announcements are unconditional (no configuration
  key; see task 10).

### Budgets and baselines (task 5)

- `src/perf/budgets.rs`: advisory `PANE_PAINT_P95_BUDGET_MS = 1` and
  `TAB_SWITCH_P95_BUDGET_MS = 1` (window_baselines measured 68–807 ns,
  linear in pane count), hard
  `MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES = 4 *
  DECORATION_PAYLOAD_BUDGET_BYTES` (32768).
- `src/perf/baselines.rs`: `pane_chrome_piece_count(n)` /
  `tab_switch_geometry_work(n)` (pub, bench-facing; the focus ring counts
  only when `pane_count > 1`) over pub(crate) `pane_split_tree_with` /
  `working_area_layout_with` (balanced comb trees, ≤ 4 panes per the
  `MAX_PANES_PER_TAB` cap). `benches/window_baselines.rs` (6th Cargo.toml
  bench entry) benches pane counts 1/2/4/8 as pure geometry work.

### Key Invariants

- Tree order is always TabList (2+ cards) → active pane hosts (pane order)
  → Status announcement node; inactive-tab hosts are unreachable.
- One announcement per real user action; no announcement on no-ops/focus
  moves/repaints; labels carry sanitized basenames only (never host paths,
  clipboard, or control chars).
- The a11y nodes are virtual — no new focusable widgets, no new
  ComponentKind/style tokens, no package-facing surface (task 9 pins
  op/facade absence).

### Source Paths

- `src/masonry_shell/mod.rs`: `accessibility` (tree chain), `announce`,
  `announce_tab_activated`/`announce_tab_created` (pub — bin crate),
  `announce_pane_change` (private), `compose_announcement`/`AnnouncementKind`
  (pub(crate)), `set_pane_document_name`, `reconcile_pane_hosts` +
  `registered_panes`, `node_window_size`/`accesskit_rect`.
- `src/masonry_pane_host.rs`: `pane_count`, `document_display_name`,
  `with_pane_count`/`set_pane_count`/`set_document_display_name` (pub(crate)),
  label formats in `accessibility`.
- `src/driver/mod.rs`: `switch_tab -> bool`, `mount_tab` announcement call
  site; `src/driver/restore.rs`: `activate_tab` announcement call site;
  `src/masonry_shell/mod.rs`: `apply_tree_change -> bool`, `remove_tab`
  announcement call site, `route_document_opened`/`route_document_event`
  label routing;
  `src/client/mod.rs`: `ClientConnectionEvent::metadata_path`.
- `src/editor/accessibility.rs`: `virtual_a11y_node_id`,
  `virtual_a11y_slots`, and `sanitize_document_display_name` (all
  crate-private; 64-character display-name cap).

### Tests

- `src/masonry_shell/mod.rs` (`cargo test --lib masonry_shell --quiet`): a11y
  tests via the `access_tree` helper (`EnableAccessTree` event then
  `redraw()` → `TreeUpdate`) cover single-tab/no-TabList, selected TabList,
  inactive-tab stashing/hiding, pane `N of M` labels, exact bounded
  announcements, and no-op silence. Consumer tests additionally apply real
  updates through `accesskit_consumer::Tree` for initial region attachment,
  tab add/reorder/remove, status changes, and stale-node removal.
- `src/masonry_pane_host.rs`: pane-count defaulting and label updates.
- `tests/performance_budgets.rs` (pins), `tests/editor_performance_
  invariants.rs` (linear chrome work, tab-switch no-reserialization source
  guard, 4-pane aggregate payload), `tests/rust_visibility_api_mapping.rs`
  (22.6 names op/facade-free, internals pub(crate)/private).
- Manual: `test-plan/13-window-splits.md` S23–S28,
  `test-plan/14-tabs.md` T50–T56.

## Phase 22.7: Hardening — Active-Tab Invariant, Connection-Owner Contract, Tab Bar Overflow, Aliases

Phase 22.7 (2026-08-09) closed review findings C3/C4/D1/D4/D5/D6/F5/A6 with
no product behavior change except the deliberate tab-bar scroll and the
additive split aliases:

### Active-tab invariant (finding D1)

`active_tab: ClientId` keeps a sentinel in the zero-tab state; the
invariant "`active_tab` names a mounted tab OR `tabs` is empty" is stated
at the field and enforced with `debug_assert!` (zero release overhead) plus
early-return guards at every public entry point that calls `active()`
(`paint`, `post_paint`, `on_text_event`, `on_pointer_event`,
`accessibility`, `apply_shell_client_command`) when `tabs.is_empty()`.
Rejected: `Option<ClientId>` with fallible accessors (~30 call sites) and
silent fallback to the first tab. The zero-tab accessibility branch labels
the shell "Clay working area shell. No mounted tabs." with empty children.
Tests: `zero_tab_shell_is_inert`, `remove_last_tab_then_reinstall`.

### Connection-owner contract (findings A1/F1/F2)

Closing pane 1 (the editor pane) with siblings must keep the tab's
connection owner — the `EditorWidget` pod — wired, because the driver
routes theme/SDUI/runtime events to `editor_widget_id_for(client_id)`
unconditionally. A probe showed that the old `pending_orphans` drain
detached the editor host on close-then-split churn, so `edit_widget` on
the editor id panicked ("Could not find widget in tree" — masonry_core
`MutateCtx::remove_child` removes the node from the arena). The fix:

- `TabChrome` gains `chrome_orphans: Vec<WidgetPod<PaneContentHost>>`
  (permanent zero-size orphans, never detached) and
  `editor_pane_id: PaneId`.
- `sync_pane_hosts_state` routes the editor pane's host into
  `chrome_orphans` instead of `pending_orphans`; `register_children`
  registers them, layout lays them at zero size, and `remove_tab` detaches
  them.
- `PaneContent` stays a `#[doc(hidden)]` closed enum (Placeholder/Editor/
  Document) with a documented extension seam — a future pane kind adds a
  variant plus host paint/register/layout branches; a package-facing
  `Custom` variant is a Plan 079+ decision. `PaneContentHost` carries the
  theme-stamping (`with_ui_theme`/`set_ui_theme`) and placeholder-fill
  logic (the dark-split bug source, finding D5) and now has direct
  `#[cfg(test)]` unit tests.

Tests: `close_editor_pane_keeps_connection_owner_wired`,
`owner_survives_repeated_split_close_cycles`,
`host_stamps_theme_on_placeholder_at_creation_and_via_setter`,
`host_content_transitions_preserve_pane_identity`.

### Tab bar overflow scroll (findings D6/F5)

The tab bar is a bespoke-painted Clay-owned row (see the Phase 22.3 tab
bar bullet). Cards shrink-to-fit until `TAB_BAR_CARD_MIN_WIDTH = 100.0`
binds, then the strip scrolls:

- `tab_bar_geometry` gains `scroll: f64` + `scroll_max: f64` (pure read:
  `scroll` is clamped to `[0, scroll_max]` on read, no write-back);
  card x positions shift by `-scroll`; `scroll_max = max(0, last_card_x1 -
  strip_right)` where the strip's right boundary is the `+` slot.
- Wheel (`PointerEvent::Scroll`) over the bar
  rect adjusts the stored `tab_bar_scroll`: `LineDelta` lines ×
  `TAB_BAR_SCROLL_STEP = 24.0`, `PixelDelta` 1:1 in pixels (precise-scroll
  input must not inherit the line multiplier — verification-pass fix), page
  deltas ignored (scrollbar wells are not wheel input); clamped to
  `[0, scroll_max]`. `set_active_tab`/card mutation paths call
  `scroll_active_card_into_view` (target = `scroll + rect.x0` when the left
  edge is offscreen, else `scroll + (rect.x1 - strip_right)` when the right
  edge passes the `+` slot, then clamp).
- Paint clips cards to `[bar.x0, new_tab_rect.x0]` and pops before the `+`
  glyph; `tab_bar_hit_test` honors the scroll offset. Overflow needs 6+
  cards at 900 px (5 cards still fit — the last shrinks to ~124 px).

Rejected: an overflow `»` menu and a real scroll-component strip
(wheel-driven offset on a fixed-height painted bar is the minimum working
solution — the catalog scroll component chrome stays for content regions).
Tests: `tab_bar_cards_never_below_min_width`, `tab_bar_wheel_scroll_clamps`,
`tab_bar_hit_test_honors_scroll`, `activating_offscreen_tab_scrolls_it_into_view`.

### Split direction aliases (finding F3)

`shell.clientSplitPaneRight` → `SplitPaneVertical` (side-by-side) and
`shell.clientSplitPaneDown` → `SplitPaneHorizontal` (stacked) map in
`ShellClientCommand::from_command_id` (src/masonry_shell/window_tabs.rs), were added to
both keybinding allowlists (`is_runtime_bindable_command` and the
`ClientUiCommand` routing branch, `src/server/ops/keybindings.rs`), and are
exported by `runtime/js/shell.js` (+ `.d.ts`). No default chords; canonical
`Ctrl+\` / `Ctrl+-` unchanged. The aliases pass the full `validate_command_id`
gate (the `bindKey` config path).

### Cleanup sweep

- Removed the `#![allow(dead_code)]` module attribute from
  `src/shell/layout.rs`; test-only layout symbols (observation types,
  `FixedSlotState` methods, `has_main_slot`, `PaneSlotLayoutError`,
  `working_area_id`/`root_pane_id`, `observable_snapshot`/`slot_observations`,
  `observe_pane_tree_node`) are now `#[cfg(test)]`-gated (the layout types
  stay production via `src/perf/baselines.rs`).
- Dropped the stale `main.rs` "clay client IPC event" eprintln; documented
  the `<=2` hover-reset and the move-tab clarity rationale; the A6 note
  (bespoke two-stroke tab-bar glyphs stay, token-driven colors, no glyph
  primitive) lives in `docs/reference/ui-components.md`.
- `PaneDocumentView` field groups decomposed into `PaneRequestBookkeeping`
  (7 request-id fields) and `PaneMenuSync` (menu/pending/session ids):
  30 → 20 fields, mechanical delegation, focused unit tests
  `request_bookkeeping_allocates_unique_ids`,
  `menu_sync_pending_semantics` (see the pane-document-views page).

## Plan 087: welcome hosting, completion projection, and review harness

Plan 087 (2026-08-15) replaces the prototype welcome document with a
Clay-owned entry state, gives completion a compact caret-adjacent popup,
and adds the repeatable review harness. The shell's role is hosting, not
new authority:

### Welcome entry state hosting

Pane hosts mount `PaneDocumentView`s that may show the retained
`WelcomeWidget` (`src/masonry_welcome.rs`) instead of the native editor:
empty-tab bootstrap snapshots and local-fallback windows enter the welcome
state, and a real `DocumentOpened` transitions the same pane view back to
the editor. The shell sees only the pane's accessibility label and its
usual `Pane N of M` numbering — welcome state is a pane-content concern
(see [Pane Document Views](pane-document-views.md)). The server keeps
owning tab/workspace/document state; welcome buttons submit the existing
client-local `documents.clientOpenFileDialog` /
`workspace.clientOpenFolderDialog` commands, so no new welcome authority
or dialog capability exists.

### Completion overlay hosting

Completion popups are not shell-owned: `PaneDocumentView` publishes a
fixed-point caret/IME `CompletionAnchor` (via `EditorWidget`) to the
chrome-level `PackageOverlayHost`, which re-lays the popup when the anchor
or item count changes. `completion_overlay_rect` (src/shell/package_ui.rs)
is the single geometry helper — below/above-caret placement clamped inside
the active pane, 480 logical-pixel width cap, eight visible-row cap. The
popup is modeless: Masonry focus stays on the editor Entry, and the
`SduiScrollViewport` keeps the selected row visible. The shell itself does
not size or place the popup; it only hosts the retained overlay region.

### Accessibility

- Welcome: pane view exposes `Role::Group` (`Welcome to Clay`) with two
  `Role::Button` children (Click actions) and a polite `Status` virtual
  node (slot `STATUS`, deterministic `virtual_a11y_node_id` identity); the
  native editor node is stashed while visible and the pane refuses text
  input.
- Completion: modeless `Menu` with `MenuItem` rows and a polite status;
  selected row carries the `selected` state; no `Dialog` role, no modal
  trap — the editor Entry keeps AT-SPI focus.
- Both surfaces are consumer-validated through
  `accesskit_consumer::Tree` in unit tests and appear in the live harness
  captures.

### Review harness

`scripts/capture-ui-review.sh --fixture <ui-review-*>` boots isolated
fixtures and captures AT-SPI dumps + screenshots with `review.status`
PASS/UNRESOLVED semantics; see
[Repeatable UI Review Harness](ui-review-harness.md) and
`docs/development/launch-and-gui-smoke.md`. Manual step IDs covering
welcome, completion, Command Centre non-regression, and splits live in
test-plan modules 01 (L12–L14), 03 (F32–F37), 04 (E16–E21), 10 (K69–K72),
11 (Q11–Q14), and 13 (S33–S35).

Known visual follow-up (not fixed by this plan): `P1-087-UI-1` — live
completion and 60+ Command Centre rows paint below their popup shells;
tracked in the plan's Further Actions.

## Plan 088 shell verification and boundaries

Task 4 is paint/layout-only over the retained shell state. `sync_shell_ui_theme` installs the active `ResolvedUiTheme`; `sync_shell_ui_typography` installs each connection's `ActiveTypography` into its `TabChrome`, mirrors only the active tab to the window-level bar, and suppresses duplicate revisions. `tab_card_display_name` filters controls and separators, bounds the display name, and falls back to `Workspace`; the tab bar's `TabList`/`Tab` labels and painted cards therefore share bounded non-path display data.

The shell owns the window-level tab row, pane focus ring, pane host placement, workspace-browser slot decision, and logical-window clipping. Packages cannot own tabs/panes or request the internal completion/centered anchors. The workspace browser is a Clay-owned SDUI surface; hidden/welcome trees reclaim its left slot, while visible package fixed panels still use `PaneSlotLayout` and never cover the editor. Status text, dirty/recovery/connection state, and focus are carried by the pane/editor surfaces rather than color alone.

Task 8 review evidence is screenshot/AT-SPI evidence, not a pixel golden. Default/light/error/large-typography welcome states passed; completion/Command Centre/multi-tab/multi-pane/narrow-wide live states remain unresolved where the host cannot safely target or resize the Clay window. The recovery capture exposed a stale WelcomeWidget connection label; keep that as a product follow-up rather than treating shell geometry tests as a visual pass.

Relevant checks: `cargo test --lib masonry_shell`; `cargo test --bin clay`; `cargo test --test editor editor_performance_invariants`; `cargo test --test editor ui_primitive_conformance`; `src/driver/reconcile.rs::tab_card_display_name_never_falls_back_to_an_absolute_path`; and the shell accessibility tests covering stashed inactive panes, selected tabs, pane labels, and polite announcements.
