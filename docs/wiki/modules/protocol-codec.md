# Protocol Codec

## Source

- `src/protocol/mod.rs`
- `src/protocol/codec.rs`
- `src/protocol/decorations.rs`
- `src/protocol/parse.rs`
- `src/protocol/folding.rs`
- `src/protocol/language_intelligence.rs`
- `src/protocol/agent.rs`
- `tests/editor_intelligence_protocol.rs`
- `tests/agent_protocol.rs`
- `tests/typography_protocol.rs`

## Overview

The protocol module defines the shared client/server IPC message contract. It uses owned Rust message types for business logic and keeps `rkyv` serialization, validation, and socket framing behind `Codec`. Wire protocol version 2 introduced `DecorationViewportRequest`; version 3 accompanies grouped native decoration chunks and analyzer-only diagnostic semantics; version 4 introduces the Phase 19 complete `RuntimeStateSnapshot` / `RuntimeGenerationInstalled` reload contract; version 5 (Plan 059) adds `ServerMessage::DecorationBatch` for single-frame multi-chunk parse updates and pairs with the `ReadPumpGuard` cancellation-safe framing pattern; protocol v15 moves `InitialDocument` and initial workspace SDUI after tab binding. Phase 28 adds protocol versions 20-23 for `FoldingRangeSet`, Link targets, InlayHint payloads, and completion recency metadata; Phase 25 bumps the pin to 24 for boxed agent messages; package UI reaches version 26. Plan 098 bumps the current wire pin to 27 for bounded document heads and pull-based chunks. Plan 099 bumps it to 28 for optional content-free performance trace IDs on viewport, parse, and decoration messages, and to 29 for the atomic viewport render protocol: `ViewportRenderRequest` (monotonic request id + visible byte bounds) is answered by exactly one `ServerMessage::ViewportRenderPatch` carrying ordered decoration/diagnostic/fold members, authoritative covered ranges, and a complete/empty/rejected terminal status. Older servers are rejected before incompatible message discriminants or payload semantics are used.

## Responsibilities

- Represent handshake and window messages: `Hello`, `Welcome`, inert behavior
  manifests/theme/typography, `TabCommand`/`TabRegistry` binding state,
  post-bind `InitialDocument`, document access, edit deltas/intents,
  acknowledgements, transactions, and errors.
- Represent Phase 5 synchronization metadata: client IDs, editable lease IDs, base document versions, behavior versions, confirmed server versions, stale-edit/read-only/lease/region-lock rejections, and resync snapshots.
- Represent Phase 9/19/Plan 060 file/workspace commands and results: workspace-root open, selected-file open, save, reload, status, list, explicit document close, document metadata, and typed file-operation failures.
- Represent Phase 12 SDUI bootstrap/update/action messages: `SduiSnapshot`, `SduiUpdate`, and `SduiAction`.
- Represent Phase 13 runtime diagnostics with severity, stable code, and sanitized message fields.
- Represent Phase 18 handoff decoration updates as bounded `DecorationSet` messages and protocol-v29 metadata-only `ViewportRenderRequest`/`ViewportRenderPatch` messages for scroll-driven window scheduling.
- Represent Phase 18.16.5 `ActiveTypography` separately from `ActiveTheme`: three bounded fallback-stack/size profiles, a revision, document defaults, and closed semantic roles.
- Define Phase 18 handoff parse shapes (`ParseEditNotification` and `IncrementalParseUpdate`) as serializable server-side data without adding parse results to hot edit-ack IPC.
- Keep viewport completion explicit: one request ID receives one complete, empty, or rejected `ViewportRenderPatch`; decoration, diagnostic, and fold members remain ordered inside that envelope.
- Encode and decode messages as `rkyv` payloads with a big-endian 4-byte length prefix.
- Reject oversized, incomplete, mismatched, or invalid frames before callers receive a protocol message.
- Avoid adding executable behavior, extension authority, direct file workspace authority, client-executed SDUI code, or AI mutation privileges.

## How It Works

`src/protocol/mod.rs` contains owned message enums and IDs. Ordinary text edits are represented as deltas (`Insert`, `Delete`, `Replace`) with byte ranges and inserted text rather than full-document payloads. Phase 5 edit messages include `document_id`, `client_id`, optional `lease_id`, `base_version`, `behavior_version`, and `transaction_id` so the server can validate authority and ordering before mutation. `ServerMessage::EditAck` returns a server-confirmed version, while `EditRejected` carries recoverable sync reasons such as stale/future versions, lease failure, read-only access, invalid ranges, or region-lock conflicts. Initial, open, reload, and resync snapshots carry `DocumentTextHead { total_bytes, first_chunk }`; ordinary edit acknowledgements remain metadata-only. `DocumentChunkRequest` identifies the authorized document, exact version, UTF-8 byte offset, and untrusted requested size. The server clamps that size to `MAX_CHUNK_BYTES` (256 KiB), slices the canonical rope at character boundaries, and returns `DocumentChunk` or typed `DocumentChunkRejected`. Completion is `offset + text.len() == total_bytes`; no redundant final flag exists.

