# Protocol Codec

## Source

- `src/protocol/mod.rs`
- `src/protocol/codec.rs`

## Overview

The protocol module defines the shared client/server IPC message contract. It uses owned Rust message types for business logic and keeps `rkyv` serialization, validation, and socket framing behind `Codec`.

## Responsibilities

- Represent handshake messages: `Hello`, `Welcome`, `InitialDocument`, inert behavior manifests, document access, edit deltas/intents, acknowledgements, transactions, and errors.
- Represent Phase 5 synchronization metadata: client IDs, editable lease IDs, base document versions, behavior versions, confirmed server versions, stale-edit/read-only/lease/region-lock rejections, and resync snapshots.
- Encode and decode messages as `rkyv` payloads with a big-endian 4-byte length prefix.
- Reject oversized, incomplete, mismatched, or invalid frames before callers receive a protocol message.
- Avoid adding executable behavior, extension authority, file workspace authority, SDUI, or AI mutation privileges.

## How It Works

`src/protocol/mod.rs` contains owned message enums and IDs. Ordinary text edits are represented as deltas (`Insert`, `Delete`, `Replace`) with byte ranges and inserted text rather than full-document payloads. Phase 5 edit messages include `document_id`, `client_id`, optional `lease_id`, `base_version`, `behavior_version`, and `transaction_id` so the server can validate authority and ordering before mutation. `ServerMessage::EditAck` returns a server-confirmed version, while `EditRejected` carries recoverable sync reasons such as stale/future versions, lease failure, read-only access, invalid ranges, or region-lock conflicts. Full document text is carried by `InitialDocument` and `ResyncSnapshot` only.

`DocumentAccess::Editable { lease_id }` records the editable lease in the access state, while read-only observers use `DocumentAccess::ReadOnly`. Region-lock conflicts are described by `RegionLockConflict` and `LockOwner` metadata so later UI/AI phases can explain why an overlapping edit was rejected without granting AI, extension, file, shell, or network authority.

`BehaviorManifest::minimal_text_editing` declares predictable built-in text editing capabilities only; it is data, not script code.

`Codec` in `src/protocol/codec.rs` serializes a client or server message with `rkyv::to_bytes`, checks the payload against `max_frame_size`, then prefixes the payload with its 32-bit length. Decode first validates the declared length against the configured maximum and the actual payload size. It then copies the payload into an aligned `rkyv::util::AlignedVec` before calling `rkyv::from_bytes`, which performs checked archived-byte validation through `bytecheck` before deserializing to the owned message type.

## Code Examples

```rust
use clay::protocol::{codec::Codec, ClientMessage, PROTOCOL_VERSION};

let codec = Codec::default();
let frame = codec.encode_client_message(&ClientMessage::Hello {
    protocol_version: PROTOCOL_VERSION,
    client_name: "clay-client".to_string(),
})?;
let message = codec.decode_client_message(&frame)?;
```

## Invariants and Constraints

- `Codec` is the only protocol serialization boundary; client/server code should not call `rkyv` directly for wire messages.
- `DEFAULT_MAX_FRAME_SIZE` is 1 MiB to prevent accidental unbounded allocation from malformed IPC frames.
- The 4-byte frame prefix is not part of the archived payload, so decode realigns payload bytes before validation.
- Behavior manifests are inert declarations of built-in behavior and do not execute JavaScript, WASM, extensions, commands, or filesystem/network operations.

## Tests

- `src/protocol/codec.rs`: round-trip tests for hello, initial documents with Unicode, behavior manifests, lease/version edit deltas, stale-edit rejection, resync snapshots, and region-lock rejection metadata.
- `src/protocol/codec.rs`: rejection tests for oversized Phase 5 frames and invalid archived bytes.
- Relevant command: `cargo test protocol`.

## Related

- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`
- `concept.md`
- `roadmap.md`
