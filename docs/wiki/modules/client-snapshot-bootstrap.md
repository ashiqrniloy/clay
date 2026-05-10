# Client Snapshot Bootstrap

## Source

- `src/client/mod.rs`
- `src/editor/surface.rs`
- `src/editor/buffer.rs`
- `src/masonry_editor.rs`
- `src/main.rs`

## Overview

The Phase 4 native app is now a client unit that can initialize the Masonry editor from a server-provided document snapshot and inert behavior manifest. The client bootstrap code is separate from rendering, layout, widget event handling, and editor buffer mutation.

## Responsibilities

- `src/client/mod.rs` connects to a configured local Unix socket, sends `ClientMessage::Hello`, and converts the expected `Welcome`, `InitialDocument`, and `BehaviorManifest` messages into `ClientInitialState`.
- `EditorSurface::load_snapshot` replaces the local shadow buffer once at startup and resets caret, selection, viewport, layout cache, and scroll state.
- `EditorSurface::install_behavior_manifest` stores the behavior version and manifest data without executing scripts.
- `EditorWidget::with_initial_state` bridges the bootstrap result into the existing Masonry widget.
- `src/main.rs` optionally takes a socket path as the first argument, loads the initial state before launching Masonry, and falls back to the existing empty local editor if bootstrap fails.

## How It Works

`load_initial_state` opens a `tokio::net::UnixStream` to the configured socket path and wraps the handshake in a five-second timeout. All wire messages still go through the shared `Codec`, so length-prefix bounds and `rkyv` validation remain centralized.

The bootstrap expects messages in this order:

1. `Welcome` with the current protocol version.
2. `InitialDocument` with document ID, server version, text, and editable/read-only access mode.
3. `BehaviorManifest` with the server-issued behavior version and inert client-first text editing capabilities.

The returned `ClientInitialState` is passed to `EditorWidget::with_initial_state`. That constructor calls `EditorSurface::load_snapshot` and `EditorSurface::install_behavior_manifest`, keeping Masonry responsible only for widget lifecycle and native input/rendering.

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

- The startup snapshot may be a full document; ordinary edits must remain delta-based in later Phase 4 tasks.
- Snapshot loading replaces the buffer once and resets local UI state; paint still extracts only the visible range through `EditorBuffer::visible_snapshot`.
- Behavior manifests are stored as inert declarations only. They do not execute JavaScript, WASM, extensions, shell commands, filesystem operations, network operations, or AI actions.
- Client bootstrap connects only to the configured local socket path. Failed decodes, unexpected messages, server errors, connection failures, and timeouts are returned as `ClientBootstrapError` values instead of panicking.
- Ongoing edit emission and acknowledgement handling are still deferred to later Phase 4 tasks; this page covers initial snapshot loading only.

## Tests

- `src/client/mod.rs`: `client_handles_initial_document_message` verifies server messages become `ClientInitialState`.
- `src/client/mod.rs`: `client_installs_minimal_behavior_manifest` verifies manifest version/access data is preserved.
- `src/editor/surface.rs`: `editor_load_snapshot_replaces_text_and_resets_caret` verifies snapshot text, metadata, caret, selection, and scroll reset.
- `src/editor/surface.rs`: `editor_installs_minimal_behavior_manifest` verifies behavior manifest storage without execution.
- Relevant commands: `cargo test client`, `cargo test editor_load_snapshot_replaces_text_and_resets_caret`, `cargo test`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `concept.md`
- `roadmap.md`
