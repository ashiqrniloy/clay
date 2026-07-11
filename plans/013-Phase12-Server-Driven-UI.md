# Phase 12: Server-Driven UI

## Objectives
- Evolve Clay from a single text-editor surface into a server-driven native UI canvas while preserving server-authoritative state and client-owned rendering/input hot paths.
- Define a minimal declarative SDUI tree for panels, labels, buttons, lists, editor views, and layout containers.
- Allow the server to publish static Rust-generated UI snapshots and bounded updates that the client maps to native Masonry widgets.
- Keep JavaScript-generated SDUI deferred to Phase 13 while documenting any public schema/helper APIs through the Clay JS API registry.
- Measure SDUI payload costs before expanding `rkyv` usage beyond the existing protocol codec boundary.

## Expected Outcome
- The server can declaratively alter parts of the native client UI through typed SDUI messages.
- The client can host multiple native panels/views, including at least one editor view backed by existing document state.
- Static Rust-generated SDUI is available before server-side JavaScript runtime wiring.
- SDUI schema/helpers exposed as public programmatic behavior are inspectable through Markdown Clay JS API docs, generated registry entries, and lookup tests.
- Ordinary typing, painting, scrolling, and text-event handling remain free of blocking IPC, JavaScript execution, full-document serialization, and server-driven layout computation.

## Phase 18.16.5 Typography Handoff

- Phase 12's server-declarative SDUI schema and client-owned native rendering boundary remain unchanged.
- Typography profiles are separate layout-affecting client state; Phase 18.16.5 must not add server-computed font geometry, package JavaScript, or full-document text to SDUI payloads.

## Tasks

