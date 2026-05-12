# Phase 6 Behavior Manifest System

## Objectives
- Replace the Phase 4/5 minimal text-editing manifest placeholder with a declarative, inert, versioned behavior manifest model owned by the server and executed by the client.
- Define manifest schema coverage for keymaps, command declarations, routing policies, indentation, tab handling, bracket/quote pairing, comment continuation, and autocomplete triggers.
- Add atomic manifest install/replacement on the client with behavior-version validation on edit transactions.
- Preserve Clay's authority boundaries: the server owns behavior definitions and canonical document validation; the client executes only server-issued declarative hot-path behavior.
- Prove ordinary text editing, Enter, Tab, and simple pairing behavior do not require a synchronous IPC, server, JavaScript, AI, file IO, or full-document round trip before the UI updates.

## Expected Outcome
- `BehaviorManifest` becomes a structured protocol/data model with stable manifest IDs or document scope, `behavior_version`, routing policy declarations, key bindings, command declarations, and declarative editor behavior sections.
- The server publishes initial and replacement manifests, increments behavior versions deterministically, validates incoming edit/intent messages against the active server behavior version, and rejects or resyncs stale behavior versions with explicit protocol outcomes.
- The client installs manifests atomically, keeps the previous manifest active until a full replacement/diff validates, routes hot-path key behavior through the installed manifest, and includes the active behavior version on edit transactions.
- Client-first predictable behavior covers ordinary insertion/deletion plus initial Enter indentation, Tab handling, bracket/quote pairing, and comment continuation as declarative rules rather than arbitrary JavaScript.
- Server-first and lock-requiring commands are declared and routed safely without executing side effects in the client.
- Tests cover manifest schema round trips, invalid manifest rejection, atomic replacement, behavior-version mismatch handling, routing policy classification, and non-blocking local edit behavior.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- No arbitrary JavaScript execution in the Rust client, no server-side `deno_core` runtime expansion, no file/workspace/shell/network/AI authority, no WASM behavior module execution, no package system, and no SDUI expansion are introduced in this phase.

## Tasks

- [x] Define the declarative behavior manifest schema and routing policy model
  - Acceptance Criteria:
    - Functional: Protocol/data types represent versioned manifests, keymaps, command declarations, routing policies, indentation, Tab behavior, bracket/quote pairing, comment continuation, autocomplete triggers, behavior scope, and validation errors.
    - Performance: Manifest payloads remain bounded and serializable as IPC messages; ordinary edit messages continue to send deltas and metadata instead of full documents.
    - Code Quality: Manifest semantics live outside codec/framing code, use explicit enums/structs instead of stringly typed routing decisions, and remain inert data with no client-side script hooks.
    - Security: The schema cannot encode arbitrary JavaScript, shell, filesystem, network, AI, or workspace side effects; permission-bearing commands are declared as server-first intents only.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 6: Define behavior manifest schema, routing policies, version/diff/atomic replacement, behavior-version validation, inert declarations, and non-blocking typing tests.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Server owns behavior definitions; client executes only inert, versioned manifests for latency-sensitive behavior.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns behavior definitions and canonical validation; client owns input handling and execution of server-issued hot-path manifests.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep protocol semantics separate from codec, include behavior version metadata, avoid full-document IPC and synchronous server/JS round trips.
      - Context7 `/rkyv/rkyv`: Derive `Archive`, `Serialize`, and `Deserialize`; validate archived bytes before access/deserialization for IPC payloads.
    - Options Considered:
      - Extend the existing `BehaviorCapability::ClientFirstTextEditing` enum minimally: small change, but too limited for keymaps, commands, and mode-specific editor behavior.
      - Add a comprehensive but inert manifest model now: larger schema, but it creates the contract needed for hot-path behavior, extensions, and later hot reload.
      - Embed scripts or expression language in manifests: flexible, but violates the no arbitrary client execution boundary.
    - Chosen Approach:
      - Introduce a structured declarative manifest module and protocol message shape with typed routing policies and behavior declarations. Keep behavior entries intentionally small and deterministic, and require future extension/JS phases to compile behavior into these inert declarations rather than sending executable code to the client.
    - API Notes and Examples:
      ```rust
      #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
      pub enum RoutingPolicy {
          ClientFirstPredictable,
          ClientFirstRequiresAck,
          ServerFirst,
          ServerFirstWithLock { lock_scope: LockScope },
          UiReactivePriority,
          Background,
      }

      pub struct BehaviorManifest {
          pub behavior_version: BehaviorVersion,
          pub keymaps: Vec<KeyBindingRule>,
          pub commands: Vec<CommandDeclaration>,
          pub editor_rules: EditorBehaviorRules,
      }
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: Replace or extend the minimal manifest structs with versioned manifest, command, routing, keymap, and rule types.
      - `src/behavior/mod.rs`: New module for manifest validation and rule helpers, if separating protocol DTOs from behavior semantics is useful.
      - `src/behavior/manifest.rs`: Tentative home for declarative manifest validation and constructors.
      - `src/lib.rs`: Export the new behavior module if needed by client/server tests.
      - `docs/wiki/modules/behavior-manifests.md`: Tentative implementation wiki page, updated in the final wiki task.
    - References:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`
  - Test Cases to Write:
    - `protocol_round_trips_behavior_manifest_schema`: Encodes/decodes a manifest with keymaps, commands, editor rules, and routing policies.
    - `manifest_rejects_executable_behavior_payloads`: Validation rejects script-like or unsupported executable entries.
    - `manifest_requires_unique_command_ids_and_key_rules`: Validation catches duplicate command IDs or ambiguous keymap declarations.
    - `manifest_declares_all_routing_policy_variants`: Tests every Phase 6 routing policy variant can be represented and serialized.

