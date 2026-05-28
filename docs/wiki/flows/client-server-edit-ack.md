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

- `client::connect` opens the platform local IPC endpoint, performs the `Hello` / `Welcome` / `InitialDocument` / `BehaviorManifest` handshake, and returns a `ClientSession`.
- `ClientSession` contains the initial editor state, a `ClientEditQueue` for outgoing edits, and an event receiver for acknowledgements/errors.
- `ClientEditQueue` owns shared synchronization metadata: confirmed server version, optimistic local version, and pending transactions.
- The background client task owns the connected async stream after startup, serializes outgoing `ClientMessage::Edit` values, receives `ServerMessage::EditAck`, `EditRejected`, `ResyncSnapshot`, `EditTransaction`, and `Error` messages, and sends `RequestResync` after recoverable synchronization rejections.
- The server connection task validates each edit/intent `behavior_version` against the server-owned active behavior manifest before mutating the canonical document.
- `src/main.rs` parses `clay server`, `clay client`, `clay smoke-gui`, bare `clay`, and advanced single-endpoint shorthand modes into an `IpcEndpoint` from `src/ipc.rs`. It keeps a multi-thread Tokio runtime alive while Masonry runs, bridges decoded client IPC events into Masonry's user-event/action path, and keeps send/receive work off the GUI input and paint paths.
- `EditorWidget` optionally forwards edit events to the queue, still renders local edits immediately, and exposes a narrow `apply_connection_event` boundary that applies real resync snapshots with `EditorSurface::load_snapshot` or installs behavior manifests on the GUI thread.

## How It Works

Startup uses a shared endpoint abstraction from `src/ipc.rs`. On Unix, the default endpoint is a Unix socket path: `$XDG_RUNTIME_DIR/clay.sock` when available, otherwise a stable per-user temp socket. On Windows, the default endpoint is a local named pipe name like `\\.\pipe\clay-<user>`. `clay server` starts the foreground server on the default endpoint unless one is supplied. `clay client` opens a client and attaches to that endpoint when a server is already running, otherwise it opens a local fallback GUI and reports the connection error. Bare `clay` first tries to attach; if no server is reachable, it reports the categorized bootstrap failure, spawns the same executable as a separate background `clay server` process with the endpoint passed as a direct child-process argument, retries the client handshake for a bounded readiness window, and then opens the client against it. Because auto-started servers are separate processes today, closing the auto-opened client does not stop the server; it must be killed explicitly.

