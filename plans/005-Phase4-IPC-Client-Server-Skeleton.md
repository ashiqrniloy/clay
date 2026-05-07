# Phase 4 IPC Client/Server Skeleton

## Objectives
- Introduce Clay's Thick Client / Asynchronous Server architecture without attempting full versioned synchronization.
- Keep the existing Masonry/Vello/Parley editor as the native client surface while adding a Tokio server boundary.
- Exchange initial document snapshots, behavior manifests, and basic edit operations over local IPC.
- Implement the first minimal slice of the approved server-authoritative document model with optimistic client shadows and server-issued client-executed behavior manifests.
- Use `rkyv` early for protocol encoding, but keep it behind a small codec abstraction so protocol semantics remain easy to evolve.
- Preserve Phase 2 editor behavior and tests while preparing the message boundary needed by Phase 5 versioned synchronization.

## Expected Outcome
- The repository has separate client/server architectural units, preferably separate binaries or clearly separated modules.
- A Tokio server listens on a local Unix Domain Socket on Linux/macOS.
- The client connects to the server, receives an initial in-memory document snapshot plus a minimal behavior manifest, and loads them into the editor.
- Local editing remains immediate for manifest-declared client-first text behavior and sends basic insert/delete/replace operations or editor intents to the server asynchronously.
- The server receives edit operations/intents, applies them to a canonical in-memory document placeholder, and sends acknowledgements or simple edit transactions, but does not yet fully reject stale edits or perform full resync.
- Protocol messages include final-architecture-compatible fields where practical, such as document IDs, client IDs, editable/read-only access mode, base document version, server version, transaction ID, and behavior version.
- Protocol messages are encoded with `rkyv` through a length-prefixed codec with validation on receive.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- No `deno_core` runtime execution, SDUI protocol, file workspace authority, region-lock conflict handling, remote SSH/Docker mode, or AI edit sessions are introduced in this phase.
- Phase 3 documentation-as-code requirements are followed for any new protocol, behavior manifest, client, or server public surfaces.

## Tasks

- [ ] Define the minimal `rkyv` protocol and length-prefixed codec boundary
  - Acceptance Criteria:
    - Functional: Protocol types can represent `Hello`, `Welcome`, `InitialDocument`, a minimal `BehaviorManifest`, editable/read-only document access, client edit operations and/or editor intents, edit transactions or acknowledgements, and error responses.
    - Performance: Encoding avoids protocol-wide heap-heavy conversion layers beyond required socket framing and inserted text ownership; received payload validation is explicit and measured later rather than assumed free. Ordinary text edits carry deltas, not full-document payloads.
    - Code Quality: Protocol types live in a focused module shared by client and server, derive `rkyv::Archive`, `rkyv::Serialize`, and `rkyv::Deserialize`, and are accessed through a `Codec` API instead of scattered serialization calls. Message fields leave room for Phase 5 versioning without coupling UI code to transport details.
    - Security: Received archived bytes are validated before access; maximum frame size is bounded to prevent accidental unbounded memory allocation from malformed or oversized IPC frames. Behavior manifests are inert declarations only, not executable scripts or extension authority.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/rkyv/rkyv`: derive `Archive`, `Serialize`, and `Deserialize`; use `rkyv::access` for safe validated zero-copy access to archived bytes; validation is recommended for untrusted data and is enabled through bytecheck support.
      - `concept.md`: IPC serialization target is `rkyv`, intended for efficient server-driven UI and text/document payloads.
      - `roadmap.md`: Updated Phase 4 guidance says use `rkyv` early, but keep the protocol small and behind a codec boundary.
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`: New protocol and behavior manifest surfaces must be added to the documentation registry and generated docs.
    - Options Considered:
      - Use JSON or bincode first: fastest to iterate, but delays proving the intended serialization stack.
      - Use `rkyv` directly throughout client/server code: performance-oriented, but couples business logic to archived representations too early.
      - Use `rkyv` behind a narrow codec abstraction: proves the intended stack while preserving protocol iteration flexibility.
    - Chosen Approach:
      - Add a shared `protocol` module with owned Rust message enums and a `codec` module responsible for `rkyv` serialization, validation, and length-prefixed framing. Keep archived-value handling inside the codec or small dispatch boundary.
      - Include a minimal behavior manifest shape now, even if it only describes built-in client-first text behavior in Phase 4. This keeps the protocol aligned with the server-authoritative/client-executed behavior decision without adding `deno_core`, hot reload, or arbitrary extension execution in this phase.
    - API Notes and Examples:
      ```rust
      #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
      pub enum ClientMessage {
          Hello { protocol_version: u32, client_name: String },
          Edit {
              document_id: u64,
              base_version: u64,
              behavior_version: u64,
              transaction_id: u64,
              operation: EditOperation,
          },
          EditorIntent {
              document_id: u64,
              base_version: u64,
              behavior_version: u64,
              intent: EditorIntent,
          },
      }

      #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
      pub enum ServerMessage {
          Welcome { client_id: u64 },
          InitialDocument { document_id: u64, version: u64, text: String, access: DocumentAccess },
          BehaviorManifest { behavior_version: u64, manifest: BehaviorManifest },
          EditAck { document_id: u64, version: u64, transaction_id: u64 },
          EditTransaction { document_id: u64, version: u64, transaction_id: u64, operations: Vec<EditOperation> },
          Error { message: String },
      }
      ```
    - Files to Create/Edit:
      - `src/protocol.rs` or `src/protocol/mod.rs`: Shared message enums and document/edit identifiers.
      - `src/protocol/codec.rs`: `rkyv` encode/decode and length-prefixed frame helpers.
      - `Cargo.toml`: Adjust `rkyv` features if validation support requires feature changes.
      - `src/lib.rs` if the project is split into library plus binaries.
    - References:
      - Context7 docs response for `/rkyv/rkyv` derive and validated access.
      - `concept.md` section 3 on `rkyv` serialization.
      - `roadmap.md` Phase 4 updated serialization guidance.
  - Test Cases to Write:
    - `protocol_round_trips_client_hello`: Encodes and decodes `ClientMessage::Hello` through the codec.
    - `protocol_round_trips_initial_document`: Encodes and decodes a snapshot containing Unicode text, version, and editable/read-only access state.
    - `protocol_round_trips_behavior_manifest`: Encodes and decodes a minimal manifest that marks basic text insertion as client-first predictable behavior.
    - `codec_rejects_oversized_frame`: A length prefix above the configured maximum is rejected before allocation.
    - `codec_rejects_invalid_archive_bytes`: Malformed payload bytes do not produce a message.

