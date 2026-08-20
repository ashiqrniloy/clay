# Client/Server Edit Acknowledgement Flow

## Source

- `src/client/mod.rs`
- `src/masonry_editor.rs`
- `src/driver/mod.rs`
- `src/app_driver.rs`
- `src/server/connection/mod.rs`
- `src/server/document.rs`

## Overview

The client keeps the IPC connection open after the initial snapshot handshake. Local editor mutations are forwarded over a bounded queue to a background Tokio connection task, and server acknowledgements or rejections are received asynchronously as client connection events. In Phase 5 the queue also tracks confirmed server versions, optimistic local versions, and pending transactions so strict server base-version enforcement does not make the GUI wait for acknowledgements.

## Responsibilities

- `client::connect` opens the platform local IPC endpoint, performs the
  `Hello` / pre-bind manifest/theme/typography / tab-binding handshake, then
  returns a `ClientSession` after the bound tab's `InitialDocument` arrives.
- `ClientSession` contains the initial editor state, a `ClientEditQueue` for outgoing edits, and an event receiver for acknowledgements/errors.
- `ClientEditQueue` owns shared synchronization metadata: confirmed server version, optimistic local version, and pending transactions.
- The background client task owns the connected async stream after startup, serializes outgoing `ClientMessage::Edit` values plus explicit non-edit requests such as `OpenSelectedFile`, receives `ServerMessage::EditAck`, `EditRejected`, `ResyncSnapshot`, `DocumentOpened`, `FileOperationFailed`, `EditTransaction`, and `Error` messages, and sends `RequestResync` after recoverable synchronization rejections.
- The server connection task validates each edit/intent `behavior_version` against the server-owned active behavior manifest before mutating the canonical document.
- `src/cli.rs` parses `clay server`, `clay client`, Linux `clay restart`, `clay smoke-gui`, bare `clay`, and advanced single-endpoint shorthand modes into an `IpcEndpoint` from `src/ipc.rs`. `src/launch.rs` keeps a multi-thread Tokio runtime alive while Masonry runs, and `src/app_driver.rs` bridges decoded client IPC events into the `Driver`/Masonry user-event path, keeping send/receive work off the GUI input and paint paths.
- `EditorWidget` optionally forwards edit events to the queue, still renders local edits immediately, and exposes a narrow `apply_connection_event` boundary that applies real resync snapshots with `EditorSurface::load_snapshot` or installs behavior manifests on the GUI thread.

## How It Works

Startup uses a shared endpoint abstraction from `src/ipc.rs`. On Unix, the default endpoint is a Unix socket path: `$XDG_RUNTIME_DIR/clay.sock` when available, otherwise a stable per-user temp socket. On Windows, the default endpoint is a local named pipe name like `\\.\pipe\clay-<user>`. `clay server` starts the foreground server on the default endpoint unless one is supplied. `clay client` opens a client and attaches to that endpoint when a server is already running, otherwise it opens a local fallback GUI and reports the connection error. Bare `clay` first tries to attach; if no server is reachable, it reports the categorized bootstrap failure, spawns the same executable as a separate background `clay server` process with the endpoint passed as a direct child-process argument, retries the client handshake for a bounded readiness window, and then opens the client against it. Because auto-started servers are separate processes today, closing the auto-opened client does not stop the server. On Linux, `clay restart` (or `cargo run -- restart`) stops only the current executable's default-endpoint server, starts a fresh background server, verifies the normal handshake, and exits; custom-endpoint and isolated smoke servers are not touched.

