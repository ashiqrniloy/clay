# Versioned Text Synchronization

## Source

- `src/protocol/mod.rs`
- `src/client/mod.rs`
- `src/editor/surface.rs`
- `src/masonry_editor.rs`
- `src/server/connection.rs`
- `src/server/document.rs`
- `src/protocol/codec.rs`

## Overview

Phase 5 synchronizes text with a server-authoritative document version and optimistic client shadow state. The editor applies manifest-declared text edits locally so typing stays immediate, while the client sends byte-range deltas to the server with a base document version, behavior version, lease metadata, and transaction ID. The server accepts only edits based on its current canonical version and returns an acknowledgement with the confirmed version; recoverable rejections trigger a simple snapshot resync.

## Responsibilities

- `EditorSurface` owns the local shadow buffer, caret, selection, viewport, behavior manifest, and document access metadata.
- `ClientEditQueue` assigns optimistic base versions, records pending transactions, and sends bounded non-blocking edit messages.
- The background client connection task writes queued edits, receives acknowledgements/rejections/resync snapshots, and updates shared sync state.
- `DocumentState` owns canonical `crop::Rope` text, server document version, lease validation, byte-range validation, region-lock checks, and accepted-version advancement.
- `Codec` remains the only wire serialization boundary; synchronization logic works with owned protocol messages after frame-size and archive validation.

## How It Works

Startup returns `ClientInitialState` from the `Hello` / `Welcome` / `InitialDocument` / `BehaviorManifest` handshake. The initial document includes the server document ID, canonical version, full text, access state, and optional lease ID. `EditorWidget::with_initial_state` loads that snapshot into `EditorSurface`; `connect_from_stream` creates a `ClientEditQueue` initialized with the server-confirmed version and access metadata.

When a text command mutates an editable editor surface, `EditorSurface::command_with_event` updates the local shadow immediately and emits an `EditorEditEvent` only when the installed behavior manifest declares client-first text editing. `EditorWidget::local_command` passes that event to `ClientEditQueue::enqueue_edit_event`; it does not perform socket I/O or wait for the server.

`ClientEditQueue` reserves the current optimistic version as the outgoing `base_version`, advances the optimistic version locally, stores a `PendingEdit`, and uses `try_send` on a bounded Tokio channel. If the queue is full or the client lacks a lease, the reservation is rolled back and the already-local UI path remains non-blocking.

The background connection task serializes outgoing `ClientMessage::Edit` values and concurrently reads server messages. `ServerMessage::EditAck` advances `confirmed_version` and removes the matching pending transaction. `ServerMessage::EditRejected` removes the rejected pending transaction and is surfaced as a `ClientConnectionEvent::EditRejected`. Rejections caused by stale/future versions, lease loss, read-only access, or region locks cause the client task to send `ClientMessage::RequestResync` with its last confirmed version.

On `RequestResync`, the server extracts a full snapshot from the canonical rope and replies with `ServerMessage::ResyncSnapshot` for the requesting client's current access state. The client applies that snapshot to its sync state by setting confirmed and optimistic versions to the snapshot version, clearing all pending edits, and storing `last_resync`. `EditorWidget::apply_connection_event` is the UI-safe boundary that applies a real resync snapshot to `EditorSurface::load_snapshot`, replacing text and resetting caret, selection, viewport, and local document metadata.

## Code Examples

```rust
// Local typing stays immediate; queueing is bounded and non-blocking.
let outcome = editor.command_with_event(EditorCommand::Insert("x"));
if let Some(event) = outcome.edit_event {
    edit_queue.enqueue_edit_event(event, transaction_id)?;
}
```

```rust
// Recoverable sync rejection causes the client task to request a snapshot.
ClientMessage::RequestResync {
    document_id,
    client_id,
    known_version: confirmed_version,
}
```

## Invariants and Constraints

- The server is authoritative for canonical text and document version increments.
- Client confirmed versions advance only from server acknowledgements or resync snapshots.
- Client optimistic versions advance when edits are queued, not when keys are pressed or acknowledged.
- Pending edits remain until acknowledgement, rejection, queue rollback, or resync recovery.
- Ordinary edit IPC carries deltas only; full text appears only in initial snapshots and explicit resync snapshots.
- Masonry input and paint handlers do not perform socket reads/writes, wait for acknowledgements, execute JavaScript, or serialize full documents.
- Base-version checks are constant-time server metadata checks before lease, range, lock, and rope mutation work.
- IPC bytes are bounded and validated by `Codec` before synchronization code sees protocol messages.
- Phase 5 does not add file/workspace authority, remote listeners, extension execution, SDUI expansion, shell/network access, or AI mutation.

## Tests

- `src/client/mod.rs`: `client_ack_advances_confirmed_version` validates acknowledgement-driven confirmed version updates.
- `src/client/mod.rs`: `client_keeps_pending_edit_until_ack_or_rejection` validates deterministic pending transaction bookkeeping.
- `src/client/mod.rs`: `client_requests_resync_after_stale_rejection` validates automatic resync requests after recoverable stale rejection.
- `src/client/mod.rs`: `client_applies_resync_snapshot_and_clears_pending_edits` validates snapshot recovery and pending cleanup.
- `src/client/mod.rs`: `real_server_end_to_end_stale_edit_rejected_then_resynced` validates stale rejection and resync through a real Unix socket server.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot` validates the UI resync boundary.
- Relevant commands: `cargo test client --quiet`, `cargo test server --quiet`, `cargo test --quiet`.

## Related

- [Client Edit Emission](client-edit-emission.md)
- [Client/Server Edit Acknowledgement Flow](client-server-edit-ack.md)
- [Document Leases and Region Locks](document-leases-and-region-locks.md)
- [Server Document State](../modules/server-document-state.md)
- [Protocol Codec](../modules/protocol-codec.md)
- `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`