- [ ] Add a Tokio Unix Domain Socket server skeleton
  - Acceptance Criteria:
    - Functional: A server binary or server module can bind a Unix socket, accept one or more client connections, respond to `Hello`, send an initial in-memory document snapshot and minimal behavior manifest, receive edit operations/intents, apply them to server-owned canonical state, and send edit acknowledgements or simple edit transactions.
    - Performance: The server uses async I/O without blocking the Tokio runtime on socket reads/writes or edit dispatch; per-connection work is spawned or otherwise isolated enough not to block the accept loop. Edit ordering is per-document rather than globally serialized across all documents.
    - Code Quality: Server startup, socket lifecycle, connection handling, and document state are separated into small modules; stale socket cleanup is handled clearly for development runs. Document state is shaped as a future per-document actor/owner rather than a stateless mirror.
    - Security: Socket path selection avoids accidentally binding in unsafe shared locations without clear permissions; frame-size limits and decode errors close or reject bad connections gracefully.
  - Approach:
    - Documentation Reviewed:
      - Context7 Tokio 1.49 docs: `tokio::net::UnixListener::bind`, `UnixListener::accept`, `UnixStream::connect`, and `UnixStream` implementing async read/write traits.
      - Tokio docs: `AsyncReadExt`/`AsyncWriteExt` style methods such as reading exact frame headers and writing full buffers are the standard async I/O path.
      - `roadmap.md` Phase 4: start with Unix Domain Sockets on Linux/macOS and leave Windows named pipes behind an abstraction.
    - Options Considered:
      - Integrate server as an in-process async task first: easy to test, but does not prove the process/IPC seam.
      - Create a separate binary now: best matches Thick Client / Asynchronous Server architecture and makes IPC explicit.
      - Support Windows named pipes immediately: more complete, but outside the Phase 4 Linux/macOS skeleton scope.
    - Chosen Approach:
      - Add a Tokio server entry point that binds a Unix socket path, accepts client connections, uses the shared codec for framed `rkyv` messages, and owns a canonical in-memory document string or rope placeholder until Phase 5/6. This keeps the server from becoming a stateless behavior service while avoiding full Phase 5 synchronization complexity.
      - Structure server document state as a small per-document owner/actor boundary where practical. Phase 4 can accept all edits without full stale rejection, but edit application should still be ordered through the server-owned document state.
      - Send an initial minimal behavior manifest along with or after the initial document snapshot so the client can treat ordinary text editing as client-first predictable behavior.
    - API Notes and Examples:
      ```rust
      use tokio::net::UnixListener;
      use tokio::io::{AsyncReadExt, AsyncWriteExt};

      let listener = UnixListener::bind(&socket_path)?;
      let (stream, _addr) = listener.accept().await?;
      ```
    - Files to Create/Edit:
      - `src/bin/clay-server.rs` or `src/server.rs`: Server entry point and lifecycle.
      - `src/server/connection.rs`: Per-client connection loop.
      - `src/server/document.rs`: Minimal in-memory document state and edit application placeholder.
      - `src/protocol/*`: Shared codec/message usage.
      - `Cargo.toml`: Add binary targets only if needed.
    - References:
      - Context7 docs response for Tokio UnixListener/UnixStream async I/O.
      - `concept.md` section 2 on asynchronous Rust server as authoritative source of truth.
      - `roadmap.md` Phase 4 server skeleton scope.
  - Test Cases to Write:
    - `server_accepts_hello_and_sends_snapshot`: A Tokio test connects a UnixStream client and verifies welcome/snapshot flow.
    - `server_sends_minimal_behavior_manifest`: Client receives a behavior manifest declaring basic text edits as client-first predictable behavior.
    - `server_acknowledges_insert_edit`: Client sends an insert operation with document/behavior version metadata and receives `EditAck`.
    - `server_rejects_invalid_frame_without_panic`: Malformed frames do not crash the server connection task.