- [x] Define the initial SDUI schema and authority boundaries
  - Acceptance Criteria:
    - Functional: A typed SDUI model represents stable node IDs, panels, labels, buttons, lists, editor views, flex/stack layout containers, document/editor bindings, and explicit user-event intents without arbitrary client-side script execution.
    - Performance: Schema fields support incremental tree replacement/update by stable node ID and avoid full-document text payloads for editor views.
    - Code Quality: SDUI types are isolated from codec mechanics, derive comparison/debug traits for tests, and keep client-transient Masonry widget state separate from server-owned declarative UI state.
    - Security: The schema carries no filesystem, network, shell, extension loading, AI mutation, remote listener, WASM, or client-side JavaScript authority; button/list actions are inert server-routed command intents.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 12: Server-Driven UI scope and expected outcome.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns canonical state/behavior definitions; client owns native rendering and transient UI state.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep protocol semantics separate from codec implementation and avoid blocking UI hot paths.
      - `src/protocol/mod.rs`: Existing protocol type style, IDs, versions, behavior manifests, and rkyv-derived messages.
      - Context7 `/websites/rs_rkyv`: `Archive`, `Serialize`, and `Deserialize` derives are the documented path for serializable payload types; byte validation/access remains explicit at the codec boundary.
    - Options Considered:
      - Model SDUI directly as Masonry widgets: fast to prototype, but couples server schema to native implementation details and weakens protocol testability.
      - Define a generic JSON-like tree: flexible, but less type-safe and harder to validate before Phase 13 JavaScript runtime exists.
      - Define a narrow Rust enum schema first: fits Phase 12, is testable, and can later gain JS facade helpers without changing client authority.
    - Chosen Approach:
      - Add a protocol-owned SDUI module/type family with declarative node enums, stable `SduiNodeId`, optional document/editor bindings, and server-routed `SduiActionIntent` values. Keep the schema inert and Rust-generated in this phase.
    - API Notes and Examples:
      ```rust
      pub enum SduiNodeKind {
          Panel { title: String, children: Vec<SduiNodeId> },
          Label { text: String },
          Button { label: String, action: SduiActionIntent },
          List { items: Vec<SduiListItem> },
          EditorView { binding: SduiEditorBinding },
          Flex { direction: SduiFlexDirection, children: Vec<SduiNodeId> },
          Stack { children: Vec<SduiNodeId> },
      }
      ```
    - Files to Create/Edit:
      - `src/protocol/sdui.rs`: New SDUI schema types and unit tests.
      - `src/protocol/mod.rs`: Re-export SDUI protocol types; wire messages remain deferred to the protocol-message task.
      - `docs/wiki/modules/server-driven-ui.md`: Added initial implementation wiki coverage for schema and authority boundaries.
      - `docs/wiki/index.md`: Linked the SDUI schema wiki page.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`
  - Test Cases to Write:
    - `sdui_schema_represents_initial_widget_kinds`: Builds panel/label/button/list/editor/flex nodes with stable IDs.
    - `sdui_editor_view_uses_document_binding_not_text_payload`: Editor views reference document IDs instead of embedding document text.
    - `sdui_actions_are_server_routed_intents`: Button/list actions contain command intent metadata only, not executable code.
    - `sdui_updates_target_stable_node_ids`: Tree updates target stable IDs for root replacement, node replacement, and node removal.
  - Verification:
    - Passed: `cargo fmt --check`
    - Passed: `cargo test sdui --quiet`

- [x] Add SDUI protocol messages and static server-generated UI snapshots
  - Acceptance Criteria:
    - Functional: The server sends an initial SDUI snapshot after connection/bootstrap and can publish bounded declarative UI tree updates with server/UI version metadata.
    - Performance: Initial SDUI snapshots are bounded, ordinary edits still use existing edit messages, and SDUI updates do not serialize full documents.
    - Code Quality: Protocol message variants remain versioned and testable; SDUI update generation lives behind server helpers rather than inside connection plumbing.
    - Security: The server validates outbound/static UI construction and inbound UI action intents; local IPC input remains fallible and codec-bounded.
  - Approach:
    - Documentation Reviewed:
      - `src/server/mod.rs`: Server owns `DocumentState`, `ActiveBehaviorManifest`, `WorkspaceState`, and connection spawning.
      - `src/server/connection.rs`: Existing handshake/bootstrap and client message handling location.
      - `src/protocol/codec.rs`: Length-prefixed codec boundary and bounded frame validation.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Include final-compatible metadata and use deltas/transactions instead of broad snapshots where practical.
      - Context7 `/websites/rs_rkyv`: Derive-based archived payloads are appropriate once measured costs justify binary encoding.
    - Options Considered:
      - Send SDUI as part of behavior manifests: reuses an existing message path, but conflates hot-path behavior rules with UI tree state.
      - Add SDUI messages as separate protocol variants: clearer ownership and update/version semantics.
      - Wait for Phase 13 JavaScript runtime: delays native canvas validation and couples two large changes.
    - Chosen Approach:
      - Add `SduiSnapshot` and `SduiUpdate` protocol variants with `ui_version`, `client_id`/document metadata where relevant, and static server-generated default UI tree helpers.
    - API Notes and Examples:
      ```rust
      ServerMessage::SduiSnapshot(SduiSnapshot {
          ui_version: 1,
          root_id,
          nodes,
      });
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: Added `SduiSnapshot`, `SduiUpdate`, and `SduiAction` protocol variants and metadata.
      - `src/server/sdui.rs`: Built static default UI snapshots, update validation, and action-intent validation helpers.
      - `src/server/mod.rs`: Owns shared static SDUI state for connections.
      - `src/server/connection.rs`: Sends initial SDUI snapshot after bootstrap and routes/validates SDUI action messages.
      - `src/protocol/codec.rs`: Added SDUI snapshot/update/action round-trip coverage.
      - `src/client/mod.rs`: Updated integration tests to consume the extra bootstrap SDUI snapshot.
      - `docs/wiki/modules/server-driven-ui.md`: Documented SDUI protocol messages, static snapshots, validation, and tests.
      - `docs/wiki/modules/protocol-codec.md`: Documented SDUI codec coverage.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documented SDUI bootstrap publication and action validation.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `src/protocol/mod.rs`
      - `src/server/connection.rs`
  - Test Cases to Write:
    - `server_sends_initial_sdui_snapshot_after_bootstrap`: Bootstrap includes a valid static UI tree without replacing document snapshot semantics.
    - `sdui_snapshot_codec_round_trips`: New SDUI protocol payloads survive encode/decode.
    - `sdui_update_and_action_codec_round_trip`: SDUI update and inbound action protocol payloads survive encode/decode.
    - `sdui_update_rejects_unknown_node_id`: Invalid updates fail with typed errors rather than panics.
    - `sdui_action_validation_rejects_unknown_command`: Invalid action commands fail with typed errors rather than panics.
  - Verification:
    - Passed: `cargo fmt --check`
    - Passed: `cargo test sdui --quiet`
    - Passed: `cargo test --all-targets --quiet`

