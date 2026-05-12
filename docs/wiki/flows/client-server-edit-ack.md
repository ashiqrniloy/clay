# Client/Server Edit Acknowledgement Flow

## Source

- `src/client/mod.rs`
- `src/masonry_editor.rs`
- `src/main.rs`
- `src/server/connection.rs`
- `src/server/document.rs`

## Overview

The client keeps the IPC connection open after the initial snapshot handshake. Local editor mutations are forwarded over a bounded queue to a background Tokio connection task, and server acknowledgements or rejections are received asynchronously as client connection events. In Phase 5 the queue also tracks confirmed server versions, optimistic local versions, and pending transactions so strict server base-version enforcement does not make the GUI wait for acknowledgements.

## Responsibilities

- `client::connect` opens the Unix socket, performs the `Hello` / `Welcome` / `InitialDocument` / `BehaviorManifest` handshake, and returns a `ClientSession`.
- `ClientSession` contains the initial editor state, a `ClientEditQueue` for outgoing edits, and an event receiver for acknowledgements/errors.
- `ClientEditQueue` owns shared synchronization metadata: confirmed server version, optimistic local version, and pending transactions.
- The background client task owns the socket after startup, serializes outgoing `ClientMessage::Edit` values, receives `ServerMessage::EditAck`, `EditRejected`, `ResyncSnapshot`, `EditTransaction`, and `Error` messages, and sends `RequestResync` after recoverable synchronization rejections.
- The server connection task validates each edit/intent `behavior_version` against the server-owned active behavior manifest before mutating the canonical document.
- `src/main.rs` parses `clay server`, `clay client`, and bare `clay` modes. It keeps a multi-thread Tokio runtime alive while Masonry runs, logs client IPC events to stderr for Phase 4 observability, and keeps send/receive work off the GUI input and paint paths.
- `EditorWidget` optionally forwards edit events to the queue, still renders local edits immediately, and exposes a narrow `apply_connection_event` boundary that applies real resync snapshots with `EditorSurface::load_snapshot`.

## How It Works

Startup uses a shared default Unix socket path from `src/ipc.rs`: `$XDG_RUNTIME_DIR/clay.sock` when available, otherwise a stable per-user temp socket. `clay server` starts the foreground server on that socket unless a path is supplied. `clay client` opens a client and attaches to that socket when a server is already running. Bare `clay` first tries to attach; if no server is reachable, it spawns the same executable as a separate background `clay server` process and then opens the client against it. Because that server is a separate process, closing the auto-opened client does not stop the server; it must be killed explicitly.

`IpcServer` owns an `ActiveBehaviorManifest` alongside the canonical `DocumentState`. The handshake sends the active manifest after `Welcome` and `InitialDocument`. The manifest is not reconstructed per connection, so future server-side hot reload can validate and publish one replacement state that all connections observe.

Client startup calls `client::connect`. The handshake is bounded by the existing five-second startup timeout. During the server handshake, `DocumentState::acquire_access` grants the first connected client an editable lease and sends later clients read-only observer snapshots. Once the initial snapshot and manifest have been read, `connect_from_stream` creates:

1. A bounded outgoing edit channel used by `ClientEditQueue`.
2. A bounded connection event channel used for acknowledgements and recoverable connection state.
3. A background Tokio task that owns the connected `UnixStream`.

`ClientEditQueue::enqueue_edit_event` reserves the current optimistic version as the outgoing edit's `base_version`, records the transaction in the pending queue, advances the optimistic version locally, and then uses bounded `try_send`. If the queue has no editable lease, or if the channel is full, the reservation is rolled back and the UI remains responsive. This allows multiple local edits to be sent without waiting for the previous acknowledgement while preserving per-document base-version ordering for editable clients only.

The background task splits the stream and uses `tokio::select!` to handle either an outgoing edit or an incoming server frame. Outgoing edits are encoded through the shared `Codec`. Incoming `EditAck` frames update confirmed-version state, remove the pending transaction, and become `ClientConnectionEvent::EditAck`. Incoming `EditRejected` frames remove the rejected pending transaction and become `ClientConnectionEvent::EditRejected`. Stale/future version, invalid behavior version, lease, read-only, and region-lock rejections immediately send a `ClientMessage::RequestResync` with the client's last confirmed version. Incoming `ResyncSnapshot` frames replace the client synchronization snapshot, set confirmed and optimistic versions to the server version, clear all pending edits, and become `ClientConnectionEvent::ResyncSnapshot`. Server errors and decode/I/O failures become non-panicking events.

