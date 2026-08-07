# Masonry Shell Runtime

## Source

- `src/masonry_shell.rs`
- `src/shell/mod.rs`
- `src/shell/layout.rs`
- `src/lib.rs`
- `src/main.rs`
- `src/masonry_editor.rs`

## Overview

The Masonry shell runtime is Clay's native root container for a window working area. It keeps `EditorWidget` as an editor component below a Clay-owned shell root instead of treating the editor widget as the whole application layout.

Phase 18.2 implements the `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` runtime foundations needed for a default one-editor pane, generic horizontal/vertical split topology, and leaf-pane slot geometry. Phase 18.3 adds crate-internal slot-aware package UI runtime state in `src/shell/package_ui.rs`: accepted fixed panel contributions are installed as versioned `FixedPackagePanel` records for explicit `left`, `right`, `top`, or `bottom` slots, and accepted transient overlays are installed separately so overlays do not consume fixed slot geometry. User-visible shell configuration remains planned in later tasks/phases. `ClayShellWidget` is `#[doc(hidden)]` and Rust-public only because the Cargo package's `clay` binary target constructs the library-owned native root; this binary-boundary visibility is not a Clay JS API, package extension point, op, facade, or registry entry.

## Responsibilities

- Own the internal Rust layout state for a working area, pane split tree, per-pane slot layout, and slot-aware package UI runtime state.
- Apply inert, version-checked shell layout updates and package UI runtime updates that replace local pane/slot/panel/overlay state only after bounded validation.
- Expose internal structural shell observations for tests and agent inspection without exposing document text or native handles.
- Register the existing `EditorWidget` as a Masonry child of `ClayShellWidget`.
- Keep native widget IDs, Masonry handles, raw callbacks, raw CSS, raw ops, and client-side JavaScript out of package-facing state.
- Compute editor child geometry from installed Clay shell state during Masonry layout without adding or removing children in the layout pass.
- Preserve existing editor input, rendering, SDUI, status, focus fallback, and action routing responsibilities inside `EditorWidget` while routing shell-root events to the editor child.

## How It Works

`src/main.rs::run_editor` constructs an `EditorWidget`, wraps it in `ClayShellWidget::single_editor(...)`, records the child editor `WidgetId`, and starts the Masonry window with the shell as the root widget. The driver sets Masonry focus fallback to the editor child and routes existing `EditorAction` handling back to that child ID, so connection events, file-open UI command results, SDUI snapshots, edit acknowledgements, and resync snapshots continue to mutate `EditorWidget` rather than the shell container.

`src/shell/layout.rs` owns the reusable shell state:

1. `WorkingAreaLayout` records the shell layout version, working-area ID, editor component binding, and installed `PaneSplitTree`.
2. `PaneSplitTree` stores a validated tree of `PaneSplitNode::Leaf` and `PaneSplitNode::Split` values with a stable active `PaneId`.
3. `SplitOrientation::Horizontal` divides a rectangle into left/right regions. `SplitOrientation::Vertical` divides a rectangle into top/bottom regions. `SplitRatio` is the first region's fraction.
4. `PaneSlotLayout` stores the mandatory `main` slot plus optional fixed `left`, `right`, `top`, and `bottom` slots for each leaf pane. `FixedSlotState` records size, min/max clamps, visibility, collapse state, and whether the slot was user-resized.
5. Validation rejects zero pane IDs, duplicate pane IDs, active panes that are not leaves in the tree, invalid/non-finite split ratios, invalid fixed-slot sizes/bounds, and trees over the internal node limit.
6. Geometry helpers traverse the installed tree to return deterministic `PaneGeometry` values, then subtract visible fixed slots from a leaf pane to produce the editor component's `main` rectangle.

`src/masonry_shell.rs` owns the Masonry container behavior. `register_children` registers the already-created editor child. During layout, the shell computes its working-area size, asks `WorkingAreaLayout` for the editor component rectangle, runs the child layout with tight constraints for that rectangle, and places the child at the computed origin. The shell's `children_ids` remains the already-registered editor child list; split geometry does not mutate the Masonry child tree.

