# Server-Driven UI Protocol Schema

## Source

- `src/protocol/sdui.rs`
- `src/protocol/mod.rs`
- `src/protocol/sdui.rs` unit tests
- `src/server/sdui.rs`
- `src/server/connection/mod.rs`
- `src/client/mod.rs`
- `src/masonry_sdui.rs`
- `src/masonry_editor.rs`
- `src/shell/components.rs`
- `src/shell/theme.rs`
- `src/editor/theme.rs`
- `src/shell/package_ui.rs`

## Overview

The server-driven UI (SDUI) schema defines an inert, typed Rust protocol model for UI trees and explicit server-published SDUI protocol messages. In production the handshake is document-free until `TabCommand::New` or `Reclaim` binds a `TabServerState`; the server then sends the bound tab's `InitialDocument` followed by an editor-only hidden-pane `SduiSnapshot` by default. No default `Workspace` tree is visible until `workspace.toggleFileBrowser` publishes the bounded file-browser tree, while explicit runtime SDUI publication can still replace the validated tree. The legacy static workspace/sidebar tree remains a validation fixture and compatibility helper, and `IpcServer::try_new` initializes `StaticSduiState::empty_for_document(1)` for runtime publication/test setup. The client reconciles decoded snapshots and updates into a **retained Masonry widget subtree** (`SduiRegionWidget`, `src/masonry_sdui_region.rs`) hosted as a real child of `EditorWidget`, and keeps the existing editor surface/state separate. The server owns declarative UI state and publishes trees/updates; the client renders through standard Masonry layout/paint/event routing. Planned public Clay JS helper docs now reserve `clay:sdui` names for JavaScript SDUI schema construction while keeping runtime UI publication explicit. After Phase 18.1, SDUI is explicitly treated as the component-tree substrate under Clay-owned shell/layout primitives (`WorkingAreaLayout`, `PaneSplitTree`, slot contracts) documented in `docs/reference/primitives/shell-layout-strategy.md`, not as a package-controlled native pane/window API. Plan 070 replaced the earlier immediate-mode `SduiNativeState::paint` compatibility bridge with the retained reconciled subtree (see [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md)).

## Responsibilities

- Represent stable SDUI node IDs, tree versions, snapshots, and bounded tree updates.
- Model panels, labels, buttons, lists, document-bound editor views, flex containers, and stack containers.
- Keep the ordinary pre-bind SDUI state document-free and the post-bind default tree editor-only/hidden, while preserving the static workspace/sidebar tree as an explicit fixture/compatibility helper.
- Express user actions as server-routed command intents with typed metadata and validate inbound action sources/commands.
- Bound runtime-published trees by raw payload size, node count, nesting depth, and per-node text length before they can allocate proportional memory.
- Reconcile decoded SDUI snapshots/updates into native client state without touching editor text state for sibling panel changes.
- Reconcile the SDUI tree into retained Masonry widgets (`SduiLabel`/`SduiButton`/`SduiListRow`/`EditorViewWidget` under a Clay-owned `SduiScrollViewport`) hosted as a real child of `EditorWidget`; Masonry routes layout, paint, pointer, focus, scroll, and accessibility through the standard widget tree.
- Route button/list activation back as `ClientMessage::SduiAction` via custom Masonry action types carrying `SduiActionIntent` payloads.
- Avoid carrying client-executable scripts or document text payloads in UI nodes.
- Document public planned SDUI schema helper names through the Clay JS API registry instead of exposing raw Rust protocol types or raw future ops as user-facing APIs.

## How It Works

`SduiTree` stores a `ui_version`, a `root_id`, and a flat list of `SduiNode` values. Each node has a stable `SduiNodeId` and a `SduiNodeKind`. Container nodes refer to children by ID instead of nesting widget state directly, so later reconciliation can replace or remove nodes by stable ID.

`SduiNodeKind::EditorView` uses `SduiEditorBinding` with a `DocumentId` and optional expected document version. The editor view schema never embeds full document text; existing document snapshot/edit protocol messages remain responsible for text synchronization.

`SduiActionIntent` represents button and list-item activations as inert command IDs, sources, and typed arguments. These intents route back to the server as bounded `ClientMessage::SduiAction` values and then through the Phase 18.8 `CommandExecutor`; they never execute filesystem, network, shell, extension-loading, WASM, AI mutation, package JavaScript, or client-side JavaScript authority on the client.