- [x] Map SDUI payloads to native Masonry UI state
  - Acceptance Criteria:
    - Functional: The client materializes SDUI panels, labels, buttons, lists, editor views, and layout containers into visible native UI regions while preserving existing editor behavior.
    - Performance: SDUI updates are applied on the GUI event loop, avoid widget mutation from Tokio tasks, and do not perform IPC or server waits in Masonry paint/text-event handlers.
    - Code Quality: SDUI reconciliation is isolated from editor text mutation logic and has deterministic tests for node creation, update, removal, and editor-view reuse.
    - Security: Raw IPC bytes never enter the widget tree; decoded/validated SDUI messages become inert native widget state and user actions become typed server intents.
  - Approach:
    - Documentation Reviewed:
      - `src/masonry_editor.rs`: Current `EditorWidget`, status bar, `EditorAction`, and connection-event application model.
      - `src/main.rs`: Existing EventLoopProxy/driver bridge from client connection events into Masonry.
      - `docs/wiki/flows/client-server-edit-ack.md`: GUI event routing and client/server edit acknowledgement flow.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Client owns native rendering and transient UI state only.
      - Context7 `/dioxuslabs/taffy`: Taffy supports flex/grid layout trees and `compute_layout`; useful if Masonry layout containers need explicit layout calculation helpers.
    - Options Considered:
      - Generate concrete Masonry widget structs for every SDUI node immediately: native and efficient, but may require larger widget-tree refactors.
      - Keep one custom `SduiRootWidget` that paints/reconciles simple nodes: smaller first step and easier to validate before richer widgets.
      - Use Taffy directly for all layout: powerful, but Masonry already participates in layout; use only if custom SDUI container layout needs it.
    - Chosen Approach:
      - Introduce a minimal SDUI client state/reconciler and native container widget that can host an existing `EditorWidget` for editor-view nodes. Use the established GUI event bridge for SDUI snapshot/update events.
    - API Notes and Examples:
      ```rust
      match event {
          ClientConnectionEvent::SduiSnapshot(snapshot) => {
              self.sdui.apply_snapshot(snapshot);
              ctx.request_render();
          }
          ClientConnectionEvent::SduiUpdate(update) => self.sdui.apply_update(update)?,
          _ => self.editor.apply_connection_event(event),
      }
      ```
    - Files to Create/Edit:
      - `src/client/mod.rs`: Added SDUI connection events, background receiver handling, and bounded typed SDUI action enqueueing.
      - `src/masonry_sdui.rs`: Added native SDUI state/reconciliation, derived editor bindings, visible text inspection, paint-time action hit regions, and simple native panel/list/button painting.
      - `src/masonry_editor.rs`: Integrated `SduiNativeState` with `EditorWidget`, applied SDUI events on the GUI event path, preserved editor state across sibling UI updates, painted the server-driven region, and routed SDUI action hits as server intents.
      - `src/lib.rs`: Exported the new `masonry_sdui` module.
      - `docs/wiki/modules/server-driven-ui.md`: Documented native SDUI reconciliation, event routing, action emission, invariants, and tests.
      - `docs/wiki/flows/client-server-edit-ack.md`: Documented SDUI event delivery through the existing non-blocking GUI bridge.
      - `docs/wiki/index.md`: Updated wiki navigation descriptions for SDUI native mapping.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `docs/wiki/flows/client-server-edit-ack.md`
  - Test Cases to Write:
    - `sdui_snapshot_replaces_native_tree_state`: Applying a snapshot updates visible SDUI state deterministically.
    - `sdui_update_preserves_editor_document_state`: Updating sibling panels does not reset editor text/caret/version state.
    - `sdui_button_action_emits_server_intent`: Native button activation/action forwarding emits a typed intent instead of running local script.
    - `client_receives_sdui_snapshot_event`: The background client connection task forwards decoded SDUI snapshots as typed GUI events.
    - `stale_sdui_update_is_ignored`: Native reconciliation rejects mismatched base UI versions without deleting existing nodes.
  - Verification:
    - Passed: `cargo fmt --check`
    - Passed: `cargo test sdui --quiet`
    - Passed: `cargo test --all-targets --quiet`