Inside the editor child, `src/masonry_editor.rs` reuses the same SDUI/package `PaneSlotLayout` main-region calculation for paint and pointer hit-testing. `EditorWidget::editor_main_rect` asks `editor_region_for_document` for the current `main` rect, paints `EditorSurface::paint_in_rect` clipped to that rect, and translates pointer positions into editor-local coordinates before caret placement or selection extension. `editor_region_for_document` reserves the Clay-owned left file-browser slot whenever a Clay-owned SDUI panel is present (`SduiNativeState::root_id` is set or an editor binding exists), not only when the SDUI editor binding matches the active document, so opening a workspace file under a new document ID never lets the editor main region overlap the left file browser. Accepted visible fixed panels therefore consume `left`/`right`/`top`/`bottom` geometry instead of covering text, while transient overlays still paint after fixed panels and may cover content by design.

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
  - Public API status: planned `clay.ui.serverRegisterWorkingAreaLayout` inventory stub only; no callable `clay:ui` facade/op is exposed in Phase 18.2.
- `PaneSplitTree`
  - Owner/source: `src/shell/layout.rs`.
  - Runtime status: internal Rust state with leaf/split nodes, active pane metadata, bounded ratios, duplicate-ID rejection, node-count bound, and deterministic geometry helpers.
  - Public API status: planned `clay.ui.serverRegisterPaneSplitTree` inventory stub only; packages cannot mutate Masonry children or provide split callbacks.
- `PaneSlotLayout`
  - Owner/source: `src/shell/layout.rs`.
  - Runtime status: internal Rust state with mandatory `main`, optional fixed `left`/`right`/`top`/`bottom` slots, finite size validation, min/max clamps, visibility, collapse state, user-resize metadata, and deterministic main/fixed-slot geometry.
  - Public API status: planned `clay.ui.serverSetPaneSlotLayout` inventory stub only; package-facing panel contributions and user layout overrides remain deferred.

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
- `src/masonry_shell.rs::tests::shell_observable_snapshot_captures_default_working_area`: validates the default structural shell observation without exposing widget handles.
- `src/masonry_shell.rs::tests::shell_observable_snapshot_captures_split_and_slots`: validates split/slot structural observation after an inert update.
- `src/masonry_shell.rs::tests::shell_observation_does_not_expose_document_text_or_native_handles`: validates the snapshot debug surface omits document/native/raw authority markers.
- `src/masonry_shell.rs::tests::shell_layout_update_rejects_stale_or_oversize_payload`: validates the shell widget update wrapper preserves layout validation.
- `src/masonry_shell.rs::tests::shell_places_editor_child_in_main_slot_rect`: validates the shell places the editor child in the slot-computed main rectangle without changing child identity.
- `src/masonry_shell.rs::tests::pane_split_tree_layout_does_not_mutate_children`: validates split geometry reads preserve registered editor child IDs.
- `src/masonry_shell.rs::tests::shell_editor_text_input_remains_client_first`: validates IME text input reaches the editor child through a `RenderRoot` with the shell as root, updates local text immediately, and enqueues a delta edit.
- `src/masonry_shell.rs::tests::shell_editor_keyboard_routing_uses_installed_behavior_manifest`: validates keyboard behavior-manifest routing, using Enter/newline, still reaches the editor child under the shell.
- `src/masonry_shell.rs::tests::shell_editor_read_only_observer_blocks_local_edit_queue`: validates read-only observer state blocks local mutation and edit queue emission through the shell root.
- `src/shell/package_ui.rs::tests::slot_panel_contribution_places_panel_in_requested_slot_and_preserves_main_editor`: validates package fixed panel slot composition through `PaneSlotLayout`.
- `src/shell/package_ui.rs::tests::slot_panel_contribution_rejects_duplicate_exclusive_slot_claims`: validates duplicate exclusive fixed-slot claims fail deterministically.
- `src/shell/package_ui.rs::tests::transient_overlay_renders_without_consuming_fixed_slot_geometry`: validates transient overlay geometry stays separate from fixed slot geometry.
- `src/masonry_shell.rs::tests::shell_routes_edit_ack_and_resync_to_editor`: validates edit acknowledgements and resync snapshots still update editor text/status state through the shell child route.
- `src/masonry_shell.rs::tests::shell_routes_sdui_snapshots_to_editor_component`: validates SDUI connection events still land in the editor component under the shell.
- `src/masonry_editor.rs::tests::fixed_package_panel_shrinks_editor_hit_region`: validates visible package fixed panels offset the editor hit region instead of letting panel coordinates place the caret under the panel.
- `src/main.rs::tests::connection_event_action_is_dispatched_to_shell_editor_child`: validates GUI connection-event user events target the shell-owned editor child.
- `src/main.rs::tests::driver_routes_editor_actions_to_shell_editor_child`: validates `AppDriver` editor action routing uses the shell editor child ID even if a shell/root source is reported.
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