`SduiTreeUpdate` and `SduiTreeOperation` define the first update shape: replace the root, replace a node, or remove a node against explicit base/new UI versions. `ServerMessage::SduiSnapshot` carries a full tree after tab binding (the default is an editor-only hidden-pane tree); `ServerMessage::SduiUpdate` carries bounded declarative updates. `ClientMessage::SduiAction` carries an inert user action intent back to the server. `ClientMessage::CommandIntent` carries the same command-execution path for behavior-manifest server-first keybindings and future transient-menu selections.

`src/server/sdui.rs` owns the static workspace/sidebar tree helper and the runtime replacement boundary. `StaticSduiState::empty_for_document` records the current document without a tree so `snapshot_message` returns `None` and bootstrap sends no default side panel. `StaticSduiState::for_document` remains available for tests and explicit compatibility paths; it builds and validates the workspace tree with a row flex root, workspace side panel, status label, refresh button, document list, and editor view bound by document ID/version. Runtime replacement through `StaticSduiState::replace_with_runtime_tree` accepts only trees that pass the same root/child/editor validation as static trees, then makes `snapshot_message` return a `ServerMessage::SduiSnapshot` again. Runtime trees and later `ReplaceNode` updates reject editor views bound to any document other than the open server-known document, so unknown document views fail safely instead of acquiring document authority. `StaticSduiState::update_message` validates update versions and target node IDs before mutating server UI state; calling it while no tree exists returns the existing empty-tree validation error. `validate_action` accepts only inert command IDs declared by the current SDUI tree and checks that button/list sources match the declared action; with no tree, invalid action intents become typed unknown-command errors instead of panics or local execution.

Runtime SDUI publication in `src/server/ops/sdui.rs` validates actions against built-in SDUI commands (`workspace.refresh`, `document.focus_active`, `document.open_recent`) plus package commands already registered in the runtime command registry. This lets first-party packages such as `@clay/markdown` publish a preview/status panel whose `Toggle Preview` button targets `markdown.togglePreview`, while still rejecting unregistered commands such as `shell.run`. If a package is disabled or invalid, its command is no longer registered and a replacement/fallback tree cannot retain stale package command authority. Open-document classification uses `apply_runtime_outputs_without_sdui`, so package activation, behavior-manifest publication, decoration publication, and parse-time diagnostics cannot erase the Clay-owned workspace/file-browser `StaticSduiState`; explicit config/runtime SDUI publication still uses `apply_runtime_outputs` and remains the only path that replaces shared SDUI validation state.

Runtime SDUI publication budgets are enforced at the `op_clay_sdui_publish_tree` boundary in `runtime_tree_from_json`, mirroring the budget discipline already used for registered package UI contributions. The raw `tree_json` is rejected by byte length (`RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES`, 16 KiB) before `serde_json` allocates a proportional `Value`, and the builder enforces a node-count cap (`RUNTIME_SDUI_TREE_MAX_NODES`, 128), a nesting-depth cap (`RUNTIME_SDUI_TREE_MAX_DEPTH`, 16), and a per-node free-text cap (`RUNTIME_SDUI_TREE_MAX_NODE_TEXT_CHARS`, 4096) on panel titles, labels, button labels, and list item labels/details. A malicious or runaway huge/deep tree is therefore rejected before it can exhaust memory or stack while building the typed `SduiTree`. The budgets live in `src/perf/budgets.rs` alongside the other SDUI/perf constants.