`clay smoke-gui` is the app-managed smoke path. The parser rejects endpoint arguments for this mode, generates a unique local endpoint with a `smoke-gui-<pid>-<sequence>` suffix, starts a child `clay server <endpoint>` process through `std::process::Command` direct arguments, waits for the ordinary client handshake with bounded retry, detects child exit before readiness, opens the GUI client only after a successful handshake, and terminates/waits for the managed child server when the GUI exits. Unix smoke endpoints are temporary `.sock` paths under `std::env::temp_dir()` and are removed after child shutdown; Windows smoke endpoints are local named pipes under `\\.\pipe\`; no shell, TCP listener, or user-managed endpoint is involved.

`IpcServer` owns an `ActiveBehaviorManifest` alongside the canonical `DocumentState`. The handshake sends the active manifest after `Welcome` and `InitialDocument`. The manifest is not reconstructed per connection, so future server-side hot reload can validate and publish one replacement state that all connections observe.

Client startup calls `client::connect`. The handshake is bounded by the existing five-second startup timeout. Bootstrap errors expose a small category enum for transport unavailable, invalid endpoint, protocol invalid, handshake failed, server rejected, and timeout states; launch code uses those categories in diagnostics rather than parsing error strings. During the server handshake, `DocumentState::acquire_access` grants the first connected client an editable lease and sends later clients read-only observer snapshots. Once the initial snapshot and manifest have been read, `connect_from_stream` creates:

1. A bounded outgoing edit channel used by `ClientEditQueue`.
2. A bounded connection event channel used for acknowledgements and recoverable connection state.
3. A background Tokio task that owns the connected Unix socket or Windows named-pipe stream selected by the platform transport and splits it with `tokio::io::split`.

`ClientEditQueue::enqueue_edit_event` reserves the current optimistic version as the outgoing edit's `base_version`, records the transaction in the pending queue, advances the optimistic version locally, and then uses bounded `try_send`. If the queue has no editable lease, or if the channel is full, the reservation is rolled back and the UI remains responsive. This allows multiple local edits to be sent without waiting for the previous acknowledgement while preserving per-document base-version ordering for editable clients only.

The background task splits any connected `AsyncRead + AsyncWrite` stream and uses `tokio::select!` to handle either an outgoing edit or an incoming server frame. Outgoing edits are encoded through the shared `Codec`. Incoming `EditAck` frames update confirmed-version state, remove the pending transaction, and become `ClientConnectionEvent::EditAck`. Incoming `EditRejected` frames remove the rejected pending transaction and become `ClientConnectionEvent::EditRejected`. Stale/future version, invalid behavior version, lease, read-only, and region-lock rejections immediately send a `ClientMessage::RequestResync` with the client's last confirmed version. Incoming `ResyncSnapshot` frames replace the client synchronization snapshot, set confirmed and optimistic versions to the server version, clear all pending edits, and become `ClientConnectionEvent::ResyncSnapshot`. Server errors and decode/I/O failures become non-panicking events.

`src/main.rs` passes both the server-provided initial state and edit queue into `EditorWidget`, then spawns a bridge task for the session's bounded `ClientConnectionEvent` receiver. The bridge logs each decoded event for diagnostics and sends a `MasonryUserEvent::Action` through `EventLoopProxy`; the winit event loop wakes up and delivers the typed `EditorAction::ClientConnection` to `Driver::on_action`. The driver mutates the editor widget only from that event-loop callback, calls `EditorWidget::apply_connection_event`, and requests render/accessibility updates when the event changed widget state. The bridge stops if the event loop closes, and it never blocks Masonry input, paint, or layout handlers on IPC work.

`EditorWidget` owns a small `EditorStatus` model separate from the text rope. Connected sessions initialize it from `ClientInitialState`; local fallback editors initialize it as `Local Fallback`; edit acknowledgements update the latest confirmed document version; resync snapshots update document id/version/access; and disconnection/error events switch the connection label to `Disconnected` while preserving the last known document metadata. The widget paints this state as a bottom status line and includes it in the accessibility label so manual GUI smoke tests can see `Connected Editable`, `Connected Read-only Observer`, `Local Fallback`, `Disconnected`, and version state without reading stderr.

The widget assigns client transaction IDs and calls `try_send` through `ClientEditQueue`. If the queue is missing or full, the local edit has already happened and the UI remains responsive. `EditorWidget::apply_connection_event` is the UI-safe connection-event boundary: it updates status on `EditAck`, `ResyncSnapshot`, `Disconnected`, and `ConnectionError`, applies a real `ResyncSnapshot` through `EditorSurface::load_snapshot` (resetting caret, selection, viewport, and local document metadata), and installs server-provided behavior manifests on the existing editor surface.

## Code Examples

```bash
# foreground server
cargo run -- server

# attach the first editable client to the running default server
cargo run -- client

# attach a second read-only observer client to the same default server
cargo run -- client

# auto-start a server if needed, then open a client
cargo run

