# Clay Implementation Roadmap

## Current Status

Clay has completed the native editor foundation and the initial server-authoritative architecture through Phase 13, plus the cross-platform launch work recorded as Phases 10 and 11. Masonry owns the native window/widget boundary, Vello renders the scene, Parley lays out text, and `crop` ropes back both local editor state and server-owned canonical document state. The editor supports local interaction including cursor movement, click-to-place caret, drag selection, selected-range editing, Unicode-safe scalar movement, viewport-bounded extraction, layout caching, scrolling, resize handling, optimistic local editing, server acknowledgements, visible connection/access/version status, and managed GUI smoke launch paths.

The client/server foundation is in place across Unix and Windows: the native client exchanges length-prefixed `rkyv` protocol messages with a Tokio server over Unix Domain Sockets on Unix and local named pipes on Windows. The server owns canonical document versions, edit validation, editable leases, read-only observer state, stale-edit rejection, resync snapshots, region-lock structures, workspace/file authority, dirty-state tracking, and save/reload behavior; the client keeps hot-path editing responsive with optimistic local edits and asynchronous acknowledgements. Server-issued inert behavior manifests provide client-executed hot-path behavior without arbitrary JavaScript execution in the Rust client.

Clay now has the self-documenting configuration and Clay JS API foundation: a facade tree, API inventory, Markdown-authored Clay JS API references, generated registry artifacts, read-only registry lookup APIs, and validation coverage for documentation, inventory, registry freshness, and Rust visibility mapping. Public programmatic behavior is expected to flow through Clay JS APIs rather than raw Rust functions or raw `Deno.core.ops`.

Phase 9 is complete for the server-side file/workspace foundation. Phase 13 moved the previously planned file/workspace, configuration, key binding, behavior, and SDUI facade stubs into a constrained server-side `deno_core` runtime where in scope. The server can evaluate `~/.config/clay/init.js` or controlled test fixtures, import curated `clay:*` facade modules, publish JavaScript-generated SDUI through server validation, compile key binding registrations into inert behavior manifests, expose selected file/workspace ops, and report sanitized runtime diagnostics through server/client UI state. JavaScript execution remains server-side and off the ordinary typing/rendering hot path.

Phases 10 and 11 were implemented before Phase 9 was finished because cross-platform local IPC and developer-friendly launch/smoke workflows became immediate validation needs. Clay now builds and runs against the Windows MSVC target, supports Unix sockets and Windows named pipes behind a shared endpoint/transport abstraction, and offers command-first launch paths such as `cargo run`, `cargo run -- server`, `cargo run -- client`, `cargo run -- smoke-gui`, and `cargo run -- smoke-gui --config-fixture runtime-sdui` with visible GUI connection/access/synchronization/runtime diagnostic status.

## Carried-Forward Follow-Up Consolidation

The following items consolidate the compromises and further actions from completed plan documents so future roadmap phases cover them explicitly instead of leaving them scattered across `plans/`:

- **Manual GUI validation:** Several completed phases relied on automated tests or bounded launch observations because the agent shell was non-interactive. Future hardening phases must include repeatable interactive smoke coverage for launch, typing, selection, multi-client read-only observer behavior, runtime SDUI, Windows GUI behavior, Unix GUI behavior, and the Phase 19 Windows Markdown open-dialog path (`cargo run -- smoke-gui --config-fixture windows-markdown-open`, `Ctrl+O`, select a `.md` file, verify Markdown status/decorations and responsive edit-only behavior). The Phase 19 interactive OS file browser remains manual, not automated; run it before treating the file-open experience as release-validated.
- **Performance and scaling:** Early full-buffer prototype compromises were mostly replaced by viewport-bounded extraction and layout caching, but rigorous large-file benchmarks, layout/render profiling, pixel-accurate scrolling refinements, and memory/latency budgets remain future product-hardening work. Phase 14 established baseline Criterion benchmarks, typed budget constants in `src/perf/budgets.rs`, deterministic hard guards for payload ceilings and queue/viewport invariants, and advisory latency/memory targets. Advisory Criterion thresholds are not yet enforced as hard CI failures because a stable CI runner does not yet exist; promotion to hard thresholds is deferred to Phase 21. If the developer-only profiling activation (`CLAY_PERF_PROFILE=1`, `--profile-perf`) is ever promoted to a stable user-facing diagnostic surface, a `clay:diagnostics` Clay JS API must be introduced with Markdown docs, inventory entry, and registry coverage before the hooks are exposed publicly; this is deferred to Phase 21 or a dedicated hardening phase.
- **Synchronization recovery:** Phase 5 uses snapshot-based resync that can discard unacknowledged optimistic edits. Richer correction transactions, pending-edit replay, user-visible pending/error reporting, and recovery UX remain future synchronization/product-hardening work.
- **Leases, locks, and collaboration:** Collaboration is currently a single-writer lease model with read-only observers. Lease transfer/steal/renewal UX, first-class region-lock ownership APIs, persistence, UI, AI/extension lock workflows, and multi-document/multi-client scaling remain future work.
- **Behavior manifests and modes:** Phase 6 installed one default manifest and whole-manifest replacement. Per-document/per-mode/package-selected manifests, richer language-specific rules such as Markdown list continuation, manifest diffs, hot reload, conflict policy, stale-version recovery, and command side-effect routing remain central to the upcoming mode/package phases.
- **Server-driven UI:** Phase 12 intentionally started with static Rust-generated SDUI, Phase 13 added JavaScript-generated SDUI, and Phase 15 added deterministic structural SDUI regression/observability coverage. Pixel-buffer/GPU snapshots remain deferred until Masonry/winit offers deterministic CI-friendly offscreen rendering. Public `clay:sdui.queryUiState` observability and user-facing SDUI layout/panel visibility configuration APIs remain deferred until package-owned UI, agent introspection, or workspace panel settings create real requirements.
- **Runtime and package iteration:** Phase 13 proves server-side JavaScript configuration and SDUI publication, and Phase 17 establishes package loading, mode ownership, deterministic conflict handling, a pnpm-delegated package-manager boundary, server-side package runtime facades, decoration transport, and parse-coordinator handoff foundations. Long-run follow-up remains for provider-facing decoration/parse Clay JS APIs, persistent shared package enable/disable state, live reload semantics, and a Markdown mode proof of concept. Phase 17 deliberately promoted `clay.packages.serverLoadPackage` while keeping decoration/parse provider APIs planned/unavailable until their public op/facade contracts are finalized.
- **Docs and CI:** API docs/registry/wiki coverage exists, but future phases should add CI for formatting, native all-target tests, Windows MSVC checks, generated registry freshness, package docs, wiki navigation, and user-facing feature coverage. A Markdown/wiki lint command should automate index-link and source-reference checks once documentation volume grows.
- **Daily editing basics:** Clipboard, undo/redo, IME/composition, themes, accessibility polish, cross-platform UI polish including macOS/Linux native file dialogs, and richer file workflows such as save-after-selected-open, selected-file conflict handling, save-as, watchers, autosave, and conflict resolution remain product-hardening work.

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

