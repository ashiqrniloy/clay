# Document Chunked Loading Flow

## Source

- `src/protocol/mod.rs` — `DocumentTextHead`, `DocumentChunk`, request, and rejection shapes.
- `src/protocol/codec.rs` — frame and request-size guards.
- `src/perf/budgets.rs` — `MAX_CHUNK_BYTES`, resident-memory, and frame budgets.
- `src/server/workspace.rs` — streamed open/reload/read and atomic save.
- `src/server/document.rs` — rope head/chunk reads and parse-window slicing.
- `src/server/connection/documents.rs` — routed chunk request handling.
- `src/server/connection/mod.rs` — connection dispatch.
- `src-tauri/src/bridge/dto.rs` — typed bridge projection.
- `src-tauri/src/bridge/forwarder.rs` — bounded delivery.
- `src-tauri/src/bridge/session.rs` — identity stamping.
- `frontend/src/bridge/types.ts` — head/chunk DTOs.
- `frontend/src/editor/sync/session.ts` — one-owner load state machine.
- `frontend/src/editor/ClayEditor.tsx` — read-only/loading projection.
- `frontend/src/editor/create-editor.ts` — CodeMirror view setup.
- Tests: `src/server/workspace.rs`, `src/server/document.rs`, `src/protocol/codec.rs`, `frontend/src/editor/sync/session.test.ts`, `tests/editor_performance.rs`.

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
- Read-only/loading status is metadata; shell notification/persistence selectors
  ignore per-ack version/pending churn.
- Trace metadata may record numeric document/version/byte counts only; fixture
  content, paths, credentials, and source text do not enter reports.

## Tests

- `src/server/workspace.rs` — large-file stream, UTF-8 head boundary, binary
  sniff, resident budget, save/reload, and cross-read behavior.
- `src/server/document.rs` — rope head/chunk bounds and canonical version checks.
- `src/protocol/codec.rs` — chunk round trips, frame bound, and invalid request
  size rejection.
- `frontend/src/editor/sync/session.test.ts` — one request per offset,
  duplicate-chunk dedupe, same-length reload, no-history assembly, and
  detach/remount restoration.
- `tests/editor_performance.rs` — 50 MiB protocol open/edit/save/reload/resync
  matrix and close retirement.

Run focused coverage with:

```bash
cargo test --lib server::workspace::tests::open_existing_file_streams_large_utf8_text_and_bounds_head
cd frontend && npm test -- --run src/editor/sync/session.test.ts
```

## Related

- [Frontend Edit Synchronization](frontend-edit-synchronization.md)
- [React CodeMirror Editor](../modules/react-codemirror-editor.md)
- [Server Document State](../modules/server-document-state.md)
- [React Client Bridge](../modules/react-client-bridge.md)
- [Editor Viewport Render Patch](editor-viewport-render-patch.md)
- `docs/reference/primitives/registry.md#documentchunktransfer`