- [x] Implement server-side manifest ownership, publishing, and behavior-version validation
  - Acceptance Criteria:
    - Functional: The server owns the active behavior manifest, sends it during connection/initialization, can publish a replacement manifest with an incremented `behavior_version`, and rejects or requests resync for edits/intents carrying invalid behavior versions.
    - Performance: Manifest publication is independent from per-edit document mutation; behavior-version checks are constant-time metadata checks and do not inspect full documents.
    - Code Quality: Server manifest state is isolated from document rope mutation and codec code; version advancement is deterministic and covered by tests.
    - Security: Clients cannot choose behavior versions to bypass server validation, and server-first or lock-requiring commands remain server-authoritative intents rather than client-side side effects.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 6: Add behavior-version validation to edit transactions and make behavior definitions server-owned.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Edits carry `behavior_version`; hot reload publishes new manifest versions; stale behavior versions are handled by synchronization policy.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns extension execution and behavior definitions.
      - `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`: Phase 5 added behavior version metadata and `InvalidBehaviorVersion` rejection.
    - Options Considered:
      - Store behavior version only in protocol constants: simple, but cannot support replacement/hot reload semantics.
      - Store one global manifest in server connection state: adequate for Phase 6 and avoids premature per-language mode complexity.
      - Store per-document/per-mode manifests now: closer to final package behavior, but too broad before Phase 7/11/15 APIs exist.
    - Chosen Approach:
      - Add a server-owned manifest state object with one active default manifest for the current document flow. Publish it on connect and expose internal replacement helpers for tests/future hot reload, while keeping per-document or package-specific manifest selection deferred.
    - API Notes and Examples:
      ```rust
      if behavior_version != self.behavior_manifest.version() {
          return ServerMessage::EditRejected {
              document_id,
              transaction_id,
              reason: EditRejection::InvalidBehaviorVersion {
                  behavior_version,
                  server_behavior_version: self.behavior_manifest.version(),
              },
          };
      }
      ```
    - Files to Create/Edit:
      - `src/server/mod.rs`: Added server behavior state wiring beside canonical document state and shared it with connection tasks.
      - `src/server/connection.rs`: Sends the active server-owned manifest on connection and validates edit/intent behavior versions before applying document mutations.
      - `src/server/behavior.rs`: Added `ActiveBehaviorManifest` for validated server-owned manifest state, deterministic replacement version advancement, and mismatch rejection construction.
      - `src/protocol/mod.rs`: No new variant was needed; existing `ServerMessage::BehaviorManifest` and `EditRejection::InvalidBehaviorVersion` covered publication and rejection outcomes.
      - `docs/wiki/modules/behavior-manifests.md`: Updated implementation notes for server-owned manifest state and behavior-version validation.
      - `docs/wiki/flows/client-server-edit-ack.md`: Updated flow notes for handshake manifest publication and invalid behavior-version recovery.
    - References:
      - `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases Written:
    - `server_sends_minimal_behavior_manifest`: Client initialization receives the active manifest with the server behavior version after welcome and initial document.
    - `server_rejects_edit_with_stale_behavior_version_without_mutating_document`: Edit rejection includes client and server behavior versions, then a same-base active-version edit is accepted to prove canonical text/version were not mutated.
    - `server_acknowledges_insert_edit`: Matching behavior version continues through Phase 5 document validation and acknowledgement.
    - `server_publish_replacement_increments_behavior_version`: Replacement validates and advances version deterministically.
    - `server_rejects_invalid_replacement_without_advancing_behavior_version`: Invalid replacement does not swap state or advance the server behavior version.
    - `server_behavior_version_validation_reports_client_and_server_versions`: Version mismatch rejection carries explicit client/server behavior versions.

- [x] Add client-side atomic manifest installation and key routing
  - Acceptance Criteria:
    - Functional: The client keeps an active manifest, validates a received replacement before installing it, atomically swaps active behavior on success, retains the previous manifest on failure, and routes declared hot-path key behavior locally.
    - Performance: Key routing performs bounded local lookups and never performs synchronous IPC, JavaScript, AI, file IO, or full-document serialization before applying `ClientFirstPredictable` behavior.
    - Code Quality: Client manifest state is separate from Masonry paint handlers and from protocol codec details; key routing is deterministic and unit-testable without a GUI.
    - Security: Client execution is limited to declarative built-in behavior rules and server-first command intents; the manifest cannot grant filesystem, network, shell, extension loading, AI mutation, or workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Manifest updates are atomic from the client's point of view and only apply to hot-path declarative behavior.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: No IPC work in Masonry paint/text-event handlers; use bounded queues for outgoing client edits.
      - Context7 `/tokio-rs/tokio`: `mpsc::channel(capacity)` provides bounded queues/backpressure for async message passing.
      - `roadmap.md` Phase 6: Install, version, diff, and atomically replace behavior manifests on clients.
    - Options Considered:
      - Interpret key behavior directly inside `EditorSurface`: fast to wire, but mixes behavior policy with rendering/editor state.
      - Add a small `ClientBehaviorState`/router that `EditorSurface` consults for key behavior: clearer separation and testability.
      - Defer routing until JavaScript APIs exist: misses Phase 6's latency goal.
    - Chosen Approach:
      - Introduce a client behavior router that validates and installs manifests, exposes the active behavior version to `ClientEditQueue`, and maps key events/editor intents to either local predictable edit operations or server-first command intents.
    - API Notes and Examples:
      ```rust
      match router.route_key(&key_event, editor_context) {
          RoutedBehavior::ClientEdit(operation) => apply_locally_and_enqueue(operation),
          RoutedBehavior::ServerIntent(intent) => enqueue_intent(intent),
          RoutedBehavior::Unhandled => {}
      }
      ```
    - Files Created/Edited:
      - `src/client/mod.rs`: Validates handshake manifests, stores background client behavior state, processes manifest replacement messages, and emits install/rejection events.
      - `src/client/behavior.rs`: Added client manifest state with atomic replacement and key-routing classification for client-first edits and server-first intents.
      - `src/editor/surface.rs`: Uses the behavior router for hot-path key actions without blocking paint/input and rejects invalid direct manifest installs.
      - `src/masonry_editor.rs`: Routes character, Enter, and Tab text events through the installed manifest while preserving bounded non-blocking edit forwarding.
      - `docs/wiki/modules/behavior-manifests.md`: Documented client atomic installation and key routing.
      - `docs/wiki/flows/client-behavior-routing.md`: Added implementation flow page for client behavior routing.
      - `docs/wiki/flows/client-edit-emission.md`: Updated behavior-manifest-gated edit emission wording.
      - `docs/wiki/index.md`: Linked the client behavior routing flow page.
    - References:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - Context7 `/tokio-rs/tokio` bounded MPSC channel docs.
  - Test Cases Written:
    - `client_installs_valid_manifest_atomically`: Active version changes only after validation succeeds.
    - `client_keeps_previous_manifest_when_replacement_invalid`: Invalid replacement does not leave partial state.
    - `client_routes_client_first_key_without_ipc_wait`: Hot-path routing classifies a character key as a local edit without IPC waiting.
    - `client_routes_tab_from_manifest_rules`: Tab key routing uses the active manifest's configured tab behavior.
    - `client_routes_server_first_command_as_intent`: Server-first command routing returns an intent and does not encode a local edit.
    - `client_installs_behavior_manifest_replacement_event`: Runtime replacement manifests from the server emit successful install events.
    - `client_rejects_invalid_behavior_manifest_replacement_event`: Invalid runtime replacements emit rejection events and do not install.
    - `editor_routes_client_first_key_through_manifest`: Routed client-first keys mutate local text and carry the active behavior version.
    - `editor_routes_server_first_key_without_local_mutation`: Routed server-first keys do not mutate local text before server response.

- [x] Implement initial declarative editor behavior rules for Enter, Tab, pairing, comments, and autocomplete triggers
  - Acceptance Criteria:
    - Functional: Manifest-declared rules support ordinary insertion/deletion, newline indentation, Tab insertion or indentation, bracket/quote pairing, comment continuation, and autocomplete trigger declaration for later UI-reactive handling.
    - Performance: Rule evaluation uses current visible/shadow editor context and small local slices; it does not serialize full documents or call the server synchronously for normal cases.
    - Code Quality: Rules are deterministic, Unicode-safe for byte-offset operations, covered by focused unit tests, and separated from future package/mode loading concerns.
    - Security: Autocomplete triggers and server-first commands are declarations only; they do not run extension code, AI tools, filesystem operations, or network calls on the client.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 6: Include indentation, tab handling, bracket/quote pairing, comment continuation, autocomplete triggers, and tests proving ordinary editing avoids server/JS round trips.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Use manifests for predictable immediate editor behavior and UI-reactive triggers; do not use manifests for arbitrary JS or side effects.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Prefer deltas, bounded queues, and non-blocking UI behavior.
    - Options Considered:
      - Implement rich language-specific indentation now: useful, but too broad before package/mode phases.
      - Implement small default text-mode rules: enough to prove the manifest execution model and keep Phase 6 bounded.
      - Treat autocomplete as a completed UI feature now: premature before UI-reactive server work and API inventory.
    - Chosen Approach:
      - Add a default text behavior manifest with simple deterministic rules: preserve leading indentation on Enter, configurable spaces-per-tab insertion, common bracket/quote pair insertion around selection or caret, line-comment continuation for simple prefixes, and trigger declarations that enqueue/categorize future UI-reactive work without implementing completion UI.
    - API Notes and Examples:
      ```rust
      pub struct EditorBehaviorRules {
          pub enter: EnterRule,
          pub tab: TabRule,
          pub pairs: Vec<PairRule>,
          pub comments: Vec<CommentContinuationRule>,
          pub autocomplete_triggers: Vec<AutocompleteTrigger>,
      }
      ```
    - Files Created/Edited:
      - `src/protocol/mod.rs`: Existing default manifest constructors and editor rule data structures provide default Enter, Tab, pair, comment, and autocomplete declarations.
      - `src/client/behavior.rs`: Added inert autocomplete trigger classification while preserving ordinary insertion and Tab routing.
      - `src/editor/surface.rs`: Executes installed manifest rules for Enter indentation/comment continuation, configured Tab insertion, and pair insertion/wrapping against the current cursor/selection.
      - `src/editor/buffer.rs`: Added bounded Unicode-safe text slice helpers for current-line and selected-range rule execution; removed the unused newline-only helper.
      - `docs/wiki/modules/behavior-manifests.md`: Documented initial declarative rule execution.
      - `docs/wiki/flows/client-behavior-routing.md`: Documented hot-path Enter/Tab/pair/comment routing and autocomplete trigger classification.
    - References:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases Written:
    - `enter_rule_preserves_leading_indentation`: Newline inserts expected indentation locally.
    - `tab_rule_inserts_configured_spaces`: Tab behavior follows manifest settings.
    - `pair_rule_wraps_selection_or_inserts_pair`: Bracket/quote rules behave predictably at caret and selection.
    - `comment_continuation_rule_continues_simple_comment_prefix`: Enter after a comment prefix continues the prefix.
    - `autocomplete_trigger_declared_without_client_side_side_effect`: Trigger classification does not run completion logic or mutate document state.

- [x] Verify manifest IPC, codec boundaries, and non-blocking hot-path behavior
  - Acceptance Criteria:
    - Functional: Manifest messages and behavior-version rejections round-trip through the existing length-prefixed `rkyv` codec; invalid archives and oversized frames remain rejected.
    - Performance: Tests demonstrate ordinary text insertion and manifest-declared hot-path rules complete locally even when IPC consumer is absent, slow, or backpressured.
    - Code Quality: Verification tests are deterministic and distinguish protocol serialization, server validation, client routing, and editor behavior concerns.
    - Security: Local IPC bytes remain fallible input; malformed manifests cannot panic the client/server or install partial behavior state.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Test codec round trips, oversized frame rejection, invalid archive rejection, behavior manifest round trips, and non-blocking editor behavior.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer automated deterministic checks for workflow-maintained artifacts.
      - Context7 `/rkyv/rkyv`: Use derives and validated access/deserialization for archived bytes.
      - Context7 `/tokio-rs/tokio`: Bounded MPSC channels provide backpressure; avoid awaiting channel sends on input hot path.
    - Options Considered:
      - Rely on manual GUI smoke testing only: insufficient for protocol and hot-path regression coverage.
      - Add focused unit/integration tests around manifest routing and codec paths: chosen because Phase 6 changes are mostly data/state-machine behavior.
      - Add full GUI automation: valuable later, but likely too heavy for this phase.
    - Chosen Approach:
      - Extend existing protocol/client/server tests with manifest fixtures, malformed frame tests, and non-blocking queue tests. Use manual GUI smoke testing only as supplemental verification.
    - API Notes and Examples:
      ```bash
      cargo fmt
      cargo test --quiet
      cargo check --quiet
      ```
    - Files Created/Edited:
      - `src/protocol/codec.rs`: Added codec tests for behavior manifest update payloads, behavior-version rejection payloads, invalid server/manifest archive bytes, and oversized manifest messages.
      - `src/protocol/mod.rs`: Existing protocol message variants covered manifest update/rejection serialization; no schema change was needed.
      - `src/client/mod.rs`: Added a full outbound queue hot-path test proving edit enqueue fails immediately through `try_send` rather than awaiting capacity.
      - `src/editor/surface.rs`: Added a manifest-declared ordinary typing test proving local mutation completes without server/JavaScript work.
      - `src/server/behavior.rs` and `src/server/connection.rs`: Existing behavior-version validation tests already covered explicit mismatch rejection and no-mutation server behavior.
      - `docs/wiki/modules/protocol-codec.md`: Documented manifest update/rejection codec coverage and malformed/oversized frame tests.
      - `docs/wiki/modules/behavior-manifests.md`: Documented non-blocking full-queue behavior and updated test coverage.
      - `docs/wiki/flows/client-edit-emission.md`: Documented the full-queue non-blocking edit emission test.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases Written:
    - `codec_round_trips_behavior_manifest_update`: Manifest update survives encode/decode through the codec boundary.
    - `codec_round_trips_behavior_version_rejection`: Behavior-version mismatch rejection metadata survives encode/decode through the codec boundary.
    - `codec_rejects_invalid_manifest_archive_bytes`: Malformed server/manifest bytes fail validation/deserialization.
    - `codec_rejects_oversized_manifest_frame`: Frame length bounds still apply to manifests.
    - `client_hot_path_does_not_await_full_ipc_queue`: Local edit enqueue returns without awaiting a full outbound queue.
    - `ordinary_typing_does_not_wait_for_server_or_javascript`: Manifest-declared character insertion mutates local shadow state before any server or JavaScript work.
    - Existing `server_behavior_version_validation_reports_client_and_server_versions` and `server_rejects_edit_with_stale_behavior_version_without_mutating_document`: Server validation still rejects stale behavior versions explicitly without canonical mutation.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: The phase implementation is reviewed for public programmatic behavior and server-side Rust public functions; required Clay JS APIs for behavior manifests, key binding inspection, command declarations, and future extension/configuration hooks are created or explicitly deferred to Phase 7 inventory with documented rationale.
    - Performance: Documentation/registry checks add no runtime hot-path work, and any generated registry validation runs only in tests or maintenance commands.
    - Code Quality: Public server-side Rust functions are either private/`pub(crate)` or mapped to explicit future/current `deno_core` op wrappers, stable Clay JS/TS facade APIs, Markdown docs, generated registry entries, and lookup coverage.
    - Security: Docs identify that manifests are inert, client JS execution is not introduced, and permission-bearing commands remain server-authoritative; raw `Deno.core.ops.op_*` calls are not the user-facing API.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Every Clay plan must include a Clay JS API verification task.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown docs are authoritative and public behavior is exposed through Clay JS APIs.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public server Rust functions require op wrappers and stable JS/TS facades, or should be private/`pub(crate)`.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Use domain modules, lower-camel exports, stable registry IDs, and user-facing names.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Include stable ID, module/export, user-facing name, key bindings, custom properties, permissions, and lookup tags.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Coverage tests must fail for missing docs/index/registry/lookup metadata.
    - Options Considered:
      - Implement full Clay JS behavior APIs during Phase 6: could be premature before Phase 7 API structure/inventory.
      - Make all new manifest internals private and defer public facade creation to Phase 7: likely appropriate unless Phase 6 introduces deliberate public programmatic behavior.
      - Add provisional docs for future APIs only: useful if public behavior is intentionally exposed now, but should avoid inventing APIs ahead of the Phase 7 inventory.
    - Chosen Approach:
      - Audit the implementation. Prefer `pub(crate)` for internal Rust manifest/server/client helpers in Phase 6. If any public programmatic behavior is intentionally introduced, document it through Clay JS API Markdown and registry paths; otherwise record the deferral to Phase 7 in implementation notes/tests without weakening the documentation-as-code contract.
    - API Notes and Examples:
      ```text
      JS module: clay:behavior
      Potential export: serverPublishManifest / inspectManifest
      Stable ID: clay.behavior.serverPublishManifest
      User-facing name: Publish Behavior Manifest
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: Add or update API docs only for intentionally public APIs introduced in Phase 6.
      - `docs/index.md`: Link any new API docs.
      - `src/**`: Make non-public Rust helpers private or `pub(crate)` where possible.
      - `src/docs/**` or registry generator paths: Tentative; update only if current registry tooling exists in this branch.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - `server_behavior_manifest_helpers_are_not_unintentionally_public`: Review/test visibility where practical.
    - `clay_js_api_docs_cover_public_behavior_manifest_api`: If public APIs are added, docs/index/registry tests fail when metadata is missing.
    - `raw_deno_ops_are_not_user_facing_behavior_api`: If op wrappers are added, facade docs remain the public surface.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Behavior-changing settings introduced by Phase 6, such as Tab width, pair insertion, comment continuation, key binding rules, routing policy declarations, and autocomplete triggers, are reviewed as configuration candidates and documented as Clay JS API configuration surfaces or explicitly deferred to Phase 8/Phase 7 with rationale.
    - Performance: Configuration metadata and registry validation do not run on the editor hot path; runtime manifest evaluation uses already-installed declarative settings.
    - Code Quality: Every implemented configuration option has user-facing name metadata, key binding metadata or an empty list, custom properties for behavior-changing fields, Markdown docs, master-index links, generated registry coverage, and tests/checks.
    - Security: Configuration does not implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace authority, or arbitrary client-side JavaScript execution.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Plans changing user-visible behavior must include a configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration starts at `~/.config/clay/init.js`; each option is a Clay JS API, not an undocumented key.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Configuration APIs require custom properties and key binding metadata.
      - `roadmap.md` Phase 8: Runtime configuration foundation comes later, but Phase 6 behavior settings should preserve the future configuration contract.
    - Options Considered:
      - Hardcode all Phase 6 default behavior with no configuration notes: fastest, but hides behavior-changing settings from the future API inventory.
      - Implement full `init.js` configuration loading now: violates phase boundaries and introduces server-side JS runtime work too early.
      - Define defaults in manifests and document/configure the public surface later: acceptable if deferral is explicit and fields are structured for future Clay JS configuration APIs.
    - Chosen Approach:
      - Keep runtime configuration loading deferred. Ensure manifest defaults are structured as future `clay:configuration`/`clay:behavior` APIs, record candidate configuration surfaces, and implement docs/registry/tests only for any configuration API intentionally exposed during Phase 6.
    - API Notes and Examples:
      ```ts
      // Future Phase 8-style configuration shape; not client-side execution.
      import { configureTabBehavior } from "clay:behavior";

      configureTabBehavior({ spacesPerTab: 4, insertSpaces: true });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: Add configuration API docs only if APIs are intentionally introduced now.
      - `docs/index.md`: Link any new configuration docs.
      - `src/behavior/manifest.rs`: Keep defaults and configurable fields explicit in manifest data structures.
      - `roadmap.md` or follow-up notes: No edit expected unless implementation discovers a needed phase-boundary correction.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `roadmap.md` Phase 8.
  - Test Cases to Write:
    - `behavior_manifest_defaults_expose_configurable_fields_structurally`: Defaults for tab/pair/comment behavior are represented in manifest data, not hidden in ad hoc code.
    - `configuration_docs_cover_exposed_behavior_settings`: If configuration APIs are added, docs/index/registry tests fail when custom properties or key binding metadata are missing.
    - `configuration_does_not_grant_side_effect_authority`: Tests/docs verify behavior configuration cannot grant filesystem/network/shell/AI/workspace permission.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
      - `.agents/skills/create-plan/references/wiki-task.md`: Final wiki task template for Clay plans.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/behavior-manifests.md
      docs/wiki/flows/client-behavior-routing.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for behavior manifest implementation pages.
      - `docs/wiki/modules/behavior-manifests.md`: Document server-owned manifest state, schema validation, and client atomic installation.
      - `docs/wiki/flows/client-behavior-routing.md`: Tentative page for hot-path key routing and async edit emission.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made
- Phase 6 uses one server-owned default text-editing manifest for the current document flow instead of per-document, per-language, per-mode, or package-selected manifests. This keeps authority/version validation explicit while deferring selection and package integration to later API/package phases.
- Manifest replacement is implemented as validated whole-manifest publication with deterministic version advancement, not as manifest diffs or a full hot-reload distribution system. Invalid replacements retain the previous manifest atomically.
- Stale behavior versions are rejected with explicit client/server version metadata; richer stale-version recovery, correction, or resync policy remains outside this phase.
- Initial editor rules are intentionally small and deterministic: ordinary insertion, Enter indentation/comment continuation, configured Tab insertion, pair insertion/wrapping, and inert autocomplete trigger classification. Rich language-specific indentation, Markdown/list continuation breadth, completion UI, diagnostics, and extension-driven behavior are deferred.
- Server-first and lock-requiring commands are declared/routed as inert intents only; this phase does not implement side-effect execution, permission checks beyond preserving authority boundaries, or lock acquisition workflows for those commands.
- Behavior configuration is represented structurally in manifest data, but runtime `init.js` configuration loading and public Clay JS configuration APIs are not introduced in this phase.
- Public Clay JS API/registry work and final wiki verification remain unchecked planned-later work, so the implemented Rust behavior/server/client helpers stay internal or `pub(crate)` where practical instead of becoming a user-facing API surface now.

## Further Actions
- Complete the unchecked Clay JS API verification task later: audit public Rust items touched by behavior manifests, keep internals private/`pub(crate)`, and add documented Clay JS APIs/registry coverage only for intentionally public behavior-manifest surfaces.
- Complete the unchecked configuration API verification task later: decide which manifest fields become user-facing `init.js`/Clay JS configuration APIs, then document defaults, custom properties, key binding metadata, and security boundaries.
- Complete the unchecked code-wiki verification task later: ensure the master wiki index and behavior/protocol/client-routing pages reflect the final Phase 6 implementation and test coverage.
- Add per-document/per-mode/package manifest selection when the extension/package and API inventory phases define the owning APIs and provenance model.
- Add manifest diff/hot-reload distribution and richer stale-version recovery once synchronization policy and multi-client behavior update semantics are finalized.
- Extend declarative editor rules incrementally for language-aware indentation, Markdown/list continuation, richer comment behavior, and UI-reactive autocomplete/diagnostic flows while preserving the no-client-JavaScript/no-side-effect manifest boundary.