## Phase 3: Self-Documenting Program Contract — Foundation Complete

Introduce documentation-as-code before Clay exposes large protocol, server, command, behavior, and extension surfaces.

Focus areas:

- Define the Markdown/frontmatter schema for Clay JS API documentation, including JS module/export, backing Rust path, `deno_core` op, searchable user-facing name, key binding metadata, custom properties, permissions/security notes, examples, options, and lookup tags.
- Establish `docs/index.md` as the master Markdown index and `docs/reference/` as the authoritative public Clay JS API reference area.
- Lock the Clay JS API boundary, naming, discovery metadata, configuration-as-API, and Markdown-authoritative registry decisions as planning constraints.
- Establish the recurring plan rule that new server-side Rust public functions, public programmatic behavior, key bindings, and configuration options must include Clay JS API, Markdown docs, generated registry, and lookup coverage in the same change.

Deferred from Phase 3 until the Phase 7 Clay JS API structure and inventory exist:

- Authoring per-API Markdown docs for current editor capabilities.
- Generating machine-readable app/agent registries and registry update commands from indexed Markdown.
- Exposing programmatic/app-facing documentation lookup over generated registry data.
- Adding coverage gates that enforce server-side Rust public function -> Clay JS API -> Markdown doc -> `docs/index.md` -> generated registry -> lookup coverage.
- Finalizing the Clay JS API documentation workflow for later protocol, behavior manifest, permission, extension, AI-tool, SDUI, and package surfaces.
- Creating or verifying concrete configuration Clay JS APIs.
- Creating or verifying concrete Clay JS APIs for public programmatic surfaces.
- Creating/updating the implementation code wiki and final Phase 3 verification tied to those deferred implementation tasks.

Expected outcome:

- Clay has the strict Clay JS API documentation contract, naming convention, and schema expectations before extension APIs and server-driven behavior multiply.
- Deferred registry, lookup, coverage, per-API documentation, configuration API, and wiki implementation tasks are explicitly waiting on the Phase 7 Clay JS API structure and current-functionality inventory.
- Human Markdown docs remain the intended source of truth for future agent/app-readable registries.

## Phase 4: IPC Client/Server Skeleton — Complete

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
- Any new server-side Rust public functions are either exposed through documented Clay JS APIs or made private/`pub(crate)`, and any new public programmatic capabilities preserve the Phase 3 documentation contract; concrete Clay JS facade/docs work can be completed in Phase 7 unless the API is intentionally introduced earlier.
- Serialization remains isolated enough that Phase 5 synchronization work can evolve message semantics without broad UI/server rewrites.

## Phase 5: Versioned Text Synchronization and Leases — Complete

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

## Phase 6: Behavior Manifest System — Complete

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

## Phase 7: Clay JS API Structure and Current Functionality Inventory — Complete

Introduce the Clay JS API module/facade structure after the IPC, synchronization, and behavior-manifest foundations exist. This phase creates the project shape and inventory needed for later documentation, configuration, extension, SDUI, AI, and package work.

Focus areas:

- Define the initial Clay JS API source tree and facade/module layout, separating stable JS/TS user APIs from raw Rust functions and raw `deno_core` ops.
- Apply the approved Clay JS API boundary: server-side Rust public functions are exposed only through explicit `deno_core` op wrappers and stable Clay JS/TS facade modules; internal Rust functions remain private or `pub(crate)`.
- Apply the approved naming model: domain modules such as `clay:editor`, concise lower-camel-case exports, server/client authority markers for editor-core APIs, globally namespaced stable registry IDs, and English `user_facing_name` metadata.
- Inventory current Clay functionality that users, configuration, commands, help, key binding, or AI agents will need as public Clay JS APIs, including text insertion, newline, Backspace/Delete, cursor movement, selection, scrolling, resize/viewport behavior, cursor style/customization, key binding management, and application actions such as Escape/quit.
- Classify each inventoried API by authority and runtime path: server-authoritative document mutation, client-local/UI behavior to be delivered through manifests or protocol, configuration API, command/key binding surface, or internal-only implementation detail.
- For each inventoried public API, record the intended JS module/export, stable registry ID, user-facing name, key binding metadata requirement, custom property metadata requirement, backing Rust owner/path if known, future op wrapper name if applicable, permissions/security notes, and documentation path.
- Identify which existing Rust functions should become non-public instead of gaining Clay JS API exposure.
- Keep JavaScript execution server-side; do not introduce arbitrary JavaScript execution in the Rust client or a synchronous keypress -> IPC -> JavaScript -> IPC -> paint path.

Expected outcome:

- Clay has an agreed Clay JS API facade structure and inventory before per-API documentation and registry generation are resumed.
- Future documentation tasks can document actual planned APIs rather than inventing docs for APIs with no project structure.
- Configuration, extension, SDUI, AI, and package phases have a shared list of public user-facing functionality to expose and validate.
- Phase 3 deferred tasks can be reactivated against concrete API names, modules, docs paths, and authority classifications.

## Phase 8: Configuration Foundation — Complete

Establish Clay's user configuration model on top of the Phase 3 self-documenting contract and Phase 7 Clay JS API structure/inventory after the IPC, synchronization, and behavior-manifest foundations exist.

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

- Clay has a documented configuration foundation before extension, SDUI, AI, and package APIs multiply.
- Users and AI agents can discover configurable behavior, default key bindings, missing key bindings, and custom properties through the generated documentation registry.
- `~/.config/clay/init.js` is the committed user-facing configuration entry point, with modular loading semantics documented.
- Configuration APIs are validated by the same Markdown/registry/lookup coverage gates as other Clay JS APIs once the deferred Phase 3 registry and lookup tasks are resumed against the Phase 7 API inventory.
- Runtime configuration execution can be implemented later by the server-side JavaScript runtime without changing the public configuration contract.

## Phase 9: File and Workspace Server — Complete

Make Clay edit real files through the authoritative server model.

Completed focus areas:

- Server-side workspace authority model for workspace roots, canonical paths, open document identity, dirty state, save/reload transitions, and duplicate opens.
- Workspace root and path validation that canonicalizes authorized paths, rejects traversal outside configured roots, rejects directories/special files for document opens, and keeps filesystem authority server-side.
- File-backed open-document registry and loading for existing UTF-8 text files, including stable document IDs, duplicate-open reuse, invalid UTF-8 errors, and preservation of existing lease/read-only observer behavior.
- Dirty state tracking after accepted edits, explicit save/reload behavior, stale metadata detection, and typed file IO failures that preserve dirty state when persistence fails.
- File/workspace IPC commands and typed protocol errors for open, save, reload, status, and document listing without full-document IPC on ordinary edits.
- Container/toolbox/distrobox-friendly environment and permission diagnostics with sanitized paths and no shell probing or extra authority.
- Clay JS API, configuration, generated registry, lookup, and code-wiki verification for public file/workspace behavior.
- No direct client filesystem authority.

Carried-forward compromises and follow-up actions:

- The Phase 9 file/workspace Clay JS APIs are documented typed facade stubs until the Phase 13 server-side JavaScript runtime can provide explicit `deno_core` op wrappers.
- The documented `clay:workspace` root metadata surface currently does not have a dedicated root-list protocol/server message; add one if UI/help surfaces need live root discovery before general Clay JS runtime wiring.
- A dedicated `docs/wiki/flows/file-open-save-reload.md` page remains optional until save-as, file watchers, autosave, or richer conflict-resolution flows make the module-level wiki too dense.

Expected outcome:

- Clay can open, edit, save, and reload files through the server.
- Workspace roots are validated server-side, open documents are keyed by stable document IDs and canonicalized paths, and duplicate opens share the existing lease/read-only observer behavior.
- Dirty state reflects accepted edits and successful saves/reloads; file IO failures return clear protocol/app errors without panics or silent data loss.
- The client remains a canvas/view/input layer.
- The server is the only component that needs workspace filesystem permissions.

## Phase 10: Windows Platform Support — Complete

Port Clay so it compiles and runs on Windows while preserving the existing Unix behavior and server-authoritative architecture.

Focus areas:

- Audit and gate Unix-only code paths.
- Introduce a platform-neutral IPC endpoint model.
- Refactor shared client/server protocol handling to generic Tokio async streams.
- Add Unix and Windows transport implementations using Unix Domain Sockets and Windows named pipes.
- Make binaries and background server startup platform-aware.
- Add Windows MSVC setup and validation documentation.
- Verify cross-platform build and IPC behavior.

Expected outcome:

- `cargo check --all-targets` succeeds on the native development target and the `x86_64-pc-windows-msvc` target when Windows prerequisites are installed.
- Bare `clay`, `clay server`, and `clay client` use platform-default local endpoints.
- Unix builds continue to use Unix domain sockets with stale-socket protections.
- Windows builds use per-user local named pipe endpoints with busy-pipe retry behavior.
- Shared client/server protocol handling remains transport-agnostic and continues to use the bounded `rkyv` codec.

## Phase 11: Developer-Friendly Launch and GUI Smoke Testing — Complete

Make Clay's developer launch paths usable without manually choosing IPC endpoints, and make GUI connection/synchronization state visible during smoke validation.

Focus areas:

- Define supported launch modes for `cargo run`, `cargo run -- server`, `cargo run -- client`, and `cargo run -- smoke-gui`.
- Add managed smoke endpoint generation and child server lifecycle cleanup.
- Improve server readiness, connection diagnostics, and default launch reliability.
- Route client connection events into the GUI event loop.
- Add visible connection, access, and synchronization status to the GUI.
- Make second-client and default-command GUI smoke testing endpoint-free.
- Update developer documentation for command-first GUI smoke validation.

Expected outcome:

- A developer can validate the current app with simple commands and no manual socket/named-pipe selection.
- Bare `cargo run` starts or reuses the platform default local server and opens a connected GUI client when possible, with local fallback status when not.
- `cargo run -- smoke-gui` creates an isolated local endpoint, starts a managed child server, waits for readiness, opens the GUI client, and cleans up when the GUI exits.
- GUI chrome/status communicates local fallback, connecting, connected editable, connected read-only, disconnected, and latest known document/version state.
- Windows named pipes and Unix sockets remain local IPC transports; no remote TCP listener, shell-mediated startup, or user-managed endpoint is required for normal smoke testing.

## Phase 12: Server-Driven UI - Complete

Evolve Clay beyond a text editor into a programmable native canvas.

Completed focus areas:

- Defined an initial SDUI schema for panels, labels, buttons, lists, editor views, and layout containers.
- Let the server send declarative UI tree updates.
- Mapped SDUI payloads to native Masonry state/widgets.
- Started with static Rust-generated SDUI before JavaScript-generated SDUI.
- Measured representative `rkyv` payload costs and established initial snapshot/update budgets.
- Integrated SDUI schema helpers into Clay JS API documentation and generated registry lookup where exposed programmatically.

Carried-forward items:

- Automated visual/layout regression coverage was picked up by Phase 15 as deterministic structural SDUI observability; pixel-buffer/GPU snapshots remain deferred until deterministic offscreen rendering is available.
- SDUI update compression or specialized payload shaping should be revisited only if representative snapshots exceed 4 KiB, simple panel updates exceed 1 KiB, or updates stop being materially smaller than equivalent snapshots.
- Documented SDUI layout/panel visibility configuration APIs should be introduced only when real user-facing layout or panel settings exist, with the go/no-go decision now assigned to Phase 16/17 package UI work.

Expected outcome:

- The server can declaratively alter parts of the native client UI.
- Clay can host multiple native panels/views.
- UI capabilities are inspectable by users and AI agents through generated documentation.

## Phase 13: Embedded JavaScript Runtime - Complete

Add the `deno_core` extension brain after the client/server/document/manifest/SDUI architecture is stable.

Completed focus areas:

- Embedded `deno_core` on an isolated server-side runtime boundary.
- Evaluated `~/.config/clay/init.js` and allowed constrained local modular configuration loading.
- Exposed stable Clay JS/TS facade APIs backed by explicit `deno_core` ops for configuration, SDUI publication, selected file/workspace operations, key bindings, behavior queries/registration, and runtime diagnostics.
- Runtime-backed the Phase 12 `clay:sdui` helpers so a real configuration fixture can construct a panel/editor UI, publish it through server validation, and deliver it to the client as a typed `SduiSnapshot`.
- Compiled JavaScript key binding/configuration registrations into behavior manifest updates; client keypress routing remains manifest-based and never calls JavaScript synchronously.
- Reported runtime errors, validation failures, and permission denials in server diagnostics and the Clay UI where practical.
- Kept JavaScript out of the ordinary typing critical path.

Carried-forward items:

- Runtime SDUI visual smoke remains a documented manual observation step; automated coverage validates launch wiring, event routing, protocol publication, and fixture evaluation.
- Package identity, package install/enable/disable, mode ownership, primitive registries, live reload semantics, and richer package-controlled rendering remain future work.

Expected outcome:

- Clay can be configured and extended through `~/.config/clay/init.js` and modular configuration files.
- The documented Phase 9 file/workspace Clay JS APIs move from planned typed facade stubs to runtime-backed, permissioned server-side operations without granting direct client filesystem authority.
- A test configuration can import `clay:sdui`, construct validated native UI, publish it through the server, and prove the client receives the resulting SDUI without executing JavaScript in the Rust client.
- Extensions can create native UI through SDUI and define hot-path behavior through manifests.
- Extension/package APIs are constrained, permissioned, documented as Clay JS APIs in Markdown, and available through generated registry lookup for users and AI agents.

## Phase 14: Performance Profiling and Benchmark Foundation

Install the measurement foundation before implementing package-controlled modes so performance is a design constraint from the start, not a retrospective hardening task.

Focus areas:

- Add large-file benchmarks for the current plain-text editor/server-client path before package-controlled rendering exists.
- Add repeatable large-file open/generate workflows for manual and automated validation.
- Measure keypress-to-local-paint latency, edit acknowledgement latency, scroll latency, layout time, render time, memory, client edit queue behavior, and IPC payload sizes.
- Add profiling hooks or trace points around editor input handling, visible extraction, Parley layout/cache invalidation, Vello/GPU rendering, SDUI application, client send queues, server edit acknowledgement, and runtime/configuration evaluation.
- Improve incremental Parley layout, viewport virtualization, pixel-accurate scrolling, and layout cache invalidation where baseline benchmarks already show regressions.
- Define performance budgets that future package/mode primitives must satisfy: no synchronous JavaScript in keypress/paint paths, bounded declarations, incremental updates, viewport-bounded rendering, and no full-document IPC for ordinary edits.
- Add CI-friendly performance guards where deterministic enough, and documented local benchmark commands where machine variance makes CI thresholds unreliable.
- Establish typed budget constants that future package, primitive, and mode implementation phases can evaluate against.
- Define the pathway for promoting developer-only profiling hooks to a stable `clay:diagnostics` Clay JS API if user-facing observability is needed in a future phase.

Expected outcome:

- Clay has baseline performance profiles before package and mode work begins.
- Future primitive, package, and Markdown-mode implementation can be evaluated against concrete latency, memory, payload, and rendering budgets.
- Performance regressions become measurable and actionable while package APIs are still being designed.

Carried-forward items:

- Advisory Criterion latency/memory thresholds (`KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `EDIT_ACK_P95_BUDGET_MS`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`, `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS`, `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB`) are documented and constant/doc-aligned but are not hard CI failures. Promoting them requires a stable CI runner; deferred to Phase 21.
- `cargo bench --no-run` is validated locally but not yet wired into an automated CI pipeline; deferred to Phase 21.
- Developer-only profiling hooks (`CLAY_PERF_PROFILE=1`, `--profile-perf`) must remain internal until a future hardening phase explicitly introduces a `clay:diagnostics` Clay JS API with Markdown docs, inventory entry, and registry coverage.

## Phase 15: SDUI, Visual Regression, and UI Observability Foundation

Add visual/layout regression and UI observability before packages start contributing mode-specific panels and rendering declarations.

Focus areas:

- Add automated visual/layout regression coverage for the current native SDUI editor/sidebar composition.
- Add headless or window-driver smoke coverage when Masonry/winit support makes status/layout observation practical.
- Improve accessibility labels/roles for editor, SDUI panels, diagnostics, and status text so tests and assistive tools can inspect UI state.
- Add structured UI observability for SDUI snapshots/updates, status text, panel identity, editor-view identity, and runtime diagnostics.
- Revisit SDUI update compression or specialized payload shaping if payload thresholds are exceeded by current or near-term test trees. Use the Phase 14 budget constants `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` (4 096 B) and `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` (1 024 B) in `src/perf/budgets.rs` as the explicit thresholds; if representative SDUI trees routinely exceed these, introduce compression or diff shaping before Phase 17 package-owned UI multiplies the payload surface.
- Keep documented SDUI layout/panel visibility configuration APIs deferred until real user-facing package or workspace panel settings exist.

Expected outcome:

- Server-driven UI has automated regression coverage before package-owned UI increases complexity.
- The Markdown/package proof of concept can validate UI behavior with observable status/layout signals instead of relying only on manual inspection.

Carried-forward items:

- **Pixel-buffer / GPU snapshot testing:** Structural `SduiObservableSnapshot` assertions are used instead of pixel-buffer snapshots because Masonry 0.4.0 has no headless render surface. If a future Masonry/winit version adds a headless render target, promote the structural regression tests to pixel-accurate snapshot tests and add a `cargo test` fixture for each shipped SDUI composition. Until then, the structural approach is the approved regression strategy.
- **`clay:sdui.queryUiState` Clay JS API:** `SduiObservableSnapshot` and `SduiStatusObservation` are `pub(crate)` test infrastructure in Phase 15 and are not exposed as Clay JS APIs. If a future agent-introspection, package-tooling, or help-system phase needs programmatic SDUI state querying, introduce a dedicated `clay:sdui.queryUiState` API with full Markdown docs, a stable registry ID, key binding metadata, custom properties, `docs/index.md` link, and generated registry coverage before exposing the type publicly.
- **SDUI layout/panel visibility configuration APIs:** These remain deferred until a real user-facing package or workspace panel settings surface exists. When a package or workspace panel setting is introduced, add a Clay JS configuration API with Markdown docs, `user_facing_name`, key bindings, custom properties, `docs/index.md` link, and generated registry entry in the same change.

## Phase 16: Mode and Package Primitive Architecture Analysis

Define the architecture needed to implement editor modes as JavaScript packages without hard-coding mode-specific behavior or rendering logic into the Rust app. This phase uses the Phase 14/15 performance and visual observability foundation to design primitives with measurable budgets from the start.

Focus areas:

