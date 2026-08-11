# Client Snapshot Bootstrap

## Source

- `src/client/mod.rs`
- `src/editor/surface.rs`
- `src/editor/typography.rs`
- `src/editor/buffer.rs`
- `src/masonry_editor.rs`
- `src/main.rs`
- `src/server/connection.rs`
- `src/ipc.rs`

## Overview

The native app starts as a client unit that initializes the Masonry editor from a server-provided document snapshot and inert behavior manifest. In Phase 5 the same bootstrap also stores editable/read-only access metadata and seeds client synchronization state, while keeping connection setup separate from rendering, layout, widget event handling, and editor buffer mutation.

## Responsibilities

- `src/ipc.rs` models the configured local IPC endpoint as a Unix socket path on Unix or a Windows local named pipe name on Windows.
- `src/client/mod.rs` connects to Unix socket endpoints on Unix and Windows named-pipe endpoints on Windows, while its shared handshake and background connection loop operate on any `AsyncRead + AsyncWrite` stream; it reads `Welcome`, manifests, theme, and validated typography, sends `TabCommand::New`/`Reclaim`, then consumes the bound tab's `InitialDocument` into `ClientInitialState`. Later SDUI, registry, capability, diagnostics, and typography updates become client connection events.
- `ClientEditQueue` is created after bootstrap with the server-confirmed document version, client ID, and optional editable lease.
- `EditorSurface::load_snapshot` replaces the local shadow buffer at startup or resync and resets caret, selection, viewport, layout cache, and scroll state.
- `EditorSurface::install_behavior_manifest` stores the behavior version and manifest data without executing scripts.
- `EditorWidget::with_initial_state` bridges the bootstrap result into the existing Masonry widget.
- `src/main.rs` starts or connects to the local server, loads the initial state before launching Masonry, and keeps ongoing IPC on background Tokio tasks.

## How It Works

`src/main.rs` parses CLI endpoint arguments through `IpcEndpoint`; `client::connect` opens a `tokio::net::UnixStream` on Unix or a Tokio `NamedPipeClient` on Windows and wraps the handshake in a five-second timeout. Windows named-pipe clients use `ClientOptions::open` and retry `ERROR_PIPE_BUSY` briefly so an auto-started or saturated server can rotate a pipe instance. `ClientBootstrapError::kind` categorizes transport, endpoint-validation, protocol, handshake, server rejection, and timeout failures so launch code can print actionable startup diagnostics without string matching on error text. Once a connected stream exists, `connect_with_workspace_root` binds a selected new-tab root, `connect_for_reclaim` binds a persisted `TabId`, and the reconnect-only `connect_for_reclaim_or_new` falls back to that root when an in-memory server registry reset rejects the old ID. `connect_from_stream`/`handshake_initial_state` plus the background `run_connection` loop remain transport-neutral over Tokio async read/write traits and use `tokio::io::split` for independent read/write halves. All wire messages still go through the shared `Codec`, so length-prefix bounds and `rkyv` validation remain centralized. Runtime diagnostics are not part of the blocking bootstrap contract; if the server publishes `ServerMessage::RuntimeDiagnostic` after startup/configuration, `run_connection` forwards it as `ClientConnectionEvent::RuntimeDiagnostic` on the background event queue.

The production bootstrap expects messages and writes in this order:

1. `Welcome` with the current protocol version.
2. One or more `BehaviorManifest` messages, then `ActiveTheme`.
3. Validated `ActiveTypography`.
4. Client writes `TabCommand::New { workspace_root }` or `TabCommand::Reclaim { tab_id }`.
5. Server writes the bound tab's `InitialDocument` and an initial workspace-pane `SduiSnapshot` (editor-only while the per-tab pane is hidden by default; the bounded tree follows `workspace.toggleFileBrowser`).

`TabRegistry` replay and the selected-file capability are buffered/installed
around this bind and then continue through the background event loop. No
production connection receives document text or workspace SDUI before its tab
binding. Scripted unit transports retain a `#[cfg(test)]` legacy fixture for
older focused handshake tests only.

