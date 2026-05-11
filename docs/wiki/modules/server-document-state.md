# Server Document State

## Source

- `src/server/document.rs`
- `src/server/connection.rs`
- `src/protocol/mod.rs`
- `src/protocol/codec.rs`

## Overview

`DocumentState` is the server-owned canonical text state for the current in-memory document. In Phase 5 it stores text in a versioned `crop::Rope` rather than a plain `String`, while protocol snapshots remain `String` values at the IPC edge for initial load and resync.

The module proves the server-authoritative text model: client edit deltas cross the IPC boundary, the server validates byte offsets and access state, the canonical rope mutates, and only accepted mutations advance the server document version.

## Responsibilities

- Own the canonical server text as a `crop::Rope`.
- Track the document ID, server document version, current editable lease holder, next lease ID, active in-memory region locks, and most recent accepted transaction ID.
- Produce `ServerMessage::InitialDocument` snapshots for handshake and `ServerMessage::ResyncSnapshot` snapshots for explicit resync requests with client-specific editable/read-only access metadata.
- Validate incoming edit operations against document ID, base document version, document access, byte ranges, active region locks, and UTF-8 character boundaries before calling panicking `crop` mutation APIs.
- Reject stale or future client base versions before mutation and return explicit synchronization outcomes.
- Grant exactly one editable lease per document. The first connected client gets `DocumentAccess::Editable { lease_id }`; later clients get `DocumentAccess::ReadOnly` until the lease holder disconnects.
- Apply valid `Insert`, `Delete`, and `Replace` operations with `Rope::insert`, `Rope::delete`, and `Rope::replace`.
- Increment the server document version exactly once per accepted edit and return `ServerMessage::EditAck` with the confirmed version.
- Register in-memory region locks with byte ranges, owner metadata, and the server version at which each lock was created.
- Return `ServerMessage::EditRejected` for stale/future base versions, invalid document IDs, read-only documents, region-lock conflicts, out-of-range edits, reversed ranges, and non-UTF-8-boundary offsets.

It does **not** persist documents, load files, execute behavior scripts, merge concurrent edits, broadcast transactions to other clients, expose public lock-management UI/API, or expose workspace/file authority. Resync requests are supported by producing server-authoritative snapshots from the current rope state.

## How It Works

`IpcServer` owns an `Arc<tokio::sync::Mutex<DocumentState>>`. Each accepted client connection gets a clone of that shared owner. The connection task decodes client frames through `Codec`, then locks the document only while applying an edit or producing an initial/resync snapshot. `ClientMessage::RequestResync` returns a `ServerMessage::ResyncSnapshot` for the requested document ID or an error for an unknown document.

`DocumentState::new` converts the startup `String` into `crop::Rope::from(text)` and starts the server version at `1`. Snapshots call `Rope::to_string()` only for initial load or resync; ordinary acknowledged edits remain delta messages and do not serialize the full document.

Before mutation, `apply_edit` checks the target document ID, then compares the client-provided base version with the current canonical server version. Lower base versions return `EditRejection::StaleVersion`; higher base versions return `EditRejection::FutureVersion`. Neither case mutates the rope, advances the server version, or records the transaction. When the base version matches, the document validates that the sending client ID and lease ID match the current editable lease. Missing leases return `LeaseRequired`; guessed, replayed, or otherwise wrong leases return `LeaseExpired`. Only the lease holder can reach byte-range and region-lock validation.

Region locks live in `DocumentState::region_locks`. `register_region_lock` validates non-empty UTF-8 byte ranges and records `lock_id`, `start`, `end`, `owner`, and `created_at_version`. User edits are converted to an affected range before mutation. Inserts conflict when the insertion offset is inside a locked half-open range (`start <= offset < end`); delete/replace spans conflict when their half-open range overlaps a lock (`edit_start < lock.end && edit_end > lock.start`). Empty replace ranges are treated like inserts for lock purposes so clients cannot bypass a lock by changing operation shape. Conflicts return `EditRejection::RegionLocked` with `RegionLockConflict` metadata and do not mutate text or advance the version.

`apply_operation` validates byte offsets with helpers that:

1. Convert protocol `u64` offsets to `usize`.
2. Reject offsets past `Rope::byte_len()` before calling `Rope::is_char_boundary`, because `crop` panics for out-of-bounds checks.
3. Reject offsets that are not UTF-8 code point boundaries.
4. Reject ranges whose start is after their end.