- Analyze the primitive categories packages need to control editor behavior and rendering: document classification, major/minor mode activation, key routing, text transforms, incremental parsing, decoration ranges, semantic spans, folding, diagnostics, completion triggers, commands, SDUI panels, status items, and package-owned configuration.
- Explicitly decide whether package tooling, agent introspection, help surfaces, or command-palette workflows need a public `clay:sdui.queryUiState` API; if they do, define the Clay JS API shape, authority boundary, privacy constraints, Markdown documentation, registry metadata, and tests before any Phase 15 `pub(crate)` observability type becomes public.
- Explicitly decide which package-owned or workspace-owned panel settings justify SDUI layout/panel visibility configuration APIs, and record their Clay JS configuration API requirements instead of adding ad hoc settings.
- Define the first version of an exhaustive-but-iterative **Clay primitive registry**: every primitive should have an owner, authority boundary, hot-path policy, Clay JS API shape, documentation metadata, test expectations, performance budget, and whether it is client-first manifest data, server-first command, SDUI state, or renderer/decorator data.
- Define how rendering customization works without arbitrary client-side JavaScript: packages produce inert declarations such as syntax/decorator spans, layout hints, block/inline render intents, or SDUI nodes; the Rust client renders validated declarations locally.
- Decide the Markdown mode POC requirements: Markdown syntax highlighting/rendering target, list continuation, heading emphasis, code block behavior, preview/decorated editor behavior, command/key binding set, and minimum file-extension/mode detection rules.
- Identify which primitives already exist through behavior manifests, SDUI, configuration, file/workspace APIs, and Phase 14/15 observability, and which primitives must be added before the Markdown POC.
- Define package-controlled rendering and parsing update strategies: bounded decoration payloads, incremental parse/update units, cancellable background parsing, viewport-prioritized results, and fallback behavior when package work lags behind local edits.
- Define security and provenance requirements for package-provided primitives: package prefix, permissions, no raw ops, no client JS, no shell/network/filesystem authority unless explicitly documented and validated.

Expected outcome:

- Clay has a concrete package/mode primitive architecture that makes Markdown mode implementable as a package instead of hard-coded Rust logic.
- The roadmap has a prioritized primitive backlog for mode behavior and rendering customization, including explicit go/no-go outcomes for public SDUI state querying and SDUI panel/layout configuration APIs.
- Future packages can extend Clay by registering documented primitives while preserving client hot-path performance, visual observability, and server authority.
- Phase 17 must not start from analysis-only primitive definitions: the Phase-17-required primitive rows in `docs/reference/primitives/backlog.md` must be implemented, documented, and test-covered by the new Phase 16.5 gate below.

Carried-forward items:

- Phase 16 produced architecture analysis, reference documents, planned API inventory stubs, advisory budget constants, and wiki coverage only. Later implementation phases must convert those stubs into real Clay JS facades, `deno_core` op wrappers, server validators, protocol/manifest extensions, generated registry entries, Markdown docs, and tests before relying on the primitives at runtime.
- Runtime package loading, package install/enable state, parse coordination, decoration transport, client decoration rendering hooks, and Markdown package behavior are intentionally deferred from Phase 16 and are assigned to Phase 16.5, Phase 17, and Phase 18 below.
- Phase 16 wiki pages intentionally link to canonical `docs/reference/primitives/` documents instead of duplicating every table. Later phases that change primitive implementation details must keep the canonical reference docs and wiki navigation aligned.
- Advisory primitive budgets (`DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `MODE_ACTIVATION_P95_BUDGET_MS`, and related package/mode payload limits) must become enforceable guard tests only after Phase 17/18 introduce representative protocol messages, fixtures, and benchmarks; the hardening target is Phase 21 unless an earlier implementation phase has enough stable data.

## Phase 16.5: Primitive Implementation Gate for Package and Mode Loading

Implement the package/mode primitives identified by Phase 16 before the package system phase begins. This is a mandatory gate between primitive architecture analysis and package loading work.

Focus areas:

- Implement the Phase-17-required primitive rows from `docs/reference/primitives/backlog.md`: `DocumentClassification`, `MajorModeActivation`, `CommandDeclaration`, `PackagePermissionDeclaration`, package-owned `KeyRoutingOverride`, package-owned `TextTransform`, package-owned `SduiPanelStatusContribution`, and `PackageOwnedConfiguration`.
- Promote the Phase 16 planned API inventory stubs for Phase-17-required primitives into real Clay JS facades, `deno_core` op wrappers, server validators, protocol or manifest extensions, permissions, Markdown documentation, generated registry coverage, and tests so those primitives are implementation surfaces rather than analysis-only entries.
- Implement static file-extension/MIME mode classification, one-active-major-mode state, atomic per-document behavior manifest selection, package-prefixed command registration, permission validation, deterministic conflict diagnostics, and package provenance metadata.
- Extend existing behavior manifest and SDUI paths only with inert, server-validated package contribution data; do not add client-side JavaScript or synchronous JavaScript work to keypress, paint, layout, scroll, or text-event handlers.
- Keep `DecorationRange` and `IncrementalParseUpdate` out of this gate unless they are needed for package loading itself; they are mandatory Phase 17 exit criteria before Phase 18 starts.

Expected outcome:

- Phase 17 can focus on package install/enable/load workflows because the primitives needed to express package modes, commands, behavior manifests, permissions, configuration, and SDUI contributions already exist.
- `cargo test` fails if Phase-17-required primitives lack Clay JS API inventory entries, documentation/index/registry coverage, permission checks, package provenance, or deterministic conflict handling.
- The Phase 17 plan starts with implemented primitives instead of re-discovering or designing primitive shapes.

## Phase 17: Package System and Mode Loading Foundation

Make Clay load installable or local JavaScript packages that contribute documented modes, commands, configuration, SDUI, and behavior manifests through the primitive registry.

A package is a small JavaScript program, with TypeScript support possible later, that interacts with Clay only through Clay JS APIs. Package runtime behavior executes on the server-side JavaScript runtime; hot-path client behavior is delivered through validated behavior manifests and renderer/decorator declarations.

Focus areas:

- Define the package manifest format, package identity/prefix, package entry points, package metadata, permissions, documented Clay JS API dependencies, primitive contributions, and generated documentation/lookup requirements.
- Use the approved npm-compatible package distribution direction: Clay delegates fetching, dependency resolution, lockfiles, integrity, caching, and registry access to an existing package manager; Clay owns validation, permissions, metadata, primitive registration, and execution boundaries. The Phase 17 implementation delegates actual fetch/remove operations to pnpm through a typed process boundary rather than embedding an npm client, so future package-store work must preserve install/enable/runtime separation while handling package-manager environment diagnostics.
- Separate install, enable/load, runtime execution, and load-time behavior contribution. Installing downloads and records a package; enabling validates Clay metadata, permissions, docs, compatibility, modes, conflicts, and primitive declarations before server-side execution.
- Support package-provided major modes and minor modes. A document has at most one active major mode; minor modes declare compatible major modes and cannot silently override major-mode behavior.
- Add per-document/per-mode behavior manifest selection and package provenance metadata.
- Add deterministic conflict handling for key bindings, commands, modes, configuration APIs, SDUI regions, decorations/render primitives, and behavior manifest entries.
- When package-owned SDUI regions make panel visibility/layout user-facing, add documented Clay JS configuration APIs with `user_facing_name`, key binding metadata, custom properties, `docs/index.md` links, generated registry entries, and coverage tests; if Phase 17 still has no real user-facing setting, record that deferral explicitly.
- If package management, package tooling, app/help, or agent workflows need to inspect live SDUI state, introduce a dedicated `clay:sdui.queryUiState` Clay JS API rather than widening Phase 15 internal snapshot structs directly; otherwise keep `SduiObservableSnapshot` and `SduiStatusObservation` internal and covered by inventory tests.
- Integrate package APIs, modes, commands, key bindings, configuration options, permissions, primitive contributions, and docs into the Clay JS API Markdown docs, generated registry, and app/help/agent lookup.
- Add tests that fail when packages omit required manifest fields, permission declarations, mode declarations, runtime/load-time separation, docs, registry entries, primitive metadata, performance metadata, or conflict metadata.
- Persist and share package enable/disable state beyond the current in-memory service so the CLI, future in-app UI, and server runtime can observe the same package store state across processes.
- Before Phase 17 is considered complete, implement the Phase-18-required primitive rows from `docs/reference/primitives/backlog.md`: `DecorationRange` and `IncrementalParseUpdate` as mandatory Markdown foundations, plus `FoldingRange` only if the Markdown POC scope promotes folding from optional/stretch to required.
- Add the bounded decoration protocol/client render hook and server-side background parse coordinator needed by Markdown mode before Phase 18 starts. These must preserve `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, viewport-prioritized delivery, stale-version rejection, cancellation, package provenance, and no package JavaScript in client paint/text-event handlers.

Expected outcome:

- Clay can load validated packages as documented JavaScript extensions that interact only through Clay JS APIs.
- Package-provided modes can be selected per document without hard-coded app logic.
- Users and AI agents can inspect installed packages, modes, commands, key bindings, configuration options, permissions, performance expectations, primitive contributions, and any approved SDUI panel/query surfaces through generated documentation and app lookup.
- Phase 18 must not start until package loading plus the required Markdown rendering/parsing primitives are implemented, documented, registered, and test-covered.

## Phase 18: Markdown Mode Package Proof of Concept

Use a real Markdown mode package as the first end-to-end proof that package-controlled editing and rendering can customize Clay without compromising performance.

Entry gate:

- Do not start Phase 18 until Phase 16.5 has implemented all Phase-17-required primitives and Phase 17 has implemented package loading plus the required Markdown POC primitives from `docs/reference/primitives/backlog.md`: mode classification/activation, command registration, permission/provenance validation, package-owned behavior/SDUI/configuration contributions, `DecorationRange`, and `IncrementalParseUpdate`.
- Treat Phase 17's Rust decoration transport and parse coordinator as handoff foundations, not final provider-facing public APIs. Promote `clay.decorations.serverPublishDecorations` and `clay.parse.serverRegisterParseHandler` only after their public op/facade contracts are finalized, then update `docs/reference/clay-js-api/api-inventory.toml`, generated registry artifacts, Markdown docs, docs index links, and wiki pages in the same change.
- If any prerequisite primitive is still analysis-only, Phase 18 must first move that primitive back into Phase 17 completion work or create a separate implementation gate; Markdown mode must not hard-code missing primitive behavior in Rust to bypass the package architecture.

Focus areas:

- Create a first-party Markdown package that declares a package prefix, major mode, supported file patterns, commands, key bindings, configuration APIs, permissions, documentation, primitive contributions, and performance expectations.
- Implement Markdown editing behavior through manifests and primitives rather than hard-coded editor logic: heading/list continuation, code block indentation behavior, pair handling where relevant, and Markdown-specific command routing.
- Implement Markdown rendering customization through package-produced inert declarations: syntax/decorator spans, heading/code/list styling, and a minimal preview/decorated editor behavior that the Rust client renders without executing package JavaScript in paint/text handlers.
- Drive the primitive registry iteratively: every missing primitive required by Markdown mode must be added as a documented Clay JS API or explicitly deferred with rationale.
- Add a fixture/workspace workflow that opens Markdown content and activates the Markdown major mode through package metadata or explicit command.
- Use Phase 14/15 instrumentation to measure large Markdown documents under package mode: startup parse cost, incremental edit cost, decoration payload size, scroll/render latency, memory use, server/client queue behavior, and visual/layout stability. Criterion baselines should be saved with `cargo bench --benches -- --save-baseline phase14-baseline` before Markdown mode work begins and compared with `--baseline-lenient phase14-baseline` after to detect regressions against the Phase 14 baseline. Measure all paths against the typed budget constants in `src/perf/budgets.rs`.
- Add Markdown-mode SDUI/decorated-editor structural regression fixtures using the Phase 15 observability path, and promote them to GPU-backed pixel snapshots in this phase if the current Masonry/winit stack supports deterministic CI-friendly offscreen captures.
- Add automated and manual smoke coverage for Markdown mode package activation, rendering, editing, server acknowledgements, reload/restart behavior, docs/registry lookup, and fallback when the package is disabled or invalid.

Expected outcome:

- Clay proves that a non-trivial editor mode can be implemented as a JavaScript package rather than hard-coded Rust.
- Markdown behavior and rendering are customized through documented primitives while ordinary typing and rendering remain responsive against measured budgets.
- The primitive registry gains real coverage from a mode POC and becomes the basis for additional modes.
- Markdown-mode UI coverage either includes deterministic pixel snapshots or records that the Phase 15 structural snapshot approach remains the supported regression strategy until the rendering stack can provide CI-friendly offscreen captures.

## Phase 18.1: Clay Shell, Working Area, and Package UI/Layout Architecture Gate

Before further Markdown-mode product work, hot reload work, or daily-editing hardening begins, define and document the Clay-owned application shell and package UI/layout contribution model approved in `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.

Entry gate:

- Treat this phase as mandatory immediately after Phase 18. Phase 19 and later roadmap work must not start until the Phase 18.1-18.5 shell/package-authoring sequence has either completed or been explicitly superseded by a later approved decision.
- Use Masonry documentation as implementation evidence: Masonry is the native widget-tree/rendering substrate, while Clay owns the package-facing working-area, pane, container, component, action, input, and style abstractions.

Focus areas:

- Define the public vocabulary and architecture: working area, pane/split tree, pane/window layout, mandatory `main` container, optional `left`/`right`/`top`/`bottom` panel slots, fixed panels, transient panels, components/elements, action intents, package state scopes, and style/theme tokens.
- Decide and document the boundary between Clay package declarations and Masonry implementation details. Packages must not directly create Masonry widgets, mutate native layout, provide raw CSS, run client-side JavaScript, call raw `Deno.core.ops`, or provide Vello/Parley callbacks.
- Inventory the current implementation gaps: `EditorWidget` as root widget, fixed-sidebar SDUI paint path, lack of pane/split tree protocol, lack of slot-aware component layout, lack of package style token declarations, lack of package state/data schema, and underdefined mouse/input declarations.
- Extend the primitive registry/backlog with shell/layout primitives: `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, and package UI/state contribution categories as needed.
- Define conflict and precedence rules for package/default/user layout: Clay shell safety rules, user configuration, active major mode, compatible minor modes, and global packages.
- Define the documentation/test contract for package UI/layout primitives, including `docs/reference/packages/creating-packages.md` as the package-author guide that must be updated in every shell/layout/package phase.
- Update `docs/reference/packages/creating-packages.md` with the architecture vocabulary, current implementation status, planned shell/layout APIs, and examples marked as implemented vs planned.

Expected outcome:

- Clay has a documented shell/layout/package UI architecture before implementation starts.
- Future package/mode plans have clear vocabulary and primitive targets instead of relying on Markdown-specific SDUI fixtures.
- The package authoring guide explains the new architecture and records which parts are available now versus planned.

## Phase 18.2: Masonry Clay Shell and Pane Runtime Foundation

Implement the native Clay shell foundation on top of Masonry so Clay, not `EditorWidget`, owns the top-level application layout.

Entry gate:

- Do not start until Phase 18.1 has defined the shell/layout primitives, Masonry boundary, package-authoring documentation expectations, and conflict rules.

Focus areas:

- Introduce a Clay-owned root shell/working-area widget above the editor, using Masonry's `RenderRoot` as the implementation substrate and keeping `EditorWidget`/editor surface as an editor component inside the shell.
- Implement the initial pane tree with one leaf pane taking the whole working area, plus generic horizontal/vertical split support that can map to Masonry `Split` or a Clay-owned split container.
- Implement a leaf pane/window layout with mandatory `main` and optional `left`, `right`, `top`, and `bottom` slots, preserving focus, viewport, caret, status, and editor input behavior.
- Model fixed panel sizing, min/max constraints, collapse/visibility state, and user resize behavior as Clay layout state rather than package-owned native widget mutation.
- Apply layout state through Masonry mutation/update/layout passes; do not add/remove children during layout and do not run package JavaScript in paint, layout, pointer, scroll, keypress, or text-event handlers.
- Keep protocol/update payloads inert, versioned, bounded, and testable; add structural shell observability similar to the Phase 15 SDUI snapshot strategy.
- Update `docs/reference/packages/creating-packages.md` with the implemented shell/pane/slot runtime behavior, examples, limitations, and testing guidance.

Expected outcome:

- Clay launches through a shell/working-area root and can represent at least one editor pane through the generic pane/slot model.
- The editor remains responsive and behavior-manifest input routing continues to work while the shell owns layout composition.
- Tests cover the shell root, pane layout, split layout, no-hot-path package execution invariant, and documentation updates.

## Phase 18.3: Slot-Aware Package UI Components, Panels, and Theme Tokens

Evolve package UI from fixed-sidebar SDUI snapshots into slot-aware Clay components that packages can declare and Clay can compose consistently.

Entry gate:

- Do not start until Phase 18.2 has a Clay shell/working-area root and leaf pane slot layout.

Focus areas:

- Extend or replace the current SDUI publication path so package UI contributions target explicit Clay slots (`main`, `left`, `right`, `top`, `bottom`, or transient overlay) rather than assuming a fixed left/sidebar paint path.
- Define the first stable Clay component catalog for package authors: editor view, panel, label, button, list, flex, stack/overlay, scroll/portal, status item, and any minimal table/dropdown/collapse/modal primitives justified by implementation scope.
- Preserve action safety: buttons, list items, dropdown selections, panel controls, and modal actions emit inert command intents targeting registered commands.
- Define fixed versus transient panel behavior, including layout participation, overlay behavior, dismissal, focus policy, accessibility role, and conflict handling.
- Replace hardcoded SDUI colors/sizes with Clay theme tokens and typed component style variables that map to Masonry properties/native styles. Raw CSS remains unsupported.
- Add package semantic theme token declarations and user override hooks only where implemented through documented Clay/package JS APIs.
- Update package metadata validation, conflict checks, payload budgets, docs/registry coverage, and structural UI tests for slot-aware package UI contributions.
- Update `docs/reference/packages/creating-packages.md` with implemented component APIs, slot examples, style/theme token examples, action examples, and anti-patterns.

Expected outcome:

- Packages can contribute slot-aware, inert UI components without direct Masonry access.
- Clay can render fixed and transient package panels consistently across modes.
- The package authoring guide contains accurate component, panel, style, and action examples.

## Phase 18.4: Package Input, Actions, State/Data, and Configuration Integration

Complete the package-facing structure around UI by standardizing how packages declare input interests, commands/actions, data/state, and user configuration for shell-aware packages.

Entry gate:

- Do not start until Phase 18.3 has slot-aware component/panel contributions and a first theme/style-token model.

Focus areas:

- Extend package contribution metadata and Clay JS facades for input declarations beyond keybindings where needed: pointer/click interests, component-scoped actions, focus scopes, mouse selection/drag policies, and mode/component context conditions.
- Preserve behavior manifests for client-first predictable text behavior and key routing; use server/client command intents for side effects, UI commands, file/workspace actions, and package actions.
- Define package state/data scopes: package-global, user-config, workspace, document, pane, and component/transient state. Decide which scopes are server-canonical, client-local, persisted, or explicitly deferred.
- Add configuration APIs for package layout/panel/style/input defaults only through documented Clay JS APIs and `~/.config/clay/init.js`, with generated registry coverage when public.
- Define user override precedence for package defaults: one-line package load, optional package options, user layout overrides, keybindings/actions, theme token overrides, and package fallback behavior.
- Add diagnostics for conflicts and invalid declarations: duplicate slots, duplicate commands, ambiguous key/mouse routing, invalid state scopes, unregistered action targets, unknown theme tokens, oversize UI payloads, and undeclared permissions.
- Update `docs/reference/packages/creating-packages.md` with implemented input/action/state/configuration APIs, examples, permission guidance, tests, and troubleshooting.

Expected outcome:

- Package authors have one coherent model for UI, input, actions, logic, data/state, configuration, and styling.
- Users can configure package defaults and workflows through documented APIs rather than copying fixture scripts or editing hidden settings.
- Tests cover package contribution validation, user overrides, behavior-manifest compatibility, command/action routing, and docs accuracy.

## Phase 18.5: Replan Markdown End-User Loading After Shell/Layout Work

After the Clay shell/package UI architecture has been implemented, revisit the pending Markdown end-user plan so Markdown consumes the new generic primitives instead of driving architecture from fixture-only behavior.

Entry gate:

- Do not start until Phases 18.1 through 18.4 have landed or their incomplete items have been explicitly deferred with decision-log-backed rationale.

Focus areas:

- Update `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md` to reflect the implemented shell, pane/slot, component, theme token, input/action, package state, and configuration APIs.
- Replace any remaining Markdown-specific UI assumptions with generic shell/package primitives, including main editor placement, optional preview/status contributions, no-default-side-panel behavior, and fixed/transient panel choices.
- Preserve the explicit one-line package loading convention and separate explicit `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` configuration.
- Update Markdown package docs, package authoring docs, roadmap follow-ups, and wiki/reference links to match the implemented architecture.
- Re-evaluate tests/manual smoke instructions so the Markdown path validates the actual app package load path rather than large smoke-fixture scripts.

Expected outcome:

- The Markdown end-user loading/UI cleanup plan is current with Clay's implemented shell/package UI architecture.
- Markdown can proceed as a normal package/mode consumer of Clay primitives rather than a special architectural bootstrap.

## Phase 19: Hot Reload and Behavior Update Semantics

Make runtime package and mode behavior changes safe and non-janky after the first package/mode path exists.

Focus areas:

- Watch or trigger package/configuration reloads.
- Re-evaluate JavaScript on the server.
- Produce new behavior manifest versions and renderer/decorator primitive versions.
- Send manifest/decorator/SDUI diffs or snapshots to affected clients.
- Atomically install behavior and rendering versions on clients.
- Define grace, rejection, correction, or resync semantics for edits made under stale behavior/rendering versions.
- Add behavior/range/document/workspace locks for package reloads, AI, or extension-driven behavior changes.
- Preserve user-visible diagnostics and rollback/fallback behavior when a package reload fails.
- Reuse Phase 14/15 instrumentation to verify reloads do not block ordinary typing/rendering and do not produce visual half-states.

Expected outcome:

- Users and AI agents can modify configuration/package behavior at runtime.
- Clients do not apply half-updated editing or rendering rules.
- Behavior and rendering changes are visible, documented, versioned, measurable, and reversible or recoverable.

## Phase 20: Daily Editing Product Hardening

Add the core editor capabilities needed for daily use after the package/mode path proves customizable editing and rendering.

Focus areas:

- Clipboard support.
- Undo/redo.
- IME/composition support.
- Theme system.
- Accessibility improvements.
- Cross-platform UI polish, including platform-native file-open dialogs for macOS/Linux that reuse the Phase 19 client UI command and selected-file grant primitives rather than adding broad client filesystem authority.
- Revisit Phase 15's deferred pixel-buffer/GPU snapshot coverage during UI hardening; if Masonry/winit now supports deterministic offscreen rendering, add pixel-accurate snapshots for shipped editor, SDUI, and mode/package compositions while keeping structural observability tests as fast headless coverage.
- Multi-document behavior, including per-document mode selection, per-document status, dirty state, leases, and package manifest versions.
- Selected-file save/conflict persistence for files opened through the Phase 19 single-file grant path, including explicit dirty-state/persistence UX before save-after-open becomes user-facing.
- Dedicated file-open/save/reload workflow documentation if selected-file save, save-as, file watchers, autosave, or conflict-resolution flows outgrow the Phase 9 module-level wiki.
- User-visible pending-edit/error reporting, reconnect/resync prompts, and richer recovery UX.

Expected outcome:

- Clay becomes usable for real editing sessions, not only architecture validation.
- Daily-use features integrate with package modes and server authority instead of bypassing them.

## Phase 21: Remote, Container, and Multi-Client Hardening

Make the server/client split useful beyond local IPC.

Focus areas:

- Remote server connection over secure transport.
- Container/toolbox/distrobox server startup and discovery.
- Live workspace-root discovery for UI/help surfaces, including a dedicated root-list protocol/server method if this is still needed before or beyond general Clay JS runtime wiring.
- SSL/TLS or SSH/tunnel strategy.
- Multiple clients connected to one server.
- Multiple documents open concurrently at scale.
- Read-only observer behavior for duplicate opens.
- Server concurrency and per-document actor scaling.
- CI coverage for `cargo fmt --check`, native `cargo test --all-targets`, Windows MSVC checks, generated registry freshness, package docs, and wiki navigation.
- Add `cargo bench --no-run` to CI to verify all Criterion benchmark targets compile on every push without running machine-variant timing loops.
- Promote Phase 14 advisory latency budget constants (`KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `EDIT_ACK_P95_BUDGET_MS`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`, `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS`) and Phase 16 primitive/package budget constants (`DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `MODE_ACTIVATION_P95_BUDGET_MS`, completion/folding payload budgets, and package/mode validation payload ceilings) to hard CI thresholds only after verifying stability across at least one consistent CI runner and representative Phase 17/18 fixtures; document the promoted values and remove the advisory-only qualifier from `docs/development/performance.md` and primitive reference docs.
- If developer-only profiling hooks have been promoted to a stable user-facing feature by this phase, verify the `clay:diagnostics` Clay JS API exists with Markdown docs, inventory entry, generated registry entry, and lookup coverage; otherwise confirm the `no_public_configuration_needed_for_internal_perf_hooks` guard test remains active.

Expected outcome:

- A host client can connect to a server running in a target development environment.
- Clay can support local, container, and remote editing without changing the client authority model.

## Phase 22: AI-Safe Mutation and Region Locks

Support AI-generated edits without corrupting user state.

Focus areas:

- Make region locks first-class.
- Require AI edit sessions to carry explicit document versions, behavior versions, mode/package primitive versions, ranges, and permission scopes.
- Add preview/apply/reject flows.
- Add conflict explanations.
- Consider transaction logs and richer correction transactions.
- Separate extension/agent permissions from direct user input.
- Lock only the needed scope: range, document, behavior, mode, rendering primitive, or workspace.

Expected outcome:

- AI agents can propose or apply changes safely.
- User edits and agent edits have explicit conflict boundaries.
- AI-visible tools and mutation capabilities are documented and inspectable.

## Phase 23: Ecosystem and Repository Hardening

Prepare Clay packages and primitive APIs for a broader ecosystem after first-party package/mode proof points exist.

Focus areas:

- Package repository policy, package publishing workflow, trust, signatures or integrity checks beyond delegated package-manager integrity, offline/local packages, registry metadata, upgrades, removal, compatibility policy, package-manager environment diagnostics, and persistent shared package enable/disable state across CLI, in-app UI, and server runtime processes.
- Documentation coverage gates for Clay JS APIs, packages, generated registries, code wiki navigation, package-provided user-facing features, primitive contributions, and mode behavior.
- User/developer package UI for install, enable, disable, upgrade, remove, inspect permissions, inspect primitive contributions, and diagnose conflicts.
- Additional first-party package/mode examples beyond Markdown, using the primitive registry to expose missing capabilities iteratively.

Expected outcome:

- Clay has a sustainable package ecosystem path after proving package-controlled editing/rendering locally.
- The primitive registry grows through real modes while remaining inspectable and performance-safe.
