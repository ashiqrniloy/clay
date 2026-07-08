# Server-Driven UI Protocol Schema

## Source

- `src/protocol/sdui.rs`
- `src/protocol/mod.rs`
- `src/protocol/sdui.rs` unit tests
- `src/server/sdui.rs`
- `src/server/connection.rs`
- `src/client/mod.rs`
- `src/masonry_sdui.rs`
- `src/masonry_editor.rs`
- `src/shell/components.rs`
- `src/shell/theme.rs`
- `src/shell/package_ui.rs`

## Overview

The server-driven UI (SDUI) schema defines an inert, typed Rust protocol model for UI trees and explicit server-published SDUI protocol messages. The end-user bootstrap path is editor-only: after the normal `Welcome`, `InitialDocument`, and `BehaviorManifest` messages, the server sends no default `Workspace` `SduiSnapshot` unless configuration/runtime code has explicitly published a tree. The legacy static workspace/sidebar tree still exists as a validation fixture and compatibility helper, but `IpcServer::try_new` initializes `StaticSduiState::empty_for_document(1)` so bare `cargo run` does not show a side panel. Explicit runtime SDUI publication, such as `cargo run -- smoke-gui --config-fixture runtime-sdui`, replaces the empty state with a validated tree and sends `ServerMessage::SduiSnapshot` through the normal bootstrap path. The client maps decoded snapshots and updates into `SduiNativeState`, paints native SDUI regions in Masonry, and keeps the existing editor surface/state separate. The server owns declarative UI state and publishes trees/updates; the client remains responsible for native rendering, input handling, focus, caret, viewport, and other transient widget state. Planned public Clay JS helper docs now reserve `clay:sdui` names for JavaScript SDUI schema construction while keeping runtime UI publication explicit. After Phase 18.1, SDUI is explicitly treated as the component-tree substrate under Clay-owned shell/layout primitives (`WorkingAreaLayout`, `PaneSplitTree`, slot contracts) documented in `docs/reference/primitives/shell-layout-strategy.md`, not as a package-controlled native pane/window API.

## Responsibilities

- Represent stable SDUI node IDs, tree versions, snapshots, and bounded tree updates.
- Model panels, labels, buttons, lists, document-bound editor views, flex containers, and stack containers.
- Keep the ordinary end-user startup SDUI state empty, while preserving the static workspace/sidebar tree as an explicit fixture/compatibility helper.
- Express user actions as server-routed command intents with typed metadata and validate inbound action sources/commands.
- Bound runtime-published trees by raw payload size, node count, nesting depth, and per-node text length before they can allocate proportional memory.
- Reconcile decoded SDUI snapshots/updates into native client state without touching editor text state for sibling panel changes.
- Paint panels, labels, buttons, lists, and editor-view markers as Masonry-native drawing and route button/list activation back as `ClientMessage::SduiAction`.
- Avoid carrying client-executable scripts or document text payloads in UI nodes.
- Document public planned SDUI schema helper names through the Clay JS API registry instead of exposing raw Rust protocol types or raw future ops as user-facing APIs.

## How It Works

`SduiTree` stores a `ui_version`, a `root_id`, and a flat list of `SduiNode` values. Each node has a stable `SduiNodeId` and a `SduiNodeKind`. Container nodes refer to children by ID instead of nesting widget state directly, so later reconciliation can replace or remove nodes by stable ID.

`SduiNodeKind::EditorView` uses `SduiEditorBinding` with a `DocumentId` and optional expected document version. The editor view schema never embeds full document text; existing document snapshot/edit protocol messages remain responsible for text synchronization.

`SduiActionIntent` represents button and list-item activations as inert command IDs, sources, and typed arguments. These intents route back to the server as bounded `ClientMessage::SduiAction` values and then through the Phase 18.8 `CommandExecutor`; they never execute filesystem, network, shell, extension-loading, WASM, AI mutation, package JavaScript, or client-side JavaScript authority on the client.

