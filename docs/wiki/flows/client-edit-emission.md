# Client Edit Emission

## Source

- `src/editor/surface.rs`
- `src/client/mod.rs`
- `src/masonry_editor.rs`
- `src/protocol/mod.rs`

## Overview

Local text mutations produce structured edit events that can be sent to the server without coupling editor state or Masonry event handling to socket I/O. The editor still applies predictable text edits immediately to its local shadow buffer. When the installed behavior manifest declares the operation client-first and the document is editable, the same mutation returns an `EditorEditEvent` containing protocol-ready document, version, behavior, and delta metadata. Read-only observer snapshots allow navigation and selection but block local text mutation and edit-event emission.

## Responsibilities

- `EditorSurface` owns local mutation, caret/selection updates, and edit-event construction.
- `EditorEditEvent` carries inert text deltas only: insert, delete, or replace byte ranges.
- `ClientEditQueue` converts edit events into `ClientMessage::Edit` values, assigns the current optimistic base version, tracks pending transactions, and applies bounded `try_send` backpressure.
- `EditorWidget` forwards successful edit events to an optional queue and assigns monotonically increasing client transaction IDs without blocking UI input.

## How It Works

Text commands flow through `EditorSurface::command_with_event`. Insert, newline, Backspace, Delete, selected-range replacement, and selected-range deletion compute the affected byte offset/range from the same cursor and selection state used by the local edit path. No post-edit diff or whole-document clone is needed.

Text-mutating commands first check `EditorSurface::is_editable`. If the surface was loaded with `DocumentAccess::ReadOnly`, insert, newline, Backspace, and Delete return an unchanged outcome without touching the buffer. Navigation and selection commands still update local UI state.

After an editable buffer mutation succeeds, `finish_edit_with_operation` updates the caret, clears the selection, keeps the caret visible, and calls `client_first_event`. That helper emits an event only when:

1. The document access mode is `DocumentAccess::Editable`.
2. A behavior manifest is installed.
3. The manifest contains `ClientFirstTextEditing` for the operation kind.

The event stores the editor's current document version and the installed manifest version as `behavior_version`. Before sending, `ClientEditQueue` replaces the outgoing `base_version` with its shared optimistic version, records the pending transaction, and advances the optimistic version. Confirmed server versions are updated only when acknowledgements arrive on the background connection task.

`EditorWidget::local_command` never performs socket I/O. If an edit event exists and a queue is attached, it calls `ClientEditQueue::enqueue_edit_event`, which uses Tokio's bounded channel `try_send`. A full or missing queue does not prevent the already-applied local edit from rendering; failed sends roll back the pending-version reservation. A queue without a lease ID rejects enqueue attempts without sending an IPC edit message, which prevents read-only client sessions from emitting mutations even if called directly.

## Code Examples

```rust
let outcome = editor.command_with_event(EditorCommand::Insert("x"));
if let Some(event) = outcome.edit_event {
    edit_queue.enqueue_edit_event(event, transaction_id)?;
}
```

## Invariants and Constraints

- Ordinary edit messages are deltas, not snapshots.
- Edit emission uses existing cursor/selection/range data and does not inspect the whole document.
- Masonry input handlers stay synchronous and local; the only optional client boundary call is bounded `try_send` plus constant-time client sync metadata updates.
- Emitted operations are inert text edits only. Keyboard/pointer events are not serialized as commands, scripts, file paths, network requests, extension calls, or AI/tool actions.
- Without an installed client-first behavior manifest, local editing still works but no client edit event is emitted.
- With `DocumentAccess::ReadOnly`, text mutation commands are no-ops; navigation and selection still work.

## Tests

- `src/editor/surface.rs`: `insert_command_emits_insert_operation` validates caret insertion metadata.
- `src/editor/surface.rs`: `edit_event_carries_behavior_version` validates installed manifest version metadata.
- `src/editor/surface.rs`: `selection_replacement_emits_replace_operation` validates selected-range replacement metadata.
- `src/editor/surface.rs`: `backspace_emits_delete_operation_at_unicode_boundary` validates Unicode scalar boundary deletion ranges.
- `src/editor/surface.rs`: `delete_forward_selected_range_emits_delete_operation` validates normalized selected deletion ranges.
- `src/editor/surface.rs`: `read_only_editor_allows_navigation_but_not_mutation` validates observer UI behavior.
- `src/editor/surface.rs`: `editor_events_do_not_block_without_ipc_consumer` validates local edits without a sender/manifest.
- `src/client/mod.rs`: `edit_event_is_enqueued_as_client_edit_message` validates event-to-protocol conversion.
- `src/client/mod.rs`: `read_only_client_queue_does_not_emit_edit_message` validates queue-side read-only enforcement.
- `src/client/mod.rs`: `bounded_edit_queue_applies_backpressure` validates bounded queue behavior and pending rollback.
- `src/client/mod.rs`: `client_keeps_pending_edit_until_ack_or_rejection` validates optimistic base-version assignment and pending state.
- Relevant commands: `cargo test editor --quiet`, `cargo test client --quiet`, `cargo test --quiet`.

## Related

- [Client Snapshot Bootstrap](../modules/client-snapshot-bootstrap.md)
- [Protocol Codec](../modules/protocol-codec.md)
- [Server IPC Skeleton](../modules/server-ipc-skeleton.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
