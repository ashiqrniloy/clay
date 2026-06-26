# Server IPC Skeleton

## Source

- `src/bin/clay-server.rs`
- `src/server/mod.rs`
- `src/server/connection.rs`
- `src/server/document.rs`
- `src/protocol/codec.rs`
- `src/ipc.rs`

## Overview

The server skeleton is a Tokio local-IPC server with platform transports for Unix Domain Sockets and Windows named pipes, plus a platform-neutral endpoint model in `src/ipc.rs` for Unix socket paths and Windows local named pipe names. It proves the local IPC/process seam and now dispatches Phase 5 versioned edits, editable/read-only lease snapshots, explicit resync requests, region-lock rejections, Phase 9 file/workspace commands, Phase 12 inert SDUI snapshots/actions, and Phase 13 runtime diagnostics without adding client filesystem authority, extension execution, remote listeners, shell/network access, or AI mutation privileges.

## How It Works

`src/ipc.rs` owns `IpcEndpoint`, default endpoint selection, smoke endpoint generation, endpoint display, and child-process argument conversion so app and binary code do not treat every IPC address as a filesystem path. On Unix, the default endpoint wraps `$XDG_RUNTIME_DIR/clay.sock` when available, otherwise a per-user temp socket. On Windows, the default endpoint is a local named pipe address of the form `\\.\pipe\clay-<user>`. `src/main.rs` uses the same endpoint model for default, client, server, and smoke launches; the smoke path starts a managed child server, optionally forwards a named configuration fixture for runtime-backed SDUI validation, polls for early child exit during bounded readiness retries, and reports categorized diagnostics before opening the GUI. `clay-server` parses a supplied endpoint through this abstraction and passes it into `ServerConfig`. On Unix, `IpcServer::run` validates the parent directory, removes only stale socket files, binds `UnixListener`, and keeps accepting connections. On Windows, `IpcServer::run` validates the local named-pipe prefix, creates a Tokio `NamedPipeServer`, awaits `connect()`, handles the already-connected race, then rotates back to create the next pipe instance. Each accepted client is handled in a spawned Tokio task so one connection does not block the accept loop, then the shared connection dispatcher runs over a generic Tokio `AsyncRead + AsyncWrite` stream rather than a Unix-specific stream type.

Each connection must send `ClientMessage::Hello` first. The server responds with:

1. `ServerMessage::Welcome`
2. `ServerMessage::InitialDocument`
3. `ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1))`
4. `ServerMessage::SduiSnapshot` for the static or runtime-replaced server-generated UI tree
5. Zero or more `ServerMessage::RuntimeDiagnostic` frames when startup/runtime configuration produced safe diagnostics
6. `ServerMessage::FileOpenCapabilityIssued` with a single-use token for the `OpenSelectedFile` authority gate (Plan 030)

During the handshake, `DocumentState::acquire_access` grants the first connected client an editable lease and returns later clients as read-only observers. After the handshake, edit messages and editor intents are translated into `EditOperation`s and applied to the target `DocumentState`. Workspace-backed document IDs are resolved through `WorkspaceState::document_handle`; otherwise the connection uses the bootstrap scratch document. The document state owns the canonical Phase 5 `crop::Rope`, validates document IDs, base versions, lease authority, region locks, byte ranges, and UTF-8 boundaries before mutating, then returns `EditAck` only for accepted mutations.

`ClientMessage::RequestResync` is handled by extracting a bounded recovery snapshot from the canonical rope through `DocumentState::resync_snapshot_message_for_client`. The snapshot preserves the requesting client's current access state and lease metadata.

`ClientMessage::SduiAction` validates inert button/list command intents against `StaticSduiState` and returns a typed protocol error for unknown commands or unknown/mismatched action sources. It does not execute client-supplied code or grant file, shell, network, extension, JavaScript, WASM, or AI authority.

Runtime diagnostics are shared server state populated by startup configuration evaluation and runtime SDUI/behavior application. The connection bootstrap clones and sends the current diagnostics after the SDUI snapshot. Diagnostics are typed and sanitized (`RuntimeDiagnostic`) so clients and GUI tests can observe syntax errors, invalid imports, permission denials, op validation failures, and validation rejections without exposing absolute paths, source snippets, secrets, or capability handles.

Phase 9 file/workspace dispatch keeps filesystem operations behind the server workspace lock. `OpenDocument` calls `WorkspaceState::open_existing_file`, returns `DocumentOpened` with a full initial snapshot plus metadata, and then publishes the current behavior manifest for the opened document. `OpenSelectedFile` uses the same selected-file grant path, then runs generic open-time mode activation: the persistent JS runtime classifies the path through `clay:modes`, lazily loads first-party `@clay/*` packages if no registered mode matches yet, `ParseCoordinator` schedules a bounded initial parse window for the registered package/mode handler, and any validated decoration update is sent as `DecorationSet`. Runtime reload reuses this same selected-file follow-up primitive for already-open workspace documents: `IpcServer::refresh_open_documents_after_reload` enumerates server-owned open documents, reruns generic mode classification/activation, schedules bounded parse refresh where a handler exists, and emits only behavior/decorations/diagnostics — no `DocumentOpened`/`DocumentReloaded` full-text snapshots for unchanged open documents. No per-open runtime root, package dist copy, temp `init.js`, or Markdown-specific Rust branch is used. `SaveDocument` and `ReloadDocument` call the workspace save/reload state machine; reload returns a snapshot, while save returns version/dirty metadata only. `GetDocumentStatus` and `ListDocuments` return `DocumentMetadata` without full text. Workspace errors are mapped to `FileOperationFailed` with stable `FileErrorCode` values and sanitized messages for outside-root paths. Files larger than `MAX_OPENABLE_FILE_BYTES` (below the 1 MiB IPC frame limit) are rejected with `FileErrorCode::FileTooLarge` before their contents are read, so an openable file always fits a single full-text frame and oversized files cannot be used as a memory-exhaustion vector. Connection shutdown releases the editable lease from both the bootstrap document and all workspace documents held by that client.

