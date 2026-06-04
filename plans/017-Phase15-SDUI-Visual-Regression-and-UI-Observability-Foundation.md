# Phase 15: SDUI, Visual Regression, and UI Observability Foundation

## Objectives
- Add automated visual/layout regression coverage for the current native SDUI editor/sidebar composition before packages start contributing mode-specific panels and rendering declarations.
- Add structured UI observability so SDUI snapshots, updates, panel identities, editor-view bindings, status text, and runtime diagnostics can be inspected by tests, the command palette, and AI agents without relying solely on manual observation.
- Improve accessibility labels and roles for SDUI panels, buttons, lists, and status text so assistive tools and headless test drivers can inspect UI state.
- Validate that current SDUI representative trees remain within the Phase 14 budget constants (`SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` = 4 096 B, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` = 1 024 B) and introduce compression or diff shaping only if those thresholds are already exceeded.
- Keep documented SDUI layout/panel visibility configuration APIs deferred until real user-facing package or workspace panel settings exist.

## Expected Outcome
- Layout and rendering logic for the current SDUI editor/sidebar composition is covered by deterministic, headless-compatible structural layout regression tests that run under `cargo test --all-targets` without a GPU or interactive window.
- A structured `SduiObservableSnapshot` type captures SDUI version, panel identities, visible texts, editor-view bindings, layout rects, and accessibility labels so tests assert exact structural expectations rather than painting to a pixel buffer.
- The Masonry `SduiNativeState` widget implements `accessibility_role`, `accessibility_label`, and `accessibility` for all rendered SDUI node kinds (Panel, Label, Button, List, EditorView, Flex, Stack).
- Payload budget guards covering `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` and `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` pass with a non-trivial representative tree, and any found-in-practice tree that exceeds either threshold triggers an explicit investigation before Phase 17 package-owned UI multiplies the payload surface.
- Clay JS API documentation, configuration docs, generated registry state, and the implementation wiki are updated or explicitly verified as unchanged for the surfaces introduced by this phase.

## Tasks

- [x] Add a `SduiObservableSnapshot` type and extraction helper
  - Acceptance Criteria:
    - Functional: A `SduiObservableSnapshot` struct can be extracted from any `SduiNativeState` and captures: `ui_version`, a sorted list of `(SduiNodeId, SduiNodeKind variant name)` pairs identifying present node kinds, all visible panel titles, all visible label texts, all visible button labels, all list item labels and IDs, all `EditorView` `document_id` and `expected_version` values, and a layout summary (sidebar present: bool, editor region non-empty: bool) where computable without a real GPU layout pass.
    - Functional: `SduiObservableSnapshot` is `Debug + PartialEq + Clone` and can be compared across calls without pixel rendering.
    - Functional: Extracting a snapshot from an empty `SduiNativeState` returns a well-formed zero-node snapshot without panicking.
    - Performance: Extraction is allocation-bounded by the node count and completes in `O(n)` tree traversal; it does not re-serialize or re-rkyv the tree.
    - Code Quality: The type lives in `src/masonry_sdui.rs` (or a new `src/sdui_observe.rs` if it grows past a screen), is `pub(crate)`, and is only widened to `pub` if a later Clay JS API requires it.
    - Security: The snapshot type does not include raw document text content, file paths, or workspace secrets; it includes only structural metadata produced by the server-validated SDUI tree.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 15: "Add structured UI observability for SDUI snapshots/updates, status text, panel identity, editor-view identity, and runtime diagnostics."
      - `src/protocol/sdui.rs`: Current `SduiTree`, `SduiNodeKind`, `SduiNativeState`, `visible_texts()`.
      - `src/masonry_sdui.rs`: `collect_visible_texts`, `find_editor_binding`, `rebuild_derived_state`.
      - `src/perf/budgets.rs`: `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Authority boundary, no full-document IPC, no client filesystem authority.
    - Options Considered:
      - Pixel-buffer snapshot testing (e.g., `insta` image snapshots): Covers pixel accuracy but requires a GPU, is brittle across font rendering environments, and cannot run in CI without a render target.
      - Structural/logical snapshot via a new `SduiObservableSnapshot` type: Deterministic, headless-compatible, environment-agnostic, and captures the structural semantics tests care about (which nodes, which texts, which editor views).
      - Re-use existing `visible_texts()` for assertions: Already public but only extracts text strings; does not capture node kind identity, editor bindings, or layout regions.
    - Chosen Approach:
      - Add a new `SduiObservableSnapshot` struct that aggregates the outputs of the existing tree-traversal helpers plus a simple layout-presence summary. This avoids a GPU dependency while giving tests a stable, comparable surface.
    - API Notes and Examples:
      ```rust
      // src/masonry_sdui.rs
      #[derive(Debug, Clone, PartialEq)]
      pub(crate) struct SduiObservableSnapshot {
          pub ui_version: SduiVersion,
          pub node_kinds: Vec<(SduiNodeId, &'static str)>, // sorted by node id
          pub panel_titles: Vec<String>,
          pub label_texts: Vec<String>,
          pub button_labels: Vec<String>,
          pub list_item_ids: Vec<String>,
          pub editor_bindings: Vec<SduiEditorBinding>,
          pub has_sidebar: bool,
          pub editor_region_non_empty: bool,
      }

      impl SduiNativeState {
          pub(crate) fn observable_snapshot(&self, widget_size: Size) -> SduiObservableSnapshot { ... }
      }
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui.rs`: Add `SduiObservableSnapshot` struct, `observable_snapshot` method on `SduiNativeState`.
      - `docs/wiki/modules/server-driven-ui.md`: Document the internal observability snapshot helper added during implementation.
    - References:
      - `roadmap.md` Phase 15
      - `src/masonry_sdui.rs` existing traversal logic
      - `src/protocol/sdui.rs` node kinds
  - Test Cases to Write:
    - `sdui_observable_snapshot_empty_state_is_well_formed`: Empty `SduiNativeState` produces a zero-node snapshot without panic.
    - `sdui_observable_snapshot_captures_representative_tree`: `representative_sdui_tree()` snapshot contains expected panel titles, label texts, button labels, list item IDs, editor binding, sidebar present, editor region non-empty.
    - `sdui_observable_snapshot_changes_after_update`: A `representative_panel_update()` applied to a seeded state produces a different snapshot from the pre-update snapshot.
    - `sdui_observable_snapshot_node_kinds_sorted_by_id`: Node-kind list is sorted ascending by `SduiNodeId` for stable comparison.
  - Verification:
    - `cargo fmt`
    - `cargo test -p clay --lib masonry_sdui`

- [x] Add structural layout regression tests for the SDUI editor/sidebar composition
  - Acceptance Criteria:
    - Functional: At least one test per SDUI node kind (Panel, Label, Button, List, EditorView, Flex, Stack) asserts expected observable fields after applying a `representative_sdui_tree()` or a named fixture tree.
    - Functional: Tests cover: initial snapshot matches expected structure; applying `representative_panel_update()` changes only the targeted label text; applying a stale update (wrong `base_ui_version`) leaves snapshot unchanged; applying a snapshot to non-empty state replaces the prior tree.
    - Functional: Tests are headless — they do not start a window, allocate a GPU surface, or require an interactive session — and run under `cargo test --all-targets`.
    - Performance: All layout regression tests complete in under 50 ms combined on a developer workstation.
    - Code Quality: Tests live in `src/masonry_sdui.rs` under `#[cfg(test)]` and are self-contained without external fixtures or network access.
    - Security: No test writes to arbitrary filesystem paths, opens sockets, or escalates process authority.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 15: "Add automated visual/layout regression coverage for the current native SDUI editor/sidebar composition."
      - `src/masonry_sdui.rs`: Existing tests (`sdui_snapshot_replaces_native_tree_state`, `stale_sdui_update_is_ignored`).
      - `src/protocol/sdui.rs`: `representative_sdui_tree`, `representative_panel_update`.
    - Options Considered:
      - Pixel snapshot testing: Requires GPU, not headless-compatible.
      - Golden-file text snapshots with `insta`: Adds a dev-dependency and review workflow; overkill for structural logic already expressible as typed assertions.
      - Direct typed struct assertions using `SduiObservableSnapshot`: Precise, stable, no extra dependencies, already idiomatic with `PartialEq`.
    - Chosen Approach:
      - Use `SduiObservableSnapshot` from the previous task for typed struct assertions. Each test builds a known tree, applies it, extracts the snapshot, and asserts specific fields. This is deterministic without a render surface.
    - API Notes and Examples:
      ```rust
      #[test]
      fn sdui_layout_regression_representative_tree() {
          let mut state = SduiNativeState::empty();
          state.apply_snapshot(representative_sdui_tree());
          let snap = state.observable_snapshot(Size::new(800.0, 600.0));
          assert!(snap.has_sidebar);
          assert!(snap.editor_region_non_empty);
          assert!(snap.panel_titles.iter().any(|t| t.contains("Workspace")));
          assert!(snap.editor_bindings.iter().any(|b| b.document_id == 7));
      }
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui.rs`: Add `#[cfg(test)]` layout regression tests using `SduiObservableSnapshot`.
    - References:
      - `src/masonry_sdui.rs` existing test module
      - `src/protocol/sdui.rs` `representative_sdui_tree`, `representative_panel_update`
  - Test Cases to Write:
    - `sdui_layout_regression_representative_tree`: Representative tree snapshot matches expected structural fields.
    - `sdui_layout_regression_panel_update_changes_label_only`: After applying `representative_panel_update()`, only the targeted label text changes; panel titles, editor bindings, and node kinds are unchanged.
    - `sdui_layout_regression_stale_update_leaves_snapshot_unchanged`: Applying a stale-version update does not alter the observable snapshot.
    - `sdui_layout_regression_snapshot_replaces_prior_tree`: Applying a new snapshot to a non-empty state replaces all prior node data in the observable snapshot.
    - `sdui_layout_regression_empty_after_root_remove`: A `RemoveNode` targeting the root node produces `has_sidebar = false` and `editor_region_non_empty = false`.
  - Verification:
    - `cargo fmt`
    - `cargo test -p clay --lib masonry_sdui`
    - `cargo test --all-targets`

- [x] Add accessibility labels and roles to SDUI-rendered widget nodes
  - Acceptance Criteria:
    - Functional: `SduiNativeState`, when used as a Masonry widget, implements `accessibility_role` returning `Role::GenericContainer` or an appropriate accessible role.
    - Functional: `SduiNativeState` implements `accessibility` (the Masonry `Widget` trait method) that emits `Node` entries for every rendered SDUI node kind using the roles available in AccessKit/Masonry 0.4.0: Panel → `Role::Pane` with label = panel title; Label → `Role::Label` with label = text content; Button → `Role::Button` with label = button label; List → `Role::List` with child entries `Role::ListItem` labeled by item label; EditorView → `Role::MultilineTextInput` with label including the bound document ID; Flex/Stack → `Role::Pane`.
    - Functional: `EditorWidget` already implements `accessibility_role` and `accessibility`; the new SDUI accessibility implementation must not regress the existing editor accessibility tests.
    - Functional: Accessibility labels are stable across equivalent SDUI trees (same tree produces same labels in same order).
    - Performance: Accessibility tree construction is driven only when Masonry calls `accessibility()`; it does not run on every paint or input event.
    - Code Quality: SDUI accessibility logic lives in `src/masonry_sdui.rs` and reuses the existing tree-traversal pattern. The `masonry::accesskit::{Node, Role}` imports already present in `src/masonry_editor.rs` are reused.
    - Security: Accessibility labels for SDUI panels do not expose file paths, workspace secrets, or server internal identifiers beyond stable document IDs already in the SDUI tree.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 15: "Improve accessibility labels/roles for editor, SDUI panels, diagnostics, and status text so tests and assistive tools can inspect UI state."
      - `src/masonry_editor.rs`: Existing `accessibility_role()` → `Role::MultilineTextInput`, `accessibility()` building `Node`, `set_label()`.
      - Masonry 0.4.0 `Widget` trait: `accessibility_role(&self) -> Role`, `accessibility(&mut self, ctx: &mut AccessCtx<'_>, node: &mut Node)`.
    - Options Considered:
      - Emit a single flat `GenericContainer` node for the entire SDUI tree: Minimal work but provides no granular accessible structure for panels, buttons, or list items.
      - Emit a per-kind accessible node tree mirroring the SDUI tree: Provides rich accessible structure and enables tests to assert per-kind labels; preferred.
    - Chosen Approach:
      - Implemented `Widget for SduiNativeState` with `accessibility_role` returning `Role::GenericContainer` at the widget root and `accessibility` iterating the SDUI node tree to emit AccessKit child `Node` entries through Masonry's `AccessCtx::tree_update()`. Added `accessibility_nodes()` as a pure headless role/label traversal for unit assertions. Masonry 0.4.0/AccessKit 0.21.1 does not expose `Role::Group`, `Role::StaticText`, or `Role::TextArea`, so the implementation uses the closest available roles: `Role::Pane`, `Role::Label`, and `Role::MultilineTextInput`.
    - API Notes and Examples:
      ```rust
      // In the Widget impl for SduiNativeState (masonry_sdui.rs)
      fn accessibility_role(&self) -> Role {
          Role::GenericContainer
      }

      fn accessibility(&mut self, ctx: &mut AccessCtx<'_>, node: &mut Node) {
          node.set_label("Server-driven UI");
          // Walk SDUI tree and append child AccessKit nodes to ctx.tree_update().
          // Panel/Flex/Stack -> Role::Pane
          // Label -> Role::Label
          // Button -> Role::Button
          // List -> Role::List with Role::ListItem children
          // EditorView -> Role::MultilineTextInput, label includes document_id
      }
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui.rs`: Add `accessibility_role` and `accessibility` to the Masonry `Widget` impl for `SduiNativeState`.
    - References:
      - `src/masonry_editor.rs` lines 559–569 (existing accessibility impl)
      - Masonry `Widget` trait `accessibility_role`, `accessibility`
      - `masonry::accesskit::{Node, Role}`
  - Test Cases to Write:
    - `sdui_accessibility_role_is_generic_container`: `SduiNativeState` reports `Role::GenericContainer` as its widget role.
    - `sdui_accessibility_panel_label_matches_title`: After applying a tree with a named panel, the accessibility label for the panel node matches the panel title.
    - `sdui_accessibility_button_label_matches_button_label`: Button node reports `Role::Button` with label matching the button's `label` field.
    - `sdui_accessibility_representative_tree_covers_all_node_kinds`: Representative tree accessibility traversal covers Flex, Panel, Stack, Label, Button, List, ListItem, and EditorView role mappings.
    - `sdui_accessibility_editor_view_label_includes_document_id`: EditorView node label includes the bound `document_id`.
    - `sdui_accessibility_empty_state_does_not_panic`: Calling accessibility traversal on an empty `SduiNativeState` does not panic.
    - `sdui_accessibility_labels_are_stable_for_equivalent_trees`: Equivalent trees produce the same role/label sequence.
  - Verification:
    - `cargo fmt`
    - `cargo test -p clay --lib masonry_sdui`
    - `cargo test -p clay --lib masonry_editor`

- [x] Add status-text and runtime-diagnostic observability to `ClientConnectionEvent` and the GUI
  - Acceptance Criteria:
    - Functional: A new `SduiStatusObservation` struct (or extension of `SduiObservableSnapshot`) captures the currently displayed connection status text, access state, sync state (version), and any active runtime diagnostic message visible in the GUI chrome.
    - Functional: `EditorWidget` exposes a `pub(crate) fn status_observation(&self) -> SduiStatusObservation` method that returns the current GUI chrome state without requiring painting or a window.
    - Functional: `SduiStatusObservation` is `Debug + PartialEq + Clone`.
    - Functional: Existing `EditorWidget` tests for connection/access/version status continue to pass; no regression.
    - Performance: `status_observation()` is a pure `&self` read with no allocation beyond cloning the active status strings.
    - Code Quality: The new method does not duplicate logic already in `accessibility_label()`; if there is overlap, one calls the other.
    - Security: Status observation does not expose file paths, server private state, or server process details beyond what is already displayed in the GUI chrome.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 15: "Add structured UI observability for … status text … and runtime diagnostics."
      - `src/masonry_editor.rs`: `accessibility_label()`, existing connection/access/version status fields.
      - `src/client/mod.rs`: `ClientConnectionEvent::RuntimeDiagnostic`, `ClientConnectionEvent::SduiSnapshot`.
    - Options Considered:
      - Re-use `accessibility_label()` string as the observability surface: It already exists but is a flat string; not typed enough for assertions about individual fields (access state vs. sync version vs. diagnostic).
      - Add a dedicated `SduiStatusObservation` struct: Typed, PartialEq-comparable, and allows tests to assert individual fields without parsing strings.
    - Chosen Approach:
      - Added a `SduiStatusObservation` struct with fields for the exact status text, connection state, access state, sync version, and optional diagnostic text. Added `status_observation()` to `EditorWidget`. `status_text()` now reads from the same observation path and `accessibility_label()` continues to include the resulting status string.
    - API Notes and Examples:
      ```rust
      #[derive(Debug, Clone, PartialEq, Eq)]
      pub(crate) struct SduiStatusObservation {
          pub status_text: String,        // exact GUI status chrome text
          pub connection_label: String,   // e.g. "Connected", "Connecting", "Local Fallback"
          pub access_label: String,       // e.g. "Editable", "Read-only Observer", "No Server"
          pub sync_version: Option<u64>,  // latest confirmed doc version
          pub diagnostic_text: Option<String>, // last runtime diagnostic if active
      }

      impl EditorWidget {
          pub(crate) fn status_observation(&self) -> SduiStatusObservation { ... }
      }
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs`: Add `SduiStatusObservation` struct and `status_observation()` method on `EditorWidget`.
      - `docs/wiki/modules/masonry-editor.md`: Document GUI status observability and runtime diagnostic flow.
      - `docs/wiki/index.md`: Link the Masonry editor status observability wiki page.
    - References:
      - `src/masonry_editor.rs` `accessibility_label`, connection/access/version state fields
      - `src/client/mod.rs` `ClientConnectionEvent`
  - Test Cases to Write:
    - `status_observation_local_fallback_state`: In the local-fallback (no server) state, `connection_label` reflects the local label, `access_label` reflects no server access, `sync_version` is `None`.
    - `status_observation_connected_editable_with_version`: After a simulated connected-editable event with version 5, `sync_version` is `Some(5)`.
    - `status_observation_diagnostic_present_after_runtime_diagnostic_event`: After receiving a `RuntimeDiagnostic` event, `diagnostic_text` is `Some(...)`.
    - `status_observation_does_not_regress_accessibility_label`: `accessibility_label()` output is consistent with the corresponding `SduiStatusObservation` fields.
  - Verification:
    - `cargo fmt`
    - `cargo test -p clay --lib masonry_editor`
    - `cargo test -p clay --lib client`

- [x] Validate SDUI payload sizes against Phase 14 budget constants and document findings
  - Acceptance Criteria:
    - Functional: The existing `sdui_snapshot_payload_stays_under_initial_budget` test in `src/protocol/codec.rs` remains passing and references `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`.
    - Functional: A new test `sdui_update_payload_stays_under_initial_budget` encodes `representative_panel_update()` and asserts the encoded length is ≤ `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` (1 024 B).
    - Functional: If either representative fixture already exceeds its budget constant, the task outcome is an explicit filed finding documented in `docs/development/performance.md` under a new "SDUI Payload Budget Findings" subsection, with the measured size and a recommended action (compression, tree shaping, or budget adjustment with rationale) before Phase 17 work begins.
    - Functional: The `performance.md` document records both the current measured snapshot and update payload sizes alongside the budget constants.
    - Performance: These are compile-time and encode-decode tests; they add no runtime overhead to the application.
    - Code Quality: Budget constants are imported from `src/perf/budgets.rs`; tests do not duplicate magic numbers.
    - Security: Tests do not write files, open sockets, or access filesystem paths.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 15: "Use the Phase 14 budget constants `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` (4 096 B) and `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` (1 024 B) … as the explicit thresholds."
      - `src/perf/budgets.rs`: `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES = 4096`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES = 1024`.
      - `src/protocol/codec.rs`: Existing `sdui_snapshot_payload_stays_under_initial_budget` test.
      - `src/protocol/sdui.rs`: `representative_sdui_tree()`, `representative_panel_update()`.
    - Options Considered:
      - Defer payload validation to Phase 17 when package-owned panels exist: Risks discovering a budget problem only after the payload surface multiplies.
      - Validate now with representative fixtures and document any headroom or gap: Preferred; establishes a known baseline before package contributions begin.
    - Chosen Approach:
      - Add the missing update-payload budget test and record current measured sizes in `performance.md`. If either threshold is already exceeded, capture findings and a remediation plan before Phase 17.
    - API Notes and Examples:
      ```rust
      // src/protocol/codec.rs
      #[test]
      fn sdui_update_payload_stays_under_initial_budget() {
          use crate::perf::budgets::SDUI_UPDATE_PAYLOAD_BUDGET_BYTES;
          let update = representative_panel_update();
          // encode via codec and measure
          assert!(encoded_len <= SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
              "SDUI update payload {} B exceeds budget {} B",
              encoded_len, SDUI_UPDATE_PAYLOAD_BUDGET_BYTES);
      }
      ```
    - Files to Create/Edit:
      - `src/protocol/codec.rs`: Add `sdui_update_payload_stays_under_initial_budget` test.
      - `docs/development/performance.md`: Add "SDUI Payload Budget Findings" subsection recording current measured sizes and budget constants.
    - References:
      - `src/perf/budgets.rs`
      - `src/protocol/codec.rs` existing snapshot budget test
      - `src/protocol/sdui.rs` representative helpers
  - Test Cases to Write:
    - `sdui_update_payload_stays_under_initial_budget`: Encoded `representative_panel_update()` is ≤ `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.
    - `sdui_snapshot_payload_stays_under_initial_budget` (existing): Verify it still imports from `budgets.rs`; update import if needed.
  - Verification:
    - `cargo fmt`
    - `cargo test -p clay --lib sdui_snapshot_payload_stays_under_initial_budget`
    - `cargo test -p clay --lib sdui_update_payload_stays_under_initial_budget`
    - `cargo test --test performance_protocol`
  - Findings:
    - Representative SDUI snapshot payload: 816 bytes against `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` = 4096 bytes; passes with 3280 bytes of headroom.
    - Representative SDUI update payload: 192 bytes against `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` = 1024 bytes; passes with 832 bytes of headroom.
    - No compression, diff shaping, or budget adjustment is needed for the current representative fixtures.

- [x] Document headless/window-driver smoke coverage approach and deferred GPU path
  - Acceptance Criteria:
    - Functional: `docs/development/performance.md` or a new `docs/development/ui-observability.md` page documents: (a) what "structural layout regression" means in Clay's context and why pixel-buffer snapshots are deferred; (b) how `SduiObservableSnapshot` and `SduiStatusObservation` are used in tests; (c) what would be needed to enable GPU-backed pixel snapshot tests if Masonry/winit adds headless rendering support; (d) how to run the SDUI regression tests locally with `cargo test -p clay --lib masonry_sdui`.
    - Functional: The doc page is linked from `docs/index.md`.
    - Functional: `docs/development/performance.md` already exists and covers benchmark/profiling content; if the SDUI observability doc is added separately, a cross-link is placed in `performance.md`.
    - Performance: Documentation-only task; zero runtime overhead.
    - Code Quality: The document follows the existing developer-doc style in `docs/development/`.
    - Security: The document does not reveal server internals, private IPC endpoints, or uncommitted security decisions.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 15: "Add headless or window-driver smoke coverage when Masonry/winit support makes status/layout observation practical."
      - `docs/development/performance.md`: Existing benchmark/profiling documentation.
      - `docs/development/launch-and-gui-smoke.md`: Existing GUI smoke doc.
    - Options Considered:
      - Inline the SDUI observability section into `performance.md`: Keeps docs consolidated but makes the performance page longer and mixes benchmark concepts with UI structural testing concepts.
      - New `docs/development/ui-observability.md`: Clearer separation; the SDUI regression and observability content is distinct enough from CPU/memory profiling to merit its own page.
    - Chosen Approach:
      - Create `docs/development/ui-observability.md` covering structural regression approach, `SduiObservableSnapshot`, `SduiStatusObservation`, headless vs. GPU path trade-offs, and local test commands. Cross-link from `performance.md` and `docs/index.md`.
    - API Notes and Examples:
      ```text
      # Running SDUI structural regression tests
      cargo test -p clay --lib masonry_sdui
      cargo test --all-targets
      ```
    - Files to Create/Edit:
      - `docs/development/ui-observability.md`: New developer guide for SDUI structural regression and observability.
      - `docs/development/performance.md`: Add cross-link to `ui-observability.md`.
      - `docs/index.md`: Link the new UI observability developer guide.
    - References:
      - `docs/development/performance.md`
      - `docs/development/launch-and-gui-smoke.md`
      - `roadmap.md` Phase 15
  - Test Cases to Write:
    - Manual review: `docs/index.md` links the new `ui-observability.md`; `performance.md` cross-links it; the new page covers all four documented topics.
  - Verification:
    - Added `docs/development/ui-observability.md` covering structural layout regression, `SduiObservableSnapshot`, `SduiStatusObservation`, deferred GPU-backed pixel snapshot prerequisites, and local test commands.
    - Linked `docs/development/ui-observability.md` from `docs/index.md` and cross-linked it from `docs/development/performance.md`.
    - Confirmed with text search that the new links and documented test commands are present.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: All server-side Rust public functions introduced or changed by this phase are either exposed through explicit `deno_core` op wrappers and stable Clay JS/TS facade modules, or made `pub(crate)` / private with a rationale comment.
    - Functional: `SduiObservableSnapshot` and `SduiStatusObservation` are `pub(crate)` and not exposed through a Clay JS API in this phase (they are test/agent-internal types); this decision is recorded explicitly in the Clay JS API inventory or a short rationale comment in source.
    - Functional: Any new server-side public functions added to the SDUI server path (e.g., a `query_sdui_observable_state` op if one is introduced) have: a Markdown doc at `docs/reference/clay-js-api/sdui/`, a stable registry ID, a searchable `user_facing_name`, a key binding entry (empty list if none), custom properties for behavior-changing settings, and a `docs/index.md` link.
    - Functional: `cargo test` fails if a Clay JS API Markdown doc, `docs/index.md` link, generated registry entry, or lookup entry is missing or stale for any new API introduced.
    - Performance: No synchronous JavaScript in the keypress/paint hot path introduced.
    - Code Quality: Raw `Deno.core.ops.op_*` calls are not the user-facing API for any new surface.
    - Security: No configuration API implicitly grants filesystem, network, shell, or extension-loading authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay JS API task requirements.
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
    - Options Considered:
      - Expose `SduiObservableSnapshot` as a Clay JS query API immediately: Premature; the type is not yet stable and its primary consumers are internal tests and future package/agent introspection phases.
      - Keep `SduiObservableSnapshot` and `SduiStatusObservation` `pub(crate)` with a rationale comment: Correct; they are implementation types for regression testing. If a future phase needs programmatic SDUI introspection for agents or packages, a dedicated `clay:sdui.queryUiState` API should be introduced then.
    - Chosen Approach:
      - Audit all Rust public functions added or changed by this phase. Keep `SduiObservableSnapshot` and `SduiStatusObservation` internal (`pub(crate)`). If any server-side function is made public and represents a user-facing capability, add Clay JS API Markdown docs, index link, and registry entry. Record the `pub(crate)` rationale for observable/status types in source comments. Update generated registry artifacts using the project registry update command.
    - API Notes and Examples:
      ```text
      # Verify registry is fresh after doc changes
      cargo run --bin update-doc-registry
      cargo test clay_js_doc_registry
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui.rs`: Rationale comment on `SduiObservableSnapshot` pub(crate) visibility.
      - `src/masonry_editor.rs`: Rationale comment on `SduiStatusObservation` pub(crate) visibility.
      - `docs/generated/`: Updated registry artifacts if any new Clay JS API Markdown docs are added.
      - `docs/reference/clay-js-api/sdui/` (if any new op is introduced): New Markdown doc file.
    - References:
      - `.agents/skills/create-plan/references/clay.md` (Clay JS API task)
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
  - Test Cases to Write:
    - `phase15_sdui_observability_surfaces_remain_internal`: Inventory/source coverage verifies `SduiObservableSnapshot`, `SduiNativeState::observable_snapshot`, `SduiStatusObservation`, and `EditorWidget::status_observation` remain `pub(crate)` and that the public `clay.sdui.*` inventory is still limited to the documented schema helpers plus `publishTree`.
    - `clay_js_doc_registry_is_fresh_after_phase15`: Running `cargo test --test clay_js_doc_registry` passes after verifying generated registry artifacts.
  - Verification:
    - `cargo fmt`
    - `cargo test --test clay_js_api_inventory`
    - `cargo test --test clay_js_doc_registry`
  - Findings:
    - No new Phase 15 public Clay JS API was required. The new observability surfaces are client/native test and agent-internal helpers, not server-side public programmatic capabilities.
    - `SduiObservableSnapshot` / `SduiNativeState::observable_snapshot` and `SduiStatusObservation` / `EditorWidget::status_observation` remain `pub(crate)` with source rationale comments.
    - Existing SDUI public APIs remain the documented `clay:sdui` schema helpers and `publishTree`; raw `Deno.core.ops.op_*` calls are still hidden behind `runtime/js/sdui.ts`.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: This phase does not introduce new user-facing SDUI layout/panel visibility configuration APIs (deferred per roadmap until real package or workspace panel settings exist); this deferral is confirmed explicitly by reviewing current SDUI configuration API docs.
    - Functional: Any SDUI-adjacent configuration surface introduced or changed by this phase (e.g., a developer opt-in for observability reporting) is documented as a Clay JS API with `user_facing_name`, key bindings, custom properties, Markdown doc, `docs/index.md` link, and generated registry entry.
    - Functional: The `cargo test` coverage gates that detect undocumented configuration APIs or behavior-changing settings missing from `custom_properties` remain passing.
    - Performance: No configuration API introduces synchronous server work on the typing hot path.
    - Code Quality: Every configuration option is a Clay JS API, not an undocumented config key in `~/.config/clay/init.js`.
    - Security: Configuration APIs preserve the no-authority-by-default model; no filesystem, network, shell, or extension-loading authority is implicitly granted.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay configuration task requirements.
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `roadmap.md` Phase 15: "Keep documented SDUI layout/panel visibility configuration APIs deferred until real user-facing package or workspace panel settings exist."
    - Options Considered:
      - Introduce an opt-in observability configuration API now: Premature; the observability types are `pub(crate)` test infrastructure, not user-facing settings.
      - Explicitly confirm deferral and leave no configuration gaps: Correct approach.
    - Chosen Approach:
      - Confirm existing SDUI configuration API docs in `docs/reference/clay-js-api/sdui/` cover the current published public surface. Confirm SDUI layout/panel visibility APIs remain deferred. If any new user-configurable behavior is added by this phase, add a Clay JS configuration API with full docs before the phase is considered complete.
    - API Notes and Examples:
      ```text
      docs/reference/clay-js-api/sdui/publish-tree.md    # existing
      # No new panel-visibility config API added in Phase 15
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/sdui/` (only if a new configurable behavior is introduced): New Markdown config API doc.
      - `docs/generated/`: Updated registry artifacts if any new config API is added.
    - References:
      - `.agents/skills/create-plan/references/clay.md` (Clay configuration task)
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - `clay_js_api_configuration_coverage_gates_pass`: `cargo test` coverage gates detecting undocumented config APIs continue to pass after Phase 15 changes.
  - Verification:
    - `cargo fmt --check`
    - `cargo test --test clay_js_api_inventory`
    - `cargo test --test clay_js_doc_registry`
  - Findings:
    - No new Phase 15 user-facing SDUI layout/panel visibility configuration API was introduced; roadmap deferral remains in effect until real package or workspace panel settings exist.
    - Current SDUI configuration-adjacent public surface remains the documented `clay:sdui` schema helpers and `publishTree` facade in `docs/reference/clay-js-api/sdui/`; no observability opt-in or panel-visibility setting was added.
    - Configuration coverage gates passed: inventory/docs/index consistency, required key binding/custom property metadata, generated registry freshness, configuration authority-denial checks, and Phase 15 internal SDUI observability inventory all remain valid.
    - No generated registry artifacts changed because no new configuration API documentation was added.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages for `masonry_sdui`, `masonry_editor` observability, and the new `ui-observability.md` developer guide.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/server-driven-ui.md
      docs/wiki/modules/masonry-editor.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for `masonry_sdui` observability and `masonry_editor` status observation changes.
      - `docs/wiki/modules/server-driven-ui.md`: Document `SduiObservableSnapshot`, `SduiNativeState.observable_snapshot`, `SduiNativeState` accessibility implementation, and SDUI payload budget validation.
      - `docs/wiki/modules/masonry-editor.md`: Document `SduiStatusObservation` and `EditorWidget.status_observation`.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Verification:
    - Reviewed `docs/wiki/index.md`, `docs/wiki/modules/server-driven-ui.md`, and `docs/wiki/modules/masonry-editor.md` against the implemented Phase 15 source paths.
    - Updated `docs/wiki/modules/server-driven-ui.md` to align the observability security statement with the actual list-item ID/label fields and to include `sdui_update_payload_stays_under_initial_budget` in the codec test list.
    - Confirmed by text search that the master wiki index links the server-driven UI and Masonry editor observability pages and that `docs/index.md` links the UI observability developer guide.

## Compromises Made
- Pixel-buffer visual snapshots remain deferred because Phase 15 intentionally uses deterministic headless structural snapshots until Masonry/winit headless rendering support is practical.
- No public Clay JS observability or SDUI panel-visibility configuration APIs were added; `SduiObservableSnapshot` and `SduiStatusObservation` remain `pub(crate)` internal test/agent surfaces.

## Further Actions
- Revisit GPU-backed pixel snapshot coverage when the rendering stack supports deterministic CI-friendly offscreen captures.
- Revisit public SDUI/query observability and panel configuration APIs only when Phase 17 package-owned UI creates real user-facing requirements.
