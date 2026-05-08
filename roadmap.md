# Clay Implementation Roadmap

## Current Status

Clay has proven the native client foundation: Masonry owns the native window/widget boundary, Vello renders the scene, Parley lays out text, and a local `crop` rope backs editable text state. The editor now supports minimally real local interaction: cursor movement, click-to-place caret, drag selection, selected-range editing, Unicode-safe scalar movement, viewport-bounded extraction, layout caching, scrolling, resize, and Phase 2 manual GUI smoke testing.

The next architectural priority is to make Clay self-documenting, then establish user configuration as documented Clay JS APIs, before introducing the client/server boundary. The approved document/behavior authority decision is recorded in `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`.

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

- The public programmatic surface is the Clay JavaScript/TypeScript API, not raw Rust public functions or raw `Deno.core.ops.op_*` calls.
- Server-side Rust public functions must have explicit `deno_core` op wrappers, stable Clay JS/TS facade APIs, Markdown documentation, and generated registry entries; functions that should remain internal must be private or `pub(crate)`.
- Every Clay JS API must include a searchable user-facing name, key binding metadata, and custom properties for every behavior-changing configurable setting.
- Markdown files plus the master `docs/index.md` are the source of truth for Clay JS API docs; generated app/agent registries and lookup APIs are derived from that indexed Markdown set.
- `cargo test` must detect missing Clay JS APIs, missing Markdown docs, missing master-index links, missing user-facing names/key binding/custom property metadata, malformed/stale generated registry entries, and missing lookup coverage. Tests must fail with actionable update commands rather than silently mutating artifacts.
- Internal implementation details belong in the project code wiki, which links to authoritative public API reference docs instead of duplicating them.
- AI agents must be able to query the app's available Clay JS APIs, commands, key bindings, configuration options, packages, modes, tools, permissions, protocol concepts, behavior manifests, and extension APIs from structured docs rather than guessing from source code.

### Configuration Requirement

Clay user configuration is a documented Clay JS API surface, not a separate ad hoc settings system.

This means:

- The user configuration entry point is `~/.config/clay/init.js`.
- `init.js` may load other local configuration files so users can keep configuration modular.
- Each configuration option is a Clay JS API with Markdown documentation, master-index inclusion, generated registry coverage, lookup access, a searchable user-facing name, key binding metadata, custom properties, and security notes.
- Key bindings are discoverable through the same Clay JS API registry. APIs with no default key binding still record an empty key binding list so users can map one.
- Configuration must not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority; permission-bearing APIs require explicit docs and server-side validation.

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

- Define the Markdown/frontmatter schema for Clay JS API documentation, including JS module/export, backing Rust path, `deno_core` op, searchable user-facing name, key binding metadata, custom properties, permissions/security notes, examples, options, and lookup tags.
- Establish `docs/index.md` as the master Markdown index and `docs/reference/` as the authoritative public Clay JS API reference area.
- Generate machine-readable app/agent registries and lookup APIs from indexed Markdown, not from a separately authored registry.
- Add tests that fail when server-side Rust public functions lack Clay JS APIs, Clay JS APIs lack Markdown docs, docs are missing from `docs/index.md`, generated registry artifacts are stale, or lookup APIs cannot find generated entries.
- Create the implementation code wiki under `docs/wiki/` for internal architecture, modules, flows, invariants, and tests.
- Establish the recurring plan rule that new server-side Rust public functions, public programmatic behavior, key bindings, and configuration options must include Clay JS API, Markdown docs, generated registry, and lookup coverage in the same change.

Expected outcome:

- Clay has a strict, testable Clay JS API documentation contract before extension APIs and server-driven behavior multiply.
- AI agents can inspect available Clay JS APIs, commands, key bindings, configuration options, protocol concepts, behavior manifest capabilities, and permissions from generated structured documentation.
- Human Markdown docs and agent/app-readable registries share `docs/index.md` and linked Markdown files as the source of truth.
- Developers and AI agents can inspect implementation knowledge through the separate project code wiki.

