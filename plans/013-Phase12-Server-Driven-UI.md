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

- [ ] Add SDUI protocol messages and static server-generated UI snapshots
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
      - `src/protocol/mod.rs`: Add SDUI message variants and metadata.
      - `src/server/sdui.rs`: Build static default UI snapshots and validate action intents.
      - `src/server/mod.rs`: Own/share current server UI state if needed.
      - `src/server/connection.rs`: Send initial snapshot and route SDUI action messages.
      - `src/protocol/codec.rs`: Add round-trip tests only if new protocol variants require codec coverage.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `src/protocol/mod.rs`
      - `src/server/connection.rs`
  - Test Cases to Write:
    - `server_sends_initial_sdui_snapshot_after_welcome`: Bootstrap includes a valid static UI tree without replacing document snapshot semantics.
    - `sdui_snapshot_codec_round_trips`: New SDUI protocol payloads survive encode/decode.
    - `sdui_update_rejects_unknown_node_id`: Invalid updates/actions fail with typed errors rather than panics.

- [ ] Map SDUI payloads to native Masonry UI state
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
      - `src/client/mod.rs`: Add SDUI connection events and background receiver handling.
      - `src/masonry_editor.rs`: Add SDUI root/container state or integrate a new SDUI widget with `EditorWidget`.
      - `src/masonry_sdui.rs`: Tentative new module for SDUI native mapping if separation is cleaner.
      - `src/main.rs`: Route SDUI events through the existing GUI action bridge.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `docs/wiki/flows/client-server-edit-ack.md`
  - Test Cases to Write:
    - `sdui_snapshot_replaces_native_tree_state`: Applying a snapshot updates visible SDUI state deterministically.
    - `sdui_update_preserves_editor_document_state`: Updating sibling panels does not reset editor text/caret/version state.
    - `sdui_button_action_emits_server_intent`: Native button activation emits a typed intent instead of running local script.

- [ ] Support multiple panels/views and editor-view composition
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
      - `src/server/sdui.rs`: Static default UI tree with editor and side/status regions.
      - `src/masonry_sdui.rs` or `src/masonry_editor.rs`: Composition support for multiple native regions.
      - `docs/development/launch-and-gui-smoke.md`: Add manual smoke expectations for visible multi-panel UI.
    - References:
      - `docs/wiki/modules/client-snapshot-bootstrap.md`
      - `src/editor/surface.rs`
      - `src/masonry_editor.rs`
  - Test Cases to Write:
    - `default_sdui_contains_editor_and_panel_regions`: Static server UI has at least an editor view and non-editor panel/list.
    - `side_panel_update_does_not_replace_editor_widget`: Applying a panel update preserves editor document/version state.
    - `editor_view_requires_known_document_binding`: Unknown document-bound editor views are rejected or rendered as a safe error placeholder.