- [ ] Refactor the native app into a client unit that can load server snapshots
  - Acceptance Criteria:
    - Functional: The existing Masonry editor can initialize from a server-provided document snapshot and minimal behavior manifest instead of only a hardcoded local empty/default buffer.
    - Performance: Loading a snapshot creates or replaces the local editor buffer once and does not introduce whole-buffer extraction in the paint path after initialization. Installing a behavior manifest is a small configuration update, not executable script loading.
    - Code Quality: Client connection/bootstrap code is separate from `EditorSurface`, `EditorBuffer`, and `EditorWidget`; Masonry remains responsible for native window/widget/render lifecycle. The editor stores document version, behavior version, and access mode separately from rendering/layout state.
    - Security: Client bootstrap connects only to the configured local IPC endpoint and treats server messages as fallible decoded input; no filesystem, network, extension, script, or WASM authority is added.
  - Approach:
    - Documentation Reviewed:
      - `concept.md`: Client owns high-frequency local state and a lightweight shadow copy, while server is authoritative.
      - `plans/002-Phase1-TextCanvasFoundation.md`: editor state boundaries are buffer, viewport, layout, painting, and Masonry widget responsibilities.
      - `plans/003-Phase2-EditorInteractionModel.md`: cursor/selection/edit behavior is local and should remain immediate.
    - Options Considered:
      - Rewrite the editor around server state immediately: too large and risks losing Phase 2 behavior.
      - Add a snapshot-loading API to the existing editor surface/buffer: minimal change that supports initial server state.
      - Delay client integration and test server only: does not prove the actual Clay client/server seam.
    - Chosen Approach:
      - Add explicit buffer replacement/snapshot initialization APIs and a client bootstrap layer that receives `InitialDocument` and `BehaviorManifest` before or during app startup. Keep local editing state and rendering unchanged after snapshot load.
      - Store the document access mode returned by the server. Phase 4 can start with one editable client, but the client surface should be able to represent read-only state without reworking editor rendering later.
    - API Notes and Examples:
      ```rust
      pub struct InitialDocument {
          pub document_id: u64,
          pub version: u64,
          pub text: String,
          pub access: DocumentAccess,
      }

      editor.load_snapshot(document_id, version, text, access);
      editor.install_behavior_manifest(behavior_version, manifest);
      ```
    - Files to Create/Edit:
      - `src/client.rs` or `src/client/mod.rs`: Client IPC bootstrap and server message handling.
      - `src/editor/buffer.rs`: Add or expose safe buffer replacement from owned text.
      - `src/editor/surface.rs`: Add document snapshot loading and reset caret/selection/viewport state.
      - `src/main.rs` or `src/bin/clay-client.rs`: Start the client app with server-provided initial state.
      - `src/masonry_editor.rs`: Accept initial editor state if construction currently assumes default state.
    - References:
      - `concept.md` section 4 on canonical/shadow state split.
      - Phase 1 and Phase 2 plans on preserving editor state boundaries.
  - Test Cases to Write:
    - `editor_load_snapshot_replaces_text_and_resets_caret`: Snapshot text becomes visible editor content and caret/selection are valid.
    - `client_handles_initial_document_message`: Client bootstrap converts a decoded server message into editor initial state.
    - `client_installs_minimal_behavior_manifest`: Client stores the behavior version and applies manifest-declared client-first text behavior without script execution.
    - Manual smoke test: Start server, start client, confirm server-provided text appears and editor remains usable.