The client connection task converts decoded `ServerMessage::SduiSnapshot` and `ServerMessage::SduiUpdate` frames into `ClientConnectionEvent` values. The existing `EventLoopProxy` bridge delivers those typed events to `EditorWidget::apply_connection_event` on the GUI event loop. `SduiNativeState` stores a flat `BTreeMap<SduiNodeId, SduiNode>`, the current UI version/root, derived editor binding, and crate-internal `PackageUiRuntimeState` for accepted fixed package panels and transient overlays. It is an **inert state holder**: it has no paint path and no pointer/focus interaction state (those were deleted in Plan 070). Snapshot application replaces the native SDUI tree state; updates apply stable-ID root/node replacements or removals only when the base UI version matches. Package UI runtime updates are separately version-gated and bounded before they install fixed `left`/`right`/`top`/`bottom` panels or transient overlay declarations. Editor text, caret, selection, viewport, document version, and behavior manifest state remain in `EditorSurface`. After Phase 18.2, `SduiNativeState` computes its left side panel rect through the internal `PaneSlotLayout` bridge in `src/shell/layout.rs`; Plan 070 renders that slot through a retained reconciled Masonry subtree (`SduiRegionWidget`) rather than an immediate-mode paint path. Phase 18.3 adds the Clay-owned component catalog in `src/shell/components.rs`, typed theme-token resolver in `src/shell/theme.rs`, and slot-aware package UI runtime in `src/shell/package_ui.rs`. The catalog accepts bounded inert component kinds (`editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`/`overlay`, `scroll`/`portal`, `statusItem`, `dropdown`, `collapse`, `modal`, `textInput`), keeps `table` explicitly deferred, validates typed style variables against core or package-declared tokens, and rejects raw CSS/colors, unknown tokens, native handles, callbacks, and client-executable code. `SduiThemeStyle` maps `typography.title`, `typography.body`, and `typography.status` to semantic variants; `TypographyRegistry` supplies the cached user-owned role stack, size, line height, row pitch, and status geometry. Package component `style.fontRole` defaults to `ui` and can select only semantic `monospace`/`proportional` on text-bearing components; concrete families, sizes, CSS, raw Parley values, and renderer callbacks remain rejected. This SDUI resolver is intentionally separate from `src/editor/theme.rs::StyleRegistry`, which resolves editor text/chrome and two-axis syntax/prose decorations selected by `theme.setTheme`. `EditorWidget` uses the SDUI editor binding to treat the composed editor view's `main` slot as the editable region while side-panel/package-panel pointer presses route only declared SDUI/package command intents or stay inert. The Clay-owned left file-browser panel scrolls through the Clay-owned `SduiScrollViewport` widget (owns scroll state, clips via `set_clip_path` in its own layout, paints a themed scrollbar via `paint_scroll_chrome` in `post_paint`); `EditorWidget::on_pointer_event` checks `SduiNativeState::scrolls_point` to distinguish sidebar vs editor scroll and skips handling for sidebar events the viewport already consumed. Scrolling is pure client-local state and never relists directories, calls the server, runs package JavaScript, or enqueues workspace actions.

`SduiNativeState::observable_snapshot` extracts a headless, comparable `SduiObservableSnapshot` without painting, serializing, or invoking a GPU layout pass. The snapshot records the UI version, a `SduiNodeId`-sorted node-kind inventory, visible panel titles, labels, button labels, list item IDs/labels, all reachable editor bindings, package fixed panel IDs/slots/component roots/rectangles, transient overlay IDs/anchors/focus/dismissal policies/rectangles, and simple layout booleans for sidebar presence and non-empty editor region. Structural layout regression tests use this typed snapshot to lock down the editor/sidebar composition, targeted label updates, stale-update rejection, snapshot replacement, root-removal behavior, package fixed slot geometry, transient overlay geometry, package action routing, and observation privacy under `cargo test --all-targets` without opening a window. It intentionally omits document text, filesystem paths, secrets, native handles, raw ops, raw CSS, callbacks, executable package code, and action payload authority; it is `pub(crate)` test/agent infrastructure rather than a Clay JS API surface.

SDUI/package accessibility now flows through the hosted Masonry widget tree. `SduiRegionWidget` reports `Role::Group` and lets its reconciled children flow to the AccessKit tree; `PackageRegionWidget` does the same. `EditorWidget::accessibility` always attaches the `region`, `panel_host`, and `overlay_host` child IDs, including empty/hidden retained hosts, so Masonry's reachable-child walk cannot orphan a subtree. Menu accessibility uses `MenuA11y` on `TransientPackageOverlay` → `PackageRegionWidget` builds synthetic `Role::Menu`/`Role::MenuItem`/`Role::Status` nodes with bounded sanitized labels and selected state. Modal Escape uses `PackageModalDismiss` and the declared inert action route; disabled package controls expose AccessKit disabled state. See [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md).

`EditorWidget::paint` fills the background and paints the editor canvas; the SDUI sidebar (`region`), fixed package panels (`panel_host`), and transient overlays (`overlay_host`) render through hosted Masonry child widgets in the children pass (above editor text); `post_paint` draws the status line. This gives z-order `chrome + editor < sidebar/panels/overlays < status`. Primary pointer presses on SDUI/package interactive widgets are handled by the reconciled widgets themselves (`SduiButton`/`SduiListRow`/`PackageButton`/`PackageListRow`/etc.), which emit custom action types carrying `SduiActionIntent` payloads; `main.rs` downcasts and enqueues a bounded inert `ClientMessage::SduiAction`. Server-first keybindings enqueue bounded `ClientMessage::CommandIntent` values with document and behavior-version metadata. Both use non-blocking `try_send`; the pointer/key handlers do not run client-side script, package JavaScript, command side effects, or wait for IPC capacity/server acknowledgement. Ordinary text editing continues through existing editor commands and edit deltas.