`SduiTreeUpdate` and `SduiTreeOperation` define the first update shape: replace the root, replace a node, or remove a node against explicit base/new UI versions. `ServerMessage::SduiSnapshot` carries the full static tree for bootstrap; `ServerMessage::SduiUpdate` carries bounded declarative updates. `ClientMessage::SduiAction` carries an inert user action intent back to the server. `ClientMessage::CommandIntent` carries the same command-execution path for behavior-manifest server-first keybindings and future transient-menu selections.

`src/server/sdui.rs` owns the static workspace/sidebar tree helper and the runtime replacement boundary. `StaticSduiState::empty_for_document` records the current document without a tree so `snapshot_message` returns `None` and bootstrap sends no default side panel. `StaticSduiState::for_document` remains available for tests and explicit compatibility paths; it builds and validates the workspace tree with a row flex root, workspace side panel, status label, refresh button, document list, and editor view bound by document ID/version. Runtime replacement through `StaticSduiState::replace_with_runtime_tree` accepts only trees that pass the same root/child/editor validation as static trees, then makes `snapshot_message` return a `ServerMessage::SduiSnapshot` again. Runtime trees and later `ReplaceNode` updates reject editor views bound to any document other than the open server-known document, so unknown document views fail safely instead of acquiring document authority. `StaticSduiState::update_message` validates update versions and target node IDs before mutating server UI state; calling it while no tree exists returns the existing empty-tree validation error. `validate_action` accepts only inert command IDs declared by the current SDUI tree and checks that button/list sources match the declared action; with no tree, invalid action intents become typed unknown-command errors instead of panics or local execution.

Runtime SDUI publication in `src/server/ops/sdui.rs` validates actions against built-in SDUI commands (`workspace.refresh`, `document.focus_active`, `document.open_recent`) plus package commands already registered in the runtime command registry. This lets first-party packages such as `@clay/markdown` publish a preview/status panel whose `Toggle Preview` button targets `markdown.togglePreview`, while still rejecting unregistered commands such as `shell.run`. If a package is disabled or invalid, its command is no longer registered and a replacement/fallback tree cannot retain stale package command authority. Open-document classification uses `apply_runtime_outputs_without_sdui`, so package activation, behavior-manifest publication, decoration publication, and parse-time diagnostics cannot erase the Clay-owned workspace/file-browser `StaticSduiState`; explicit config/runtime SDUI publication still uses `apply_runtime_outputs` and remains the only path that replaces shared SDUI validation state.

Runtime SDUI publication budgets are enforced at the `op_clay_sdui_publish_tree` boundary in `runtime_tree_from_json`, mirroring the budget discipline already used for registered package UI contributions. The raw `tree_json` is rejected by byte length (`RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES`, 16 KiB) before `serde_json` allocates a proportional `Value`, and the builder enforces a node-count cap (`RUNTIME_SDUI_TREE_MAX_NODES`, 128), a nesting-depth cap (`RUNTIME_SDUI_TREE_MAX_DEPTH`, 16), and a per-node free-text cap (`RUNTIME_SDUI_TREE_MAX_NODE_TEXT_CHARS`, 4096) on panel titles, labels, button labels, and list item labels/details. A malicious or runaway huge/deep tree is therefore rejected before it can exhaust memory or stack while building the typed `SduiTree`. The budgets live in `src/perf/budgets.rs` alongside the other SDUI/perf constants.