`DocumentAccess::Editable { lease_id }` records the editable lease in the access state, while read-only observers use `DocumentAccess::ReadOnly`. Region-lock conflicts are described by `RegionLockConflict` and `LockOwner` metadata so later UI/AI phases can explain why an overlapping edit was rejected without granting AI, extension, file, shell, or network authority.

Phase 9 adds server-first file/workspace variants. `ClientMessage::OpenDocument`, `SaveDocument`, `ReloadDocument`, `GetDocumentStatus`, and `ListDocuments` carry client/document/workspace IDs and relative paths for server validation. Phase 19 adds `ClientMessage::OpenSelectedFile { client_id, selected_path }` for explicit user-selected native dialog results; the server still canonicalizes and validates the path before opening it. `ServerMessage::DocumentOpened` and `DocumentReloaded` carry bounded document heads; remaining text uses the same versioned chunk request/response primitive as initial and resync transfer. `DocumentSaved`, `DocumentStatus`, and `DocumentList` carry metadata only. `ClientMessage::CloseDocument { client_id, document_id, force }` releases one connection's access holder; dirty documents require explicit `force`. `ServerMessage::DocumentClosed` acknowledges whether access was released. Final-holder close and disconnect trigger server-side coordinator/session cleanup rather than leaving document-scoped caches alive. `ServerMessage::FileOperationFailed` uses `FileErrorCode` so callers can branch on stable errors such as `NotFound`, `OutsideRoot`, `InvalidUtf8`, `PermissionDenied`, `UnsupportedFileType`, `DirtyDocument`, and `StaleFileMetadata` without string matching.

Phase 12 adds SDUI protocol variants without a second serialization path. `ServerMessage::SduiSnapshot` carries a validated static `SduiTree` after bootstrap, `ServerMessage::SduiUpdate` carries bounded tree operations with base/new UI versions, and `ClientMessage::SduiAction` carries an inert action intent back to the server. Editor views bind by document ID/version; SDUI messages do not include full document text.

Phase 13 adds `RuntimeDiagnostic` and `ServerMessage::RuntimeDiagnostic` for server-side JavaScript/configuration errors. Diagnostics carry `DiagnosticSeverity`, stable Clay error code, and sanitized actionable message; they do not carry raw source snippets, absolute paths, environment dumps, tokens, or authority handles.

Phase 17 adds `ServerMessage::DecorationSet` for validated inline editor decorations. The message reuses the same codec boundary; server-side decoration validation enforces document version, viewport byte range, package provenance, inert style tokens, and `DECORATION_PAYLOAD_BUDGET_BYTES` before publication. Plan 059 (protocol version 5) adds `ServerMessage::DecorationBatch(Vec<DecorationSet>)` so multi-chunk parse updates ship in a single frame; single-chunk updates retain the plain `DecorationSet` wire shape. Protocol v29 `ClientMessage::ViewportRenderRequest` carries only client/document/version IDs, a monotonic request id, visible byte bounds, and optional trace metadata; it never carries document text. The server validates those fields, clamps the range to the document's total bytes before allocating parse windows, reads canonical text from the already-open document, and answers with exactly one `ServerMessage::ViewportRenderPatch` per request id: rejected (unknown document/stale version/invalid range/activation failure), empty (no registered handler), or complete with ordered decoration/diagnostic/fold members and covered ranges derived from those members. Edit-driven parse updates keep the per-update `DecorationBatch`/`DecorationSet`/`DiagnosticSet`/`FoldingRangeSet` frames. Phase 17 also defines `src/protocol/parse.rs` shapes for parse notifications and incremental parse updates; those types are `rkyv`-serializable for downstream/cache use, but the coordinator keeps parse updates server-side rather than adding them to ordinary edit acknowledgement messages.

`BehaviorManifest::minimal_text_editing` now builds the default declarative text behavior manifest with an ID, behavior version, scope, document font role, key bindings, command declarations, routing policies, and editor rules; it is data, not script code. `core.text` defaults proportional and `core.code` defaults monospace.

Phase 18.16.5 adds a separate `ServerMessage::ActiveTypography(ActiveTypography)` wire shape. Each snapshot has a revision and the user-owned `monospace`, `proportional`, and `ui` `FontProfile`s. A profile accepts at most eight non-control family names of at most 128 bytes, requires a final generic fallback, and accepts finite 6–96 logical-pixel sizes. `src/server/connection/mod.rs` sends the current snapshot as the final pre-bind handshake lane, after `ActiveTheme`, and broadcasts later revisions. `src/client/mod.rs` revalidates bootstrap/live snapshots before `TypographyRegistry` installation, keeping geometry-affecting updates separate from theme colors.

