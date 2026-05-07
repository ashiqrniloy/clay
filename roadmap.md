# Clay Implementation Roadmap

## Current Status

Clay has proven the native client foundation: Masonry owns the native window/widget boundary, Vello renders the scene, Parley lays out text, and a local `crop` rope backs editable text state. The editor now supports minimally real local interaction: cursor movement, click-to-place caret, drag selection, selected-range editing, Unicode-safe scalar movement, viewport-bounded extraction, layout caching, scrolling, resize, and Phase 2 manual GUI smoke testing.

The next architectural priority is to make Clay self-documenting before introducing the client/server boundary. The approved document/behavior authority decision is recorded in `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`.

## Architectural Decisions Now Locked

### Document and Behavior Authority

Clay uses **server-authoritative documents with optimistic client shadows and server-issued client-executed behavior manifests**.

- The server owns canonical document state, document versions, edit transactions, file/workspace authority, extension execution, behavior definitions, leases, locks, and eventual AI/tool orchestration.
- The client owns rendering, input handling, viewport/caret/selection transient state, local shadow ropes, pending edit queues, and execution of server-issued hot-path behavior manifests.
- Each document has one editable lease at a time. Additional clients opening the same document are read-only observers until the lease is released or transferred.
- Ordinary predictable text edits are client-first and asynchronous to the server.
- Commands with file, workspace, extension, AI, shell, or unknown side effects are server-first.
- JavaScript extensions run on the server and produce versioned behavior manifests; the Rust client does not execute arbitrary JavaScript.
- Hot reload means publishing a new behavior manifest version and atomically installing it on clients.
- WASM client behavior modules remain a future option for sandboxed hot-path extension behavior, but are not part of the immediate IPC phase.

### Performance Rules

- No ordinary keypress may require a synchronous IPC -> Rust server -> `deno_core`/V8 -> Rust server -> IPC path before the client can update the visible editor.
- No full-document IPC for normal edits.
- No blocking IPC or server work in Masonry paint/text-event handlers.
- Per-document edit ordering is required; global serialization of all documents is not acceptable.
- UI-reactive server work such as completion, diagnostics, hover, and inline AI suggestions must be asynchronous, cancellable, and prioritized separately from background work.

### Documentation as Code Requirement

Clay must become a **self-documenting program** as early as possible. Documentation is not optional supporting prose; it is part of the code contract and must be inspectable by both users and AI agents.

This means:

- Public modules, protocol messages, commands, behavior manifest entries, permissions, server APIs, client APIs, and extension APIs must carry machine-readable and human-readable documentation.
- Documentation must be generated or validated from source-of-truth code/metadata where practical.
- Undocumented public protocol/API/command/manifest surfaces should fail tests or CI once the documentation contract is introduced.
- AI agents must be able to query the app's available commands, tools, permissions, protocol concepts, behavior manifests, and extension APIs from structured docs rather than guessing from source code.
- User-facing help, extension author docs, and agent tool descriptions should be generated from the same registry/metadata to prevent drift.

## Phase 1: Text Canvas Foundation — Complete

Stabilize the Phase 0 prototype into a maintainable native text canvas module before adding server complexity.

Focus areas:

- Separate buffer, viewport, layout, painting, and widget responsibilities.
- Replace whole-buffer visible text assumptions with viewport/range-based text extraction.
- Add viewport state for scroll offset and visible line/window bounds.
- Introduce layout dirty-state tracking so Parley layout is rebuilt only when text, width, viewport, or font state changes.
- Preserve the single-process prototype while preparing the client shadow-state boundary needed by future IPC.

Expected outcome:

- The client handles larger buffers without whole-document rendering assumptions.
- The editor surface has explicit state boundaries that can later receive server-provided document slices.
- Masonry remains the owner of native event/widget/render lifecycle.

## Phase 2: Editor Interaction Model — Complete

Move from append/backspace demo behavior to a minimal real editor interaction model.

Focus areas:

- Cursor model.
- Hit-testing from pointer position to text offset.
- Insert/delete at cursor.
- Newline handling.
- Basic selection model and drag selection.
- Keyboard navigation: arrows, Home/End, and basic scrolling behavior.
- Unicode-safe offset movement tests.

Expected outcome:

- Clay has a minimally usable native editor surface.
- The client owns high-frequency local interaction state required for optimistic editing.
- Local edits are represented as byte-offset/range operations suitable for protocol messages.

## Phase 3: Self-Documenting Program Contract

Introduce documentation-as-code before Clay exposes large protocol, server, command, behavior, and extension surfaces.

