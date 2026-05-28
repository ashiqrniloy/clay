# Server IPC Skeleton

## Source

- `src/bin/clay-server.rs`
- `src/server/mod.rs`
- `src/server/connection.rs`
- `src/server/document.rs`
- `src/protocol/codec.rs`
- `src/ipc.rs`

## Overview

The server skeleton is a Tokio local-IPC server with platform transports for Unix Domain Sockets and Windows named pipes, plus a platform-neutral endpoint model in `src/ipc.rs` for Unix socket paths and Windows local named pipe names. It proves the local IPC/process seam and now dispatches Phase 5 versioned edits, editable/read-only lease snapshots, explicit resync requests, and region-lock rejections without adding file workspace authority, extension execution, SDUI, remote listeners, shell/network access, or AI mutation privileges.

## How It Works

`src/ipc.rs` owns `IpcEndpoint`, default endpoint selection, smoke endpoint generation, endpoint display, and child-process argument conversion so app and binary code do not treat every IPC address as a filesystem path. On Unix, the default endpoint wraps `$XDG_RUNTIME_DIR/clay.sock` when available, otherwise a per-user temp socket. On Windows, the default endpoint is a local named pipe address of the form `\\.\pipe\clay-<user>`. `src/main.rs` uses the same endpoint model for default, client, server, and smoke launches; the smoke path starts a managed child server, polls for early child exit during bounded readiness retries, and reports categorized diagnostics before opening the GUI. `clay-server` parses a supplied endpoint through this abstraction and passes it into `ServerConfig`. On Unix, `IpcServer::run` validates the parent directory, removes only stale socket files, binds `UnixListener`, and keeps accepting connections. On Windows, `IpcServer::run` validates the local named-pipe prefix, creates a Tokio `NamedPipeServer`, awaits `connect()`, handles the already-connected race, then rotates back to create the next pipe instance. Each accepted client is handled in a spawned Tokio task so one connection does not block the accept loop, then the shared connection dispatcher runs over a generic Tokio `AsyncRead + AsyncWrite` stream rather than a Unix-specific stream type.

Each connection must send `ClientMessage::Hello` first. The server responds with:

1. `ServerMessage::Welcome`
2. `ServerMessage::InitialDocument`
3. `ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1))`

During the handshake, `DocumentState::acquire_access` grants the first connected client an editable lease and returns later clients as read-only observers. After the handshake, edit messages and editor intents are translated into `EditOperation`s and applied to the shared `DocumentState`. The document state owns the canonical Phase 5 `crop::Rope`, validates document IDs, base versions, lease authority, region locks, byte ranges, and UTF-8 boundaries before mutating, then returns `EditAck` only for accepted mutations.

`ClientMessage::RequestResync` is handled by extracting a bounded recovery snapshot from the canonical rope through `DocumentState::resync_snapshot_message_for_client`. The snapshot preserves the requesting client's current access state and lease metadata. Connection shutdown releases the editable lease only when the disconnected client is the active lease holder.

## Invariants and Constraints

- Socket and named-pipe I/O use Tokio async reads/writes; connection handling is isolated from the accept loop and transport-neutral after listener accept/connect.
- Wire messages continue to go through `Codec`; server code does not call `rkyv` directly.
- Frame-size validation and archive validation happen before messages reach the server dispatch loop.
- Endpoint construction is a cheap string/path selection step with no IPC, filesystem scan, shell execution, network listener, or blocking work.
- Default and smoke child servers are launched through `std::process::Command` with direct `server <endpoint>` arguments, inherited/controlled stdio, and no shell. Smoke readiness fails if the managed child exits before the client handshake succeeds.
- Stale socket cleanup is Unix-only, removes only filesystem socket nodes, and refuses to replace normal files.
- Windows endpoint defaults and transport bindings are local named pipe names, not TCP or remote listeners.
- Ordinary accepted edit responses are metadata acknowledgements; full text snapshots are reserved for initial load and explicit resync recovery.
- Version fields are enforced by `DocumentState` before mutation; stale/future edits are rejected and can trigger client resync.

## Tests

- `src/server/connection.rs`: handshake, initial document, behavior manifest, editable/read-only access, edit acknowledgement, resync response, and malformed-frame handling over generic in-memory async streams.
- `src/server/document.rs`: canonical rope edit application, base-version enforcement, lease validation, region-lock rejection, and UTF-8 boundary rejection.
- `src/ipc.rs`: endpoint tests verify platform-valid default endpoint selection, isolated smoke endpoints, and printable diagnostics.
- `src/main.rs`: launch tests verify direct child-process command construction, bounded readiness retry diagnostics, local-fallback messages, and early child-exit handling for smoke mode.
- `src/server/mod.rs`: listener-level Unix socket accept smoke test plus end-to-end stale-resync and region-lock rejection coverage.
- `src/client/mod.rs`: Windows named-pipe integration tests cover initial snapshot delivery, edit acknowledgement, read-only second-client behavior, and stale-edit resync recovery.
- Relevant commands: `cargo test server --quiet`, `cargo test protocol --quiet`, `cargo check --quiet`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Server Document State](server-document-state.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `roadmap.md`