The returned `ClientInitialState` is passed to `EditorWidget::with_initial_state`. That constructor calls `EditorSurface::load_snapshot`, `EditorSurface::install_behavior_manifest`, and installs `TypographyRegistry` before first paint, keeping Masonry responsible only for widget lifecycle and native input/rendering. `TypographyRegistry` parses each bounded family name once into Parley `FontFamily` values; generic fallback stays local and no paint/layout path performs IPC, JavaScript, filesystem scans, or font-name parsing. `connect_from_stream` also returns a `ClientEditQueue` and event receiver so later edits, acknowledgements, rejections, and resync snapshots stay on background tasks instead of in the GUI hot path.

## Code Examples

```rust
let endpoint = clay::ipc::default_endpoint();
let state = tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .build()?
    .block_on(clay::client::load_initial_state(&endpoint))?;

let widget = clay::masonry_editor::EditorWidget::with_initial_state(state);
```

## Invariants and Constraints

- Startup and resync snapshots may be full documents; ordinary edits remain delta-based.
- Snapshot loading replaces the buffer and resets local UI state; paint still extracts only the visible range through `EditorBuffer::visible_snapshot`.
- Behavior manifests are stored as inert declarations only. They do not execute JavaScript, WASM, extensions, shell commands, filesystem operations, network operations, or AI actions.
- Client bootstrap connects only to the configured local IPC endpoint: Unix sockets on Unix and local named pipes on Windows. Failed decodes, unexpected messages, server errors, connection failures, endpoint validation errors, and timeouts are returned as categorized `ClientBootstrapError` values instead of panicking.
- Editable/read-only access from the server is authoritative. Read-only snapshots allow navigation/selection but block local text mutation and edit queue emission.
- Runtime diagnostics are asynchronous status events. They update UI status text but do not block bootstrap, rendering, typing, edit queueing, or behavior routing.
- `ActiveTypography` is revalidated both during bootstrap and when received live. Invalid/stale snapshots are ignored without changing cached typography, layout, or scroll state. A newer revision resets stale editor layout/visual-scroll state; `src/main.rs` requests Masonry layout, render, and accessibility updates exactly for that changed event.

## Tests

- `src/client/mod.rs`: `client_handles_initial_document_message` verifies server messages become `ClientInitialState` with version, access, and default `ActiveTypography` metadata over a generic in-memory async stream.
- `src/editor/typography.rs`: registry tests cover role/size/revision resolution, equal-revision no-op behavior, and preservation of a generic fallback after a missing named font.
- `src/masonry_editor.rs`: `live_typography_update_requests_layout_render_and_accessibility` verifies a newer live snapshot changes the widget once and raises one layout invalidation.
- `src/main.rs`: `connect_retry_reports_last_error` verifies bounded startup retry returns an actionable readiness error with the last categorized connection failure, and `client_mode_falls_back_with_status_when_server_missing` verifies fallback diagnostics include endpoint and error category.
- `src/client/mod.rs`: behavior-manifest tests verify manifest version/access data is preserved, and `client_receives_runtime_diagnostic_event` verifies runtime diagnostic protocol events reach the client event queue.
- `src/editor/surface.rs`: `editor_load_snapshot_replaces_text_and_resets_caret` verifies snapshot text, metadata, caret, selection, and scroll reset.
- `src/editor/surface.rs`: `editor_installs_minimal_behavior_manifest` verifies behavior manifest storage without execution.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot` verifies later resync snapshots use the same safe loading boundary, and `runtime_diagnostic_updates_status_text` verifies runtime diagnostics become visible GUI status text.
- Windows transport tests in `src/client/mod.rs`: named-pipe deferred hidden/visible initial snapshot, edit acknowledgement, independent per-tab welcome documents, and stale-edit resync recovery.
- Relevant commands: `cargo test --lib client --quiet`, `cargo test --lib windows_named_pipe --quiet`, `cargo test --lib windows_second_client_gets_independent_welcome_document --quiet`, `cargo test --lib windows_named_pipe_stale_edit_rejected_then_resynced --quiet`, `cargo test editor_load_snapshot_replaces_text_and_resets_caret --quiet`, `cargo test --quiet`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `concept.md`
- `roadmap.md`