`src/shell/layout_persist.rs` serializes user-modified state (split ratios ≠ 0.5, slots with `resized_by_user` or `collapsed`) to `~/.config/clay/layout.json` via `serde_json`. `save_layout`/`load_layout` handle I/O; `apply_persisted_state` restores with validation (skips invalid entries). Persistence is debounced ≥ 500ms in `ClayShellWidget::persist_debounced()`. Corrupt/missing files fall back to defaults.

### Focus and Input Routing

`PaneSplitTree::next_pane()`/`prev_pane()` traverse panes in reading order (in-order, wrapping). Tab/Shift+Tab in `on_text_event` moves focus; `set_focus_pane()` validates membership. A focus ring paints on the active pane when multiple panes exist (`paint_focus_ring`). `focused_pane_rect()` provides the transient surface anchor for overlays/menus.

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
- `src/masonry_shell.rs`: Pointer/keyboard event handlers, paint, persistence debounce.
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

`ShellClientCommand` (20 variants, `src/masonry_shell.rs`) parses the `clay.shell.client*` command IDs: the 12 pane commands (`clientSplitPaneVertical/Horizontal`, `clientAddEqualPane`, `clientClosePane`, `clientFocusPaneNext/Prev`, `clientResizePaneLeft/Right/Up/Down`, `clientMovePaneNext/Prev`) plus the 8 Phase 22.4 tab commands (`clientTabNext/Prev/New/Close/MoveLeft/MoveRight`, `clientTabActivate(u32)`, `clientTabMoveTo(u32)` — 1-based card positions). Command names follow the vim convention: a "vertical" split places panes side by side (`SplitOrientation::Horizontal`), a "horizontal" split stacks them.

Dispatch path: `main.rs` resolves `ClientUiCommandResult::ShellCommand`; tab commands are intercepted by the driver's `apply_tab_command` **before** the widget (the widget's tab arms stay inert) — the driver resolves card positions from its `tab_order` policy and routes through the shared execution paths the tab bar also uses (activate/close/new/move), so chords and clicks share one code path. Pane commands reach `render_root.edit_widget(shell_widget_id).apply_shell_client_command(...)` — tree ops go through `WorkingAreaLayout::replace_pane_tree` (new in 22.1; bumps the layout version) + `reconcile_pane_hosts`; resize goes through `keyboard_resize` + `commit_split_drag`. No server round-trip, no JS runtime, no IPC in the dispatch path (guarded by `shell_command_dispatch_requires_no_server_or_js_runtime`).

