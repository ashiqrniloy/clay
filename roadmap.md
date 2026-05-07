# Clay Implementation Roadmap

## Current Status

Phase 0 has successfully proven the highest-risk native client stack within its intended scope: Masonry owns the native window/widget boundary, Vello renders the scene, Parley lays out text, and a local `crop` rope backs editable text state. The final GUI smoke test has been run successfully.

The remaining Phase 0 compromises shape the next steps:

- The editor currently materializes the whole rope into a `String` for visible text.
- Parley layout is rebuilt during paint from the current visible text.
- Masonry owns the event loop and Vello/wgpu surface lifecycle, so Clay should lean into Masonry as the native client shell.

## Phase 1: Text Canvas Foundation

Stabilize the Phase 0 prototype into a maintainable native text canvas module before adding server complexity.

Focus areas:

- Separate buffer, viewport, layout, painting, and widget responsibilities more clearly.
- Replace whole-buffer `visible_text()` assumptions with viewport/range-based text extraction.
- Add viewport state for scroll offset and visible line/window bounds.
- Introduce layout dirty-state tracking so Parley layout is rebuilt only when text, width, viewport, or font state changes.
- Preserve the current single-process local prototype while preparing the client shadow-state boundary needed by future IPC.

Expected outcome:

- The client can handle larger buffers without whole-document rendering assumptions.
- The editor surface has explicit state boundaries that can later receive server-provided document slices.
- Masonry remains the owner of native event/widget/render lifecycle.

## Phase 2: Editor Interaction Model

Move from append/backspace demo behavior to a minimal real editor interaction model.

Focus areas:

- Cursor model.
- Hit-testing from pointer position to text offset.
- Insert/delete at cursor.
- Newline handling.
- Basic selection model.
- Keyboard navigation: arrows, Home/End, and basic scrolling behavior.
- Unicode-safe offset movement tests.

Expected outcome:

- Clay has a minimally usable native editor surface.
- The client owns high-frequency local interaction state required for optimistic editing.

## Phase 3: IPC Client/Server Skeleton

Introduce the Thick Client / Asynchronous Server architecture without solving full synchronization yet.

Focus areas:

- Run the Phase 2 manual GUI smoke pass before cutting the client/server seam.
- Scaffold an async Rust server using Tokio.
- Keep the Masonry/Vello client separate from the server boundary.
- Add a local IPC transport abstraction.
- Start with Unix Domain Sockets on Linux/macOS; leave Windows named-pipe support behind the transport abstraction.
- Define initial lifecycle messages: connect, initial document snapshot, edit event, acknowledgement.
- Use `rkyv` early for protocol encoding, but keep it behind a narrow codec boundary.
- Validate received archived payloads before access and treat local IPC bytes as fallible input.
- Keep the Phase 3 protocol intentionally small; do not make `rkyv` performance proving the phase's main goal.
- Preserve a benchmark/swap point around the codec so future measurements can compare message shapes and payload sizes.

Expected outcome:

- Client and server run as separate architectural units.
- Server can send initial document state.
- Client can send basic edit operations.
- Protocol messages are `rkyv`-serializable and exchanged through a length-prefixed local IPC frame.
- Serialization remains isolated enough that Phase 4 synchronization work can evolve message semantics without broad UI/server rewrites.

## Phase 4: Versioned Text Synchronization

Implement the canonical/shadow text model described in `concept.md`.

Focus areas:

- Server owns canonical `crop` ropes.
- Client owns lightweight visible/shadow document state.
- Add document version numbers.
- Add edit messages with base versions.
- Add server acknowledgements and stale-edit rejection.
- Add simple resync behavior.
- Introduce region lock data structures and basic enforcement.

Expected outcome:

- Local typing can remain immediate while the server remains authoritative.
- Stale or conflicting edits are detectable.
- The architecture can support future AI-driven edits safely.

## Phase 5: File and Workspace Server

Make Clay edit real files through the authoritative server model.

Focus areas:

- Server-side file loading and saving.
- Workspace root handling.
- Open document registry.
- Dirty state tracking.
- Save/reload behavior.
- Clear errors for file IO failures.
- No direct client filesystem authority.

Expected outcome:

- Clay can open, edit, and save files through the server.
- The client remains a canvas/view/input layer.

## Phase 6: Server-Driven UI

Evolve Clay beyond a text editor into a programmable native canvas.

Focus areas:

- Define an initial SDUI schema for panels, labels, buttons, lists, editor views, and layout containers.
- Let the server send declarative UI tree updates.
- Map SDUI payloads to native Masonry widgets.
- Start with static Rust-generated SDUI before introducing JavaScript.
- Decide where `rkyv` becomes necessary based on measured payload costs.

Expected outcome:

- The server can declaratively alter parts of the native client UI.
- Clay can host multiple native panels/views.

## Phase 7: Embedded JavaScript Runtime

Add the `deno_core` extension brain after the client/server/document architecture is stable.

Focus areas:

- Embed `deno_core` on an isolated server-side runtime thread/task boundary.
- Evaluate `init.js`.
- Expose a small Rust API surface to JavaScript: create panel, open document, register command, mutate SDUI tree.
- Add permissions before exposing filesystem or network APIs.
- Report runtime errors in the Clay UI.

Expected outcome:

- Clay can be configured and extended through `init.js`.
- Extensions can create native UI through SDUI.
- The first extension API remains constrained and auditable.

## Phase 8: AI-Safe Mutation and Region Locks

Support AI-generated edits without corrupting user state.

Focus areas:

- Make region locks first-class.
- Require AI edit sessions to carry explicit document versions and ranges.
- Add preview/apply/reject flows.
- Add conflict explanations.
- Consider transaction logs.
- Separate extension/agent permissions from direct user input.

Expected outcome:

- AI agents can propose or apply changes safely.
- User edits and agent edits have explicit conflict boundaries.

## Phase 9: Product Hardening

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

Expected outcome:

- Clay becomes a robust native programmable editor environment rather than only a proof of architecture.