# app-managed GUI smoke mode; creates an isolated endpoint and cleans up its child server
cargo run -- smoke-gui
```

## Invariants and Constraints

- No Masonry input or paint handler performs socket reads/writes, drains IPC channels, or waits for an acknowledgement.
- GUI status rendering is derived from already-owned widget state and uses concise labels; it does not display raw endpoint paths or perform IPC round trips.
- The outgoing edit queue is bounded to avoid unbounded memory growth under server stalls.
- The server remains authoritative for the canonical document and version increments.
- Client confirmed-version state advances only from `EditAck`; optimistic version state advances locally when an edit is queued.
- Pending transactions stay queued until the corresponding acknowledgement, rejection, or resync recovery arrives.
- Default `server` and `client` launch modes all derive the same platform-local endpoint from `default_endpoint()`, so second-client GUI smoke testing does not require copying a named pipe or socket path.
- Only the current server lease holder can successfully mutate; observer clients keep read-only access metadata and cannot enqueue or pass server validation for edits.
- Strict stale/future base-version enforcement happens on the server before text mutation; simple Phase 5 recovery uses full snapshots only after explicit resync requests.
- Behavior-version enforcement is a server-owned manifest metadata check before document mutation; rejected behavior versions do not advance the canonical document version.
- IPC input is still decoded and validated through the shared length-prefixed `rkyv` codec before any `ClientConnectionEvent` is bridged into Masonry.
- No JavaScript execution, file/workspace authority, remote listener, extension loading, shell-mediated startup, or AI mutation authority is added by this flow.
- Endpoint connect/listen code is platform-gated at the transport boundary: Unix uses Unix domain sockets with stale-socket protection, and Windows uses local Tokio named pipes with busy-pipe retry on the client side. Shared client and server protocol loops remain generic over Tokio async streams.

## Tests

- `src/client/mod.rs`: `end_to_end_client_receives_initial_snapshot` validates the connected client receives a snapshot during handshake.
- `src/client/mod.rs`: `end_to_end_client_receives_behavior_manifest` validates manifest delivery before edit emission.
- `src/client/mod.rs`: `end_to_end_edit_gets_acknowledged` validates queued edit send and ack receipt with a paired socket.
- `src/client/mod.rs`: `client_ack_advances_confirmed_version` validates ack-driven confirmed-version updates and pending cleanup.
- `src/client/mod.rs`: `client_keeps_pending_edit_until_ack_or_rejection` validates deterministic pending transaction bookkeeping.
- `src/client/mod.rs`: `client_requests_resync_after_stale_rejection` validates automatic resync requests after recoverable rejection.
- `src/client/mod.rs`: `client_applies_resync_snapshot_and_clears_pending_edits` validates Unicode snapshot recovery and pending cleanup.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot` validates the UI-safe resync event boundary.
- `src/masonry_editor.rs`: status tests validate connected editable, read-only observer, local fallback, and edit-ack version updates.
- `src/client/mod.rs`: `end_to_end_second_client_is_read_only` validates duplicate client observer access through `IpcServer` on a real Unix socket.
- `src/client/mod.rs`: `real_server_end_to_end_edit_gets_acknowledged` validates the same edit/ack path through `IpcServer` on a real Unix socket.
- `src/client/mod.rs`: `windows_named_pipe_client_receives_initial_snapshot`, `windows_named_pipe_edit_gets_acknowledged`, `windows_second_client_is_read_only`, and `windows_named_pipe_stale_edit_rejected_then_resynced` validate the Windows named-pipe transport.
- `src/client/mod.rs`: `real_server_end_to_end_stale_edit_rejected_then_resynced` validates stale-version rejection and explicit resync recovery through `IpcServer` on a real Unix socket; the Windows named-pipe stale/resync test exercises the same protocol over the Windows transport.
- `src/server/mod.rs`: `real_server_end_to_end_region_locked_edit_rejected` validates region-lock conflict metadata across the real Unix socket server path.
- `src/server/connection.rs`: `server_rejects_edit_with_stale_behavior_version_without_mutating_document` validates behavior-version mismatch rejection before canonical mutation.
- `src/server/behavior.rs`: replacement publishing tests validate deterministic manifest version advancement and invalid replacement rollback.
- `src/ipc.rs`: `smoke_endpoint_is_platform_local_and_unique` validates that managed smoke endpoints are unique and remain platform-local.
- `src/main.rs`: CLI parser tests validate `server`, `client`, `smoke-gui`, bare auto modes, default endpoint behavior, the shared default endpoint used by foreground server and repeated default clients, extra-argument failures, smoke-owned endpoint selection, and platform endpoint argument parsing; `auto_start_uses_current_exe_without_shell` and `managed_server_command_uses_current_exe_without_shell` validate shell-free server command construction with direct endpoint arguments; `connect_retry_reports_last_error`, `client_mode_falls_back_with_status_when_server_missing`, and `smoke_mode_fails_if_child_server_exits_before_ready` validate readiness and fallback diagnostics; `connection_event_action_is_dispatched_to_driver` validates that client connection events are wrapped as Masonry actions targeted at the editor widget.
- Relevant commands: `cargo test client --quiet`, `cargo test server --quiet`, `cargo test --quiet`.

## Related

- [Launch and GUI Smoke Validation](../../development/launch-and-gui-smoke.md)
- [Windows MSVC Development](../../development/windows.md)
- [Client Edit Emission](client-edit-emission.md)
- [Versioned Text Synchronization](versioned-text-synchronization.md)
- [Document Leases and Region Locks](document-leases-and-region-locks.md)
- [Client Snapshot Bootstrap](../modules/client-snapshot-bootstrap.md)
- [Server IPC Skeleton](../modules/server-ipc-skeleton.md)
- [Protocol Codec](../modules/protocol-codec.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