- [ ] Emit client edit operations from local editor mutations
  - Acceptance Criteria:
    - Functional: Insert, newline, Backspace, Delete, selected-range replacement, and selected-range deletion produce basic client edit operations with valid byte offsets/ranges while preserving immediate local edits when the installed behavior manifest marks them client-first predictable.
    - Performance: Edit operation creation reuses known cursor/selection/range information from the local edit path and does not inspect or clone the whole document. Ordinary edits are enqueued asynchronously with bounded backpressure.
    - Code Quality: Local editor commands return structured edit descriptions or events without coupling `EditorSurface` directly to socket I/O; IPC sending is handled by a client-side queue/connection layer. Events carry document version and behavior version metadata where available.
    - Security: Only inert text edit operations are emitted; keyboard/pointer actions do not become commands, file paths, scripts, network requests, extension calls, or arbitrary behavior execution.
  - Approach:
    - Documentation Reviewed:
      - `plans/003-Phase2-EditorInteractionModel.md`: editor mutations are byte-offset/range based over `crop`, with selection replacement and deletion already represented internally.
      - `concept.md`: client will eventually send edits with version tracking; Phase 4 intentionally omits version enforcement.
      - `roadmap.md`: Phase 4 sends basic edit operations; Phase 5 adds versions, stale rejection, resync, and region locks.
    - Options Considered:
      - Compute diffs after every local edit: generic but expensive and unnecessary because edit commands already know the changed range.
      - Let editor surface send directly to IPC: simple but tangles UI state with transport.
      - Return local edit events and have the client layer enqueue protocol messages: clean seam for Phase 5 versioned edit messages.
    - Chosen Approach:
      - Introduce an `EditorEditEvent` or equivalent internal edit description returned by successful editor commands. The Masonry/client boundary forwards these events to an async client connection queue that serializes `ClientMessage::Edit` with the current document version, behavior version, and client transaction ID.
      - Treat Phase 4 version fields as protocol shape and observability data, not full synchronization enforcement. Full stale-edit rejection and resync remain Phase 5.
    - API Notes and Examples:
      ```rust
      pub enum EditOperation {
          Insert { byte_offset: u64, text: String },
          Delete { start: u64, end: u64 },
          Replace { start: u64, end: u64, text: String },
      }
      ```
    - Files to Create/Edit:
      - `src/editor/surface.rs`: Return structured edit events from mutation commands.
      - `src/editor/buffer.rs`: Ensure edit helpers expose changed ranges/caret outcomes.
      - `src/client.rs` or `src/client/connection.rs`: Queue and send edit protocol messages.
      - `src/masonry_editor.rs`: Forward edit events without blocking UI handling.
      - `src/protocol.rs`: Shared `EditOperation` protocol type.
    - References:
      - `plans/003-Phase2-EditorInteractionModel.md` cursor, selection, and range-edit tasks.
      - `roadmap.md` Phase 4 and Phase 5 boundary.
  - Test Cases to Write:
    - `insert_command_emits_insert_operation`: Typing at a caret returns the expected byte offset and text.
    - `edit_event_carries_behavior_version`: A client-first edit event includes the installed behavior version for server validation.
    - `selection_replacement_emits_replace_operation`: Replacing selected text emits a normalized range and replacement text.
    - `backspace_emits_delete_operation`: Backspace at a Unicode boundary emits the previous scalar range.
    - `editor_events_do_not_block_without_ipc_consumer`: Local editing still works if no sender is attached in tests.

