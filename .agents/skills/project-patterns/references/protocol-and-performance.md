# Protocol and Performance Pattern

## IPC Protocol

- Keep protocol semantics separate from codec implementation.
- Keep the Rust server transport on `rkyv` behind a small length-prefixed codec boundary. Tauri Rust translates validated server messages into bounded JSON-compatible frontend DTOs and typed channels; React never decodes archived bytes.
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
- No Tauri/server IPC wait in CodeMirror transaction application, React render, layout, or browser input handlers.
- Use bounded queues for outgoing client edits.
- Use per-document edit ordering, not global serialization across all documents.
- Use deltas/transactions instead of snapshots except for an atomically installed runtime-generation replacement whose mutually dependent state fits one bounded frame.
- Initial tab load, selected/path open (`DocumentOpened`), reload (`DocumentReloaded`), resync, and persisted-document restore opens use bounded chunked transfer: a head message carrying the first chunk plus total byte length, then versioned client-driven `DocumentChunkRequest`s clamped to `MAX_CHUNK_BYTES` and served by rope slices - never whole-document single-frame text. Runtime-generation refresh uses bounded rope inputs rather than full open-document strings. The per-file open gate is replaced by a server-owned session resident-memory budget plus binary sniffing; neither is user configuration. Editing gates on per-document load completion; typing never waits on chunk fetches. Trusted-runtime public open/reload JSON APIs must retain an explicit heap-safe bounded contract.
- Chunked document transfer decision source: `decision-logs/2026-08-25-1253-chunked-document-loading.md`.
- Runtime-generation snapshots use the existing 1 MiB frame ceiling, complete latest-state recovery after broadcast lag, and one client acknowledgement only after validation and atomic install. Consider diffs/chunking only after measured payload/install thresholds justify a separate protocol decision.
- Runtime-generation snapshot decision source: `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
- Daily-editing local application (undo/redo inverse apply, clipboard cut/paste command handling, IME preedit paint, active-document chrome switch) must stay off IPC waits; save/conflict/open authority remain server-first/background relative to paint.
- Bound undo history at 256 entries per document and retained client document sessions at 64; clear a document's undo/redo on full resync/hard open-replace for that document.
- Daily-editing semantics decision source: `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`.
- Make UI-reactive server work cancellable and priority-aware.
- Run advisory syntax through one bounded latest-wins session per document/grammar. Connection dispatch returns after canonical work and required responses; native parser work runs on a bounded blocking executor, never directly on Tokio workers.
- Viewport syntax uses explicit request IDs and complete success/empty/rejected atomic patches. Tauri may coalesce obsolete whole patches only; it must never coalesce required sibling patch members independently.
- Keep package-selected syntax authority and executable syntax-management behavior server-side. CodeMirror owns local text/viewport and inert render projection, not arbitrary package parser execution. A client-local parser is a separate metric-gated decision, not the default performance fix.
- Incremental syntax highlighting parses once per accepted document version/grammar stream over a stable bounded window, using exact edit metadata and changed-range queries; decoration transport/cache chunking must not multiply parser jobs over the same window.
- Newer syntax versions cancel or coalesce superseded work, but the latest edit remains eligible immediately; do not use whitespace-only or idle-only parse scheduling.
- Client decoration state may interpolate inert spans through optimistic edits for visual continuity, while server-issued current-version syntax remains authoritative. Existing narrow syntax may inherit appended Unicode alphanumeric/underscore suffixes; whitespace, newline, punctuation, and structural edits end narrow-token inheritance.
- Every authoritative decoration chunk must contain complete capture state for exactly the UTF-8-safe range it replaces. Expand changed-range query coverage to complete touched replacement chunks before publication; never publish a wider authoritative range than was fully queried.
- Applying current authoritative decorations must replace only the declared viewport. Subtract that exact range from overlapping provisional package/layer state, preserve geometry outside it, and coalesce only local compatible residuals; never delete a whole provisional chunk merely because it overlaps authority.
- Keep syntax beneath slower semantic layers, reject stale decoration versions, and add no client parser unless measured optimized server latency justifies a separate decision.
- Syntax continuity decision sources: `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`, superseding `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`, and `decision-logs/2026-07-19-2238-exact-range-provisional-decoration-replacement.md`.
- Syntax session/viewport patch decision source: `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`.
- Keep background AI/indexing/file work from delaying input confirmations or UI-reactive work.

## Testing Guidance

Plans involving protocol/performance should include tests for:

- Codec round trips.
- Oversized frame rejection.
- Invalid archive rejection.
- Delta edit messages with version metadata.
- Behavior manifest round trips when relevant.
- Non-blocking editor behavior when IPC consumer is absent or slow.