The schema stays separate from the `rkyv` codec boundary even though payload types derive `Archive`, `Serialize`, and `Deserialize` for protocol use.

### Phase 20.4: active-theme routing and interaction states

Phase 20.4 (restyle-only, no kind/token/variable/API change) closed the gap between the Phase 20.1/20.2 token/primitive substrate and the SDUI paint path. Plan 070 later moved interaction state from god-object fields onto per-widget Masonry `EventCtx`/`QueryCtx` state; the interaction-state **contract** below is unchanged.

- **Active-theme routing.** `SduiThemeStyle::from_ui_theme(&ResolvedUiTheme)` resolves panel padding (`spacing.md` × `spacing_scale()`), backgrounds, text, and typography from the **active** theme. SDUI/package widgets read the active `ResolvedUiTheme` via their `ui_theme` field; theme `clay.contributions.designTokens` overrides flow through to component paint automatically.
- **Interaction states.** Each reconciled widget derives `InteractionState` (Disabled > Active > Hover > Focus > Rest) from Masonry `EventCtx`/`QueryCtx` state (`is_disabled`/`is_active`/`is_focus_target`/`is_hovered`), not from god-object `pointer_pos`/`pointer_pressed`/`focused_action` fields (deleted in Plan 070). Buttons fill via `component_state_color(theme, "surface.control", state)` with a `paint_focus_ring` on Focus; list rows fill via `list_row_fill_color(theme, state, selected)`; labels/status items use `disabled_text_color` when disabled.
- **Disabled gating.** Disabled buttons/list items derive disabled from `intent.is_none()` (package) or Masonry `is_disabled()`; they are not hit-testable and cannot emit a `ClientMessage::SduiAction`.
- **`PackageUiComponentTree.disabled`/`PackageUiListItem.disabled`** are `pub(crate)` fields (default `false`, parsed from JSON) so package-declared components can express the Disabled state.

See [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md) for the widget inventory and action routing.

Public programmatic documentation for SDUI lives under `docs/reference/clay-js-api/sdui/`. Those pages define the `clay:sdui` facade exports (`definePanel`, `defineLabel`, `defineButton`, `defineList`, `defineEditorView`, `defineFlex`, and `defineStack`) and are linked from `docs/index.md` for generated registry lookup. In Phase 13, `runtime/js/sdui.js` included through `src/server/facades.rs` calls `op_clay_sdui_define_node` for inert helper objects and `op_clay_sdui_publish_tree` for explicit publication. Publication converts the JSON object graph into typed Rust `SduiTree` state at the server boundary; the client still receives only typed `SduiSnapshot`/`SduiUpdate` protocol messages and never executable JavaScript.

## Plan 088 retained containment and responsive layout

The production SDUI/package path is one retained Masonry tree: `EditorWidget::paint` draws the editor canvas, child hosts draw sidebar/panels/overlays, and `post_paint` draws status. `SduiRegionWidget::layout` and `PackageRegionWidget::layout` clip their own subtrees to their owning bounds; `SduiScrollViewport`, panel/overlay hosts, and `PackageModal` mark child clipping for the renderer and accessibility tree. Nested package `scroll` components receive bounded flex space so long content such as `@clay/settings` scrolls inside its fixed panel instead of painting below it.

The SDUI left-slot decision is constraint-driven: cached `ResolvedUiTheme::PanelDefaults` plus the configured UI body metric determine whether a pane can reserve the workspace browser while leaving a usable editor main region. Long labels are clipped to typography-derived rows while their full accessible names remain available. Hidden/welcome trees omit the workspace panel and reclaim the left slot. No package breakpoint, raw size, filesystem read, JavaScript, or IPC work enters layout/paint/input.