Phase 19 adds `src/protocol/runtime.rs` with `RuntimeGenerationId`, `RuntimeStateSnapshot`, `DocumentRuntimeRenderState`, and `PackageUiSnapshot`. `ServerMessage::RuntimeStateSnapshot` carries one complete connection-scoped generation (behavior, active theme, typography, SDUI, versioned package UI, per-document decoration/diagnostic resets, and runtime diagnostics) under the existing 1 MiB frame ceiling. Clients acknowledge only with `ClientMessage::RuntimeGenerationInstalled { client_id, runtime_generation_id }` after validation. Snapshots never carry document source text, absolute paths, tokens, grants, or executable callbacks. Package UI contribution payloads remain empty until package UI publication crosses IPC; the version still advances with the runtime generation so clients clear previous package UI under the same install boundary.

`Codec` in `src/protocol/codec.rs` serializes a client or server message with `rkyv::to_bytes`, checks the payload against `max_frame_size`, then prefixes the payload with its 32-bit length. Decode first validates the declared length against the configured maximum and the actual payload size. It then copies the payload into an aligned `rkyv::util::AlignedVec` before calling `rkyv::from_bytes`, which performs checked archived-byte validation through `bytecheck` before deserializing to the owned message type. Behavior manifest publications and behavior-version rejection messages cross this same boundary; there is no manifest-specific serialization side channel.

### Plan 086 archive hardening

Clay pins `rkyv = 0.8.17`. The decode boundary retains generic `CheckBytes<HighValidator>` and `Deserialize` bounds, rejects oversized declarations before payload copy, and has no unchecked fallback. `src/protocol/codec.rs` adds deterministic truncation, mutation, misaligned-length, and read-side oversized-declaration corpus tests. These assert rejection-or-validated-decode without panics or out-of-bounds access rather than requiring every corrupted byte sequence to fail: rkyv can validate a mutated archive whose fields still describe a different valid message. The fixed rkyv 0.8.16 advisories are never placed in `.cargo/audit.toml`; `docs/development/security.md` records the upgrade and evidence.

Plan 089 adds `compact_generated_frame_mutations_fail_closed_without_panicking`,
a bounded 64-seed × client/server corpus. Each case applies one deterministic
truncation, length-prefix mismatch, payload bit mutation, or oversized
prefix. Framing errors are rejected before archive access, truncated archives
must fail bytecheck, and every mutated decode is wrapped in `catch_unwind` so
malformed local-IPC bytes cannot panic the test boundary. This complements,
without replacing, the larger Plan 086 truncation/mutation sweeps.

## Code Examples

```rust
use clay::protocol::{codec::Codec, ClientMessage, PROTOCOL_VERSION};

let codec = Codec::default();
let frame = codec.encode_client_message(&ClientMessage::Hello {
    protocol_version: PROTOCOL_VERSION,
    client_name: "clay-client".to_string(),
})?;
let message = codec.decode_client_message(&frame)?;
```

## Cancellation-Safe Framing (Plan 059)

`Codec::read_server_message` and `Codec::read_client_message` call `tokio::io::AsyncReadExt::read_exact`, which is **not** cancellation-safe: a `tokio::select!` in the caller can drop the future mid-frame, leaving the stream position desynchronised. The caller then reads payload bytes as the next frame's length header, producing a corrupt frame-size error or archival validation failure.

Plan 059 removes the mid-frame drop risk by giving every connection a dedicated read-pump task that owns the reader half and pushes complete decoded messages into a bounded `mpsc` channel. The main `select!` loop races only `mpsc::recv()` calls (both branches cancellation-safe). A `ReadPumpGuard` (newtype over `tokio::task::AbortHandle` in `src/protocol/codec.rs`) aborts the pump task on `Drop` so every return/error path cleans up.

```
stream ──tokio::io::split──▶ reader ──[read-pump task]──▶ mpsc tx
                              writer ◀──[main select! loop]── mpsc rx
```

- **Client** (`src/client/mod.rs::run_connection`): already split via `tokio::io::split`; read-pump task spawned with `EDIT_QUEUE_CAPACITY` (256) channel.
- **Server** (`src/server/connection/mod.rs::handle_connection_with_analysis`): adds `tokio::io::split` after the sequential handshake; write half keeps the name `stream` so all 49 existing write sites need zero changes. Channel capacity 64 (single-client backpressure).

`ReadPumpGuard` is `pub(crate)` in `src/protocol/codec.rs` (derives `Debug`, not `Clone`/`Copy`) and is shared between client and server since `codec.rs` is the framed-transport boundary both sides share. `Codec` is `Copy` so pump task and main loop each get their own cheap copy.

## Invariants and Constraints

