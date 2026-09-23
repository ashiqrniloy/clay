# Document Chunked Loading Flow

## Source

- `src/protocol/mod.rs` — `DocumentTextHead`, `DocumentChunk`, request, and rejection shapes.
- `src/protocol/codec.rs` — frame and request-size guards.
- `src/perf/budgets.rs` — `MAX_CHUNK_BYTES`, resident-memory, and frame budgets.
- `src/server/workspace/mod.rs` — streamed open/reload/read and atomic save.
- `src/server/document.rs` — rope head/chunk reads and parse-window slicing.
- `src/server/connection/documents.rs` — routed chunk request handling.
- `src/server/ops/documents.rs` — package-facing `clay:documents` open/reload budget gate (Plan 126).
- `src/server/connection/mod.rs` — connection dispatch.
- `src-tauri/src/bridge/dto.rs` — typed bridge projection.
- `src-tauri/src/bridge/forwarder.rs` — bounded delivery.
- `src-tauri/src/bridge/session.rs` — identity stamping.
- `frontend/src/bridge/types.ts` — head DTO (generated `DocumentTextHead`) and the hand-written chunk payloads that arrive inside client events.
- `frontend/src/editor/sync/session.ts` — one-owner load state machine.
- `frontend/src/editor/ClayEditor.tsx` — read-only/loading projection.
- `frontend/src/editor/create-editor.ts` — CodeMirror view setup.
- `tests/large_document.rs` — real-server 50 MiB open/edit/save/reload and refusal scenarios.
- `tests/performance_budgets.rs` — source-shape guard against per-chunk allocation regressions.
- Tests: `src/server/workspace/mod.rs`, `src/server/document.rs`, `src/protocol/codec.rs`, `frontend/src/editor/sync/session.test.ts`, and `tests/editor_performance.rs`.

## Overview

Plan 098 uses a bounded head-plus-pull-chunks protocol for open, reload, and
resync. The server reads file content into its canonical rope, sends a bounded
`DocumentTextHead`, and serves versioned chunk requests. The frontend paints the
head immediately, appends chunks in order, and enables editing only after the
reported byte length is assembled.

The frontend does not build a second full string buffer. A pane's
`DocumentSession` keeps one current CodeMirror `Text`: `view.state.doc` while a
view is attached, or `detachedDoc` only while detached. Chunk writes are
programmatic, no-history transactions.

Plan 126 (Document Access Path Hardening) draws the line between this client path
and the package path: `documents.open`/`documents.reload` — the `clay:documents`
ops that hand a whole document to JavaScript — refuse documents over
`DOCUMENTS_OP_MAX_DOCUMENT_BYTES` (256 KiB) with a typed
`documents.document_too_large` error before any rope→`String` conversion, while
the head/chunk flow above stays deliberately ungated so the editor can open
multi-MiB files. Windowed server consumers (completion, language intelligence,
the parse-window prefix) read rope windows through the shared boundary helpers
instead of materializing the document, so a large open costs O(head + window)
server work rather than O(document) per consumer.

P1-1 keeps the mandatory full-rope server load but removes avoidable allocation
churn: `read_file_streamed` reuses one scratch buffer and carries up to three
incomplete UTF-8 bytes across read boundaries instead of allocating a combined
vector for every 64 KiB read. The debug open-to-head guard is
`max(500 ms, bytes / 25 MiB/s)`; full assembly retains a 5 s bound. These are
regression guards for the resident-rope design, not new runtime configuration.

## Flow

```text
Open/Reload/Resync
  -> server validates file, UTF-8, binary sniff, resident budget
  -> DocumentTextHead { totalBytes, firstChunk }
  -> session installs head into the one current Text
  -> loading/read-only edit gate + visible-head syntax viewport request
  -> one DocumentChunkRequest
  -> DocumentChunk at the requested UTF-8 offset
  -> append as no-history transaction
  -> request next offset after response
  -> totalBytes reached: clear loading, enable editing
```

1. `DocumentState::document_text_head` returns the first chunk at or below
   `MAX_CHUNK_BYTES` (256 KiB), adjusted to a UTF-8 boundary, plus total bytes.
2. `createDocumentSession.startLoad` installs `firstChunk` immediately, records
   the wire-byte offset, sets `loading`, and sends at most one
   `documentChunkRequest` at a time. Loading blocks edits but not
   `viewportRenderRequest`: the authoritative loaded prefix asks for visible
   syntax immediately. The next chunk offset is learned from the actual
   returned UTF-8 byte length; fixed chunk strides are not assumed.
3. `handle_document_chunk_request` routes by client/document identity, checks
   access/version/offset/size, slices the canonical rope, clamps the response,
   and returns `DocumentChunk` or typed `DocumentChunkRejected`.
4. `appendLoaded` appends to the attached CodeMirror document or the detached
   `Text` snapshot using `programmaticAnnotations()`. It never emits an edit or
   creates an undo entry. Duplicate, unsolicited, stale, or rejected chunks do
   not advance assembly.
5. When `nextAppend >= totalBytes`, `finishLoad` clears the gate and marks the
   editor ready. During loading, `ClayEditor` derives read-only from metadata
   and reconfigures the compartment only when the boolean changes.
6. Reload/resync marks the old load complete, clears pending edit state, and
   starts a fresh head/serialization. Content equality is used for snapshot
   installation, so equal-length changed text is not mistaken for unchanged
   text.