`clay smoke-gui` is the app-managed smoke path. The parser rejects endpoint arguments for this mode, generates a unique local endpoint with a `smoke-gui-<pid>-<sequence>` suffix, starts a child `clay server <endpoint>` process through `std::process::Command` direct arguments, waits for the ordinary client handshake with bounded retry, detects child exit before readiness, opens the GUI client only after a successful handshake, and terminates/waits for the managed child server when the GUI exits. Unix smoke endpoints are temporary `.sock` paths under `std::env::temp_dir()` and are removed after child shutdown; Windows smoke endpoints are local named pipes under `\\.\pipe\`; no shell, TCP listener, or user-managed endpoint is involved. The optional `--config-fixture runtime-sdui` smoke flag resolves a named repository fixture and forwards that name to the managed child server so runtime-backed configuration can publish SDUI before the GUI connects.

`IpcServer` owns an `ActiveBehaviorManifest` alongside its bootstrap/runtime
state. The handshake sends the active manifest after `Welcome` and before tab
binding; the bound `TabServerState` supplies `InitialDocument` after
`TabCommand::New`/`Reclaim`. The manifest is not reconstructed per connection,
so future server-side hot reload can validate and publish one replacement
state that all connections observe, while document/workspace authority stays
inside each routed tab state.

Client startup calls `client::connect`. The handshake is bounded by the existing five-second startup timeout. Bootstrap errors expose a small category enum for transport unavailable, invalid endpoint, protocol invalid, handshake failed, server rejected, and timeout states; launch code uses those categories in diagnostics rather than parsing error strings. After the client sends its tab-binding command, the server calls
`DocumentState::acquire_access` on that tab's welcome document. File-backed
opens use the same routed tab workspace and per-document lease rules; separate
tabs do not observe one another's documents. Once the pre-bind lanes and the
bound initial snapshot have been read, `connect_from_stream` creates:

1. A bounded outgoing edit channel used by `ClientEditQueue`.
2. A bounded connection event channel used for acknowledgements and recoverable connection state.
3. A background Tokio task that owns the connected Unix socket or Windows named-pipe stream selected by the platform transport and splits it with `tokio::io::split`.

`ClientEditQueue::enqueue_edit_event` reserves the current optimistic version as the outgoing edit's `base_version`, records the transaction in the pending queue, advances the optimistic version locally, and then uses bounded `try_send`. If the queue has no editable lease, or if the channel is full, the reservation is rolled back and the UI remains responsive. This allows multiple local edits to be sent without waiting for the previous acknowledgement while preserving per-document base-version ordering for editable clients only. `ClientEditQueue::enqueue_open_selected_file` uses the same bounded non-blocking channel but sends an explicit `ClientMessage::OpenSelectedFile` without reserving an edit transaction or serializing document text.

The background task splits any connected `AsyncRead + AsyncWrite` stream and uses `tokio::select!` to handle either an outgoing queued message or an incoming server frame. Outgoing messages are encoded through the shared `Codec`. Incoming `EditAck` frames update confirmed-version state, remove the pending transaction, and become `ClientConnectionEvent::EditAck`. Incoming `EditRejected` frames remove the rejected pending transaction and become `ClientConnectionEvent::EditRejected`. Stale/future version, invalid behavior version, lease, read-only, and region-lock rejections immediately send a `ClientMessage::RequestResync` with the client's last confirmed version. Incoming `ResyncSnapshot` frames replace the client synchronization snapshot, set confirmed and optimistic versions to the server version, clear all pending edits, and become `ClientConnectionEvent::ResyncSnapshot`. Incoming `DocumentOpened` frames from selected-file or workspace opens also reset confirmed/optimistic versions, clear pending edits, and become `ClientConnectionEvent::DocumentOpened` so the GUI can replace the buffer. `FileOperationFailed` becomes a typed client event/status instead of panicking. Server errors and decode/I/O failures become non-panicking events.

`src/driver/mod.rs` passes both the server-provided initial state and edit queue into `EditorWidget`, then spawns a bridge task for the session's bounded `ClientConnectionEvent` receiver. The bridge logs each decoded event for diagnostics and sends a `MasonryUserEvent::Action` through `EventLoopProxy`; the winit event loop wakes up and delivers the typed `EditorAction::ClientConnection` to `Driver::on_action`. The driver mutates the editor widget only from that event-loop callback, calls `EditorWidget::apply_connection_event`, and requests render/accessibility updates when the event changed widget state. SDUI snapshot/update messages use the same path and are reconciled into inert native `SduiNativeState` only after decoding. The bridge stops if the event loop closes, and it never blocks Masonry input, paint, or layout handlers on IPC work.

`EditorWidget` owns a small `EditorStatus` model separate from the text rope. Connected sessions initialize it from `ClientInitialState`; local fallback editors initialize it as `Local Fallback`; edit acknowledgements update the latest confirmed document version; resync snapshots update document id/version/access and clear sync-recovery chrome; edit rejections set sanitized diagnostics (auto-resync classes note "requesting resync"; invalid-range/document open Resync/Dismiss menus); disconnection/error events switch the connection label to `Disconnected` with reconnect guidance while preserving the last known document metadata. Pending outbound edit depth is observed live from `ClientEditQueue` and appended to status/accessibility. Explicit `editor.clientRequestResync` / `clientDismissRecovery` client UI commands reuse `RequestResync` and clear recovery chrome without escalating authority. The widget paints this state as a bottom status line and includes it in the accessibility label so manual GUI smoke tests can see `Connected Editable`, `Connected Read-only Observer`, `Local Fallback`, `Disconnected`, pending-edit depth, recovery summaries, and version state without reading stderr.

The widget assigns client transaction IDs and calls `try_send` through `ClientEditQueue`. If the queue is missing or full, the local edit has already happened and the UI remains responsive. `EditorWidget::request_selected_file_open` also uses bounded `try_send` and reports a runtime diagnostic when the editor is disconnected or the queue is full. `EditorWidget::apply_connection_event` is the UI-safe connection-event boundary: it updates status on `EditAck`, `ResyncSnapshot`, `DocumentOpened`, `FileOperationFailed`, `Disconnected`, and `ConnectionError`, applies real `ResyncSnapshot`/`DocumentOpened` snapshots through `EditorSurface::load_snapshot` (resetting caret, selection, viewport, and local document metadata), updates the edit queue lease/version after `DocumentOpened`, and installs server-provided behavior manifests on the existing editor surface.

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

# Linux: replace the default background server and verify readiness
cargo run -- restart

# app-managed GUI smoke mode; creates an isolated endpoint and cleans up its child server
cargo run -- smoke-gui

# same managed smoke lifecycle, with a JavaScript configuration fixture publishing SDUI
cargo run -- smoke-gui --config-fixture runtime-sdui
```