Defaults ship in `default_keymaps()` (`src/protocol/mod.rs`) as `Global`-context `ClientUiCommand` rules: `Ctrl+\` / `Ctrl+-` / `Ctrl+Shift+\` / `Ctrl+Alt+W`, `Ctrl+Alt+Left|Right` (focus), `Ctrl+Alt+Shift+arrows` (resize), `Ctrl+Alt+[|]` (move), and the 22.4 tab chords (`Ctrl+Tab`, `Ctrl+Shift+Tab`, `Ctrl+T`, `Ctrl+Shift+W`, `Ctrl+<N>` 1..=9, `Ctrl+Shift+[|]`, `Ctrl+Shift+<N>` 1..=9). All IDs are registered bindable in `is_runtime_bindable_command` with `ClientUiCommand` routing (22.4 numbered families via `tab_family_variant`, 1..=9 only), so `bindKey(key, id, { scope: "global" })` in init.js overrides them. `ClientBehaviorState::route_key` matches in two passes — `EditorTextFocus` rules first, then `Global` — so an editor-context binding for the same chord always wins over a shell default.

Known 22.1 ceiling: `Global` rules route through the editor's key path, so the chords don't fire while a placeholder pane (no editor focus) is active.

### Shell Preferences Transport

`setPaneFocusPolicy({ paneFocusPolicy: "click" | "cursor" })` (`clay:shell` facade, `runtime/js/shell.js`) calls `op_clay_shell_set_pane_focus_policy` (admin-only op), which validates the closed enum and publishes through `ClayOpState::publish_shell_preferences`. The value travels the same broadcast-lane pattern as `CaretStyleOverride`: service-level channel + store (`ClayJsRuntimeService`), connection handler forwards it as `ServerMessage::ShellPreferences` (protocol v10; also sent during the initial handshake), and the client applies it as `ClientConnectionEvent::ShellPreferences` → `ClayShellWidget::set_pane_focus_policy` live. Unknown values fail evaluation with a `clay.shell.invalid_pane_focus_policy` diagnostic.

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
- Topology persistence (split trees across restarts) is deferred to Phase 22.5; `layout.json` still records only ratio/slot state.

### Source Paths

- `src/shell/layout.rs`: lifecycle ops, `MAX_PANES_PER_TAB`, `PaneResizeDirection`, `replace_pane_tree`.
- `src/masonry_pane_host.rs`: `PaneContentHost` widget (placeholder/document content, `set_document_view`/`clear_content`).
- `src/masonry_shell.rs`: `pane_hosts` map, `pane_targets` routing map, reconcile, focus policy, `PaneFocused` actions, `ShellClientCommand` dispatch.
- `src/main.rs`: shell action target + `ShellCommand`/`ShellPreferences` dispatch.
- `src/server/ops/shell.rs`: `op_clay_shell_set_pane_focus_policy`.
- `src/client/behavior.rs`: two-pass `EditorTextFocus`-before-`Global` route_key matching.
- `src/protocol/mod.rs`: `ShellPreferences`, shell default keymaps/commands, protocol v10.
- `runtime/js/shell.js` / `shell.d.ts`: command-ID helpers + `setPaneFocusPolicy`.
- `docs/reference/clay-js-api/shell/`: per-command and configuration API docs (authoritative public surface).

### Tests

- `src/shell/layout.rs`: cap rejection, equal-area redivision (2/3/4 panes, area equality), close/merge/focus-handoff, move swaps + end no-ops + ratio preservation, keyboard resize bordering/deepest/clamp/no-divider. Command: `cargo test --lib shell::layout --quiet`.
- `src/masonry_shell.rs`: host identity stability across tree mutations, orphan detachment, placeholder hosting, per-pane placement, click-to-focus on placeholders, focus-policy behavior (default, follows-cursor, drag-skip), all 12 command dispatches, 4-pane cap enforcement, the no-server/no-JS hot-path guard, and (22.2) independent per-pane document views with document-scoped routing, typing-isolation hot-path guard, routing-target cleanup on pane close, and concurrent per-pane major modes isolated across behavior manifests. Command: `cargo test --lib masonry_shell --quiet`.
- `src/server/js_runtime.rs`: policy publish/reject/default-unset through real init.js evaluation.
- `src/server/ops/keybindings.rs`: all 12 shell IDs bindable + `ClientUiCommand`-routed; unknown `clay.shell.*` rejected.
- `src/protocol/mod.rs`: defaults present with `Global` context and `ClientUi` authority.

## Phase 22.3: Tabs as Independent Client Views (multi-connection model)

Phase 22.3 (2026-08-06) makes each tab an independent client view with its own server connection. The server side (protocol v11, `src/server/tab_registry.rs`) is covered in the tab-registry task; this section covers the client-side multi-connection model:

- `ClayShellWidget` now owns `tabs: BTreeMap<ClientId, TabChrome>` + `active_tab: ClientId`. `TabChrome` (new, `src/masonry_shell.rs`) bundles one tab's `WorkingAreaLayout`, `pane_hosts`, `pane_targets`, `pane_focus_policy`, and `pending_orphans` — everything that was previously shell-level single-tab state. The shell hosts **every** tab's hosts as registered children; only the active tab's hosts are laid out at their pane rects, inactive tabs' hosts are laid out at zero size (the Phase 22.1 `pending_orphans` protocol) so their widgets stay in the tree and keep receiving connection events without painting or hit-testing.
- Tabs are keyed by the connection's `ClientId` (the client-known identity at mount time; the server-assigned `TabId` arrives asynchronously via the registry snapshot and is tracked by the app driver). `install_tab` mounts a tab's chrome (first tab becomes active; later tabs are retained until `set_active_tab`), `set_active_tab` switches with one layout pass and resets in-flight drag sessions, `tab_for_chrome` resolves an event's tab from its chrome id, and per-tab queries (`pane_targets_for`, `pane_host_id_for`, `editor_widget_id_for`, `set_pane_focus_policy_for`, …) scope routing to one tab. The no-arg routing methods delegate to the active tab, so single-tab behavior is the pre-22.3 experience.
- `src/main.rs` `Driver` owns `tabs: BTreeMap<ClientId, TabState>` (`TabState` = the connection's `ClientEditQueue` clone, per-tab `pending_opens`, and the server-assigned `tab_id`), `active_tab`, and the latest `registry` snapshot. The session itself is consumed at mount (initial state → chrome, events → per-tab bridge, edit queue → chrome + `TabState`). `mount_tab` (open-tab mechanism: builds the chrome + `TabChrome`, sends `TabCommand::New`, installs + activates, spawns the per-tab event bridge) and `switch_tab` are wired by the lifecycle task's affordance; `apply_tab_registry` fills each tab's `tab_id` by matching `client_id`.
- Event routing: each tab's bridge tags its events with that tab's chrome `WidgetId`; the driver resolves the tab via `tab_for_chrome` and routes document-scoped events, fan-outs, runtime snapshots, and editor commands to **that tab's** targets only. `ShellPreferences` applies to the sending tab's focus policy; `TabRegistry` snapshots are driver-level state. Menus display in the tab's own chrome overlay. Focus follows only for the active tab (an inactive tab's open never steals focus).
- `ClientEditQueue::enqueue_tab_command` sends `ClientMessage::TabCommand` on a connection.

- The tab bar (Phase 22.3): a shell-owned window-level row painted only when more than one card is present. It is carved in `ClayShellWidget::layout()` (the plan's "equivalent shell-level row" option — `PaneSlotLayout::compute_geometry` is per-pane, so a window-level row cannot live there without changing package slot semantics). The bar is token-driven end to end: `tab_card_chrome` (src/shell/primitives.rs) resolves `surface.list`/`surface.selected` rests, `surface.hover`, `surface.active`, `text.primary`/`text.muted`/`text.disabled`, the `accent.primary` focus ring (painted for the `Focus` state; keyboard focus traversal between cards is a 22.6 ceiling — the 22.4 chords activate/close/move tabs without moving card focus), and the dimmed disabled close. The label uses the UI `Status` typography variant (`UiTextVariant::Status` via the shell's default `TypographyRegistry`; active-typography sync is not wired in 22.3) painted through the shared `paint_sdui_text` with a clip layer. The close glyph is two inline strokes. Cards are registry-driven: the driver pushes `TabCard`s from the server snapshot (order + names = root display path's final segment); mounted tabs awaiting their registry entry are appended with close disabled. Card order is the numbering authority for the 22.4 numbered commands — `clientTabActivate.<N>`/`clientTabMoveTo.<N>` resolve 1-based against this exact card list (registry order, entry-less mounted tabs appended; beyond-count = silent no-op). Clicking a card submits `EditorAction::TabBar(Activate)`; the driver switches optimistically and the server registry reconciles — the server now pushes a `TabRegistry` snapshot even for rejected `Activate`/`Close`/`OpenWorkspace`/move commands, so a rejected optimistic switch reverts. Clicking the close glyph submits `Close`; the driver refuses closing the last mounted tab; a dirty tab gets the 22.4 driver-owned tab-confirm menu (`tab_close_confirm_session`) instead of closing; a clean (or confirmed) close sends `TabCommand::Close` on that tab's own connection, and the server's pushed snapshot (entry removed) drives removal (`apply_registry_reconcile` uninstalls the chrome, switches + focuses the remaining tab, refreshes cards). Removals are skipped against an empty registry (server restart; the lifecycle task re-registers via `Reclaim`/`New`).

### Lifecycle (22.3 task 7: open / close / switch / reconnect)

- **Open tab**: the tab bar's `+` affordance (`TabBarAction::NewTab`, right edge of the bar, present whenever the bar is) opens the native folder picker; the picked folder becomes the new tab's workspace root. The driver connects a fresh session on the runtime and `mount_tab` sends `TabCommand::New` (server-validated via `add_root`), mounts a new chrome + default single-pane split tree, spawns its event bridge, and switches to it. Connection failures (`OpenTabFailed` — e.g. the `MAX_ACTIVE_CONNECTIONS` cap) surface a `clay.tabs.open_failed` diagnostic on the active tab's chrome and never mount a tab. `Ctrl+T` (22.4) routes to the same `open_new_tab_dialog` path as `+` — same flow, same in-flight guard (a second request while the picker is open is ignored).
- **Close tab**: the driver's `close_tab` (shared by card `✕` and `Ctrl+Shift+W`) first passes `tab_close_allowed` (never the last tab — the window never goes to zero tabs) and then the 22.4 dirty inventory `dirty_documents_in_tab` (replaces the 22.3 `guard_tab_close` first-dirty-view walk): a clean tab enqueues `TabCommand::Close` directly; a dirty tab gets the driver-owned confirm session (`clay::shell::tab_close_confirm_session` — Save all and close / Discard and close / Cancel) hosted on the active pane view + chrome overlay, with `pending_close_after_saves`/`advance_pending_close_after_saves` counting `DocumentSaved` acks before the close enqueues (cancelled on `FileOperationFailed`/disconnect). A clean close is server-authoritative: the server removes the registry entry and ends the connection, so the permit + leases release through the existing disconnect cleanup; the removal snapshot (observed on the other tabs' connections — the closing connection never reads its own broadcast update) drives `apply_registry_reconcile` uninstall.
- **Switch tab**: card click → optimistic `switch_tab` + `TabCommand::Activate`; the server registry reconciles rejections.
- **Reconnect**: on `Disconnected`/`ConnectionError` for a tab, `start_tab_reconnect` spawns a per-tab task that retries `client::connect` with the existing `connect_with_retry_while` backoff (50 × 20 ms, then 200 ms between cycles) until it succeeds or the tab is removed (per-tab `Arc<AtomicBool>` cancellation flag set by `apply_registry_reconcile`). On success the driver: swaps the fresh session's queue into the chrome and every pane view (`reconnect` — clears the disconnect recovery menu and re-arms the reinstall so the next `DocumentOpened` for the active document is not swallowed by the 22.2 duplicate-open no-op); re-keys the tab (`rekey_tab`: the `TabChrome` moves wholesale, widget ids stay stable, `active_tab` and card client ids follow); rebinds the registry via `TabCommand::Reclaim { tab_id }` (or `New` when the tab never got its id); re-opens every document the tab holds (`documents_for_reopen` — active + retained identities retained per-session since 22.3 — through the plain `OpenDocument` path, since a fresh connection holds no selected-file capability); spawns a new event bridge to the same chrome; restores focus when the tab was active. In-flight `pending_opens` are cleared (they died with the old connection). Split trees and per-pane document state restore from the retained in-memory `TabChrome`/sessions; a full client process restart reclaims the server's in-memory registry per tab on reconnect. Multi-client reclamation and server-restart disk restore are 21/22.5 ceilings.

### Key Invariants

- One connection per tab: separate `ClientEditQueue`/sync state, chrome (SDUI region, panels, overlays, runtime generation), split tree, pane targets, focus policy, and pending-open attribution. Editing in one tab never mutates another tab's state.
- Inactive tabs are retained in-tree at zero size: stable `WidgetId`s across switches, no paint/layout/hit-test work, but connection events still apply so a switched-in tab is current.
- The client-side tab map is view/routing state and grants nothing; the server registry is authoritative for tab order/active/ids.
- `layout.json` persistence saves only while a single tab is open (per-tab persistence is 22.5).
- Multi-client tab reclamation and full server-restart disk restore are out of scope (Phase 21 / 22.5).

### Source Paths

- `src/masonry_shell.rs`: `TabChrome`, `tabs`/`active_tab`, `install_tab`/`set_active_tab`/`tab_for_chrome`, per-tab routing queries, zero-size inactive layout; tab bar: `TabCard`, `set_tab_cards`, `remove_tab`, `tab_bar_geometry`/`tab_bar_hit_test`, bar paint + pointer handling, `TAB_BAR_*` constants.
- `src/shell/primitives.rs`: `tab_card_chrome` state resolver (`TabCardChrome`).
- `src/masonry_pane_document.rs`: `reconnect` (queue swap + reinstall re-arm + menu clear), `documents_for_reopen`, per-session `workspace_root_id`/`path` retention.
- `src/editor/document_session.rs`: `RetainedDocumentSession` open identity; `reopen_documents`.
- `src/masonry_editor.rs`: `EditorAction::TabBar` + `TabBarAction`.
- `src/main.rs`: `TabRegistryReconcile` (pure `apply_tab_registry` + shell-side `apply_registry_reconcile`), `TabBar` action handler (optimistic activate / dirty-guarded close / `NewTab` affordance), bootstrap `TabCommand::New`, reconnect (`start_tab_reconnect`, `ReconnectTabConnected` handler: queue swap, rekey, `Reclaim`, `OpenDocument` re-opens, new bridge), `guard_tab_close`, `tab_close_allowed`, `tab_card_display_name`.
- `src/main.rs`: `TabState`, `mount_tab`/`switch_tab`/`apply_tab_registry`, per-tab event routing, per-tab bridges.
- `src/client/mod.rs`: `ClientEditQueue::enqueue_tab_command`.
- `src/protocol/mod.rs`: `TabId`, `TabEntry`, `TabRegistrySnapshot`, `TabCommand`, `ClientMessage::TabCommand`, `ServerMessage::TabRegistry`, protocol v11.
- `src/server/tab_registry.rs`: server-authoritative in-memory registry.

### Tests

- `src/masonry_shell.rs`: install-then-activate, stable chrome/host ids across switches, inactive hosts laid out at zero size, single-tab shape unchanged, per-tab routing-target and focus-policy isolation; tab bar: hidden below two cards (single-tab geometry unchanged), geometry/carve with two cards, activate/close/no-op click actions, hover tracking, remove-tab uninstall + active fallback. Command: `cargo test --lib masonry_shell --quiet`.
- `src/shell/primitives.rs`: `tab_card_chrome` resolves every state (Rest/Hover/Active/Focus/Disabled × selected). Command: `cargo test --lib shell::primitives --quiet`.
- `src/client/mod.rs`: real-server tab command end-to-end — bootstrap `New` registers the tab (name/order in the snapshot), a rejected `Activate` pushes a reconciling snapshot, `Close` ends the connection + removes the registry entry (observed on a fresh connection's replay), a dropped connection's entry is reclaimed by a new connection (`Reclaim` keeps `TabId`, rebinds `ClientId`), and the connection cap refuses excess connections. Command: `cargo test --lib client::tests::real_server_tab --quiet`, `cargo test --lib real_server_ --quiet`.
- `src/main.rs` (bin): registry reconciliation — fills tab ids + builds cards, removes closed tabs + activates the remaining one, skips removals on empty (restart) snapshots. Command: `cargo test --bin clay --quiet`.
- `src/main.rs`: registry application fills server-assigned tab ids; per-tab edit queues are isolated channels. Command: `cargo test --lib main::tests --quiet`.
- Server-side registry tests: `cargo test --lib server::tab_registry --quiet`.

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