The package contract remains unchanged: component kinds, style variables, typed tokens, overlay anchors, and permissions are additive-only and no new Plan 088 public API was added. Host conformance still rejects raw styles/authority and enforces WCAG contrast/state completeness at install/theme-apply boundaries.

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
- Production pre-bind handshake sends no SDUI snapshot; after tab binding, the workspace pane's default-hidden state sends one bounded editor-only snapshot, while toggling publishes the bounded file-browser tree. Explicit runtime-generated snapshots remain bootstrap/resync-style state, not ordinary edit traffic.
- SDUI snapshots/updates enter the widget only after protocol decoding and client event routing; raw IPC bytes never enter Masonry state.
- SDUI update reconciliation is version-gated and isolated from editor text mutation logic.
- The composed editor view must bind to the current open document before it can constrain the native editor region; unknown document bindings are rejected server-side or treated as safe non-authoritative UI on the client.
- Ordinary text edits continue to use edit deltas/acknowledgements and do not serialize full documents as part of SDUI.
- Inbound SDUI actions and keybinding/menu command intents are command intents only; the server normalizes them into `CommandExecutionRequest` and sends them through `CommandExecutor`. Runtime-published package trees still validate action targets before publication; stale or unknown runtime action IDs are rejected by command execution instead of a UI-specific dispatcher. They do not grant filesystem, network, shell, extension-loading, WASM, AI mutation, package JavaScript, or client-side JavaScript authority.
- Open-time runtime outputs update behavior/decorations only and do not replace `StaticSduiState`; workspace/file-browser action validation remains Clay-owned across document opens, mode activation, and parse timeout diagnostics.
- Runtime-published SDUI trees are bounded by payload bytes, node count, nesting depth, and per-node text length; oversized, too-deep, or too-large trees are rejected at the publication boundary before allocation.
- `clay:sdui` helpers build inert JavaScript object graphs; only `publishTree` crosses into typed Rust validation/publication, and it does not make protocol DTOs, native Masonry reconciliation, or server validation helpers directly callable from JavaScript.
- SDUI nodes remain package-contributed component data and do not let packages create panes/splits/slots, mutate Clay's working-area topology, or request direct Masonry/native widget construction.
- SDUI theme tokens and editor text themes are separate primitives. `ThemeTokenResolver` resolves typed component scalars; `StyleRegistry` resolves base editor UI plus `TokenType`/`Modifiers` styles. Do not use SDUI nodes, raw CSS, or component theme tokens to bypass `StyleRegistry` for editor syntax/prose rendering.

## Tests