The client connection task converts decoded `ServerMessage::SduiSnapshot` and `ServerMessage::SduiUpdate` frames into `ClientConnectionEvent` values. The existing `EventLoopProxy` bridge delivers those typed events to `EditorWidget::apply_connection_event` on the GUI event loop. `SduiNativeState` stores a flat `BTreeMap<SduiNodeId, SduiNode>`, the current UI version/root, derived editor binding, paint-time action hit regions, and crate-internal `PackageUiRuntimeState` for accepted fixed package panels and transient overlays. Snapshot application replaces the native SDUI tree state; updates apply stable-ID root/node replacements or removals only when the base UI version matches. Package UI runtime updates are separately version-gated and bounded before they install fixed `left`/`right`/`top`/`bottom` panels or transient overlay declarations. Editor text, caret, selection, viewport, document version, and behavior manifest state remain in `EditorSurface`. After the Phase 18.2 slot task, `SduiNativeState` computes its temporary left side panel through the internal `PaneSlotLayout` bridge in `src/shell/layout.rs`; Phase 18.3 now merges that compatibility bridge with Phase 18.3 package-facing panel contributions and package fixed panel slot layout, so package panels can consume explicit Clay slots while the existing editor remains in the mandatory `main` slot. Phase 18.3 adds the Clay-owned component catalog in `src/shell/components.rs`, typed theme-token resolver in `src/shell/theme.rs`, and slot-aware package UI runtime in `src/shell/package_ui.rs`. The catalog accepts bounded inert component kinds (`editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`/`overlay`, `scroll`/`portal`, and `statusItem`), keeps `table`, `dropdown`, `collapse`, and `modal` explicitly deferred, validates typed style variables against core or package-declared tokens, and rejects raw CSS/colors, unknown tokens, native handles, callbacks, and client-executable code. `SduiNativeState` now reads resolved Clay theme values for compatibility/package panel colors, row sizing, padding, and text sizes instead of owning package-facing hardcoded style constants. `EditorWidget` uses the SDUI editor binding to treat the composed editor view's `main` slot as the editable region while side-panel/package-panel pointer presses route only declared SDUI/package command intents or stay inert. The Clay-owned left file-browser panel supports client-local vertical scroll: `SduiNativeState` keeps a `scroll_offset` (pixels) clamped to the panel content/viewport height measured during paint, paints panel content inside a `push_clip_layer` so scrolled-out rows do not paint over the editor, and rebuilds action hit regions against the scrolled cursor so a click hits the row currently under the pointer. `EditorWidget::on_pointer_event` routes `PointerEvent::Scroll` to `SduiNativeState::scrolls_point`/`scroll_vertical_pixels`/`scroll_lines` only when the pointer is inside the left panel; otherwise the editor scroll path is unchanged. Scrolling is pure client-local paint/action math and never relists directories, calls the server, runs package JavaScript, or enqueues workspace actions; the offset resets to zero on every snapshot/update.

`SduiNativeState::observable_snapshot` extracts a headless, comparable `SduiObservableSnapshot` without painting, serializing, or invoking a GPU layout pass. The snapshot records the UI version, a `SduiNodeId`-sorted node-kind inventory, visible panel titles, labels, button labels, list item IDs/labels, all reachable editor bindings, package fixed panel IDs/slots/component roots/rectangles, transient overlay IDs/anchors/focus/dismissal policies/rectangles, and simple layout booleans for sidebar presence and non-empty editor region. Structural layout regression tests use this typed snapshot to lock down the editor/sidebar composition, targeted label updates, stale-update rejection, snapshot replacement, root-removal behavior, package fixed slot geometry, transient overlay geometry, package action routing, and observation privacy under `cargo test --all-targets` without opening a window. It intentionally omits document text, filesystem paths, secrets, native handles, raw ops, raw CSS, callbacks, executable package code, and action payload authority; it is `pub(crate)` test/agent infrastructure rather than a Clay JS API surface.

`SduiNativeState` also implements Masonry's `Widget` accessibility hooks for the SDUI tree. Its widget root reports `Role::GenericContainer` and labels itself "Server-driven UI". During Masonry accessibility passes, it mirrors the reachable SDUI node tree into AccessKit nodes: panels and layout containers use `Role::Pane`, labels use `Role::Label`, buttons use `Role::Button`, lists use `Role::List` with `Role::ListItem` children labeled from item labels, and editor views use `Role::MultilineTextInput` with a label containing the bound document ID. The helper `accessibility_nodes()` exposes the same stable role/label traversal for headless unit tests without constructing a Masonry `AccessCtx`. The traversal is demand-driven by `accessibility()` and guarded against repeated node visits; it does not run during paint or input handling and does not expose document text, filesystem paths, or secrets.

