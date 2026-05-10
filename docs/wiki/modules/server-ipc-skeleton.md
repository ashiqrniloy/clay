# Server IPC Skeleton

## Source

- `src/bin/clay-server.rs`
- `src/server/mod.rs`
- `src/server/connection.rs`
- `src/server/document.rs`
- `src/protocol/codec.rs`

## Overview

The Phase 4 server skeleton is a Tokio Unix Domain Socket server. It proves the IPC/process seam without adding full Phase 5 synchronization, file workspace authority, extension execution, SDUI, remote listeners, or AI mutation privileges.

## How It Works

`clay-server` chooses a socket path from the first CLI argument, or defaults to `$XDG_RUNTIME_DIR/clay.sock` when available. `IpcServer::run` validates the parent directory, removes only stale socket files, binds `UnixListener`, and keeps accepting connections. Each accepted client is handled in a spawned Tokio task so one connection does not block the accept loop.

Each connection must send `ClientMessage::Hello` first. The server responds with:

1. `ServerMessage::Welcome`
2. `ServerMessage::InitialDocument`
3. `ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1))`

After the handshake, edit messages and editor intents are translated into `EditOperation`s and applied to `DocumentState`. The document state owns the canonical Phase 4 in-memory string and validates document IDs, editable access, byte ranges, and UTF-8 boundaries before mutating. Successful edits increment the server document version and return `EditAck`.

## Invariants and Constraints

- Socket I/O uses Tokio async reads/writes; connection handling is isolated from the accept loop.
- Wire messages continue to go through `Codec`; server code does not call `rkyv` directly.
- Frame-size validation and archive validation happen before messages reach the server dispatch loop.
- Stale socket cleanup removes only filesystem socket nodes and refuses to replace normal files.
- Version fields are carried for Phase 5 compatibility, but stale edit rejection and resync are intentionally deferred.

## Tests

- `src/server/connection.rs`: handshake, initial document, behavior manifest, edit acknowledgement, and malformed-frame handling.
- `src/server/document.rs`: edit application and UTF-8 boundary rejection.
- `src/server/mod.rs`: listener-level Unix socket accept smoke test.
- Relevant commands: `cargo test server`, `cargo test protocol`, `cargo check`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Server Document State](server-document-state.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `roadmap.md`