## Phase 4: Configuration Foundation

Establish Clay's user configuration model on top of the Phase 3 self-documenting Clay JS API contract before the IPC/server boundary grows.

Focus areas:

- Use `~/.config/clay/init.js` as the user configuration entry point.
- Allow `init.js` to load other local configuration files so users can keep configuration modular.
- Treat every configuration option as a Clay JS API, not as an undocumented key/value setting.
- Define initial configuration Clay JS APIs for key binding management and editor customization, starting with documented planned surfaces where runtime execution is not ready yet.
- Record default key bindings, including empty defaults, in Clay JS API docs and generated registry entries.
- Record custom properties for every behavior-changing setting, such as cursor style color, blinking, and shape.
- Ensure configuration APIs have Markdown docs, `docs/index.md` links, generated registry entries, lookup access, and tests that fail when metadata is missing or stale.
- Keep configuration loading local and server-side; do not introduce client-side arbitrary JavaScript execution.
- Preserve the no-authority-by-default security model: configuration cannot grant filesystem, network, shell, extension loading, AI mutation, or workspace access without explicit documented permissions and server-side validation.

Expected outcome:

- Clay has a documented configuration foundation before IPC, server, behavior manifest, and extension APIs multiply.
- Users and AI agents can discover configurable behavior, default key bindings, missing key bindings, and custom properties through the generated documentation registry.
- `~/.config/clay/init.js` is the committed user-facing configuration entry point, with modular loading semantics documented.
- Configuration APIs are validated by the same Markdown/registry/lookup coverage gates as other Clay JS APIs.
- Runtime configuration execution can be implemented later by the server-side JavaScript runtime without changing the public configuration contract.

## Phase 5: IPC Client/Server Skeleton

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
- Keep the Phase 5 protocol intentionally small; do not make `rkyv` performance proving the phase's main goal.
- Preserve a benchmark/swap point around the codec so future measurements can compare message shapes and payload sizes.

Expected outcome:

- Client and server run as separate architectural units.
- Server owns a canonical in-memory document placeholder rather than acting as a stateless behavior service.
- Server can send initial document state and a minimal behavior manifest.
- Client can apply manifest-declared client-first text edits immediately and send basic edit operations asynchronously.
- Protocol messages are `rkyv`-serializable and exchanged through a length-prefixed local IPC frame.
- Any new server-side Rust public functions are either exposed through documented Clay JS APIs or made private/`pub(crate)`, and any new public programmatic capabilities follow Phase 3 Markdown/registry checks.
- Serialization remains isolated enough that Phase 6 synchronization work can evolve message semantics without broad UI/server rewrites.

## Phase 6: Versioned Text Synchronization and Leases

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

## Phase 7: Behavior Manifest System

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

## Phase 8: File and Workspace Server

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

## Phase 9: Server-Driven UI

Evolve Clay beyond a text editor into a programmable native canvas.

Focus areas:

- Define an initial SDUI schema for panels, labels, buttons, lists, editor views, and layout containers.
- Let the server send declarative UI tree updates.
- Map SDUI payloads to native Masonry widgets.
- Start with static Rust-generated SDUI before introducing JavaScript-generated SDUI.
- Decide where `rkyv` becomes necessary based on measured payload costs.
- Integrate SDUI schema helpers into Clay JS API documentation and generated registry lookup where they are exposed programmatically.

Expected outcome:

- The server can declaratively alter parts of the native client UI.
- Clay can host multiple native panels/views.
- UI capabilities are inspectable by users and AI agents through generated documentation.

## Phase 10: Embedded JavaScript Runtime

Add the `deno_core` extension brain after the client/server/document/manifest architecture is stable.

Focus areas:

- Embed `deno_core` on an isolated server-side runtime thread/task boundary.
- Evaluate `~/.config/clay/init.js` and allow it to load modular local configuration files.
- Expose stable Clay JS/TS facade APIs backed by explicit `deno_core` ops: create panel, open document, register command, register behavior manifest entries, mutate SDUI tree, configure documented settings, bind keys, and prepare package runtime/load-time entry point support.
- Compile JavaScript extension registrations into behavior manifest updates.
- Add permissions before exposing filesystem, network, shell, AI, or workspace mutation APIs.
- Report runtime errors in the Clay UI.
- Keep JavaScript out of the ordinary typing critical path.

Expected outcome:

- Clay can be configured and extended through `~/.config/clay/init.js` and modular configuration files.
- Extensions can create native UI through SDUI and define hot-path behavior through manifests.
- Extension/package APIs are constrained, permissioned, documented as Clay JS APIs in Markdown, and available through generated registry lookup for users and AI agents.

## Phase 11: Hot Reload and Behavior Update Semantics

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

## Phase 12: AI-Safe Mutation and Region Locks

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

## Phase 13: Remote, Container, and Multi-Client Hardening

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

## Phase 14: Package System

Make Clay extensible through installable packages that use documented Clay JS APIs.

A package is a small pure JavaScript program, with TypeScript support possible later, that interacts with Clay through Clay JS APIs. Installed packages should become available to the user immediately after being added to the Clay app, subject to permission checks and behavior manifest updates.

Focus areas:

- Define the package manifest format, package entry points, package metadata, permissions, documented Clay JS API dependencies, and generated documentation/lookup requirements.
- Run package JavaScript with server-side `deno_core` at runtime; do not execute arbitrary package JavaScript in the Rust client.
- Separate package code into runtime entry points and load-time behavior entry points. Runtime code handles package behavior through Clay JS APIs; load-time code contributes behavior manifest changes during package loading, not installation.
- Require each package to explicitly declare which code runs at runtime and which code runs during loading to update client/server behavior manifests.
- Support package-provided behavior through major modes and minor modes, similar to Emacs.
- Require packages that define minor modes to declare the major mode they apply to; a minor mode is active only when its declared major mode is operational.
- Allow a package-provided major mode to take over behavior on top of the default mode.
- Enforce that two major modes cannot be active simultaneously for the same document/context.
- Define deterministic conflict handling so one package cannot silently override another package's behavior manifest entries, key bindings, commands, or configuration APIs.
- Integrate package APIs, modes, commands, key bindings, configuration options, permissions, and behavior manifest contributions into the Clay JS API Markdown docs, generated registry, and app/help/agent lookup.
- Define how packages are installed, loaded, enabled, disabled, upgraded, and removed without corrupting user configuration or active documents.
- Consider and define the package repository and distribution system during implementation, including package identity, versioning, trust, signatures or integrity checks, publishing workflow, dependency resolution, offline/local packages, and registry metadata.
- Add tests that fail when packages omit required manifest fields, permission declarations, mode declarations, runtime/load-time separation, docs, registry entries, or conflict metadata.

Expected outcome:

- Clay can load packages as documented JavaScript extensions that interact only through Clay JS APIs.
- Package runtime behavior executes on the server-side JavaScript runtime, while hot-path client behavior is delivered through validated behavior manifests.
- Major/minor mode rules prevent packages from silently overriding incompatible behavior.
- Users and AI agents can inspect installed packages, modes, commands, key bindings, configuration options, permissions, and behavior contributions through generated documentation and app lookup.
- The package repository/distribution model is defined well enough for future package publishing and installation work.

## Phase 15: Product Hardening

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
- Documentation coverage gates for Clay JS APIs, packages, generated registries, code wiki navigation, and user-facing features.

Expected outcome:

- Clay becomes a robust native programmable editor environment rather than only a proof of architecture.
- Performance remains bounded by available machine resources rather than avoidable architecture bottlenecks.
- Clay remains inspectable by users and AI agents as it grows.
