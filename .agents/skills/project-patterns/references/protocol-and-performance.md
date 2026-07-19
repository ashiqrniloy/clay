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
- Use deltas/transactions instead of snapshots except for initial load, resync, or an atomically installed runtime-generation replacement whose mutually dependent state fits one bounded frame.
- Runtime-generation snapshots use the existing 1 MiB frame ceiling, complete latest-state recovery after broadcast lag, and one client acknowledgement only after validation and atomic install. Consider diffs/chunking only after measured payload/install thresholds justify a separate protocol decision.
- Runtime-generation snapshot decision source: `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
- Daily-editing local application (undo/redo inverse apply, clipboard cut/paste command handling, IME preedit paint, active-document chrome switch) must stay off IPC waits; save/conflict/open authority remain server-first/background relative to paint.
- Bound undo history at 256 entries per document and retained client document sessions at 64; clear a document's undo/redo on full resync/hard open-replace for that document.
- Daily-editing semantics decision source: `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`.
- Make UI-reactive server work cancellable and priority-aware.
- Incremental syntax highlighting parses once per accepted document version/grammar stream over a stable bounded window, using exact edit metadata and changed-range queries; decoration transport/cache chunking must not multiply parser jobs over the same window.
- Newer syntax versions cancel or coalesce superseded work, but the latest edit remains eligible immediately; do not use whitespace-only or idle-only parse scheduling.
- Client decoration state may interpolate inert spans through optimistic edits for visual continuity, while server-issued current-version syntax remains authoritative. Existing narrow syntax may inherit appended Unicode alphanumeric/underscore suffixes; whitespace, newline, punctuation, and structural edits end narrow-token inheritance.
- Every authoritative decoration chunk must contain complete capture state for exactly the UTF-8-safe range it replaces. Expand changed-range query coverage to complete touched replacement chunks before publication; never publish a wider authoritative range than was fully queried.
- Keep syntax beneath slower semantic layers, reject stale decoration versions, and add no client parser unless measured optimized server latency justifies a separate decision.
- Syntax continuity decision sources: `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md` and superseding `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`.
- Keep background AI/indexing/file work from delaying input confirmations or UI-reactive work.

## Testing Guidance

Plans involving protocol/performance should include tests for:

- Codec round trips.
- Oversized frame rejection.
- Invalid archive rejection.
- Delta edit messages with version metadata.
- Behavior manifest round trips when relevant.
- Non-blocking editor behavior when IPC consumer is absent or slow.