- [x] Support multiple panels/views and editor-view composition
  - Acceptance Criteria:
    - Functional: The default static UI demonstrates at least two native regions, such as an editor view plus a side/status panel or list, and can update one region without replacing the whole editor.
    - Performance: Layout and rendering remain viewport-bounded for editor content; side-panel/list updates do not trigger full-document extraction or global serialization.
    - Code Quality: Panel/view identity is stable across updates, with clear ownership of document-bound editor state versus server-declarative surrounding UI state.
    - Security: Additional panels do not grant new workspace/file authority and cannot issue undocumented server commands.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 12 expected outcome: Clay can host multiple native panels/views.
      - `src/editor/surface.rs` and `src/masonry_editor.rs`: Existing editor state and widget behavior.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Existing snapshot bootstrap and client initial state assumptions.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Viewport-bounded rendering and no full-document IPC for ordinary edits.
    - Options Considered:
      - Implement arbitrary tab/split management now: useful later, but too broad for initial SDUI.
      - Demonstrate one editor plus one inert panel/list: proves composition while keeping document authority unchanged.
      - Build multi-document views now: belongs more naturally with later remote/multi-client hardening.
    - Chosen Approach:
      - Compose an SDUI root with a document-bound `EditorView` and one or more inert server-owned panels/lists. Keep multi-document editing out of scope unless needed for tests.
    - API Notes and Examples:
      ```rust
      SduiNodeKind::Flex {
          direction: SduiFlexDirection::Row,
          children: vec![sidebar_id, editor_id],
      }
      ```
    - Files to Create/Edit:
      - `src/server/sdui.rs`: Static default UI tree with editor and side/status regions; validation for editor views bound to the known open document.
      - `src/masonry_sdui.rs`: Editor-region helpers and tests for document-bound composed editor views.
      - `src/masonry_editor.rs`: Pointer composition guard so side-panel presses do not mutate editor state unless they hit declared SDUI actions.
      - `docs/development/launch-and-gui-smoke.md`: Added manual smoke expectations for visible multi-panel UI.
      - `docs/wiki/modules/server-driven-ui.md`: Documented multi-region composition, editor binding validation, and tests.
      - `docs/wiki/index.md`: Updated SDUI wiki navigation summary.
    - References:
      - `docs/wiki/modules/client-snapshot-bootstrap.md`
      - `src/editor/surface.rs`
      - `src/masonry_editor.rs`
  - Test Cases to Write:
    - `default_sdui_contains_editor_and_panel_regions`: Static server UI has at least an editor view and non-editor panel/list.
    - `side_panel_update_does_not_replace_editor_widget`: Applying a panel update preserves editor document/version state.
    - `editor_view_requires_known_document_binding`: Unknown document-bound editor views are rejected or rendered as a safe error placeholder.
    - `editor_region_is_bounded_when_document_bound_editor_view_is_present`: A known document-bound editor view creates a bounded editor region beside the side panel.
    - `unknown_editor_view_document_uses_safe_full_editor_region`: Unknown bindings do not claim the composed editor region.
  - Verification:
    - Passed: `cargo fmt --check`
    - Passed: `cargo test sdui --quiet`
    - Passed: `cargo test --all-targets --quiet`