`EditorWidget` paints the editor first, then overlays the current package fixed panels, server-driven compatibility panel/list/button region, transient package overlays, and the existing status bar. Fixed package panels use `PackageUiRuntimeState::slot_layout()` and `PaneSlotLayout` geometry; transient overlays use the working area/main/pointer anchor policy without adding fixed slots. Primary pointer presses that hit an SDUI or package panel action region enqueue a bounded inert `ClientMessage::SduiAction` command intent through the existing client queue sender. Server-first keybindings enqueue bounded `ClientMessage::CommandIntent` values with document and behavior-version metadata. Both use non-blocking `try_send`; the pointer/key handlers do not run client-side script, package JavaScript, command side effects, or wait for IPC capacity/server acknowledgement. Ordinary text editing continues through existing editor commands and edit deltas.

The schema stays separate from the `rkyv` codec boundary even though payload types derive `Archive`, `Serialize`, and `Deserialize` for protocol use.

Public programmatic documentation for SDUI lives under `docs/reference/clay-js-api/sdui/`. Those pages define the `clay:sdui` facade exports (`definePanel`, `defineLabel`, `defineButton`, `defineList`, `defineEditorView`, `defineFlex`, and `defineStack`) and are linked from `docs/index.md` for generated registry lookup. In Phase 13, `runtime/js/sdui.ts` and the embedded runtime ESM facade call `op_clay_sdui_define_node` for inert helper objects and `op_clay_sdui_publish_tree` for explicit publication. Publication converts the JSON object graph into typed Rust `SduiTree` state at the server boundary; the client still receives only typed `SduiSnapshot`/`SduiUpdate` protocol messages and never executable JavaScript.

## Payload Costs and Codec Scope

Representative SDUI payload costs are measured in `src/protocol/codec.rs` with deterministic unit tests that construct a static multi-region tree without starting the GUI. Current `rkyv` payload sizes, excluding the 4-byte length prefix, are:

- Initial representative `ServerMessage::SduiSnapshot`: 816 bytes, below the 4 KiB initial SDUI budget.
- Representative one-label side-panel `ServerMessage::SduiUpdate`: 192 bytes, below the 1 KiB panel-update budget and smaller than the snapshot.

The scoped codec decision is to keep SDUI on the existing length-prefixed `rkyv` protocol path for wire messages only. Phase 12 does not add ad hoc JSON, a second wire format, specialized SDUI compression, or `rkyv` access from Masonry/native UI state. More specialized diff compression should be reconsidered only if representative snapshots exceed 4 KiB, simple panel updates exceed 1 KiB, or update payloads stop being materially smaller than equivalent snapshots.

## Code Examples

```rust
use clay::protocol::{
    SduiEditorBinding, SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
};

let root_id = SduiNodeId(1);
let editor_id = SduiNodeId(2);
let tree = SduiTree {
    ui_version: 1,
    root_id,
    nodes: vec![
        SduiNode::new(
            root_id,
            SduiNodeKind::Flex {
                direction: SduiFlexDirection::Row,
                children: vec![editor_id],
            },
        ),
        SduiNode::new(
            editor_id,
            SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id: 42,
                    expected_version: None,
                },
            },
        ),
    ],
};
```

## Invariants and Constraints

