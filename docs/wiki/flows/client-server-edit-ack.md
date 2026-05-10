# Client/Server Edit Acknowledgement Flow

## Source

- `src/client/mod.rs`
- `src/masonry_editor.rs`
- `src/main.rs`
- `src/server/connection.rs`
- `src/server/document.rs`

## Overview

The Phase 4 client now keeps the IPC connection open after the initial snapshot handshake. Local editor mutations can be forwarded over a bounded queue to a background Tokio connection task, and server acknowledgements are received asynchronously as client connection events. This proves the local Unix socket edit/ack loop without making the GUI wait for server confirmation.

## Responsibilities

- `client::connect` opens the Unix socket, performs the `Hello` / `Welcome` / `InitialDocument` / `BehaviorManifest` handshake, and returns a `ClientSession`.
- `ClientSession` contains the initial editor state, a `ClientEditQueue` for outgoing edits, and an event receiver for acknowledgements/errors.
- The background client task owns the socket after startup, serializes outgoing `ClientMessage::Edit` values, and receives `ServerMessage::EditAck`, `EditTransaction`, and `Error` messages.
- `src/main.rs` parses `clay server`, `clay client`, and bare `clay` modes. It keeps a multi-thread Tokio runtime alive while Masonry runs, logs client IPC events to stderr for Phase 4 observability, and keeps send/receive work off the GUI input and paint paths.
- `EditorWidget` optionally forwards edit events to the queue but still renders local edits immediately.

## How It Works

Startup uses a shared default Unix socket path from `src/ipc.rs`: `$XDG_RUNTIME_DIR/clay.sock` when available, otherwise a stable per-user temp socket. `clay server` starts the foreground server on that socket unless a path is supplied. `clay client` opens a client and attaches to that socket when a server is already running. Bare `clay` first tries to attach; if no server is reachable, it spawns the same executable as a separate background `clay server` process and then opens the client against it. Because that server is a separate process, closing the auto-opened client does not stop the server; it must be killed explicitly.

Client startup calls `client::connect`. The handshake is bounded by the existing five-second startup timeout. Once the initial snapshot and manifest have been read, `connect_from_stream` creates:

1. A bounded outgoing edit channel used by `ClientEditQueue`.
2. A bounded connection event channel used for acknowledgements and recoverable connection state.
3. A background Tokio task that owns the connected `UnixStream`.

The background task splits the stream and uses `tokio::select!` to handle either an outgoing edit or an incoming server frame. Outgoing edits are encoded through the shared `Codec`. Incoming `EditAck` frames become `ClientConnectionEvent::EditAck`; server errors and decode/I/O failures become non-panicking events.

`src/main.rs` passes both the server-provided initial state and edit queue into `EditorWidget`, and spawns a small event logger that prints `ClientConnectionEvent` values such as `EditAck` to stderr. The widget assigns client transaction IDs and calls `try_send` through `ClientEditQueue`. If the queue is missing or full, the local edit has already happened and the UI remains responsive; later phases can surface stronger pending-edit diagnostics.

## Code Examples

```bash
# foreground server
cargo run -- server

# attach a client to the running server
cargo run -- client

# auto-start a server if needed, then open a client
cargo run
```

## Invariants and Constraints

- No Masonry input or paint handler performs socket reads/writes or waits for an acknowledgement.
- The outgoing edit queue is bounded to avoid unbounded memory growth under server stalls.
- The server remains authoritative for the canonical document and version increments.
- Phase 4 observes acknowledgements but does not yet update local confirmed-version state, reject stale edits, or resync; those are Phase 5 responsibilities.
- IPC input is still decoded and validated through the shared length-prefixed `rkyv` codec.
- No JavaScript execution, file/workspace authority, remote listener, extension loading, or AI mutation authority is added by this flow.

## Tests

- `src/client/mod.rs`: `end_to_end_client_receives_initial_snapshot` validates the connected client receives a snapshot during handshake.
- `src/client/mod.rs`: `end_to_end_client_receives_behavior_manifest` validates manifest delivery before edit emission.
- `src/client/mod.rs`: `end_to_end_edit_gets_acknowledged` validates queued edit send and ack receipt with a paired socket.
- `src/client/mod.rs`: `real_server_end_to_end_edit_gets_acknowledged` validates the same edit/ack path through `IpcServer` on a real Unix socket.
- `src/main.rs`: CLI parser tests validate `server`, `client`, and bare auto modes.
- Relevant commands: `cargo test client --quiet`, `cargo test server --quiet`, `cargo test --quiet`.

## Related

- [Client Edit Emission](client-edit-emission.md)
- [Client Snapshot Bootstrap](../modules/client-snapshot-bootstrap.md)
- [Server IPC Skeleton](../modules/server-ipc-skeleton.md)
- [Protocol Codec](../modules/protocol-codec.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