- [ ] Wire end-to-end client/server acknowledgement flow
  - Acceptance Criteria:
    - Functional: With server and client running, the client connects, receives initial text and behavior manifest, edits locally for manifest-declared client-first text behavior, sends edit messages, and receives acknowledgements without blocking the GUI.
    - Performance: IPC send/receive work does not run synchronously in Masonry paint or text-event handlers; GUI input remains optimistic and redraw-on-demand. No Phase 4 keypress requires a synchronous server/JavaScript round trip.
    - Code Quality: Async connection tasks communicate with the synchronous Masonry UI through a narrow channel or bootstrap boundary; connection errors are surfaced clearly without panics.
    - Security: Failed decodes, disconnects, oversized frames, and server error messages are handled as recoverable local errors; no reconnection storm or unbounded queue growth is introduced.
  - Approach:
    - Documentation Reviewed:
      - Tokio docs: `UnixStream::connect`, async read/write traits, and spawned async tasks are appropriate for non-blocking IPC clients.
      - Masonry Phase 0-2 plans: Masonry owns the event loop, widget routing, and render lifecycle; editor input should not block.
      - `roadmap.md`: Phase 4 proves initial document and edit operation delivery only, not full synchronization correctness.
    - Options Considered:
      - Block startup until server snapshot arrives: easiest for first integration, but should be bounded with clear errors.
      - Run client IPC on a background Tokio runtime/thread and communicate via channels: avoids blocking Masonry and prepares for ongoing acknowledgements.
      - Fully merge Tokio and Masonry event loops: likely too complex and unnecessary for this skeleton.
    - Chosen Approach:
      - Use a small client IPC runtime/task boundary. For the first implementation, either obtain the initial snapshot and behavior manifest before launching Masonry or launch with a loading/default state and apply them through a UI-safe channel if available. Edits are sent over a bounded queue to the IPC task.
      - Keep acknowledgements observational in Phase 4 unless the server returns a correction transaction. Phase 5 will make version confirmation, stale rejection, and resync authoritative.
    - API Notes and Examples:
      ```rust
      let stream = tokio::net::UnixStream::connect(socket_path).await?;
      send_frame(&mut stream, &ClientMessage::Hello {
          protocol_version: 1,
          client_name: "clay-client".to_string(),
      }).await?;
      ```
    - Files to Create/Edit:
      - `src/client/connection.rs`: Async connection loop and bounded outgoing edit queue.
      - `src/main.rs` or `src/bin/clay-client.rs`: Client/server startup wiring.
      - `src/masonry_editor.rs`: Non-blocking edit-event forwarding.
      - `src/protocol/codec.rs`: Shared frame read/write helpers used by both sides.
    - References:
      - Context7 Tokio docs response for UnixStream async I/O.
      - `concept.md` Thick Client / Asynchronous Server model.
      - `roadmap.md` Phase 4 expected outcome.
  - Test Cases to Write:
    - `end_to_end_client_receives_initial_snapshot`: Integration test starts server, connects client transport, and receives text.
    - `end_to_end_client_receives_behavior_manifest`: Integration test receives the minimal behavior manifest before client-first edits are emitted.
    - `end_to_end_edit_gets_acknowledged`: Integration test sends an edit with version metadata and receives `EditAck`.
    - Manual smoke test: Run server and client, type in the client, confirm no GUI stalls and server logs/observes edit acknowledgements.

- [ ] Preserve Phase 2 behavior and document Phase 4 compromises
  - Acceptance Criteria:
    - Functional: Phase 2 editor behavior still passes automated tests and manual smoke testing after IPC integration.
    - Performance: Existing bounded extraction and layout cache tests remain valid; IPC integration does not add whole-buffer paint extraction or blocking GUI edit paths.
    - Code Quality: `cargo fmt`, `cargo test`, and `cargo check` pass; new client/server/protocol tests are deterministic and clean up temporary socket paths.
    - Security: The completed phase still has no extension execution, no SDUI commands, no file workspace authority, no region locks, no remote network listener, and no AI mutation privileges.
  - Approach:
    - Documentation Reviewed:
      - Rust/Cargo standard tooling: `cargo fmt`, `cargo test`, and `cargo check` are the verification baseline established in earlier plans.
      - `plans/001-003`: Prior phases require preserving local editor behavior, bounded rendering assumptions, and no server/extension/file authority until their planned phases.
    - Options Considered:
      - Mark Phase 4 complete after server/client integration only: risks losing the local-editor guarantees that make optimistic editing viable.
      - Run full regression and record compromises: preserves confidence before Phase 5 versioned synchronization.
    - Chosen Approach:
      - Run all Cargo verification commands and a final manual client/server smoke test. Update this plan with completed checkboxes, compromises made, and concrete follow-up actions.
    - API Notes and Examples:
      ```bash
      cargo fmt
      cargo test
      cargo check
      cargo run --bin clay-server
      cargo run --bin clay-client
      ```
    - Files to Create/Edit:
      - `plans/005-Phase4-IPC-Client-Server-Skeleton.md`: Mark completed tasks and fill post-implementation notes.
      - Any test files/modules added during Phase 4.
    - References:
      - `plans/001-Phase0-NativeTextCanvas.md`
      - `plans/002-Phase1-TextCanvasFoundation.md`
      - `plans/003-Phase2-EditorInteractionModel.md`
      - `roadmap.md` Phase 4 expected outcome.
  - Test Cases to Write:
    - `phase3_regression_commands`: `cargo fmt`, `cargo test`, and `cargo check` all pass.
    - Manual GUI/IPC smoke test: Start server, start client, verify initial text, edit locally, observe acknowledgements, resize/scroll/select/navigate, and exit cleanly.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