- `SduiNodeId` values are stable reconciliation keys, not Masonry widget IDs.
- SDUI schema state is server-owned declarative state; client-owned native widget state remains outside the schema.
- Editor views bind to documents by ID/version and do not serialize full document contents.
- Actions are command intents only and do not contain executable code or permission-bearing authorities.
- Codec serialization stays behind `src/protocol/codec.rs`; SDUI modules define semantics and helper construction only.
- The default editor-only bootstrap sends no SDUI snapshot; explicit runtime-generated snapshots are bootstrap/resync-style state, not ordinary edit traffic.
- SDUI snapshots/updates enter the widget only after protocol decoding and client event routing; raw IPC bytes never enter Masonry state.
- SDUI update reconciliation is version-gated and isolated from editor text mutation logic.
- The composed editor view must bind to the current open document before it can constrain the native editor region; unknown document bindings are rejected server-side or treated as safe non-authoritative UI on the client.
- Ordinary text edits continue to use edit deltas/acknowledgements and do not serialize full documents as part of SDUI.
- Inbound SDUI actions and keybinding/menu command intents are command intents only; the server normalizes them into `CommandExecutionRequest` and sends them through `CommandExecutor`. Runtime-published package trees still validate action targets before publication; stale or unknown runtime action IDs are rejected by command execution instead of a UI-specific dispatcher. They do not grant filesystem, network, shell, extension-loading, WASM, AI mutation, package JavaScript, or client-side JavaScript authority.
- Open-time runtime outputs update behavior/decorations only and do not replace `StaticSduiState`; workspace/file-browser action validation remains Clay-owned across document opens, mode activation, and parse timeout diagnostics.
- Runtime-published SDUI trees are bounded by payload bytes, node count, nesting depth, and per-node text length; oversized, too-deep, or too-large trees are rejected at the publication boundary before allocation.
- `clay:sdui` helpers build inert JavaScript object graphs; only `publishTree` crosses into typed Rust validation/publication, and it does not make protocol DTOs, native Masonry reconciliation, or server validation helpers directly callable from JavaScript.
- SDUI nodes remain package-contributed component data and do not let packages create panes/splits/slots, mutate Clay's working-area topology, or request direct Masonry/native widget construction.

## Tests