- `src/protocol/sdui.rs`: `sdui_schema_represents_initial_widget_kinds` validates all initial widget/layout kinds.
- `src/protocol/sdui.rs`: `sdui_editor_view_uses_document_binding_not_text_payload` validates editor binding without embedded text.
- `src/protocol/sdui.rs`: `sdui_actions_are_server_routed_intents` validates inert command intent shape.
- `src/server/connection/mod.rs`: `sdui_actions_and_keybinding_intents_share_command_execution_path`, `package_ui_unregistered_action_is_rejected_by_command_execution`, and `file_browser_action_survives_markdown_open_followup_diagnostic` validate that SDUI/package UI actions and keybinding/menu command intents share the Phase 18.8 executor path and that open-time follow-ups preserve Clay-owned file-browser validation.
- `src/protocol/sdui.rs`: `sdui_updates_target_stable_node_ids` validates stable-ID update operations.
- `src/server/sdui.rs`: `empty_sdui_state_publishes_no_snapshot` validates the end-user default no-snapshot state, and `default_sdui_tree_is_valid_and_static` validates explicit static snapshot construction.
- `src/server/js_runtime/mod.rs`: `configuration_can_publish_sdui_snapshot`, `js_generated_sdui_rejects_unknown_document_binding`, and `js_generated_sdui_rejects_executable_action_payloads` validate the runtime SDUI publication boundary.
- `src/server/sdui.rs`: `default_sdui_contains_editor_and_panel_regions` validates the default multi-region tree shape.
- `src/server/sdui.rs`: `editor_view_requires_known_document_binding` validates that SDUI updates cannot bind an editor view to an unknown document.
- `src/server/sdui.rs`: `sdui_update_rejects_unknown_node_id` validates bounded update target checks.
- `src/server/sdui.rs`: `sdui_action_validation_rejects_unknown_command` validates inbound action intent command filtering.
- `src/server/mod.rs`: `open_time_runtime_sdui_output_does_not_replace_workspace_browser_state` validates the behavior/decorations-only open-time runtime application path.
- `src/server/js_runtime/mod.rs`: `markdown_config_fixture_opens_workspace_and_publishes_status_sdui` validates package-owned SDUI publication can target registered package commands and rejects stale/unregistered authority at publication time.
- `src/server/ops/sdui.rs`: `runtime_tree_too_large_rejected`, `runtime_tree_too_deep_rejected`, `runtime_tree_too_many_nodes_rejected`, `runtime_tree_text_too_long_rejected`, and `runtime_tree_within_budgets_loads` validate the runtime SDUI publication budget discipline.
- `src/server/connection/mod.rs` / `src/server/mod.rs`: deferred-handshake coverage validates no pre-bind snapshot, an editor-only hidden post-bind snapshot, and a visible selected-root tree after the per-tab toggle; `client_receives_js_generated_sdui_snapshot` validates explicit runtime-generated snapshot emission.
- `src/client/mod.rs`: `client_receives_sdui_snapshot_event` validates decoded SDUI event delivery and `sdui_button_action_emits_server_intent` validates bounded typed action emission.
- `src/masonry_sdui.rs`: `sdui_snapshot_replaces_native_tree_state`, `sdui_update_preserves_editor_document_state`, `editor_region_is_bounded_when_document_bound_editor_view_is_present`, `hidden_workspace_browser_reclaims_left_slot`, `narrow_workspace_browser_yields_its_slot_without_overlapping_editor`, `large_ui_typography_yields_sidebar_before_main_region_is_unusable`, `slot_panel_contribution_places_panel_in_requested_slot_and_preserves_main_editor`, `transient_overlay_renders_without_consuming_fixed_slot_geometry`, `slot_ui_observation_omits_document_text_native_handles_and_raw_authority`, `sdui_renderer_uses_resolved_theme_tokens_for_panel_styles`, `workspace_browser_reserves_left_slot_after_document_id_changes`, and `stale_sdui_update_is_ignored` validate native reconciliation, responsive hidden/visible slot geometry, package fixed/overlay geometry, resolved SDUI theme-token style reads, privacy-preserving observations, and inert action routing. The editor region reserves the Clay-owned left slot by SDUI panel presence so opening a workspace file under a new document ID cannot overlap the file browser. `scrolls_point_routes_scroll_to_file_browser_only_inside_left_pane` validates the scroll-routing boundary (sidebar scroll is now owned by `SduiScrollViewport`).
- `tests/editor_performance_invariants.rs`: `style_registry_is_single_source_of_color_for_paint_paths` guards that editor syntax/prose/base UI colors stay in `StyleRegistry`, not SDUI theme-token shortcuts or paint-path literals.
- `src/masonry_sdui.rs`: `sdui_observable_snapshot_empty_state_is_well_formed`, `sdui_observable_snapshot_captures_representative_tree`, `sdui_observable_snapshot_changes_after_update`, and `sdui_observable_snapshot_node_kinds_sorted_by_id` validate headless SDUI observability extraction.
- `src/masonry_sdui.rs`: `sdui_layout_regression_representative_tree`, `sdui_layout_regression_panel_update_changes_label_only`, `sdui_layout_regression_stale_update_leaves_snapshot_unchanged`, `sdui_layout_regression_snapshot_replaces_prior_tree`, and `sdui_layout_regression_empty_after_root_remove` validate the current SDUI editor/sidebar composition through typed structural snapshots instead of pixel rendering.
- `src/masonry_package_region.rs`: `hosted_menu_overlay_exposes_menu_role_and_item_accessibility_labels` validates hosted menu accessibility (Menu/MenuItem/Status roles + bounded selected labels) via `MenuA11y`; `overlay_host_reconcile_updates_menu_selection`, `package_region_accessibility_marks_children_as_clipped`, and `centered_command_center_scrolls_60_results_without_overflow` validate retained selection, clipping semantics, and centered containment. SDUI/package accessibility flows through the hosted Masonry widget tree (see [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md)).
- `src/masonry_editor.rs`: SDUI snapshot/update tests validate GUI-thread application, side-panel updates, and editor document-state preservation.
- `src/protocol/codec.rs`: `sdui_snapshot_codec_round_trips` and `sdui_update_and_action_codec_round_trip` validate wire-codec coverage.
- `src/protocol/codec.rs`: `sdui_snapshot_payload_stays_under_initial_budget`, `sdui_update_payload_stays_under_initial_budget`, `sdui_update_payload_smaller_than_snapshot_for_panel_change`, and `oversized_sdui_frame_is_rejected` validate representative SDUI payload budgets and bounded frame rejection.
- `tests/clay_js_doc_registry.rs`: `generated_registry_contains_phase12_sdui_schema_helpers` validates public planned `clay:sdui` helper docs, registry entries, lookup tags, empty key binding defaults, custom property discovery, and no-authority security metadata.
- Commands: `cargo test sdui --quiet`, `cargo test --all-targets --quiet`

## Related

- [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md)
- [Protocol Codec](protocol-codec.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