Only after these checks does the module call the `crop` mutation API:

```rust
self.text.insert(offset, text);
self.text.delete(start..end);
self.text.replace(start..end, text);
```

On success, `version` increments once, `last_transaction_id` records the accepted transaction, and the server returns:

```rust
ServerMessage::EditAck {
    document_id,
    confirmed_version: self.version,
    transaction_id,
}
```

## Code Examples

```rust
let mut document = DocumentState::new(1, "Hello 🌎".to_string(), DocumentAccess::ReadOnly);
let access = document.acquire_access(7);
assert_eq!(access, DocumentAccess::Editable { lease_id: 1 });

let response = document.apply_edit(
    1,
    7,
    Some(1),
    1, // client base version must match the canonical server version
    42,
    EditOperation::Insert {
        byte_offset: 6,
        text: "Clay ".to_string(),
    },
);
```

The response is an `EditAck` with confirmed version `2`, and the canonical rope contains `Hello Clay 🌎`.

## Invariants and Constraints

- The server is authoritative for canonical text and document version increments.
- Accepted edits increment the version exactly once.
- Rejected edits do not mutate the rope, advance the version, or record a transaction ID.
- Offsets and ranges are byte-based because the protocol and editor mutation events use UTF-8 byte offsets.
- Byte offsets must be in bounds and at UTF-8 character boundaries before touching `crop` APIs that can panic.
- A document has at most one active editable lease; observer clients receive snapshots as read-only and cannot pass server lease validation.
- Region locks are in-memory server metadata. They block overlapping user edits after lease validation and before rope mutation.
- Region lock ranges are non-empty, in-bounds, and UTF-8 boundary aligned.
- Disconnecting the current lease holder releases the lease. Existing observers stay read-only until they reconnect or later explicit transfer UI exists.
- Initial/resync snapshots extract full text from the rope; ordinary edit acknowledgements do not send full-document text.
- The server version is authoritative; clients cannot advance it by sending forged future base versions.
- Version checks are constant-time metadata comparisons before any text mutation.
- Document state is protected by a Tokio mutex, so connection tasks do not mutate the canonical rope concurrently.
- Region-lock owner metadata can name server/client/extension/AI owners for future phases, but this module does not introduce extension execution or AI mutation authority.
- There is no file-system persistence, extension execution, SDUI command handling, remote listener, shell/network access, or AI mutation authority in this module.

## Tests

- `src/server/document.rs`: `server_document_uses_rope_for_insert_delete_replace` validates insert/delete/replace mutation and version acknowledgements.
- `src/server/document.rs`: `server_document_rejects_non_boundary_rope_edit_without_panic` validates UTF-8 boundary rejection before `crop` mutation.
- `src/server/document.rs`: `server_document_rejects_out_of_range_rope_edit` validates range checks prevent panics and preserve version state.
- `src/server/document.rs`: `server_document_snapshot_preserves_unicode` validates Unicode snapshot extraction from the rope.
- `src/server/document.rs`: `server_document_version_advances_once_per_accepted_edit` validates accepted/rejected version behavior and transaction metadata.
- `src/server/document.rs`: `server_accepts_edit_at_current_base_version`, `server_rejects_stale_base_version`, and `server_rejects_future_base_version` validate strict base-version enforcement.
- `src/server/document.rs`: `first_client_receives_editable_lease`, `second_client_receives_read_only_access`, `server_rejects_edit_without_current_lease`, and `lease_released_or_retained_on_disconnect_matches_policy` validate lease grant, observer, validation, and release behavior.
- `src/server/document.rs`: `server_rejects_insert_inside_region_lock`, `server_rejects_delete_overlapping_region_lock`, `server_accepts_edit_outside_region_lock`, `region_lock_range_validation_rejects_invalid_boundaries`, and `region_lock_conflict_reports_range_metadata` validate in-memory lock registration, overlap checks, version preservation on conflict, and protocol-ready rejection metadata.
- `src/server/connection.rs`: server connection tests validate handshake snapshots, edit acknowledgements, resync snapshot responses, and malformed input handling.
- `src/server/mod.rs`: `real_server_end_to_end_region_locked_edit_rejected` validates that in-memory region locks reject overlapping edits and preserve conflict metadata across the real Unix socket IPC path.
- Relevant commands: `cargo fmt`, `cargo test --quiet`, `cargo check --quiet`.

## Related

- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Protocol Codec](protocol-codec.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`
