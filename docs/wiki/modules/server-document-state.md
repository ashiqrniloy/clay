# Server Document State

## Source

- `src/server/document.rs`
- `src/server/connection.rs`
- `src/protocol/mod.rs`
- `src/protocol/codec.rs`

## Overview

`DocumentState` is the Phase 4 server-owned canonical document placeholder. It is intentionally small: one in-memory UTF-8 `String`, one document ID, one server document version, and an editable/read-only access mode. The purpose is to prove that edits cross the IPC boundary and are applied by server-owned state before acknowledgements are issued, without implementing Phase 5 stale-edit rejection, resync, region locks, or persistence.

## Responsibilities

- Own the canonical Phase 4 text for the server process.
- Produce the initial `ServerMessage::InitialDocument` snapshot sent after a client `Hello`.
- Validate incoming edit operations against document ID, document access, byte ranges, and UTF-8 character boundaries.
- Apply valid `Insert`, `Delete`, and `Replace` operations to the canonical text.
- Increment the server document version once per accepted edit and return `ServerMessage::EditAck`.
- Return protocol `Error` messages for invalid document IDs, read-only documents, out-of-range edits, reversed ranges, and non-UTF-8-boundary offsets.

It does **not** persist documents, load files, execute behavior scripts, enforce stale client versions, merge concurrent edits, broadcast transactions to other clients, or expose workspace/file authority.

## How It Works

`IpcServer` owns an `Arc<tokio::sync::Mutex<DocumentState>>`. Each accepted client connection gets a clone of that shared owner. The connection task decodes client frames through `Codec`, then locks the document only while applying an edit or producing the initial snapshot.

`DocumentState::default` creates document `1` with the Phase 4 welcome text:

```text
Welcome to Clay's Phase 4 IPC server.
```

During handshake, `connection.rs` calls `initial_document_message`, which clones the current server text into a snapshot message. After handshake, `ClientMessage::Edit` carries a transaction ID and an `EditOperation`. `apply_edit` checks the document ID and access mode first. It then delegates range validation to helpers that convert protocol `u64` byte offsets into `usize`, reject offsets past the current text length, and require `String::is_char_boundary` so edits cannot split a multi-byte scalar.

On success, the operation mutates the server string, increments `version`, and returns:

```rust
ServerMessage::EditAck {
    document_id,
    version,
    transaction_id,
}
```

The acknowledgement version is the new server-side version after the mutation. Phase 4 clients log these acknowledgements for observability; they do not yet use them as confirmed-version state.

## Code Examples

```rust
let mut document = DocumentState::new(1, "Hi".to_string(), DocumentAccess::Editable);
let response = document.apply_edit(
    1,
    42,
    EditOperation::Insert {
        byte_offset: 2,
        text: " Clay".to_string(),
    },
);
```

The response is an `EditAck` with version `2` and transaction ID `42`.

## Invariants and Constraints

- The server is authoritative for canonical text and document version increments.
- Accepted edits increment the version exactly once.
- Invalid edits return protocol errors and do not mutate text or increment the version.
- Offsets and ranges are byte-based because the protocol and editor mutation events use byte offsets.
- Byte offsets must be valid UTF-8 boundaries before touching the `String`.
- Version fields from the client are protocol shape/observability data in Phase 4; stale edit rejection is deferred to Phase 5.
- Document state is protected by a Tokio mutex, so connection tasks do not mutate the canonical string concurrently.
- There is no file-system persistence, extension execution, SDUI command handling, remote listener, or AI mutation authority in this module.

## Tests

- `src/server/document.rs`: `document_state_applies_insert_and_acknowledges_version` validates mutation, version increment, and ack metadata.
- `src/server/document.rs`: `document_state_rejects_non_boundary_edit` validates UTF-8 boundary rejection.
- `src/server/connection.rs`: server connection tests validate that accepted edit messages become acknowledgements and malformed input is handled without panics.
- Relevant commands: `cargo test server --quiet`, `cargo test --quiet`.

## Related

- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Protocol Codec](protocol-codec.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