When a workspace file opens or a resync arrives, `ClientEditQueue::update_opened_document_authority` resets the existing shared `ClientSyncState` in place. The connection task and GUI queue therefore continue observing the same `Arc<Mutex<ClientSyncState>>`; replacing the `Arc` would orphan acknowledgement/rejection updates and leave later edits on stale optimistic versions. Same-document resync replaces server-authoritative text but preserves the caret at its previous byte offset, clamped to the new UTF-8 document boundary.

## Invariants and Constraints

- No Masonry input or paint handler performs socket reads/writes, drains IPC channels, or waits for an acknowledgement.
- GUI status rendering is derived from already-owned widget state and uses concise labels; it does not display raw endpoint paths or perform IPC round trips.
- The outgoing edit queue is bounded to avoid unbounded memory growth under server stalls.
- The server remains authoritative for the canonical document and version increments.
- Client confirmed-version state advances from `EditAck` for normal edits and is reset by full open/resync snapshots; optimistic version state advances locally only when an edit is queued.
- Pending transactions stay queued until the corresponding acknowledgement, rejection, or resync recovery arrives.
- Default `server` and `client` launch modes all derive the same platform-local endpoint from `default_endpoint()`, so second-client GUI smoke testing does not require copying a named pipe or socket path.
- Only the current server lease holder can successfully mutate; observer clients keep read-only access metadata and cannot enqueue or pass server validation for edits.
- Strict stale/future base-version enforcement happens on the server before text mutation; full snapshots are limited to bootstrap, explicit document open/selected-file open, and explicit resync recovery.
- Behavior-version enforcement is a server-owned manifest metadata check before document mutation; rejected behavior versions do not advance the canonical document version.
- IPC input is still decoded and validated through the shared length-prefixed `rkyv` codec before any `ClientConnectionEvent` is bridged into Masonry.
- Server-driven UI events are inert decoded tree/update values; Masonry receives native state changes and emits typed action intents, never raw IPC bytes or executable scripts.
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
- `src/client/mod.rs`: `selected_file_open_request_emits_non_edit_message`, `client_applies_document_opened_snapshot_from_selected_file`, and `client_receives_file_operation_failed_event` validate selected-file request/event handling.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot`, `document_opened_event_replaces_editor_snapshot`, and `opened_file_edits_continue_as_deltas` validate the UI-safe snapshot boundary and delta edits after selected-file open.
- `src/masonry_editor.rs`: status tests validate connected editable, read-only observer, local fallback, and edit-ack version updates.
- `src/client/mod.rs`: `end_to_end_second_client_gets_independent_welcome_document` validates that a second real-server tab receives its own editable welcome document and distinct document ID.
- `src/client/mod.rs`: `real_server_end_to_end_edit_gets_acknowledged` validates the same edit/ack path through `IpcServer` on a real Unix socket.
- `src/client/mod.rs`: `windows_named_pipe_client_receives_initial_snapshot`, `windows_named_pipe_edit_gets_acknowledged`, `windows_second_client_gets_independent_welcome_document`, and `windows_named_pipe_stale_edit_rejected_then_resynced` validate the Windows named-pipe transport.
- `src/client/mod.rs`: `real_server_end_to_end_stale_edit_rejected_then_resynced` validates stale-version rejection and explicit resync recovery through `IpcServer` on a real Unix socket; the Windows named-pipe stale/resync test exercises the same protocol over the Windows transport.
- `src/server/mod.rs`: `real_server_end_to_end_region_locked_edit_rejected` validates region-lock conflict metadata across the real Unix socket server path.
- `src/server/connection/mod.rs`: `server_rejects_edit_with_stale_behavior_version_without_mutating_document` validates behavior-version mismatch rejection before canonical mutation.
- `src/server/behavior.rs`: replacement publishing tests validate deterministic manifest version advancement and invalid replacement rollback.
- `src/ipc.rs`: `smoke_endpoint_is_platform_local_and_unique` validates that managed smoke endpoints are unique and remain platform-local.
- `src/main.rs`: CLI parser tests validate `server`, `client`, `smoke-gui`, bare auto modes, default endpoint behavior, the shared default endpoint used by foreground server and repeated default clients, extra-argument failures, smoke-owned endpoint selection, config-fixture parsing, and platform endpoint argument parsing; `auto_start_uses_current_exe_without_shell`, `managed_server_command_uses_current_exe_without_shell`, and `managed_server_command_forwards_config_fixture_without_shell` validate shell-free server command construction with direct endpoint/config-fixture arguments; `connect_retry_reports_last_error`, `client_mode_falls_back_with_status_when_server_missing`, and `smoke_mode_fails_if_child_server_exits_before_ready` validate readiness and fallback diagnostics; `connection_event_action_is_dispatched_to_driver` validates that client connection events are wrapped as Masonry actions targeted at the editor widget; `smoke_launch_routes_sdui_events_to_gui` validates that an SDUI snapshot follows the same non-blocking Masonry action bridge.
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