- [x] Measure SDUI payload costs and decide scoped `rkyv` usage
  - Acceptance Criteria:
    - Functional: SDUI snapshot/update size and encode/decode costs are measured with representative static trees and documented in tests or developer notes.
    - Performance: Measurements compare snapshot versus update payloads and establish a threshold for when binary `rkyv` encoding is required versus when simpler construction paths are acceptable internally.
    - Code Quality: Bench/test helpers are deterministic, do not require GUI startup, and keep codec decisions behind the protocol codec boundary.
    - Security: Oversized or malformed SDUI frames remain rejected by bounded codec validation.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/websites/rs_rkyv`: `to_bytes`, `from_bytes`, byte validation/`CheckBytes`, and `Archive`/`Serialize`/`Deserialize` derives are the current documented serialization flow.
      - `src/protocol/codec.rs`: Existing bounded length-prefixed codec and validation tests.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Use rkyv behind a small codec boundary and validate archived bytes before access.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic checks for workflow-maintained artifacts.
    - Options Considered:
      - Encode all SDUI with `rkyv` immediately: consistent with current protocol, but may optimize before measuring payload costs.
      - Use ad hoc JSON for SDUI while prototyping: easier inspection, but introduces a second wire format and duplicate validation path.
      - Keep SDUI inside the existing codec and add measurements: preserves protocol consistency while documenting payload tradeoffs.
    - Chosen Approach:
      - Keep SDUI protocol variants compatible with the existing length-prefixed `rkyv` codec, add representative size/round-trip checks, and defer specialized SDUI compression or alternate wire formats until payloads exceed documented budgets.
      - Measured representative payloads, excluding the 4-byte length prefix: initial multi-region SDUI snapshot is 816 bytes against a 4 KiB budget; simple side-panel label update is 192 bytes against a 1 KiB budget and remains smaller than the equivalent snapshot.
      - Scoped `rkyv` decision: use `rkyv` only at the protocol codec boundary for SDUI wire messages; keep Masonry/client-native state and server helper construction on typed Rust structs, with no ad hoc JSON or second SDUI wire format in Phase 12.
    - API Notes and Examples:
      ```rust
      let bytes = Codec::default().encode(&ServerMessage::SduiSnapshot(snapshot))?;
      assert!(bytes.len() <= MAX_EXPECTED_INITIAL_SDUI_BYTES);
      ```
    - Files to Create/Edit:
      - `src/protocol/codec.rs`: Added representative SDUI payload budget checks, snapshot/update size comparison, and SDUI-specific oversized-frame coverage.
      - `src/protocol/sdui.rs`: Added representative tree/update builders for deterministic tests.
      - `docs/wiki/modules/server-driven-ui.md`: Documented measured payload sizes, thresholds, and scoped `rkyv` usage decision.
      - `docs/development/launch-and-gui-smoke.md`: Noted that SDUI payload diagnostics are unit-test validated rather than GUI-smoke visible.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - Context7 `/websites/rs_rkyv`
  - Test Cases to Write:
    - `sdui_snapshot_payload_stays_under_initial_budget`: Representative initial UI payload remains under a documented threshold.
    - `sdui_update_payload_smaller_than_snapshot_for_panel_change`: A simple panel/list update does not require resending the whole tree.
    - `oversized_sdui_frame_is_rejected`: Existing codec frame bounds reject oversized SDUI payloads.
  - Verification:
    - Passed: `cargo fmt --check`
    - Passed: `cargo test sdui --quiet`
    - Passed: `cargo test --all-targets --quiet`

- [x] Verify launch/smoke behavior for server-driven UI
  - Acceptance Criteria:
    - Functional: `cargo run -- smoke-gui` shows the static server-driven multi-region UI, connected/editable/read-only status still works, and ordinary editing still receives acknowledgements.
    - Performance: GUI smoke validation confirms SDUI event handling is asynchronous and no UI handler blocks on server/IPC/JavaScript.
    - Code Quality: Automated tests cover protocol/client/widget behavior; manual smoke steps are documented for native visual validation.
    - Security: Smoke mode remains local IPC only with no remote TCP listener, shell-mediated startup, JavaScript runtime execution, or extra filesystem authority.
  - Approach:
    - Documentation Reviewed:
      - `plans/012-Developer-Friendly-Launch-and-GUI-Smoke.md`: Current launch/smoke behavior, GUI status, and child server lifecycle.
      - `docs/development/launch-and-gui-smoke.md`: Developer-facing smoke commands and expectations.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer automated checks plus documented manual verification where GUI observation is required.
    - Options Considered:
      - Rely only on unit tests: misses visual layout and launch integration regressions.
      - Add full automated GUI screenshot testing now: valuable but likely too heavy before UI composition stabilizes.
      - Combine unit/protocol tests with documented `smoke-gui` validation: appropriate for this phase.
    - Chosen Approach:
      - Extend existing launch docs and tests to include SDUI connection events and visible multi-panel state, with manual smoke validation as the native GUI check.
    - API Notes and Examples:
      ```powershell
      cargo run -- smoke-gui
      cargo test --all-targets --quiet
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`: SDUI visual expectations and troubleshooting notes were already present and verified.
      - `src/main.rs`: Added `smoke_launch_routes_sdui_events_to_gui` to cover SDUI snapshot delivery through the launch GUI action bridge.
      - `src/client/mod.rs`: Verified existing `client_receives_sdui_snapshot_event` coverage for SDUI connection event delivery.
      - `docs/wiki/flows/client-server-edit-ack.md`: Updated the implementation wiki test inventory for SDUI GUI event routing.
    - References:
      - `plans/012-Developer-Friendly-Launch-and-GUI-Smoke.md`
      - `docs/development/launch-and-gui-smoke.md`
  - Test Cases to Write:
    - `client_receives_sdui_snapshot_event`: Connection task emits an SDUI GUI event after server snapshot receipt.
    - `smoke_launch_routes_sdui_events_to_gui`: Existing bridge routes SDUI events without blocking.
    - Manual smoke: Run `cargo run -- smoke-gui`, confirm editor plus server-driven panel/list render and editing acknowledgement/status still update.
  - Verification:
    - Passed: `cargo fmt --check`
    - Passed: `cargo test --all-targets --quiet` (213 library tests, 19 binary tests, and all other target suites passed)
    - Observed: bounded `timeout 8s cargo run -- smoke-gui` launch check reached managed local server startup, client connection, SDUI snapshot receipt through the GUI event bridge, and Masonry window creation before the harness timeout intentionally stopped the GUI; stderr showed the multi-region SDUI tree (`Workspace`, `Refresh`, list item, and document-bound `EditorView`) with no TCP listener, shell startup, JavaScript execution, or filesystem authority beyond the local managed endpoint.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Public SDUI schema/helper behavior introduced by this phase has planned or implemented Clay JS API docs, stable IDs, JS module/export names, user-facing names, lookup tags, and generated registry entries; non-public Rust helpers are private or `pub(crate)`.
    - Performance: Registry generation and lookup checks remain offline/test-time operations and add no work to editor input/paint paths.
    - Code Quality: All server-side Rust public functions introduced or changed by SDUI work are inventoried and either exposed through explicit future `deno_core` op wrappers plus stable facades or intentionally kept internal.
    - Security: SDUI API docs state that Phase 12 helpers create inert declarative UI only and do not grant filesystem, network, shell, extension loading, AI mutation, package, WASM, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API task for every Clay plan.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public surface is Clay JS/TS facade, not raw Rust or raw ops.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Apply module/export/stable-ID/user-facing-name naming layers.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required metadata including key bindings and custom properties.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown plus `docs/index.md` is authoritative.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Registry freshness and lookup coverage tests.
      - `docs/reference/clay-js-api/schema.md`: Required Markdown frontmatter/body sections.
    - Options Considered:
      - Skip JS API docs because runtime JavaScript is Phase 13: violates documentation-as-code and makes future SDUI APIs undiscoverable.
      - Document only broad SDUI concepts: useful, but insufficient for generated registry and agent lookup.
      - Add planned facade docs for schema helpers now: matches Phase 8/9 precedent while deferring runtime ops until Phase 13.
    - Chosen Approach:
      - Add planned Clay JS API docs for SDUI schema helpers that are exposed programmatically, likely under `clay:sdui`, with facades/op wrappers marked planned until Phase 13. Keep internal Rust reconciliation helpers out of the public registry.
    - API Notes and Examples:
      ```ts
      import { definePanel, defineEditorView } from "clay:sdui";

      const root = definePanel({
        title: "Workspace",
        children: [defineEditorView({ documentId })],
      });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/sdui/*.md`: Added planned SDUI schema/helper API docs for `definePanel`, `defineLabel`, `defineButton`, `defineList`, `defineEditorView`, `defineFlex`, and `defineStack`.
      - `docs/reference/clay-js-api/api-inventory.toml`: Added public planned SDUI helper inventory entries with facade/op/Rust mappings, hot-path notes, key binding/custom property metadata, and no-authority security notes.
      - `docs/index.md`: Linked every SDUI API doc under **Clay JS API Registry Source Files**.
      - `docs/generated/clay-js-api-registry.json`: Regenerated after Markdown docs changed.
      - `runtime/js/sdui.ts`: Added planned `clay:sdui` facade stub exports and shared SDUI TypeScript helper types.
      - `runtime/js/mod.ts`: Re-exported the SDUI facade namespace for source-tree organization.
      - `tests/clay_js_doc_registry.rs`: Added generated-registry lookup/security coverage for Phase 12 SDUI helpers.
      - `docs/wiki/modules/clay-js-facade-skeleton.md`: Documented the SDUI facade stub implementation.
      - `docs/wiki/modules/clay-js-doc-registry.md`: Documented SDUI generated-registry coverage.
      - `docs/wiki/modules/server-driven-ui.md`: Linked the public SDUI helper docs to the SDUI implementation wiki.
      - `docs/wiki/index.md`: Updated wiki navigation for SDUI facade/registry coverage.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - `cargo run --bin update-doc-registry`: Regenerates the registry after SDUI docs are linked.
    - `cargo test docs::registry --quiet`: Fails if SDUI API docs, metadata, index links, generated entries, or lookup coverage are missing/stale.
    - Public Rust inventory review: Confirm each new server-side public function is either covered by a Clay JS API doc/op/facade plan or made private/`pub(crate)`.
  - Verification:
    - Passed: `cargo run --bin update-doc-registry`
    - Passed: `cargo fmt --check`
    - Passed: `cargo test --test clay_js_doc_registry --quiet`
    - Passed: `cargo test --test clay_js_facade_layout --quiet`
    - Passed: `cargo test --test rust_visibility_api_mapping --quiet`
    - Passed: `cargo test --test clay_js_api_inventory --quiet`
    - Passed: `cargo test --all-targets --quiet`
    - Reviewed: SDUI server helpers introduced in Phase 12 are `pub(crate)`; no new server-side `pub` function required public Rust exposure. Public SDUI protocol DTOs remain wire/schema types documented behind planned `clay:sdui` facade helpers rather than direct raw Rust or op APIs.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Any behavior-changing SDUI customization exposed to users, such as default panel visibility, layout preferences, or key-bindable SDUI commands, is documented as a Clay JS API/configuration surface rather than an undocumented setting.
    - Performance: Configuration API docs and registry checks are offline/test-time; no configuration evaluation or JavaScript execution is introduced in Phase 12.
    - Code Quality: SDUI configuration docs include default key bindings or empty key binding lists, custom properties for every behavior-changing setting, examples, return/async behavior, errors, and lookup tags.
    - Security: Configuration does not implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration starts at `~/.config/clay/init.js` and every option is a Clay JS API.
      - `docs/reference/clay-js-api/configuration.md`: Current configuration contract and modular loading semantics.
      - `docs/index.md`: Master index and registry source list.
    - Options Considered:
      - Add no SDUI configuration in Phase 12: valid if the static UI has no user-visible behavior-changing settings, but must be explicitly verified.
      - Add ad hoc Rust constants for layout defaults: easy, but not discoverable or configurable later.
      - Document planned configuration APIs only for real settings: keeps the registry accurate without inventing unnecessary options.
    - Chosen Approach:
      - Reviewed final SDUI behavior for user-visible customization. No new Phase 12 SDUI configuration APIs are needed because the active UI is a static Rust-generated snapshot with no exposed user settings for default panel visibility, layout preferences, or key-bindable SDUI commands. Existing planned `clay:sdui` schema helper docs already carry empty default `key_bindings`, behavior-changing helper `custom_properties`, planned-runtime status, and no-authority security notes for future Phase 13 construction.
    - API Notes and Examples:
      ```ts
      // Future Phase 13 runtime example if an SDUI layout setting is introduced.
      import { configureDefaultPanels } from "clay:sdui";

      configureDefaultPanels({ sidebar: "visible" });
      ```
    - Files to Create/Edit:
      - `plans/013-Phase12-Server-Driven-UI.md`: Recorded the no-config review and verification result.
      - No new `docs/reference/clay-js-api/sdui/*.md`, `docs/reference/clay-js-api/configuration.md`, or `docs/index.md` changes were needed for this task because no behavior-changing SDUI configuration API was introduced.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/configuration.md`
  - Test Cases to Write:
    - Registry metadata check: SDUI configuration/custom properties are present when behavior-changing settings exist.
    - No-config review: If no SDUI configuration APIs are added, document why static Phase 12 UI has no user-configurable behavior yet.
    - `cargo test docs::registry --quiet`: Fails for missing key binding/custom property metadata on any added SDUI configuration API.
  - Verification:
    - Reviewed: `src/server/sdui.rs`, `src/protocol/sdui.rs`, `runtime/js/sdui.ts`, `docs/reference/clay-js-api/sdui/*.md`, `docs/reference/clay-js-api/configuration.md`, `docs/index.md`, `tests/clay_js_doc_registry.rs`, and `tests/clay_js_api_inventory.rs`.
    - Verified: Phase 12 active SDUI has no user-exposed default panel visibility, layout preference, or key-bindable SDUI command configuration; static sidebar/editor composition remains Rust-generated and server validated.
    - Verified: Planned `clay:sdui` helper docs already include empty default key binding lists, custom property metadata for behavior-changing helper options, lookup tags, planned-runtime status, and no-authority security notes; no additional configuration docs or registry source links are required.
    - Passed: `cargo run --bin update-doc-registry`
    - Passed: `cargo test --test clay_js_doc_registry --quiet`
    - Passed: `cargo test --test clay_js_api_inventory --quiet`

- [x] Run final verification for Phase 12
  - Acceptance Criteria:
    - Functional: SDUI schema, protocol, server snapshot generation, client native mapping, multi-panel UI, and public docs/registry behavior pass automated tests and manual smoke validation.
    - Performance: Tests or notes demonstrate bounded SDUI payloads and no regression to ordinary editing responsiveness.
    - Code Quality: Formatting, all-target tests, docs registry checks, and launch smoke documentation are current.
    - Security: Verification confirms no new remote listener, shell startup path, arbitrary client JavaScript, direct client filesystem authority, or undocumented permission-bearing API was introduced.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Deterministic validation for maintained artifacts.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Final authority, hot-path, docs-as-code, configuration, security, and phase-boundary checks.
    - Options Considered:
      - Validate only changed modules: faster, but SDUI crosses protocol/client/server/docs boundaries.
      - Run full relevant Rust test suite plus manual GUI smoke: slower, but appropriate for a cross-cutting UI/protocol phase.
    - Chosen Approach:
      - Run formatting, registry generation/check workflow, all-target tests, and a manual GUI smoke pass before marking implementation complete.
    - API Notes and Examples:
      ```powershell
      cargo fmt --check
      cargo test --all-targets --quiet
      cargo run --bin update-doc-registry
      cargo run -- smoke-gui
      ```
    - Files to Create/Edit:
      - `plans/013-Phase12-Server-Driven-UI.md`: Update task checkboxes, verification notes, compromises, and follow-up actions after execution.
      - `docs/development/launch-and-gui-smoke.md`: Update if final smoke behavior differs from planned expectations.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
  - Test Cases to Write:
    - Full suite: `cargo test --all-targets --quiet` passes.
    - Formatting: `cargo fmt --check` passes.
    - Registry freshness: Generated registry is current after SDUI docs are linked.
    - Manual smoke: `cargo run -- smoke-gui` renders SDUI and editing/status behavior works.
  - Verification:
    - Passed: `cargo fmt --check`
    - Passed: `cargo run --bin update-doc-registry`
    - Passed: `cargo test --all-targets --quiet` (213 library tests, 19 binary tests, and all other target suites passed)
    - Observed: bounded `timeout 8s cargo run -- smoke-gui` reached managed local IPC server startup, client connection, typed `SduiSnapshot` receipt, Masonry window creation, and native event dispatch before the timeout intentionally stopped the long-running GUI. The observed SDUI tree contained the `Workspace` panel, `Refresh` button, list item, and document-bound `EditorView`; startup used a local named pipe and showed no remote TCP listener, shell-mediated startup, JavaScript execution, or added filesystem authority.
    - Verified: final Phase 12 checks cover SDUI schema/protocol/server/client/widget/docs registry behavior, bounded payload tests from earlier tasks, non-blocking GUI event routing, static multi-panel UI smoke behavior, and the planned no-new-configuration/API authority boundaries.

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
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Verification:
    - Reviewed: `docs/wiki/index.md`, `docs/wiki/modules/server-driven-ui.md`, `docs/wiki/modules/protocol-codec.md`, `docs/wiki/modules/server-ipc-skeleton.md`, `docs/wiki/flows/client-server-edit-ack.md`, `docs/wiki/modules/clay-js-facade-skeleton.md`, and `docs/wiki/modules/clay-js-doc-registry.md`.
    - Verified: the master wiki index links every current wiki page, including Phase 12 SDUI implementation pages and related client/server/protocol/facade/registry pages.
    - Verified: the SDUI wiki coverage explains the changed source/test paths, server-owned declarative tree model, static snapshots, protocol messages, native Masonry reconciliation, multi-region editor/sidebar composition, payload budgets, `rkyv` codec scope, public planned `clay:sdui` helper docs, and security/authority boundaries.
    - Verified: no additional wiki edits were required because the implementation-specific wiki pages had already been updated during Phase 12 tasks and matched the final code/test inventory.
    - Passed: manual wiki link/index review using a script that checks all `docs/wiki/**/*.md` pages are linked from `docs/wiki/index.md`.

## Compromises Made
- Phase 12 keeps SDUI publication Rust-generated and static; JavaScript-generated SDUI remains deferred to Phase 13 behind planned `clay:sdui` facade docs and stubs.
- GUI smoke verification used a bounded launch observation instead of automated screenshot assertions; visual/manual smoke expectations remain documented in `docs/development/launch-and-gui-smoke.md`.
- SDUI payload optimization is limited to measured budget checks on the existing length-prefixed `rkyv` codec boundary; no specialized diff compression or alternate wire format was added.
- No Phase 12 SDUI configuration API was added because the active UI has no user-exposed layout/visibility settings yet.

## Further Actions
- Phase 13: wire JavaScript SDUI construction through explicit Clay JS facades and server-side validation, preserving the no-client-script-execution and server-authoritative boundaries.
- Medium priority: add automated visual/layout regression coverage if the native SDUI canvas grows beyond the current static editor/sidebar composition.
- Medium priority: revisit SDUI update compression or specialized payload shaping only if representative snapshots exceed 4 KiB, simple panel updates exceed 1 KiB, or updates stop being materially smaller than equivalent snapshots.
- Low priority: introduce documented SDUI configuration APIs only when real user-facing layout or panel-visibility settings are implemented.
