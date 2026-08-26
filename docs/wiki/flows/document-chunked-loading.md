# Document Chunked Loading Flow

## Source

- `src/protocol/mod.rs` — `DocumentTextHead`, `DocumentChunk`, `DocumentChunkRequest`, `DocumentChunkRejected`
- `src/protocol/codec.rs` — frame-size guard against oversized chunks
- `src/perf/budgets.rs` — `MAX_CHUNK_BYTES`, `DEFAULT_MAX_FRAME_SIZE`
- `src/server/workspace.rs` — `open_existing_file`, `open_selected_file`, `reload_document`, `read_file_streamed`
- `src/server/document.rs` — rope read, `parse_windows_covering`
- `src/server/connection/mod.rs` — `handle_document_chunk_request`
- `src-tauri/src/bridge/dto.rs`, `forwarder.rs`, `session.rs` — Tauri bridge projection
- `src-tauri/src/bridge/session.rs` — `DocumentChunkRequest` identity-stamped before enqueue
- `frontend/src/bridge/types.ts` — `DocumentTextHeadDto`, `DocumentChunkDto`, `DocumentChunkRejectedDto`
- `frontend/src/editor/create-editor.ts` — `installInitial` head paint + progressive chunk assembly
- `frontend/src/editor/sync/session.ts` — chunk request state machine (one outstanding, versioned, append-til-complete)
- `frontend/src/editor/ClayEditor.tsx` — read-only guard during assembly, loading status
- `tests/large_document.rs` — 50 MiB open/edit/save/reload roundtrip, budget/binary refusal assertions
- `tests/protocol/codec.rs` — chunk frame-size and round-trip tests
- `tests/server/workspace.rs` — large-file streaming, head bound, budget accounting, binary sniff tests
- `scripts/large-document-smoke.sh` — private-socket large-file manual smoke path

## Overview

Plan 098 replaces the pre-v27 full-text-in-one-IPC-frame transfer with a
bounded head-plus-pull-chunks protocol. Every initial document transfer (open,
reload, resync, bootstrap layout restore) sends a `DocumentTextHead` carrying
the first ≤ 256 KiB and total byte count. The client requests remaining text
through versioned `DocumentChunkRequest` messages, each receiving at most
`MAX_CHUNK_BYTES` (256 KiB). This removes the previous 768 KiB per-file open
ceiling and prevents 1 MiB+ IPC frames without adding protocol complexity.

## Flow

```
Client                                  Server
  |                                       |
  |-- ClientMessage::OpenDocument ------->|
  |                                       |-- stream UTF-8 from disk
  |                                       |   (budget check, binary sniff)
  |                                       |-- reserve resident memory
  |                                       |-- create DocumentState
  |<-- ServerMessage::DocumentOpened -----|
  |    { head: DocumentTextHead            |
  |      { totalBytes, firstChunk } }     |
  |                                       |
  |-- paint firstChunk into CodeMirror    |
  |-- set read-only, show loading status  |
  |                                       |
  |-- ClientMessage::DocumentChunkRequest->|
  |    { documentId, version, offset }    |
  |                                       |-- validate access, version, offset
  |                                       |-- slice rope at offset, clamp to 256 KiB
  |<-- ServerMessage::DocumentChunk ------|
  |    { documentId, version,             |
  |      offset, bytes, isComplete }      |
  |                                       |
  |-- append bytes as annotated tx        |
  |-- if !isComplete, request next chunk  |
  |-- once totalBytes assembled:          |
  |     clear loading status              |
  |     enable editing                    |
  |                                       |
  |   ... normal edit flow ...            |
  |                                       |
  |-- ClientMessage::SaveDocument ------->|
  |                                       |-- clone rope (Arc, O(1))
  |                                       |-- stream chunks through atomic write
  |<-- ServerMessage::DocumentSaved ------|
  |                                       |
  |-- ClientMessage::ReloadDocument ----->|
  |                                       |-- stream from disk (budget, binary)
  |<-- ServerMessage::DocumentReloaded ---|
  |    { head: DocumentTextHead           |
  |      { totalBytes, firstChunk } }     |
  |                                       |
  |-- progressive assembly same as open   |
```

### Consumers

Every server→client document-text transfer uses `DocumentTextHead`:

1. **Initial open** (`DocumentOpened`): first open of a file-backed document.
2. **Reload** (`DocumentReloaded`): disk refresh of an existing document.
3. **Resync** (`ResyncSnapshot`): after connection loss or stale-edit rejection.
4. **Bootstrap layout restore**: persisted tab state reads document text via
   the same `open_existing_file` / head path.
5. **Open selected file** (`OpenSelectedFile`): server-issued capability token
   + explicit user picker path → same head+chunks path as 1.

### Rejections

`DocumentChunkRejected` carries a typed reason:

- `InvalidSize` — requested byte range exceeds `MAX_CHUNK_BYTES`.
- `OutOfRange` — offset past total bytes or negative.
- `StaleVersion` — client version does not match current document version.
- `AccessDenied` — requesting client holds no lease or the document was closed.
- `DocumentClosed` — document was closed concurrently.

The client clears pending chunk state on any rejection and does not retry
automatically. The server keeps no per-chunk-request state beyond validation;
a rejected request leaves no server-side artifact.

## Invariants and Constraints

- **No full-document IPC frame**: every wire message stays at or below
  `MAX_CHUNK_BYTES` (256 KiB) and falls within `DEFAULT_MAX_FRAME_SIZE`
  (1 MiB). Even a 50 MiB document produces a series of ≤ 256 KiB frames.
- **Server-owned security budgets**: `DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES`
  (256 MiB) and `BINARY_SNIFF_BYTES` (8 KiB) are checked during the
  streaming read phase, before any head or chunk is sent. A budget or binary
  failure returns `FileOperationFailed` with no document state installed.
- **Chunk clamp**: each chunk request returns at most `MAX_CHUNK_BYTES`.
  The server validates offset and length against both the bound and the
  actual rope length. Clients must not assume chunk count equals byte count
  divided by chunk size (the final chunk may be smaller).
- **In-flight window**: at most one outstanding chunk request per document.
  The client sends the next request only after the previous chunk arrives.
  This bounds bridge/frontend memory without explicit window tracking.
- **Assembly outside hot paths**: chunk assembly happens via CodeMirror
  transactions annotated as `programmatic` (visible in the change history
  but not emitted as user edits). The assembly never blocks React render,
  paint, layout, input handlers, or keypress-to-local-paint paths.
- **Atomic saves stream rope chunks**: saves clone the rope (Arc, O(1)) and
  stream its internal chunks through `atomic_write_chunks`. No full-document
  `String` is materialized for either open or save.
- **Versioned requests**: chunk requests include the document version the
  client believes is current. If the document was edited or reloaded between
  head and chunk, the server rejects with `StaleVersion` and the client
  re-requests a fresh head (via reconnect/resync/status).
- **Read-only during assembly**: CodeMirror is set read-only and shows
  "Loading full document…" until `totalBytes` is reached. Undo/redo history
  is not polluted by the assembly transactions.

## Code Examples

```typescript
// Frontend chunk assembly (simplified)
async function assembleChunks(
  session: Session,
  docId: string,
  version: number,
  totalBytes: number,
  head: string
) {
  let buffer = head;
  let offset = head.length;
  while (offset < totalBytes) {
    const chunk = await session.requestChunk(docId, version, offset);
    buffer += chunk.bytes;
    offset += chunk.bytes.length;
  }
  return buffer;
}
```

```rust
// Server chunk response
pub fn document_chunk_message(
    document: &Mutex<DocumentState>,
    client_id: ClientId,
    request: &DocumentChunkRequest,
) -> Result<ServerMessage, ()> {
    let doc = document.lock().unwrap();
    doc.check_access(client_id)?;
    doc.ensure_version(request.document_version)?;
    let offset = request.offset as usize;
    if offset > doc.rope.len() {
        return Err(()); // OutOfRange
    }
    let available = doc.rope.len() - offset;
    let chunk_size = available.min(MAX_CHUNK_BYTES);
    let bytes = doc.rope.slice(offset..offset + chunk_size).to_string();
    let is_complete = offset + chunk_size >= doc.rope.len();
    Ok(ServerMessage::DocumentChunk(DocumentChunk {
        document_id: request.document_id,
        document_version: doc.version(),
        offset: request.offset,
        bytes,
        is_complete,
    }))
}
```

## Tests

- `tests/large_document.rs::large_document_open_edit_save_reload_roundtrip_is_chunked`:
  50 MiB open through head+chunks (256 KiB each), edit, streamed save via
  `atomic_write_chunks`, reload equality. Asserts chunk bounds, UTF-8
  equality, no oversized frames.
- `tests/large_document.rs::oversize_and_binary_files_refuse_with_visible_errors`:
  resident-budget refusal at 257 MiB (exceeds 256 MiB budget) and binary
  sniff refusal for NUL in first 8 KiB. Both return typed `FileOperationFailed`
  diagnostics.
- `src/server/workspace.rs`: large-file streaming, head bounds, budget
  accounting, binary sniff, cross-read UTF-8 carry tests.
- `tests/protocol/codec.rs`: chunk frame-size upper-bound round trips.
- `tests/documentation_coverage.rs`: wiki navigation guard covers all linked
  pages including this flow page.
- `tests/primitives_docs.rs::document_chunk_transfer_primitive_is_bounded_and_documented`:
  verifies registry, index, protocol-codec wiki, bridge wiki, and budget
  constant names.