- `src/protocol/sdui.rs`: `sdui_schema_represents_initial_widget_kinds` validates all initial widget/layout kinds.
- `src/protocol/sdui.rs`: `sdui_editor_view_uses_document_binding_not_text_payload` validates editor binding without embedded text.
- `src/protocol/sdui.rs`: `sdui_actions_are_server_routed_intents` validates inert command intent shape.
- `src/server/connection.rs`: `sdui_actions_and_keybinding_intents_share_command_execution_path`, `package_ui_unregistered_action_is_rejected_by_command_execution`, and `file_browser_action_survives_markdown_open_followup_diagnostic` validate that SDUI/package UI actions and keybinding/menu command intents share the Phase 18.8 executor path and that open-time follow-ups preserve Clay-owned file-browser validation.
- `src/protocol/sdui.rs`: `sdui_updates_target_stable_node_ids` validates stable-ID update operations.
- `src/server/sdui.rs`: `empty_sdui_state_publishes_no_snapshot` validates the end-user default no-snapshot state, and `default_sdui_tree_is_valid_and_static` validates explicit static snapshot construction.
- `src/server/js_runtime.rs`: `configuration_can_publish_sdui_snapshot`, `js_generated_sdui_rejects_unknown_document_binding`, and `js_generated_sdui_rejects_executable_action_payloads` validate the runtime SDUI publication boundary.
- `src/server/sdui.rs`: `default_sdui_contains_editor_and_panel_regions` validates the default multi-region tree shape.
- `src/server/sdui.rs`: `editor_view_requires_known_document_binding` validates that SDUI updates cannot bind an editor view to an unknown document.
- `src/server/sdui.rs`: `sdui_update_rejects_unknown_node_id` validates bounded update target checks.
- `src/server/sdui.rs`: `sdui_action_validation_rejects_unknown_command` validates inbound action intent command filtering.
- `src/server/mod.rs`: `open_time_runtime_sdui_output_does_not_replace_workspace_browser_state` validates the behavior/decorations-only open-time runtime application path.
- `src/server/js_runtime.rs`: `markdown_config_fixture_opens_workspace_and_publishes_status_sdui` validates package-owned SDUI publication can target registered package commands and rejects stale/unregistered authority at publication time.
- `src/server/ops/sdui.rs`: `runtime_tree_too_large_rejected`, `runtime_tree_too_deep_rejected`, `runtime_tree_too_many_nodes_rejected`, `runtime_tree_text_too_long_rejected`, and `runtime_tree_within_budgets_loads` validate the runtime SDUI publication budget discipline.
- `src/server/connection.rs`: `server_does_not_send_default_workspace_sdui_snapshot_after_bootstrap` validates the ordinary no-default-panel bootstrap, and `client_receives_js_generated_sdui_snapshot` validates explicit runtime-generated snapshot emission.
- `src/client/mod.rs`: `client_receives_sdui_snapshot_event` validates decoded SDUI event delivery and `sdui_button_action_emits_server_intent` validates bounded typed action emission.
- `src/masonry_sdui.rs`: `sdui_snapshot_replaces_native_tree_state`, `sdui_update_preserves_editor_document_state`, `editor_region_is_bounded_when_document_bound_editor_view_is_present`, `slot_panel_contribution_places_panel_in_requested_slot_and_preserves_main_editor`, `transient_overlay_renders_without_consuming_fixed_slot_geometry`, `slot_ui_actions_emit_registered_command_intents_only`, `slot_ui_observation_omits_document_text_native_handles_and_raw_authority`, `sdui_actions_still_emit_server_intents_from_slot_geometry`, `sdui_renderer_uses_resolved_theme_tokens_for_panel_styles`, `workspace_browser_reserves_left_slot_after_document_id_changes`, and `stale_sdui_update_is_ignored` validate native reconciliation, slot-bridged editor-region composition, package fixed/overlay geometry, resolved theme-token style reads, privacy-preserving observations, and inert action routing. The editor region reserves the Clay-owned left slot by SDUI panel presence so opening a workspace file under a new document ID cannot overlap the file browser. `file_browser_scroll_reveals_later_rows_without_relisting`, `file_browser_scrolled_action_hits_visible_row`, and `scrolls_point_routes_scroll_to_file_browser_only_inside_left_pane` validate client-local file-browser scroll, scrolled action hit testing, and the scroll-routing boundary.
- `src/masonry_sdui.rs`: `sdui_observable_snapshot_empty_state_is_well_formed`, `sdui_observable_snapshot_captures_representative_tree`, `sdui_observable_snapshot_changes_after_update`, and `sdui_observable_snapshot_node_kinds_sorted_by_id` validate headless SDUI observability extraction.
- `src/masonry_sdui.rs`: `sdui_layout_regression_representative_tree`, `sdui_layout_regression_panel_update_changes_label_only`, `sdui_layout_regression_stale_update_leaves_snapshot_unchanged`, `sdui_layout_regression_snapshot_replaces_prior_tree`, and `sdui_layout_regression_empty_after_root_remove` validate the current SDUI editor/sidebar composition through typed structural snapshots instead of pixel rendering.
- `src/masonry_sdui.rs`: `sdui_accessibility_role_is_generic_container`, `sdui_accessibility_panel_label_matches_title`, `sdui_accessibility_button_label_matches_button_label`, `sdui_accessibility_list_items_match_item_labels`, `sdui_accessibility_representative_tree_covers_all_node_kinds`, `sdui_accessibility_editor_view_label_includes_document_id`, `sdui_accessibility_empty_state_does_not_panic`, and `sdui_accessibility_labels_are_stable_for_equivalent_trees` validate SDUI accessibility roles and labels headlessly.
- `src/masonry_editor.rs`: SDUI snapshot/update tests validate GUI-thread application, side-panel updates, and editor document-state preservation.
- `src/protocol/codec.rs`: `sdui_snapshot_codec_round_trips` and `sdui_update_and_action_codec_round_trip` validate wire-codec coverage.
- `src/protocol/codec.rs`: `sdui_snapshot_payload_stays_under_initial_budget`, `sdui_update_payload_stays_under_initial_budget`, `sdui_update_payload_smaller_than_snapshot_for_panel_change`, and `oversized_sdui_frame_is_rejected` validate representative SDUI payload budgets and bounded frame rejection.
- `tests/clay_js_doc_registry.rs`: `generated_registry_contains_phase12_sdui_schema_helpers` validates public planned `clay:sdui` helper docs, registry entries, lookup tags, empty key binding defaults, custom property discovery, and no-authority security metadata.
- Commands: `cargo test sdui --quiet`, `cargo test --all-targets --quiet`

## Related

- [Protocol Codec](protocol-codec.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
