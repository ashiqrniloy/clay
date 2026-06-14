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
- `EditorWidget` continues to own editor hot-path input/rendering, SDUI content rendering, status chrome, edit queue behavior, viewport, caret, and selection.

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
- `src/main.rs::tests::connection_event_action_is_dispatched_to_shell_editor_child`: validates GUI connection-event user events target the shell-owned editor child.
- `src/main.rs::tests::driver_routes_editor_actions_to_shell_editor_child`: validates `AppDriver` editor action routing uses the shell editor child ID even if a shell/root source is reported.
- Command: `CARGO_TARGET_DIR=target/pi-verify cargo test --lib shell --quiet`
- Command: `CARGO_TARGET_DIR=target/pi-verify cargo test --lib masonry_shell --quiet`
- Command: `CARGO_TARGET_DIR=target/pi-verify cargo test --lib main --quiet`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.2 Shell Runtime Primitive Review](phase18.2-shell-runtime-primitive-review.md)
- [Phase 18.1 Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- `plans/025-Phase18.2-Masonry-Clay-Shell-and-Pane-Runtime-Foundation.md`