## Transport Hardening

Plan 030 (code-review remediation) hardens the local transport endpoints so that only the owning user can connect by default.

- **Unix domain sockets**: `IpcServer::run` validates the socket parent directory, removes only stale socket files, binds `UnixListener`, and then applies `0o600` permissions via `fs::set_permissions`. It also verifies that the parent directory is owned by the current process UID (`libc::getuid()`); if another user owns the directory, binding fails with `ServerError::EndpointOwnership` before a socket is created. This prevents an attacker from pre-creating a world-writable directory and tricking the server into exposing a socket there.
- **Windows named pipes**: `create_named_pipe_server` builds a custom `SECURITY_DESCRIPTOR` with a discretionary ACL containing a single access-allowed ACE for the current user SID. The descriptor is passed to `CreateNamedPipe` through `ServerOptions::create_with_security_attributes_raw`, replacing the default descriptor that would also grant read access to `Everyone` and the anonymous account. The token/ACL memory is allocated only for the duration of `CreateNamedPipe`; the kernel copies the descriptor into the pipe object, so the user-mode buffers are freed on return.
- **Temp-directory fallback**: the default endpoint still falls back to a per-user temp socket when `XDG_RUNTIME_DIR` is unavailable. The ownership check applies to the temp directory as well, so this fallback remains user-scoped in practice on single-user desktops. The long-term recommendation is still to run under a private runtime directory such as `XDG_RUNTIME_DIR`.

These changes are transport-layer boundaries, not application-level authentication. They complement the workspace/file authority model by ensuring another unprivileged same-machine account cannot connect to the IPC endpoint in the first place.

## Invariants and Constraints

- Socket and named-pipe I/O use Tokio async reads/writes; connection handling is isolated from the accept loop and transport-neutral after listener accept/connect.
- Wire messages continue to go through `Codec`; server code does not call `rkyv` directly.
- Frame-size validation and archive validation happen before messages reach the server dispatch loop.
- Endpoint construction is a cheap string/path selection step with no IPC, filesystem scan, shell execution, network listener, or blocking work.
- Default and smoke child servers are launched through `std::process::Command` with direct `server <endpoint>` arguments, inherited/controlled stdio, and no shell. Runtime SDUI smoke adds direct `--config-fixture runtime-sdui` child arguments only after resolving the repository fixture name. Smoke readiness fails if the managed child exits before the client handshake succeeds.
- Stale socket cleanup is Unix-only, removes only filesystem socket nodes, and refuses to replace normal files.
- Windows endpoint defaults and transport bindings are local named pipe names, not TCP or remote listeners.
- Ordinary accepted edit responses are metadata acknowledgements; full text snapshots are reserved for initial load, file open, file reload, and explicit resync recovery.
- Runtime reload refresh emits only follow-up behavior/decorations/diagnostics for already-open documents.
- Runtime diagnostic publication is asynchronous bootstrap/status traffic and is never sent from Masonry paint, text-event handling, or ordinary edit acknowledgement paths.
- Version fields are enforced by `DocumentState` before mutation; stale/future edits are rejected and can trigger client resync.

## Tests

- `src/server/connection.rs`: handshake, initial document, behavior manifest, SDUI snapshot publication, runtime diagnostic publication, editable/read-only access, edit acknowledgement, resync response, file/workspace open/status dispatch, generic selected-file open-time parse activation, typed file IO failures, and malformed-frame handling over generic in-memory async streams.
- `tests/selected_file_markdown_smoke.rs`: non-GUI IPC smoke that starts `IpcServer`, loads `@clay/markdown` through `init.js`, proves a raw `OpenSelectedFile` without the server-issued capability is rejected, then consumes the replenished capability and asserts Markdown `BehaviorManifest` plus syntax `DecorationSet` output.
- `src/server/document.rs`: canonical rope edit application, base-version enforcement, lease validation, region-lock rejection, and UTF-8 boundary rejection.
- `src/ipc.rs`: endpoint tests verify platform-valid default endpoint selection, isolated smoke endpoints, and printable diagnostics.
- `src/main.rs`: launch tests verify direct child-process command construction, config-fixture smoke forwarding, bounded readiness retry diagnostics, local-fallback messages, and early child-exit handling for smoke mode.
- `src/server/mod.rs`: listener-level Unix socket accept smoke test plus end-to-end stale-resync, region-lock rejection, and runtime reload open-document refresh coverage; Plan 030 adds `unix_socket_is_created_with_owner_only_permissions` and `windows_pipe_creation_applies_current_user_security_descriptor` to verify `0o600` permissions and the current-user-only DACL respectively.
- `src/client/mod.rs`: Windows named-pipe integration tests cover initial snapshot delivery, edit acknowledgement, read-only second-client behavior, and stale-edit resync recovery; tests are now robust to an ambient default `~/.config/clay/init.js` that publishes a behavior manifest.
- Relevant commands: `cargo test server --quiet`, `cargo test protocol --quiet`, `cargo check --quiet`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Server Document State](server-document-state.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `roadmap.md`