- `Codec` is the only protocol serialization boundary; client/server code should not call `rkyv` directly for wire messages.
- Adding, removing, or reordering a wire enum variant requires incrementing `PROTOCOL_VERSION`; handshake rejection prevents stale server processes from decoding changed discriminants.
- `DEFAULT_MAX_FRAME_SIZE` is 1 MiB to prevent accidental unbounded allocation from malformed IPC frames; the read-side declaration gate runs before payload allocation/copy.
- `MAX_CHUNK_BYTES` is 256 KiB. Values below four bytes reject, larger requests clamp, stale versions and invalid/non-UTF-8-boundary offsets return typed rejection, and document access-holder lookup runs before rope slicing.
- `rkyv` stays pinned at 0.8.17 for the fixed shared-pointer and hash-table validation advisories; archive validation cannot be disabled through configuration.
- The 4-byte frame prefix is not part of the archived payload, so decode realigns payload bytes before validation.
- Framed reads (`read_exact`) must be isolated in a spawned read-pump task that survives `select!` cancellation. Main loops race only cancellation-safe `mpsc::recv()` calls. `ReadPumpGuard` aborts the pump on `Drop` so no orphan task outlives the connection.
- Behavior manifests are inert declarations of built-in behavior and do not execute JavaScript, WASM, extensions, commands, or filesystem/network operations.
- File/workspace protocol messages carry workspace-relative or selected-file display paths and typed error codes; server-side workspace validation remains the authority for canonical host paths and selected-file grants.
- Post-Hello legacy `client_id` fields are correlation/compatibility data only: the connection task compares every one against its handshake-owned identity before dispatch. Document IDs likewise grant no access; open/access-holder state gates save, reload, status, list, resync, close, and output subscriptions.
- SDUI protocol messages are inert declarative state or server-routed action intents; validation lives in server helpers, while codec validation remains byte/frame focused.
- Decoration protocol messages are inert span data; decoration validation lives in `src/server/decorations.rs`, while codec validation remains byte/frame focused.
- Typography profile validation runs before publication/installation, not in codec or paint. The codec still validates archived bytes and frame length; it never discovers installed fonts or performs font I/O.
- Parse protocol shapes are inert server-side data; parse validation/scheduling lives in `src/server/parse_coordinator.rs`, and parse results are not sent over the hot edit-ack path.
- Runtime diagnostics are status payloads only. They report safe runtime/configuration failure detail and do not grant client-side JavaScript, filesystem, shell, network, package, WASM, AI, or workspace authority.

## Tests

- `src/protocol/codec.rs`: round-trip tests for hello, v15 tab commands/registry
  snapshots, deferred initial documents with Unicode, behavior manifest
  schema/publication updates, behavior-version rejection metadata, lease/version
  edit deltas, stale-edit rejection, resync snapshots, region-lock rejection
  metadata, file/workspace commands including `OpenSelectedFile` and
  `CloseDocument`, v29 viewport render requests/patches, workspace result messages,
  typed file-operation failures, v27 document heads/chunk requests/chunks/rejections with frame and size guards, v29 viewport render requests/complete-empty-rejected patches, SDUI snapshot/update/action messages, and runtime diagnostic messages.
- `src/server/connection/mod.rs`: table-driven forged-ID coverage for every post-Hello message family plus close/access/subscription cleanup tests.
- `src/protocol/codec.rs::protocol_round_trips_viewport_render_patches` and
  `src/server/connection/mod.rs::viewport_render_requests_answer_one_patch_per_request_id`:
  verify the shared codec boundary, patch statuses, request identity, and
  request-scoped delivery.
- `tests/typography_protocol.rs`: codec round trip for all profiles/revision plus invalid profile and role-layer rejection coverage.
- `src/client/mod.rs` and `src/server/connection/mod.rs`: bootstrap ordering and live-delivery tests consume the fifth `ActiveTypography` frame before post-bootstrap SDUI/capability traffic.
- `src/protocol/codec.rs`: rejection tests for oversized Phase 5 frames, oversized manifest messages, invalid client archived bytes, invalid server/manifest archived bytes, compact generated framing/archive mutations, truncation sweeps, deterministic byte mutations, misaligned declared lengths, and read-side oversized declarations.
- `tests/editor_intelligence_protocol.rs`: Phase 28 codec compatibility for the protocol version 29 handshake pin, folding, Link/InlayHint `DecorationSet`/`DecorationBatch`, inert `DecorationTarget` click/hover data, completion recency, hover request/results, and bounded malformed-frame rejection.
- `tests/agent_protocol.rs`: Phase 25 boxed agent command/message round-trips, secret omission, malformed-frame rejection, reserved `agent` domain.
- Relevant command: `cargo test protocol`.

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Decoration Transport](decoration-transport.md)
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
- `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`
- `concept.md`
- `roadmap.md`