- [ ] Measure SDUI payload costs and decide scoped `rkyv` usage
  - Acceptance Criteria:
    - Functional: SDUI snapshot/update size and encode/decode costs are measured with representative static trees and documented in tests or developer notes.
    - Performance: Measurements compare snapshot versus update payloads and establish a threshold for when binary `rkyv` encoding is required versus when simpler construction paths are acceptable internally.
    - Code Quality: Bench/test helpers are deterministic, do not require GUI startup, and keep codec decisions behind the protocol codec boundary.
    - Security: Oversized or malformed SDUI frames remain rejected by bounded codec validation.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/websites/rs_rkyv`: `to_bytes`, `access`, `deserialize`, and derive macros are the current documented serialization flow.
      - `src/protocol/codec.rs`: Existing bounded length-prefixed codec and validation tests.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Use rkyv behind a small codec boundary and validate archived bytes before access.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic checks for workflow-maintained artifacts.
    - Options Considered:
      - Encode all SDUI with `rkyv` immediately: consistent with current protocol, but may optimize before measuring payload costs.
      - Use ad hoc JSON for SDUI while prototyping: easier inspection, but introduces a second wire format and duplicate validation path.
      - Keep SDUI inside the existing codec and add measurements: preserves protocol consistency while documenting payload tradeoffs.
    - Chosen Approach:
      - Keep SDUI protocol variants compatible with the existing `rkyv` codec, add representative size/round-trip checks, and document any decision to defer specialized diff compression until payloads justify it.
    - API Notes and Examples:
      ```rust
      let bytes = Codec::default().encode(&ServerMessage::SduiSnapshot(snapshot))?;
      assert!(bytes.len() <= MAX_EXPECTED_INITIAL_SDUI_BYTES);
      ```
    - Files to Create/Edit:
      - `src/protocol/codec.rs`: SDUI payload round-trip and oversized-frame coverage if not already generic.
      - `src/protocol/sdui.rs`: Representative tree builders for deterministic tests.
      - `docs/development/launch-and-gui-smoke.md`: Note expected payload/diagnostic behavior only if visible to developers.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - Context7 `/websites/rs_rkyv`
  - Test Cases to Write:
    - `sdui_snapshot_payload_stays_under_initial_budget`: Representative initial UI payload remains under a documented threshold.
    - `sdui_update_payload_smaller_than_snapshot_for_panel_change`: A simple panel/list update does not require resending the whole tree.
    - `oversized_sdui_frame_is_rejected`: Existing codec frame bounds reject oversized SDUI payloads.

- [ ] Verify launch/smoke behavior for server-driven UI
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
      - `docs/development/launch-and-gui-smoke.md`: Add SDUI visual expectations and troubleshooting notes.
      - `src/main.rs`: Add routing tests only if launch action/event plumbing changes.
      - `src/client/mod.rs`: Add tests for SDUI connection event delivery.
    - References:
      - `plans/012-Developer-Friendly-Launch-and-GUI-Smoke.md`
      - `docs/development/launch-and-gui-smoke.md`
  - Test Cases to Write:
    - `client_receives_sdui_snapshot_event`: Connection task emits an SDUI GUI event after server snapshot receipt.
    - `smoke_launch_routes_sdui_events_to_gui`: Existing bridge routes SDUI events without blocking.
    - Manual smoke: Run `cargo run -- smoke-gui`, confirm editor plus server-driven panel/list render and editing acknowledgement/status still update.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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
      - `docs/reference/clay-js-api/sdui/*.md`: Planned SDUI schema/helper API docs.
      - `docs/index.md`: Link every SDUI API doc under **Clay JS API Registry Source Files**.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after Markdown docs change.
      - `src/docs/registry.rs`: Add/adjust validation only if SDUI metadata needs new allowed values.
      - `runtime/js/sdui.ts`: Tentative planned facade stub path if the current facade tree is extended in this phase.
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

- [ ] Create or verify Clay configuration APIs
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
      - Review final SDUI behavior for user-visible customization. Add planned configuration API docs only for real behavior-changing settings; otherwise record that no new SDUI configuration APIs are needed for the static Rust-generated UI.
    - API Notes and Examples:
      ```ts
      // Future Phase 13 runtime example if an SDUI layout setting is introduced.
      import { configureDefaultPanels } from "clay:sdui";

      configureDefaultPanels({ sidebar: "visible" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/sdui/*.md`: Add configuration/custom property metadata where relevant.
      - `docs/reference/clay-js-api/configuration.md`: Link or mention SDUI configuration APIs only if introduced.
      - `docs/index.md`: Link any new configuration API docs under registry sources.
      - `docs/generated/clay-js-api-registry.json`: Regenerate if docs change.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/configuration.md`
  - Test Cases to Write:
    - Registry metadata check: SDUI configuration/custom properties are present when behavior-changing settings exist.
    - No-config review: If no SDUI configuration APIs are added, document why static Phase 12 UI has no user-configurable behavior yet.
    - `cargo test docs::registry --quiet`: Fails for missing key binding/custom property metadata on any added SDUI configuration API.

- [ ] Run final verification for Phase 12
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

- [ ] Update or verify the code wiki after implementation
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

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