`src/main.rs` passes both the server-provided initial state and edit queue into `EditorWidget`, and spawns a small event logger that prints `ClientConnectionEvent` values such as `EditAck` to stderr. The widget assigns client transaction IDs and calls `try_send` through `ClientEditQueue`. If the queue is missing or full, the local edit has already happened and the UI remains responsive. `EditorWidget::apply_connection_event` is the UI-safe resync boundary: it ignores non-resync events and applies a real `ResyncSnapshot` through `EditorSurface::load_snapshot`, which resets caret, selection, viewport, and local document metadata.

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
- Client confirmed-version state advances only from `EditAck`; optimistic version state advances locally when an edit is queued.
- Pending transactions stay queued until the corresponding acknowledgement, rejection, or resync recovery arrives.
- Only the current server lease holder can successfully mutate; observer clients keep read-only access metadata and cannot enqueue or pass server validation for edits.
- Strict stale/future base-version enforcement happens on the server before text mutation; simple Phase 5 recovery uses full snapshots only after explicit resync requests.
- Behavior-version enforcement is a server-owned manifest metadata check before document mutation; rejected behavior versions do not advance the canonical document version.
- IPC input is still decoded and validated through the shared length-prefixed `rkyv` codec.
- No JavaScript execution, file/workspace authority, remote listener, extension loading, or AI mutation authority is added by this flow.

## Tests

- `src/client/mod.rs`: `end_to_end_client_receives_initial_snapshot` validates the connected client receives a snapshot during handshake.
- `src/client/mod.rs`: `end_to_end_client_receives_behavior_manifest` validates manifest delivery before edit emission.
- `src/client/mod.rs`: `end_to_end_edit_gets_acknowledged` validates queued edit send and ack receipt with a paired socket.
- `src/client/mod.rs`: `client_ack_advances_confirmed_version` validates ack-driven confirmed-version updates and pending cleanup.
- `src/client/mod.rs`: `client_keeps_pending_edit_until_ack_or_rejection` validates deterministic pending transaction bookkeeping.
- `src/client/mod.rs`: `client_requests_resync_after_stale_rejection` validates automatic resync requests after recoverable rejection.
- `src/client/mod.rs`: `client_applies_resync_snapshot_and_clears_pending_edits` validates Unicode snapshot recovery and pending cleanup.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot` validates the UI-safe resync event boundary.
- `src/client/mod.rs`: `end_to_end_second_client_is_read_only` validates duplicate client observer access through `IpcServer` on a real Unix socket.
- `src/client/mod.rs`: `real_server_end_to_end_edit_gets_acknowledged` validates the same edit/ack path through `IpcServer` on a real Unix socket.
- `src/client/mod.rs`: `real_server_end_to_end_stale_edit_rejected_then_resynced` validates stale-version rejection and explicit resync recovery through `IpcServer` on a real Unix socket.
- `src/server/mod.rs`: `real_server_end_to_end_region_locked_edit_rejected` validates region-lock conflict metadata across the real Unix socket server path.
- `src/server/connection.rs`: `server_rejects_edit_with_stale_behavior_version_without_mutating_document` validates behavior-version mismatch rejection before canonical mutation.
- `src/server/behavior.rs`: replacement publishing tests validate deterministic manifest version advancement and invalid replacement rollback.
- `src/main.rs`: CLI parser tests validate `server`, `client`, and bare auto modes.
- Relevant commands: `cargo test client --quiet`, `cargo test server --quiet`, `cargo test --quiet`.

## Related

- [Client Edit Emission](client-edit-emission.md)
- [Versioned Text Synchronization](versioned-text-synchronization.md)
- [Document Leases and Region Locks](document-leases-and-region-locks.md)
- [Client Snapshot Bootstrap](../modules/client-snapshot-bootstrap.md)
- [Server IPC Skeleton](../modules/server-ipc-skeleton.md)
- [Protocol Codec](../modules/protocol-codec.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