7. Save clones the server rope root and streams chunks through the existing
   atomic write path; neither save nor open requires a whole-document frontend
   `String`.

## Code Example

```typescript
// The session owns this state, not React or a second string buffer.
if (view) {
  view.dispatch({
    changes: { from: view.state.doc.length, insert: chunkText },
    annotations: programmaticAnnotations(),
  });
} else {
  detachedDoc = detachedDoc.append(textOf(chunkText));
}
```

`programmaticAnnotations()` combines the programmatic origin with
`Transaction.addToHistory.of(false)`.

## Consumers

The same head/chunk path serves:

- `DocumentOpened` for an existing workspace file;
- `DocumentReloaded`;
- `ResyncSnapshot` after stale edits or reconnect;
- layout restore opens after tab/root binding; and
- explicit selected-file opens after server-side grant validation.

## Rejections and fallback

- `InvalidRequestSize` — requested chunk size is below the minimum or exceeds
  the accepted request shape.
- `OutOfRange` — offset is outside canonical rope bytes.
- `StaleVersion` — the document changed between head and chunk request; the
  session requests a fresh resync.
- `AccessDenied`, `UnknownDocument`, or `DocumentClosed` — route/access no
  longer exists; assembly stops and the pane shows a sanitized diagnostic.
- Binary, invalid UTF-8, resident-memory, or workspace failures happen before
  document installation and surface as typed file-operation errors.

The client does not retry arbitrary chunk failures, and it never edits a
partially assembled document. A new head/reload/resync is the recovery boundary.

## Invariants and Constraints

- Every chunk is at most `MAX_CHUNK_BYTES` (256 KiB) and below the 1 MiB frame
  limit; offsets are UTF-8 byte boundaries.
- The P1-1 debug head budget is `max(500 ms, bytes / 25 MiB/s)` and the full
  50 MiB fixture load stays under 5 s; the budget catches allocation/throughput
  regressions without pretending a fixed 500 ms is realistic for a full-rope
  debug load.
- One outstanding chunk request exists per pane/document. Request state is
  bounded and no server chunk queue is retained. The in-flight rule is:

  ```text
  one outstanding request
  ```

  It is not a buffered chunk window.

- The server owns canonical text, versions, leases, filesystem access, binary
  sniffing, and resident-memory accounting.
- The frontend owns one current `Text`; there is no app-wide document-session
  singleton and no React-held source string.
- Head/chunk/reload/resync transactions are annotated and excluded from undo and
  edit emission. Ordinary user edits remain compact deltas.
- The package documents ops are budgeted (256 KiB, typed refusal, no path/content
  in the message); this client chunked path is not, so a document refused by the
  package op can still be open and editable in the editor.
- Read-only/loading status is metadata; shell notification/persistence selectors
  ignore per-ack version/pending churn.
- Trace metadata may record numeric document/version/byte counts only; fixture
  content, paths, credentials, and source text do not enter reports.

## Tests

- `src/server/workspace/mod.rs` — large-file stream, UTF-8 head boundary, binary
  sniff, resident budget, save/reload, and cross-read behavior.
- `src/server/document.rs` — rope head/chunk bounds and canonical version checks.
- `src/protocol/codec.rs` — chunk round trips, frame bound, and invalid request
  size rejection.
- `frontend/src/editor/sync/session.test.ts` — one request per offset,
  duplicate-chunk dedupe, same-length reload, no-history assembly, and
  detach/remount restoration.
- `tests/large_document.rs` — 50 MiB protocol open/edit/save/reload matrix,
  chunk ceilings, UTF-8 preservation, and oversize/binary refusals.
- `tests/editor_performance.rs` — protocol open/edit/save/reload/resync matrix
  and close retirement.
- `tests/performance_budgets.rs` — source guard for the hoisted read buffer and
  absence of the old per-chunk `combined` allocation, plus the package-op and
  provider-window budget pins (`chunked_document_security_budgets_are_pinned`,
  `plan126_provider_document_window_budgets_are_pinned_and_documented`).
- `src/server/js_runtime/tests/document_and_git_facades.rs` — `documents_open_over_budget_returns_typed_error`
  (open and reload legs, no document text to JS) and
  `documents_open_under_budget_unchanged` (golden contract below the cap).
- `src/server/connection/tests/language_intelligence.rs` — `static_completion_on_large_document_matches_small_document_results`
  (windowed consumer parity on a 4 MiB document).

Run focused coverage with:

```bash
cargo test --test runtime large_document::
cargo test --lib server::workspace::tests::open_existing_file_streams_large_utf8_text_and_bounds_head
cd frontend && bun run test src/editor/sync/session.test.ts
```

## Related

- [Frontend Edit Synchronization](frontend-edit-synchronization.md)
- [React CodeMirror Editor](../modules/react-codemirror-editor.md)
- [Server Document State](../modules/server-document-state.md)
- [React Client Bridge](../modules/react-client-bridge.md)
- [Editor Viewport Render Patch](editor-viewport-render-patch.md)
- [Server File Workspace Model](../modules/server-file-workspace.md) — the package-op budget that stops short of this path
- [Completion Snippet Expansion](../modules/completion-snippet-expansion.md) and [Language Intelligence](../modules/language-intelligence.md) — windowed consumers over the same rope
- `docs/reference/primitives/registry.md#documentchunktransfer`
