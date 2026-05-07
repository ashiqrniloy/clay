# Protocol and Performance Pattern

## IPC Protocol

- Keep protocol semantics separate from codec implementation.
- Use `rkyv` behind a small length-prefixed codec boundary.
- Validate received archived bytes before access.
- Bound frame sizes before allocation.
- Treat all IPC input as fallible, even local IPC.

## Message Shape

Include final-compatible metadata where practical:

- `document_id`
- `client_id`
- editable/read-only access state
- `lease_id` when editable
- base document version
- server version
- client transaction ID
- behavior version

Phase 3 may not fully enforce these fields, but plans should avoid message shapes that require later UI/protocol rewrites.

## Performance Rules

- No full-document IPC for ordinary edits.
- No synchronous server/JavaScript round trip before rendering normal typing.
- No IPC work in Masonry paint or text-event handlers.
- Use bounded queues for outgoing client edits.
- Use per-document edit ordering, not global serialization across all documents.
- Use deltas/transactions instead of snapshots except for initial load or resync.
- Make UI-reactive server work cancellable and priority-aware.
- Keep background AI/indexing/file work from delaying input confirmations or UI-reactive work.

## Testing Guidance

Plans involving protocol/performance should include tests for:

- Codec round trips.
- Oversized frame rejection.
- Invalid archive rejection.
- Delta edit messages with version metadata.
- Behavior manifest round trips when relevant.
- Non-blocking editor behavior when IPC consumer is absent or slow.