Focus areas:

- Define a documentation registry model for public Clay concepts: protocol messages, commands, permissions, behavior manifest entries, server APIs, client APIs, and future extension APIs.
- Decide where documentation metadata lives: Rust attributes/macros, structured TOML/JSON/RON/YAML files, generated Markdown, or a combination.
- Add a `docs/` or `reference/` source layout that is easy for humans and AI agents to inspect.
- Add tests that fail when registered public protocol/API/command/manifest entries lack required documentation fields.
- Generate a machine-readable index for agents and a human-readable Markdown reference from the same source.
- Establish a rule that new public surfaces must include documentation in the same change that introduces them.

Expected outcome:

- Clay has a strict, testable documentation contract before extension APIs and server-driven behavior multiply.
- AI agents can inspect available commands, protocol concepts, behavior manifest capabilities, and permissions from structured documentation.
- Human docs and agent-readable docs share a source of truth.

## Phase 4: IPC Client/Server Skeleton

Introduce the Thick Client / Asynchronous Server architecture without solving full synchronization yet.

Focus areas:

- Scaffold an async Rust server using Tokio.
- Keep the Masonry/Vello client separate from the server boundary.
- Add a local IPC transport abstraction.
- Start with Unix Domain Sockets on Linux/macOS; leave Windows named-pipe support behind the transport abstraction.
- Define initial lifecycle messages: connect, welcome, initial document snapshot, minimal behavior manifest, client edit event, acknowledgement or simple edit transaction, and error.
- Use `rkyv` early for protocol encoding, but keep it behind a narrow codec boundary.
- Validate received archived payloads before access and treat local IPC bytes as fallible input.
- Include final-compatible protocol metadata where practical: document ID, client ID, editable/read-only access state, base document version, server version, transaction ID, and behavior version.
- Keep the Phase 4 protocol intentionally small; do not make `rkyv` performance proving the phase's main goal.
- Preserve a benchmark/swap point around the codec so future measurements can compare message shapes and payload sizes.

Expected outcome:

- Client and server run as separate architectural units.
- Server owns a canonical in-memory document placeholder rather than acting as a stateless behavior service.
- Server can send initial document state and a minimal behavior manifest.
- Client can apply manifest-declared client-first text edits immediately and send basic edit operations asynchronously.
- Protocol messages are `rkyv`-serializable and exchanged through a length-prefixed local IPC frame.
- Serialization remains isolated enough that Phase 5 synchronization work can evolve message semantics without broad UI/server rewrites.

## Phase 5: Versioned Text Synchronization and Leases

Implement the canonical/shadow text model described in `concept.md` and the approved authority decision.

Focus areas:

- Server owns canonical `crop` ropes.
- Client owns lightweight visible/shadow document state.
- Add enforced document version numbers.
- Add edit messages with base versions and behavior versions.
- Add server acknowledgements with confirmed versions and transaction IDs.
- Add stale-edit rejection and simple resync behavior.
- Add one-editable-client document leases and read-only observer clients.
- Introduce region lock data structures and basic enforcement.
- Preserve immediate local typing for manifest-declared client-first behavior.

Expected outcome:

- Local typing remains immediate while the server remains authoritative.
- Stale or conflicting edits are detectable.
- Duplicate clients cannot edit the same file simultaneously.
- The architecture can support future AI-driven edits safely.

## Phase 6: Behavior Manifest System

Make server-owned editor behavior executable on the client for hot-path latency without making the client authoritative.

Focus areas:

- Define the behavior manifest schema for keymaps, routing policies, indentation, tab handling, bracket/quote pairing, comment continuation, autocomplete triggers, and command declarations.
- Classify commands by routing policy: client-first predictable, client-first requiring acknowledgement, server-first, server-first with range/document/behavior/workspace lock, UI-reactive priority lane, or background.
- Install, version, diff, and atomically replace behavior manifests on clients.
- Add behavior-version validation to edit transactions.
- Keep manifests inert and declarative; no arbitrary JavaScript execution in the client.
- Add tests proving ordinary text editing does not wait for a server/JavaScript round trip.

Expected outcome:

- Hot-path editing behavior can be defined by the server and executed locally by the client.
- Auto-indent, Enter, Tab, and simple mode-specific behavior can be immediate without janky correction in normal cases.
- Server-first commands remain authoritative and safe.

## Phase 7: File and Workspace Server

Make Clay edit real files through the authoritative server model.

Focus areas:

- Server-side file loading and saving.
- Workspace root handling.
- Open document registry.
- Dirty state tracking.
- Save/reload behavior.
- Clear errors for file IO failures.
- Container/toolbox/distrobox-friendly environment and permission handling.
- No direct client filesystem authority.

Expected outcome:

- Clay can open, edit, and save files through the server.
- The client remains a canvas/view/input layer.
- The server is the only component that needs workspace filesystem permissions.

## Phase 8: Server-Driven UI

Evolve Clay beyond a text editor into a programmable native canvas.

Focus areas:

- Define an initial SDUI schema for panels, labels, buttons, lists, editor views, and layout containers.
- Let the server send declarative UI tree updates.
- Map SDUI payloads to native Masonry widgets.
- Start with static Rust-generated SDUI before introducing JavaScript-generated SDUI.
- Decide where `rkyv` becomes necessary based on measured payload costs.
- Integrate SDUI schema documentation into the self-documenting registry.

Expected outcome:

- The server can declaratively alter parts of the native client UI.
- Clay can host multiple native panels/views.
- UI capabilities are inspectable by users and AI agents through generated documentation.

## Phase 9: Embedded JavaScript Runtime

Add the `deno_core` extension brain after the client/server/document/manifest architecture is stable.

Focus areas:

- Embed `deno_core` on an isolated server-side runtime thread/task boundary.
- Evaluate `init.js`.
- Expose a small documented Rust API surface to JavaScript: create panel, open document, register command, register behavior manifest entries, mutate SDUI tree.
- Compile JavaScript extension registrations into behavior manifest updates.
- Add permissions before exposing filesystem, network, shell, AI, or workspace mutation APIs.
- Report runtime errors in the Clay UI.
- Keep JavaScript out of the ordinary typing critical path.

Expected outcome:

- Clay can be configured and extended through `init.js`.
- Extensions can create native UI through SDUI and define hot-path behavior through manifests.
- Extension APIs are constrained, permissioned, and documented for users and AI agents.

## Phase 10: Hot Reload and Behavior Update Semantics

Make runtime behavior changes safe and non-janky.

Focus areas:

- Watch or trigger extension reloads.
- Re-evaluate JavaScript on the server.
- Produce a new behavior manifest version.
- Send manifest diffs or snapshots to affected clients.
- Atomically install behavior versions on clients.
- Define grace, rejection, or lock semantics for edits made under stale behavior versions.
- Add behavior/range/document/workspace locks for AI or extension-driven behavior changes.

Expected outcome:

- Users and AI agents can modify behavior at runtime.
- Clients do not apply half-updated editing rules.
- Behavior changes are visible, documented, versioned, and reversible or recoverable.

## Phase 11: AI-Safe Mutation and Region Locks

Support AI-generated edits without corrupting user state.

Focus areas:

- Make region locks first-class.
- Require AI edit sessions to carry explicit document versions, behavior versions, ranges, and permission scopes.
- Add preview/apply/reject flows.
- Add conflict explanations.
- Consider transaction logs.
- Separate extension/agent permissions from direct user input.
- Lock only the needed scope: range, document, behavior, or workspace.

Expected outcome:

- AI agents can propose or apply changes safely.
- User edits and agent edits have explicit conflict boundaries.
- AI-visible tools and mutation capabilities are documented and inspectable.

## Phase 12: Remote, Container, and Multi-Client Hardening

Make the server/client split useful beyond local IPC.

Focus areas:

- Remote server connection over secure transport.
- Container/toolbox/distrobox server startup and discovery.
- SSL/TLS or SSH/tunnel strategy.
- Multiple clients connected to one server.
- Multiple documents open concurrently.
- Read-only observer behavior for duplicate opens.
- Server concurrency and per-document actor scaling.

Expected outcome:

- A host client can connect to a server running in a target development environment.
- Clay can support local, container, and remote editing without changing the client authority model.

## Phase 13: Product Hardening

Move from architectural prototype toward a daily-usable application.

Focus areas:

- Large-file benchmarks.
- Incremental Parley layout improvements.
- Viewport virtualization refinements.
- GPU/render profiling.
- Multi-document behavior.
- Accessibility improvements.
- IME/composition support.
- Clipboard support.
- Undo/redo.
- Theme system.
- Cross-platform polish.
- Documentation coverage gates for public APIs and user-facing features.

Expected outcome:

- Clay becomes a robust native programmable editor environment rather than only a proof of architecture.
- Performance remains bounded by available machine resources rather than avoidable architecture bottlenecks.
- Clay remains inspectable by users and AI agents as it grows.
