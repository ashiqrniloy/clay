# Document Leases and Region Locks

## Source

- `src/protocol/mod.rs`
- `src/server/document.rs`
- `src/server/connection.rs`
- `src/client/mod.rs`
- `src/editor/surface.rs`

## Overview

Phase 5 adds server-owned edit authority for the current in-memory document. A document can have one editable lease holder; additional clients are read-only observers. The same server document state also owns in-memory region locks that reject overlapping user edits before canonical rope mutation. These mechanisms protect the server-authoritative document without introducing AI, extension, file, shell, network, or workspace authority.

## Responsibilities

- `DocumentState` grants and releases the single editable lease, validates edit messages against the active lease, and reports read-only/lease failures as recoverable protocol rejections.
- `DocumentState` registers internal in-memory region locks, validates lock ranges, detects overlap with incoming edits, and returns conflict metadata.
- `ServerMessage::InitialDocument` and `ServerMessage::ResyncSnapshot` carry `DocumentAccess` and optional lease ID so the client knows whether it can emit edits.
- `ClientEditQueue` refuses to enqueue edit messages without an editable lease.
- `EditorSurface` prevents local text mutation in read-only access while preserving navigation and selection behavior.

## How It Works

During handshake, `send_welcome_snapshot_and_manifest` locks the shared `DocumentState` and calls `acquire_access(client_id)`. If no lease is active, the client receives `DocumentAccess::Editable { lease_id }`; if another client holds the lease, the new client receives `DocumentAccess::ReadOnly`. A reconnecting holder can receive the same editable access while it remains active. When a connection ends or errors, `handle_connection` calls `release_access(client_id)`, which clears the active lease only for the holder.

Every edit message includes both `client_id` and `lease_id`. `DocumentState::apply_edit` rejects edits before mutation when there is no active lease, the message omits a lease, or the client/lease pair does not match the active holder. Missing authority returns `LeaseRequired`; stale, guessed, or replayed lease IDs return `LeaseExpired { lease_id }`. Read-only clients are stopped twice: `EditorSurface` does not produce mutation events for read-only snapshots, and `ClientEditQueue` rolls back pending reservations instead of sending an edit when no lease ID is configured.

Region locks are server-internal metadata stored in `DocumentState::region_locks`. `register_region_lock` validates that each lock range is non-empty, in bounds, and aligned to UTF-8 character boundaries, then records a lock ID, byte range, owner metadata, and the document version at which the lock was created. Phase 5 exposes conflict metadata in protocol rejections, but it does not expose public lock-management APIs or AI/extension mutation authority.

Before rope mutation, the server converts each edit into an affected range. Inserts conflict when the insertion offset falls inside a locked half-open range. Delete and replace spans conflict when their half-open byte range overlaps a lock. Empty replace ranges are treated like inserts so a client cannot bypass a lock by changing operation shape. A conflict returns `EditRejection::RegionLocked { conflict }` with the lock ID, range, owner, and creation version; the canonical rope, document version, and last transaction ID remain unchanged.

## Code Examples

```rust
// First client becomes the editable lease holder.
let access = document.acquire_access(7);
assert_eq!(access, DocumentAccess::Editable { lease_id: 1 });

// Later clients are observers until the lease is released.
let observer_access = document.acquire_access(8);
assert_eq!(observer_access, DocumentAccess::ReadOnly);
```

```rust
// Internal lock registration blocks overlapping edits.
let lock_id = document.register_region_lock(6, 11, LockOwner::Server)?;
let response = document.apply_edit(
    1,
    7,
    Some(1),
    1,
    42,
    EditOperation::Insert { byte_offset: 8, text: "x".to_string() },
);
```

## Invariants and Constraints

- A document has at most one active editable lease.
- Only the active lease holder can reach range validation and canonical rope mutation.
- Missing, guessed, replayed, or stale lease metadata cannot grant mutation authority.
- Read-only observer clients can navigate/select but cannot locally mutate through normal editor commands or enqueue IPC edits through `ClientEditQueue`.
- Region lock ranges are byte based, non-empty, in bounds, and UTF-8 boundary aligned.
- Lock checks are bounded by active lock count and avoid scanning document text when no locks exist.
- Rejected lease or lock edits do not mutate text, increment versions, or update transaction metadata.
- Lock owner variants include future server/client/extension/AI metadata for explanations only; Phase 5 does not add extension execution, AI mutation, file/workspace access, shell/network access, remote listeners, or SDUI commands.

## Tests

- `src/server/document.rs`: `first_client_receives_editable_lease` validates initial lease grant.
- `src/server/document.rs`: `second_client_receives_read_only_access` validates observer access.
- `src/server/document.rs`: `server_rejects_edit_without_current_lease` validates missing/wrong lease rejection.
- `src/server/document.rs`: `lease_released_or_retained_on_disconnect_matches_policy` validates deterministic lease release behavior.
- `src/editor/surface.rs`: `read_only_editor_allows_navigation_but_not_mutation` validates observer UI behavior.
- `src/client/mod.rs`: `read_only_client_queue_does_not_emit_edit_message` validates queue-side authority enforcement.
- `src/server/document.rs`: region-lock tests validate insert/delete conflicts, non-overlapping edits, invalid lock range rejection, and conflict metadata.
- `src/server/mod.rs`: `real_server_end_to_end_region_locked_edit_rejected` validates region-lock conflicts across the real Unix socket IPC path.
- Relevant commands: `cargo test server --quiet`, `cargo test client --quiet`, `cargo test --quiet`.

## Related

- [Versioned Text Synchronization](versioned-text-synchronization.md)
- [Server Document State](../modules/server-document-state.md)
- [Client Edit Emission](client-edit-emission.md)
- [Protocol Codec](../modules/protocol-codec.md)
- `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`
