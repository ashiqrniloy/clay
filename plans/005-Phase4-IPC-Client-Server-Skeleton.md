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
- Phase 3 documentation-as-code requirements are followed for any new server-side Rust public functions, public programmatic capabilities, and configuration surfaces: expose through Clay JS APIs where applicable, document in Markdown with user-facing names/key bindings/custom properties, link from `docs/index.md`, update generated registries, and keep internal implementation details in the code wiki.

## Tasks

- [x] Define the minimal `rkyv` protocol and length-prefixed codec boundary
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
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`: New public programmatic capabilities must be documented as Clay JS APIs with Markdown, registry, and lookup coverage when they are exposed.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Server-side Rust public functions must have Clay JS facades or be made private/`pub(crate)`.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Registry/docs checks should fail without mutating generated files.
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
      - `docs/wiki/index.md`: Created initial code wiki navigation for the protocol codec implementation.
      - `docs/wiki/modules/protocol-codec.md`: Documented protocol messages, codec framing, validation, constraints, and tests.
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
  - Completed Notes:
    - Added `src/protocol/mod.rs` with shared owned protocol types, Phase 4 version fields, edit deltas/intents, inert behavior manifests, document access, acknowledgements/transactions, and error responses.
    - Added `src/protocol/codec.rs` with a bounded `Codec` API for `rkyv` encode/decode and 4-byte big-endian length-prefixed frames. Decode rejects oversize/mismatched frames before payload allocation and validates archived bytes with `rkyv::from_bytes`/bytecheck before returning owned messages.
    - Added `src/lib.rs` so shared protocol code can be used by future client/server binaries and tests.
    - Added the protocol codec wiki page required by project wiki maintenance.
    - Verification passed: `cargo fmt`, `cargo test protocol --quiet`, `cargo check --quiet`, and `cargo test --quiet`.

- [x] Add a Tokio Unix Domain Socket server skeleton
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
  - Completed Notes:
    - Added `src/bin/clay-server.rs` as the Phase 4 server binary. It accepts an optional socket path argument and otherwise uses `$XDG_RUNTIME_DIR/clay.sock` when available, with a process-scoped temp fallback for development.
    - Added `src/server/mod.rs` with `IpcServer`, `ServerConfig`, Unix socket binding, stale socket cleanup that removes only socket nodes, and an async accept loop that spawns isolated per-connection tasks.
    - Added `src/server/connection.rs` with the required `Hello` handshake, `Welcome`/`InitialDocument`/minimal `BehaviorManifest` responses, edit and editor-intent dispatch, decode-error handling, and edit acknowledgements.
    - Added `src/server/document.rs` with server-owned canonical in-memory document state, editable/read-only checks, document ID validation, UTF-8 byte-boundary/range validation, edit application, and version increments.
    - Extended `src/protocol/codec.rs` with async Tokio frame read/write helpers so server code uses the shared bounded `rkyv` codec boundary instead of direct serialization calls.
    - Added `docs/wiki/modules/server-ipc-skeleton.md` and linked it from `docs/wiki/index.md` to document the server lifecycle, handshake, document state, constraints, and tests.
    - Verification passed: `cargo fmt`, `cargo test server --quiet`, `cargo test protocol --quiet`, and `cargo check --quiet`.

- [x] Refactor the native app into a client unit that can load server snapshots
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
      - `src/lib.rs`: Expose the client/editor/widget units to the package binary and tests.
      - `docs/wiki/index.md`: Link the client snapshot bootstrap implementation page.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Document client bootstrap, snapshot loading, behavior manifest storage, constraints, and tests.
    - References:
      - `concept.md` section 4 on canonical/shadow state split.
      - Phase 1 and Phase 2 plans on preserving editor state boundaries.
  - Test Cases to Write:
    - `editor_load_snapshot_replaces_text_and_resets_caret`: Snapshot text becomes visible editor content and caret/selection are valid.
    - `client_handles_initial_document_message`: Client bootstrap converts a decoded server message into editor initial state.
    - `client_installs_minimal_behavior_manifest`: Client stores the behavior version and applies manifest-declared client-first text behavior without script execution.
    - Manual smoke test: Start server, start client, confirm server-provided text appears and editor remains usable.
  - Completed Notes:
    - Added `src/client/mod.rs` with a bounded Tokio Unix socket bootstrap that sends `Hello`, receives `Welcome`/`InitialDocument`/`BehaviorManifest`, validates message ordering through the shared `Codec`, returns structured `ClientInitialState`, and reports fallible IPC/input as `ClientBootstrapError` without panics.
    - Exposed `editor` and `masonry_editor` from `src/lib.rs` so the package binary can be a thin client shell over shared client/editor units.
    - Added `EditorSurface::load_snapshot`, `EditorSurface::install_behavior_manifest`, and `EditorDocumentState` so the editor stores document ID, server version, access mode, behavior version, and manifest separately from rendering/layout state. Snapshot loading replaces the buffer once and resets caret, selection, viewport, layout cache, and scroll state.
    - Added `EditorWidget::with_initial_state` and updated `src/main.rs` to optionally load a server snapshot from the first CLI socket-path argument before launching Masonry, falling back to the existing empty local editor on bootstrap failure.
    - Added `docs/wiki/modules/client-snapshot-bootstrap.md` and linked it from `docs/wiki/index.md`.
    - Verification passed: `cargo fmt`, `cargo test client --quiet`, `cargo test editor_load_snapshot_replaces_text_and_resets_caret --quiet`, `cargo test --quiet`, and `cargo check --quiet`.

- [x] Emit client edit operations from local editor mutations
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
      - `docs/wiki/index.md`: Link the client edit emission flow page.
      - `docs/wiki/flows/client-edit-emission.md`: Document local edit events, manifest gating, bounded queueing, constraints, and tests.
    - References:
      - `plans/003-Phase2-EditorInteractionModel.md` cursor, selection, and range-edit tasks.
      - `roadmap.md` Phase 4 and Phase 5 boundary.
  - Test Cases to Write:
    - `insert_command_emits_insert_operation`: Typing at a caret returns the expected byte offset and text.
    - `edit_event_carries_behavior_version`: A client-first edit event includes the installed behavior version for server validation.
    - `selection_replacement_emits_replace_operation`: Replacing selected text emits a normalized range and replacement text.
    - `backspace_emits_delete_operation`: Backspace at a Unicode boundary emits the previous scalar range.
    - `editor_events_do_not_block_without_ipc_consumer`: Local editing still works if no sender is attached in tests.
  - Completed Notes:
    - Added `EditorEditEvent` and `EditorCommandOutcome` in `src/editor/surface.rs`. Insert, newline, Backspace, Delete, selected-range replacement, and selected-range deletion now apply locally first and return protocol `EditOperation` deltas with document ID, base document version, and behavior version when the document is editable and the installed behavior manifest declares the operation client-first.
    - Kept existing boolean editor command APIs as compatibility wrappers over the event-returning path, so Phase 2 tests and callers continue to work.
    - Added `ClientEditQueue` in `src/client/mod.rs`, backed by a bounded Tokio `mpsc` channel and `try_send`, to convert editor events into `ClientMessage::Edit` without socket I/O in editor code.
    - Updated `EditorWidget` in `src/masonry_editor.rs` to optionally forward edit events to a client queue with monotonic transaction IDs while ignoring absent/full queues so local input does not block.
    - Added tests for insert metadata, behavior-version metadata, selected replacement, Unicode-boundary Backspace, selected Delete, local editing without an IPC consumer, event-to-message conversion, and bounded queue backpressure.
    - Added `docs/wiki/flows/client-edit-emission.md` and linked it from `docs/wiki/index.md`.
    - Verification passed: `cargo fmt`, `cargo test editor --quiet`, `cargo test client --quiet`, `cargo test --quiet`, and `cargo check --quiet`.

- [x] Wire end-to-end client/server acknowledgement flow
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
      - `docs/wiki/index.md`: Link the client/server acknowledgement flow page.
      - `docs/wiki/flows/client-server-edit-ack.md`: Document startup, background IPC, acknowledgement events, constraints, and tests.
    - References:
      - Context7 Tokio docs response for UnixStream async I/O.
      - `concept.md` Thick Client / Asynchronous Server model.
      - `roadmap.md` Phase 4 expected outcome.
  - Test Cases to Write:
    - `end_to_end_client_receives_initial_snapshot`: Integration test starts server, connects client transport, and receives text.
    - `end_to_end_client_receives_behavior_manifest`: Integration test receives the minimal behavior manifest before client-first edits are emitted.
    - `end_to_end_edit_gets_acknowledged`: Integration test sends an edit with version metadata and receives `EditAck`.
    - Manual smoke test: Run server and client, type in the client, confirm no GUI stalls and server logs/observes edit acknowledgements.
  - Completed Notes:
    - Added `ClientSession` and `ClientConnectionEvent` in `src/client/mod.rs`. `client::connect` now performs the initial handshake, keeps the Unix socket open, and returns initial state plus a bounded edit queue and acknowledgement/error event receiver.
    - Added a background Tokio client connection loop that uses `tokio::select!` to write queued `ClientMessage::Edit` values and read server `EditAck`, `EditTransaction`, and `Error` frames without blocking the GUI.
    - Updated `src/main.rs` to parse `clay server`, `clay client`, and bare `clay` modes. `clay server` starts the foreground server, `clay client` attaches to the shared default socket when a server is running, and bare `clay` starts a separate background `clay server` process if one is not reachable before opening the client. The auto-started server is not embedded in the client process, so closing the client does not stop it. The client path keeps a multi-thread Tokio runtime alive while Masonry runs, passes `EditorWidget` both the server snapshot and edit queue, and logs client IPC acknowledgement/error events to stderr for Phase 4 observability.
    - Preserved optimistic local editing: `EditorWidget` still applies edits immediately and only performs bounded `try_send` forwarding from input handlers.
    - Added end-to-end client tests for snapshot receipt, behavior manifest receipt, edit acknowledgement over a paired socket, and edit acknowledgement through the real `IpcServer` over a Unix socket.
    - Added `src/ipc.rs` with the shared default socket path used by the main `clay` binary and the `clay-server` compatibility binary.
    - Added `docs/wiki/flows/client-server-edit-ack.md` and linked it from `docs/wiki/index.md`.
    - Verification passed: `cargo fmt`, `cargo test client --quiet`, `cargo test server --quiet`, `cargo test --quiet`, and `cargo check --quiet`.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: The Phase 4 implementation is reviewed and the Clay JS APIs needed for extensibility, configuration, customization, user search/help, key binding, AI-agent discovery, and future public programmatic use are proposed or created. All server-side Rust public functions introduced or changed by Phase 4 are inventoried; each has a stable Clay JS/TS facade API backed by an explicit `deno_core` op wrapper when it is a public programmatic capability, or is made private/`pub(crate)` when it should remain internal. Every Clay JS API has Markdown docs linked from `docs/index.md`, generated registry coverage, and lookup access.
    - Performance: Clay JS API and documentation checks do not add synchronous work to Masonry input/paint paths or ordinary edit IPC; JavaScript remains server-side and outside the keypress hot path.
    - Code Quality: Rust implementation functions, op wrappers, JS/TS facade exports, Markdown docs, generated registry entries, and lookup metadata use stable names that are easy to map in tests. Each API doc includes a searchable user-facing name, default key bindings or an empty key binding list, and custom properties for behavior-changing settings.
    - Security: Raw `Deno.core.ops.op_*` calls and arbitrary Rust functions are not user-facing APIs; authority checks remain at the server/API boundary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay plans require a JS API task for public programmatic surfaces and Rust public functions.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public programmatic APIs are Clay JS facades backed by explicit ops.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Clay JS API docs include user-facing names, key binding metadata, and custom properties.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Clay JS API docs are Markdown-authoritative and registry-generated.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Tests must fail for missing APIs, docs, index links, stale registries, or lookup gaps.
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`: Approved Rust-to-JS exposure boundary.
    - Options Considered:
      - Leave Phase 4 server/client public Rust functions undocumented until `deno_core` is embedded: rejected because it creates drift before extension APIs arrive.
      - Document raw protocol/server Rust functions directly as the user API: rejected because Clay JS APIs are the public programmatic surface.
      - Add explicit inventory and coverage now while allowing internal helpers to become private/`pub(crate)`: chosen to keep Phase 4 aligned with Phase 3 without adding JavaScript runtime execution.
    - Chosen Approach:
      - During Phase 4 implementation, review protocol/server/client surfaces for extensibility and customization, inventory server-side Rust public functions, and either route them through documented Clay JS API facade metadata or narrow visibility. For APIs that are not executable until the server-side JS runtime phase, add stable planned facade docs/metadata and tests that preserve the mapping without putting JavaScript on the hot path.
    - API Notes and Examples:
      ```text
      server Rust function -> deno_core op wrapper -> Clay JS/TS facade -> docs/reference/clay-js-api/** -> docs/index.md -> generated registry -> lookup test
      ```
    - Files to Create/Edit:
      - `src/server/**`: Narrow internal helper visibility or mark public API functions for facade coverage.
      - `src/protocol/**`: Ensure protocol helpers exposed as public programmatic capabilities have Clay JS API documentation or remain internal.
      - `docs/reference/clay-js-api/**/*.md`: Add/update Clay JS API docs for Phase 4 public capabilities, including user-facing names, key bindings, and custom properties.
      - `docs/index.md`: Link new Clay JS API docs.
      - `generated/**` or equivalent: Update generated registry artifacts using the project command.
      - `tests/docs_contract.rs` or module tests: Add Phase 4 coverage mappings.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`
  - Test Cases to Write:
    - `phase4_server_public_rust_functions_have_clay_js_api`: Fails when a Phase 4 server-side Rust public function lacks a Clay JS API or is not made non-public.
    - `phase4_clay_js_api_docs_are_indexed_and_generated`: Fails when Phase 4 Clay JS API docs are missing from `docs/index.md`, generated registry output, or lookup APIs.
    - `phase4_clay_js_api_discovery_metadata_is_complete`: Fails when user-facing name, key binding metadata, or custom property metadata is missing or malformed.
    - `phase4_generated_doc_registry_is_current`: Fails with `cargo run --bin update-doc-registry` instructions when generated registry artifacts are stale.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The Phase 4 implementation is reviewed for user-configurable behavior, key bindings, IPC/server/client customization points, and extensibility needs. Any configuration option introduced by Phase 4 is represented as a Clay JS API documented in Markdown, linked from `docs/index.md`, included in generated registry output, and lookup-accessible. The plan preserves `~/.config/clay/init.js` as the configuration entry point, with modular file loading supported when runtime configuration loading is implemented.
    - Performance: Configuration APIs and docs do not add synchronous JavaScript, IPC, or server work to Masonry input/paint paths; ordinary typing remains client-first where the behavior manifest permits it.
    - Code Quality: Configuration uses documented Clay JS APIs and `custom_properties` metadata instead of ad hoc undocumented keys.
    - Security: Configuration does not implicitly grant filesystem, network, shell, extension loading, AI mutation, remote listener, or workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay plans require a separate configuration task when adding user-visible behavior or public programmatic surfaces.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration is loaded from `~/.config/clay/init.js` and each option is a Clay JS API.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Configuration APIs need user-facing names, key bindings, and custom properties.
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`: Approved configuration model.
    - Options Considered:
      - Defer all configuration review until a later preferences phase: rejected because Phase 4 introduces protocol/server/client surfaces that should not accidentally become unconfigurable or undocumented.
      - Add ad hoc config keys for IPC paths or behavior toggles: rejected because every configuration option is a Clay JS API.
      - Review Phase 4 surfaces and document planned configuration APIs without putting JS on the hot path: chosen to preserve Phase 4 scope and the Phase 3 documentation contract.
    - Chosen Approach:
      - Review Phase 4 protocol, server startup, client bootstrap, behavior manifest, and editor edit-event surfaces for configuration needs. Document any configuration APIs as Clay JS APIs with `custom_properties`; if runtime config loading is out of scope, mark them as planned docs/metadata and add coverage tests so the mapping does not drift.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      import { configureIpc } from "clay:client";

      configureIpc({ socketPath: "~/.local/state/clay/clay.sock" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**/*.md`: Add/update configuration API docs for Phase 4 surfaces.
      - `docs/index.md`: Link configuration API docs.
      - `generated/**` or equivalent: Update generated registry artifacts using the project command.
      - `tests/docs_contract.rs` or module tests: Add configuration metadata coverage.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - `phase4_configuration_apis_are_documented`: Fails when Phase 4 configuration APIs lack Markdown docs, index links, generated registry entries, or lookup access.
    - `phase4_configuration_custom_properties_are_complete`: Fails when behavior-changing configuration settings are absent from `custom_properties`.
    - `phase4_configuration_does_not_enter_input_hot_path`: Verifies configuration handling does not add synchronous JavaScript/IPC/server work to editor input handling.

- [x] Update or verify the code wiki after Phase 4 implementation
  - Acceptance Criteria:
    - Functional: After Phase 4 implementation and tests pass, the code wiki documents or verifies the new client/server/protocol/codec/document-state implementation and links every relevant page from `docs/wiki/index.md`.
    - Performance: Wiki updates add no runtime work and document hot-path guarantees: no blocking IPC in input/paint handlers, bounded queues, delta edits, and no synchronous server/JavaScript round trip for ordinary typing.
    - Code Quality: Wiki pages explain what changed code does, how it works, key invariants/tradeoffs, source/test paths, and examples where useful; public API usage links to authoritative `docs/reference/` pages instead of duplicating them.
    - Security: Wiki pages document IPC validation, frame-size bounds, socket-path assumptions, and absent authorities such as extension execution, SDUI, file workspace authority, remote listeners, and AI mutation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow, public-reference linking boundary, and quality bar.
      - `.agents/skills/project-wiki/references/page-template.md`: Use the default page template when creating substantial pages.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic checks for wiki/docs maintenance where practical.
    - Options Considered:
      - Update wiki after each Phase 4 task: more granular, but noisy while protocol/server boundaries are still changing.
      - Update once after Phase 4 verification: chosen to keep implementation education aligned with final code.
    - Chosen Approach:
      - After Phase 4 implementation and verification pass, update `docs/wiki/` pages for protocol/codec, server lifecycle, client bootstrap, editor edit-event flow, and end-to-end acknowledgement flow. Link to Clay JS API reference docs for public programmatic usage.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/protocol-codec.md
      docs/wiki/modules/server-document-state.md
      docs/wiki/flows/client-server-edit-ack.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Link new or updated Phase 4 implementation pages.
      - `docs/wiki/modules/protocol-codec.md`: Explain protocol messages, codec boundary, validation, and tests.
      - `docs/wiki/modules/server-document-state.md`: Explain server document ownership and edit application placeholder.
      - `docs/wiki/flows/client-server-edit-ack.md`: Explain initial snapshot, manifest install, local edit event, async send, and ack flow.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-wiki/references/page-template.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm `docs/wiki/index.md` links Phase 4 pages and the pages explain implementation flow, source paths, tests, performance constraints, and security boundaries.
  - Completed Notes:
    - Verified the existing Phase 4 wiki pages for protocol codec, server IPC lifecycle, client snapshot bootstrap, client edit emission, and client/server acknowledgement flow.
    - Added `docs/wiki/modules/server-document-state.md` to document server-owned canonical text, edit validation, UTF-8 boundary checks, version increments, acknowledgement generation, and Phase 5 synchronization boundaries.
    - Updated `docs/wiki/index.md` so every Phase 4 wiki page is discoverable, and cross-linked the server document state page from the server IPC page.
    - Verified all `docs/wiki/**/*.md` pages except the index are linked from `docs/wiki/index.md` with a deterministic `python3` check.
    - Verification passed: `cargo test --quiet` and `cargo check --quiet`.
    - The Clay JS API and configuration API tasks remain intentionally unchecked/skipped for now per user direction.

- [x] Preserve Phase 2 behavior and document Phase 4 compromises
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
    - `phase4_regression_commands`: `cargo fmt`, `cargo test`, and `cargo check` all pass.
    - Manual GUI/IPC smoke test: Start server, start client, verify initial text, edit locally, observe acknowledgements, resize/scroll/select/navigate, and exit cleanly.
  - Completed Notes:
    - Ran final regression verification: `cargo fmt`, `cargo test --quiet`, and `cargo check --quiet` all passed.
    - Automated tests include the preserved editor behavior covered by earlier Phase 2-style unit tests for insertion, newline, Backspace/Delete, cursor movement, selection, bounded visible text extraction, and layout/cache behavior, plus Phase 4 protocol/client/server tests.
    - IPC integration preserves the Phase 2 hot path: Masonry input handlers apply local editor edits immediately, emit only delta edit events, and use bounded non-blocking queue forwarding instead of synchronous socket or server calls.
    - Manual GUI/IPC smoke testing was performed during the task sequence: `cargo run` / `cargo run -- client` produced server `EditAck` events with monotonically increasing versions and transaction IDs while typing continued locally.
    - Confirmed the completed Phase 4 scope still excludes extension execution, SDUI commands, file workspace authority, region locks, remote network listeners, and AI mutation privileges.
    - The Clay JS API and configuration API tasks remain intentionally unchecked/skipped per user direction; they are not blockers for preserving Phase 2 behavior or documenting Phase 4 implementation compromises.

## Compromises Made
- Full Phase 5 synchronization is intentionally deferred. Phase 4 carries document version, behavior version, and transaction IDs, but acknowledgements are observational; the client does not yet maintain confirmed-version state, reject stale edits, resync after divergence, or merge concurrent edits.
- Server document state is an in-memory placeholder. It proves server ownership, edit validation, UTF-8 boundary checks, and version increments, but does not persist files, manage workspaces, or model multi-document/project state.
- Bare `clay` auto-starts a separate background `clay server` process when no server is reachable. This matches the desired process lifetime semantics, but Phase 4 does not yet provide a first-class server shutdown/status command beyond normal process management.
- Client connection errors and server acknowledgements are logged to stderr for Phase 4 observability. They are not yet surfaced in the UI with diagnostics, pending-edit indicators, reconnect controls, or resync prompts.
- The outgoing edit queue is bounded and non-blocking. If the queue is absent/full, local edits continue and the send failure is intentionally not user-visible yet; Phase 5 should decide how to track unsent/pending edits.
- The Clay JS API and configuration API plan tasks were skipped by user direction. Phase 4 therefore has internal Rust/client/server APIs and CLI behavior, but no completed Clay JS facade/configuration documentation pass for these surfaces yet.

## Further Actions
- High priority: Implement Phase 5 confirmed-version tracking, stale edit rejection, server correction transactions, reconnect/resync behavior, and multi-client convergence tests.
- High priority: Add user-visible client IPC status, pending-edit/error reporting, and an explicit server lifecycle command set such as status/stop once CLI UX is designed.
- Medium priority: Complete the skipped Clay JS API and configuration API review so public programmatic surfaces and configurable IPC behavior are documented through the approved Clay JS facade model.
- Medium priority: Replace the in-memory server document placeholder with a proper document/workspace owner when file/workspace authority is introduced in a later phase.
- Low priority: Convert the ad hoc wiki-link validation command into a checked repository test or docs validation script if wiki maintenance grows.
