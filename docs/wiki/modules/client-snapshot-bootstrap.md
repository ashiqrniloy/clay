# Client Snapshot Bootstrap

## Source

- `src/client/mod.rs`
- `src/editor/surface.rs`
- `src/editor/buffer.rs`
- `src/masonry_editor.rs`
- `src/main.rs`

## Overview

The native app starts as a client unit that initializes the Masonry editor from a server-provided document snapshot and inert behavior manifest. In Phase 5 the same bootstrap also stores editable/read-only access metadata and seeds client synchronization state, while keeping connection setup separate from rendering, layout, widget event handling, and editor buffer mutation.

## Responsibilities

- `src/client/mod.rs` connects to a configured local Unix socket, sends `ClientMessage::Hello`, and converts the expected `Welcome`, `InitialDocument`, and `BehaviorManifest` messages into `ClientInitialState`.
- `ClientEditQueue` is created after bootstrap with the server-confirmed document version, client ID, and optional editable lease.
- `EditorSurface::load_snapshot` replaces the local shadow buffer at startup or resync and resets caret, selection, viewport, layout cache, and scroll state.
- `EditorSurface::install_behavior_manifest` stores the behavior version and manifest data without executing scripts.
- `EditorWidget::with_initial_state` bridges the bootstrap result into the existing Masonry widget.
- `src/main.rs` starts or connects to the local server, loads the initial state before launching Masonry, and keeps ongoing IPC on background Tokio tasks.

## How It Works

`load_initial_state` opens a `tokio::net::UnixStream` to the configured socket path and wraps the handshake in a five-second timeout. All wire messages still go through the shared `Codec`, so length-prefix bounds and `rkyv` validation remain centralized.

The bootstrap expects messages in this order:

1. `Welcome` with the current protocol version.
2. `InitialDocument` with document ID, server version, text, editable/read-only access mode, and optional lease ID.
3. `BehaviorManifest` with the server-issued behavior version and inert client-first text editing capabilities.

The returned `ClientInitialState` is passed to `EditorWidget::with_initial_state`. That constructor calls `EditorSurface::load_snapshot` and `EditorSurface::install_behavior_manifest`, keeping Masonry responsible only for widget lifecycle and native input/rendering. `connect_from_stream` also returns a `ClientEditQueue` and event receiver so later edits, acknowledgements, rejections, and resync snapshots stay on background tasks instead of in the GUI hot path.

## Code Examples

```rust
let state = tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .build()?
    .block_on(clay::client::load_initial_state("/run/user/1000/clay.sock"))?;

let widget = clay::masonry_editor::EditorWidget::with_initial_state(state);
```

## Invariants and Constraints

- Startup and resync snapshots may be full documents; ordinary edits remain delta-based.
- Snapshot loading replaces the buffer and resets local UI state; paint still extracts only the visible range through `EditorBuffer::visible_snapshot`.
- Behavior manifests are stored as inert declarations only. They do not execute JavaScript, WASM, extensions, shell commands, filesystem operations, network operations, or AI actions.
- Client bootstrap connects only to the configured local socket path. Failed decodes, unexpected messages, server errors, connection failures, and timeouts are returned as `ClientBootstrapError` values instead of panicking.
- Editable/read-only access from the server is authoritative. Read-only snapshots allow navigation/selection but block local text mutation and edit queue emission.

## Tests

- `src/client/mod.rs`: `client_handles_initial_document_message` verifies server messages become `ClientInitialState`.
- `src/client/mod.rs`: `client_handles_initial_document_message` verifies server messages become `ClientInitialState` with version and access metadata.
- `src/client/mod.rs`: behavior-manifest tests verify manifest version/access data is preserved.
- `src/editor/surface.rs`: `editor_load_snapshot_replaces_text_and_resets_caret` verifies snapshot text, metadata, caret, selection, and scroll reset.
- `src/editor/surface.rs`: `editor_installs_minimal_behavior_manifest` verifies behavior manifest storage without execution.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot` verifies later resync snapshots use the same safe loading boundary.
- Relevant commands: `cargo test client --quiet`, `cargo test editor_load_snapshot_replaces_text_and_resets_caret --quiet`, `cargo test --quiet`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `concept.md`
- `roadmap.md`
