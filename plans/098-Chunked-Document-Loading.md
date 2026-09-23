# Plan 098: Chunked Document Loading — Remove the File Size Ceiling

## Objectives

- Remove `MAX_OPENABLE_FILE_BYTES` (768 KiB) as a hard per-file open gate so users can work with large files (multi-MiB and beyond).
- Replace full-text-in-one-frame document transfer (initial handshake, selected/path open, reload, resync, and persisted-document restore opens) with a bounded chunked transfer protocol that keeps the 1 MiB IPC frame ceiling and Clay's "no full-document IPC" performance rules.
- Replace the per-file size gate with a session-level resident-memory budget (`LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` = 256 MiB, already reserved) plus binary-content sniffing, with rejections surfaced through the existing status-bar/pane diagnostic path (Plan 097 follow-up fix).
- Keep server authority over documents, versions, validation, and file/workspace ownership unchanged; keep ordinary typing off IPC waits.

## Expected Outcome

- Opening any UTF-8 text file that fits the session resident-memory budget succeeds: after bounded streaming validation/build completes, the client paints the first chunk and fetches the remainder in bounded frames.
- Files larger than the session memory budget, and binary files, are refused with typed, user-visible errors (no silent failures).
- Initial/open/reload/resync transfer, runtime-generation open-document refresh, and atomic save avoid unbounded whole-document `String` materialization on the server; the 1 MiB frame cap is never exceeded.
- Protocol version bumps 26 → 27; the supervisor protocol probe gates mixed-version adoption as implemented in Plan 097.

## Authority Boundary Statement

- **Server (Rust)** owns canonical documents, the rope, versions, chunk serving, validation (binary sniff, budget), file/workspace authority, and atomic save. Chunk requests are ordinary server-authoritative requests; the server clamps sizes and validates ranges.
- **Tauri shell** remains a narrow transport bridge; chunk payloads pass through the existing validated rkyv → JSON DTO path within frame bounds.
- **React/CodeMirror** owns progressive editor-doc assembly, loading-state presentation, and the local hot path; typing stays CodeMirror-local and never waits on chunk fetches (editing gates on load completion for the affected document only).
- No new authority is introduced: no filesystem/network/shell/script/AI authority changes; packages are unaffected; no package JavaScript participates in chunking.

## Tasks

- [x] Record the decision log for chunked document loading
  - Acceptance Criteria:
    - Functional: `decision-logs/` contains an approved entry recording: removal of the per-file size ceiling, adoption of chunked head+chunk transfer for initial/open/reload/resync and persisted restore opens, session memory budget as the replacement guard, binary sniffing, and the protocol version bump.
    - Code Quality: The entry follows the existing decision-log format and cites this plan.
    - Security: The entry states what authority is *not* introduced (none; transport-only change) and that the memory budget remains a server-owned security budget, not user configuration.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `src/perf/budgets.rs:320-360` (current ceiling rationale and reserved 256 MiB budget)
      - `docs/development/performance.md` (full-document IPC prohibition)
    - Options Considered:
      - Raise the ceiling and frame cap together: rejected — unbounded frames block the connection read loop and violate documented performance rules.
      - Keep ceiling, improve error surfacing only: rejected — user explicitly requires large-file support.
    - Chosen Approach:
      - Chunked transfer with pull-based client-driven chunk requests; decision log records user approval from this plan's creation conversation.
    - API Notes and Examples:
      ```text
      decision-logs/2026-08-25-1253-chunked-document-loading.md
      ```
    - Files to Create/Edit:
      - `decision-logs/2026-08-25-1253-chunked-document-loading.md`: approved decision entry.
    - References:
      - `plans/098-Chunked-Document-Loading.md`
      - User instruction 2026-08-25 (no file size ceiling; large-file work is required)
  - Test Cases to Write:
    - None (documentation task); verified by repository diff/format checks and direct decision-log structure review.
  - Completion Evidence:
    - Created `decision-logs/2026-08-25-1253-chunked-document-loading.md` with approved decision, context, alternatives, evidence, references, and consequences.
    - Updated `.agents/skills/project-patterns/references/protocol-and-performance.md` with the reusable bounded chunk-transfer rule and decision-log source.
    - Verified the decision log cites this plan and records no new filesystem, network, shell, script, AI, or package authority.
    - Verification passed: `git diff --check`, decision-log structure validation, and `cargo test --test protocol documentation_coverage` (10 passed).

- [x] Baseline: inventory current document-transfer surfaces and guard pins
  - Acceptance Criteria:
    - Functional: A written inventory (in this plan's task evidence or the wiki draft) lists every full-text transfer/materialization surface: `InitialDocument`, `ResyncSnapshot`, `DocumentOpened`, `DocumentReloaded` (src/protocol/mod.rs:2718-2790), runtime-generation refresh `open_document_snapshots` (src/server/workspace.rs:1254), `save_io`/`reload_io` full-text materialization (src/server/workspace.rs:2209-2255), trusted-runtime `documents.serverOpenDocument`/reload JSON snapshots (src/server/ops/documents.rs), and frontend/Tauri consumers (`InitialDocumentDto`, `frontend/src/editor/sync/session.ts:142-374`).
    - Performance: Baseline numbers recorded: current frame cap (1 MiB), ceiling (768 KiB), reserved budget (256 MiB).
    - Code Quality: Every guard test pinning the old coupling is enumerated: `src/protocol/codec.rs:1075-1083` (`MAX_OPENABLE_FILE_BYTES < DEFAULT_MAX_FRAME_SIZE`), budget tests in `src/server/workspace.rs:3663-3727`, `tests/performance_budgets.rs` references.
    - Security: The inventory notes where the size gate currently acts as the memory-exhaustion guard so its replacement is explicit.
  - Approach:
    - Documentation Reviewed:
      - `src/perf/budgets.rs` (budget constants and comments)
      - `src/protocol/codec.rs` (frame bounds and guard test)
      - `src/server/workspace.rs` (open/read/save paths)
      - `src/server/document.rs` (rope snapshot/parse-window APIs)
    - Options Considered:
      - Skip inventory and design directly: rejected - initial/open/reload/resync and persisted-restore paths share the transfer mechanism while runtime refresh, save, analyzers, and trusted-runtime APIs have adjacent full-text materialization; missing one leaves a latent unbounded path.
    - Chosen Approach:
      - Grep-driven inventory committed into the protocol task's `Files to Create/Edit` before implementation.
    - API Notes and Examples:
      ```bash
      rg -n 'InitialDocument|ResyncSnapshot|open_document_snapshots|MAX_OPENABLE_FILE_BYTES' src/ frontend/src/ tests/
      ```
    - Files to Create/Edit:
      - None (analysis task; outputs feed tasks below).
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - None (inventory feeds later tests).
  - Completion Evidence:
    - Protocol payloads carrying full text: `ServerMessage::InitialDocument`, `ResyncSnapshot`, `DocumentOpened`, and `DocumentReloaded` in `src/protocol/mod.rs:2718-2790`; request origins are `ClientMessage::RequestResync`, `OpenDocument`, `OpenSelectedFile`, and `ReloadDocument`.
    - Server producers: `DocumentState::initial_document_message` and `resync_snapshot_message_for_client` (`src/server/document.rs:151-183`); `open_document_response` and `reload_document_response` (`src/server/connection/documents.rs:225-440`); command/path open conversion (`src/server/connection/workspace.rs:296`); and `write_document_open_response`, which sends the snapshot and immediately consumes its text for parse/analysis follow-up (`src/server/connection/workspace.rs:45-83`).
    - Runtime-refresh materialization: `OpenDocumentSnapshot { metadata, text }` (`src/server/workspace.rs:353-359`), `WorkspaceState::open_document_snapshots` (`src/server/workspace.rs:1254-1267`), and the runtime reload refresh caller (`src/server/mod.rs:1233-1245`). This is not frontend bootstrap/layout restore; persisted panes reopen through ordinary `OpenDocument` requests. Runtime reload currently materializes every open document's full text for reclassification, parse startup, and analysis refresh even though classification and native parse need bounded rope slices and document analysis already rejects text above 256 KiB.
    - Save/reload and public runtime paths: `save_io` clones `document.text()` before atomic write (`src/server/workspace.rs:2209-2245`); `reload_io` uses the bounded full-text reader (`src/server/workspace.rs:2246-2255`); `src/server/ops/documents.rs` returns full `text` fields from `serverOpenDocument` and `serverReloadDocument`, so the Clay JS API task must explicitly preserve the runtime heap bound or define a chunk-aware contract.
    - Frontend/Tauri consumers: `InitialDocumentDto` carries `text` (`src-tauri/src/bridge/dto.rs:185-207`, `frontend/src/bridge/types.ts:34-40`); Rust client bootstrap consumes `InitialDocument.text` (`src/client/mod.rs:1373-1463`); `frontend/src/editor/sync/session.ts` uses `replaceText` for initial, resync, opened, and reloaded events (`:142-153`, `:310-385`).
    - Guard pins: `DEFAULT_MAX_FRAME_SIZE = 1 MiB` (`src/protocol/codec.rs:13`); `full_text_snapshot_exceeding_frame_limit_is_rejected_at_encode` plus the `MAX_OPENABLE_FILE_BYTES < DEFAULT_MAX_FRAME_SIZE` compile-time assertion (`src/protocol/codec.rs:1073-1101`); `MAX_OPENABLE_FILE_BYTES = 768 KiB` and its rationale (`src/perf/budgets.rs:331-352`); open/selected/reload/boundary/grow-during-read tests (`src/server/workspace.rs:3663-3760`, `:4048-4075`); and `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB = 256` plus documentation/value guards (`tests/performance_budgets.rs:90-108`, `:497`).
    - Additional coupling: `MAX_OPENABLE_FILE_BYTES` is also the data-only full-context ceiling for all five first-party native grammar descriptors in `src/server/syntax.rs:306-394`, with the bound asserted at `src/server/syntax.rs:2431`; removing the file-open gate must replace this parse-context constant rather than silently shrinking grammar behavior.
    - Scope adjustment recorded: tasks 3-5 now must chunk `DocumentOpened`/`DocumentReloaded` as well as `InitialDocument`/`ResyncSnapshot`, and task 10 must review the existing public JS full-text APIs before the old constant is deleted. The approved decision log and protocol-performance pattern were corrected to name these additional paths.
    - Verification passed: inventory commands (`rg`), `git diff --check`, `cargo test --test protocol documentation_coverage` (10 passed), and `cargo test --test protocol performance_budgets` (26 passed).

- [x] Review existing document-transfer primitives and plan the generic chunk primitive
  - Acceptance Criteria:
    - Functional: A primitive assessment states what existing primitives already provide and specifies one generic `DocumentChunkTransfer` protocol primitive for initial handshake, `DocumentOpened`, `DocumentReloaded`, `ResyncSnapshot`, and persisted-document restore opens. Runtime-generation refresh and save reuse the same rope range/chunk foundations without pretending disk writes are protocol transfers.
    - Code Quality: The primitive is document-generic, has no mode/package/language branch, and separates reusable transfer metadata/range validation from parse-specific `ParseWindowSnapshot` fields.
    - Security: Chunk serving reuses the existing authorized-document lookup/access-holder gate, validates the requested document version and exact UTF-8 start boundary, clamps response size server-side, and returns a typed rejection rather than leaving the client loading forever.
    - Performance: Chunk extraction copies only one bounded rope slice; protocol chunks remain on the bounded asynchronous connection/bridge lanes; no chunk fetch, assembly, parse, file IO, or package JavaScript enters CodeMirror transaction application, React render, paint, layout, or browser input handlers.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/audit.md`, `docs/reference/primitives/registry.md`
      - `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/desktop-typed-bridge.md`, `docs/wiki/modules/react-codemirror-editor.md`, `docs/wiki/modules/server-file-workspace.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`, `.agents/skills/project-patterns/references/authority-boundaries.md`, `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `src/server/document.rs`, `src/server/workspace.rs`, `src/server/connection/documents.rs`, `src/protocol/mod.rs`, `src/protocol/codec.rs`, `src/client/mod.rs`, `src-tauri/src/bridge/{dto,forwarder,session}.rs`, `frontend/src/editor/sync/session.ts`
      - Exact local `crop` v0.4.3 source: `Rope::byte_slice`, `Rope::chunks`, `Rope::is_char_boundary`, `RopeBuilder::append`; Context7 was queried three ways but did not index the Rust text-rope crate, so version-exact Cargo registry source is authoritative.
      - UI-stack review for the later loading-state consumer: `.agents/skills/clay-ui/SKILL.md`, `.agents/skills/clay-ui/references/components.md`, `.agents/skills/clay-ui/references/tokens.md`, `.agents/skills/impeccable/SKILL.md`, `.agents/skills/full-output-enforcement/SKILL.md`, `.agents/skills/high-end-visual-design/SKILL.md`, `.agents/skills/design-taste-frontend/SKILL.md`.
    - Options Considered:
      - Reuse `ParseWindowSnapshot` directly: rejected - it carries package/mode IDs, guard context, base positions, and parse lifecycle semantics unrelated to document transfer.
      - Separate chunk protocols for initial/open/reload/resync: rejected - duplicated boundary, version, authority, and completion rules would drift.
      - Push all chunks after each head: rejected by the approved decision - requires per-client server streaming/flow-control state.
      - Pull-based shared head/request/chunk/rejection messages over the existing ordered connection: chosen - server remains stateless between requests, requests can pipeline four-deep, and document/version/offset identify interleaved responses without a new transfer registry.
      - Include both `final` and `totalBytes`: rejected - redundant completion sources can disagree; completion is exactly `offset + text.len() == head.totalBytes`.
      - Hold the document mutex while streaming save chunks: rejected - it would delay canonical edit acknowledgements; clone Crop's Arc-backed `Rope` under the lock, then iterate `Rope::chunks()` after releasing it.
    - Chosen Approach:
      - Add `DocumentTextHead { total_bytes, first_chunk }` to all full-text snapshot families. Add `DocumentChunkRequest { document_id, document_version, offset, max_bytes }`, `DocumentChunk { document_id, document_version, offset, text }`, and a typed `DocumentChunkRejected` response. The client stores head total/version, validates every exact next offset, derives completion from total bytes, and restarts through resync on stale version.
      - Add one `DocumentState` rope-slice helper that validates version/start, floors the end to a UTF-8 boundary, and copies at most `MAX_CHUNK_BYTES`; route requests only after `document_for_message` confirms access. Values above the cap are clamped; zero or too-small requests and non-boundary/out-of-range offsets are rejected deterministically.
      - Reuse Crop primitives rather than inventing a rope layer: `RopeBuilder` plus a standard-library UTF-8 carry buffer (up to three trailing bytes) for streamed file reads, `Rope::byte_slice` for protocol chunks/leading content/parse windows, and an O(1) Arc-backed `Rope::clone` plus `Rope::chunks` for save.
      - Persisted layout restore already issues ordinary `OpenDocument` requests per pane, so it receives `DocumentOpened` heads. `open_document_snapshots` is an internal runtime-generation refresh path, not frontend bootstrap; replace its full text with metadata plus bounded rope-derived classification/parse/analysis inputs in the server refresh task.
    - API Notes and Examples:
      ```rust
      pub struct DocumentTextHead {
          pub total_bytes: u64,
          pub first_chunk: String,
      }

      DocumentChunkRequest {
          document_id: DocumentId,
          document_version: DocumentVersion,
          offset: u64,
          max_bytes: u32,
      }
      DocumentChunk {
          document_id: DocumentId,
          document_version: DocumentVersion,
          offset: u64,
          text: String,
      }
      ```
    - Files to Create/Edit:
      - None in this assessment task. Protocol/server/frontend tasks below implement the primitive; final documentation adds `DocumentChunkTransfer` to `docs/reference/primitives/{index,registry}.md`, the wiki flow, and deterministic primitive coverage.
    - References:
      - `.agents/skills/create-plan/references/clay.md` (primitive-first requirement)
      - `decision-logs/2026-08-25-1253-chunked-document-loading.md`
  - Test Cases to Write:
    - Deferred to implementation tasks: UTF-8 boundary and concatenation equality, max-size clamp, zero/small/invalid offset rejection, stale-version restart, unauthorized-document denial, interleaved document routing, and streamed-save equality.
  - Completion Evidence:
    - Existing foundations confirmed: Crop v0.4.3 provides byte-indexed rope slicing, boundary checks, incremental `RopeBuilder`, O(1) Arc-root clone, and chunk iteration; `DocumentState` already owns the rope/version/access holders and parse-window slicing; codec rejects over-limit frames before allocation; Rust/Tauri live lanes preserve bounded FIFO delivery.
    - Existing access path confirmed: `document_for_message` resolves only the connection's welcome document or a workspace document whose `DocumentState::has_access(client_id)` is true. The chunk route will reuse this gate, then validate requested version and range under the document lock.
    - Existing frontend owner confirmed: `frontend/src/editor/sync/session.ts` exclusively installs initial/open/reload/resync text and CodeMirror owns live text. Progressive assembly belongs there, but its current retained `authoritativeText: String`/`snapshotText(): string` must become load state backed by CodeMirror `Text` so the new path does not recreate a giant contiguous string.
    - Adjacent full-text consumers identified: open follow-up classification needs only bounded leading content; native syntax can consume existing rope parse windows; document analysis must copy text only when `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` (256 KiB) permits it; runtime-generation refresh must stop calling full-text `open_document_snapshots`; save must stream an O(1) rope snapshot outside the document lock.
    - Plan corrected: removed the false `open_document_snapshots -> BootstrapDto` assumption, added versioned typed chunk rejection and stale-transfer restart, removed redundant `final`, required the exact Crop UTF-8 carry behavior, and preserved persisted restore through ordinary `DocumentOpened` heads.
    - Verification passed: `git diff --check`, `cargo test --test protocol primitives_docs` (29 passed), `cargo test --test protocol documentation_coverage` (10 passed), and `cargo test --test protocol performance_budgets` (26 passed).

- [x] Protocol v27: chunked document transfer messages, codec bounds, and guards
  - Acceptance Criteria:
    - Functional: Protocol version bumps to 27. `InitialDocument`, `ResyncSnapshot`, `DocumentOpened`, and `DocumentReloaded` carry `DocumentTextHead`. New versioned `DocumentChunkRequest`, `DocumentChunk`, and typed `DocumentChunkRejected` messages serve every snapshot family. Unknown-version peers fail cleanly via the existing probe/handshake.
    - Performance: `MAX_CHUNK_BYTES` (256 KiB) const added; server clamps `max_bytes`; guard test proves `MAX_CHUNK_BYTES` + envelope < `DEFAULT_MAX_FRAME_SIZE`; a 1 GiB document requires only bounded per-frame allocations.
    - Code Quality: serde camelCase naming consistent with existing DTOs; messages carry document ID, document version, and offset; completion derives from the head's total bytes (no redundant `final` flag); the codec guard test replacing `MAX_OPENABLE_FILE_BYTES < DEFAULT_MAX_FRAME_SIZE` pins the new invariant.
    - Security: Frame-size validation happens before allocation (existing codec path); chunk requests are treated as fallible input; no execution or authority semantics attach to chunk messages.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/mod.rs` (enum conventions, version history comments)
      - `src/protocol/codec.rs` (length-prefix, bounds, guard tests)
      - rkyv 0.8.17 exact local rustdoc/source (`cargo tree -i rkyv`, `cargo doc -p rkyv --no-deps`) plus Context7 `/websites/rs_rkyv` checked-access and enum round-trip guidance
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
    - Options Considered:
      - Push-based server streams all chunks after head: rejected — needs server-side per-client streaming state and flow control; pull-based reuses request/response and lets the client prioritize/pipeline.
      - Pull-based client-driven chunks with bounded in-flight window: chosen — stateless server reads, resumable, testable, matches viewport-request precedent.
      - gRPC-style separate transfer channel: rejected — new transport for no measured need.
    - Chosen Approach:
      - Head message carries first chunk plus total bytes; client issues versioned `DocumentChunkRequest`s with at most four in flight until the exact next offset equals `total_bytes`. Start offsets must be UTF-8 boundaries; server floors each bounded end to a boundary. Stale versions and invalid ranges return typed rejection so loading cannot hang silently.
    - API Notes and Examples:
      ```rust
      // src/perf/budgets.rs
      pub const MAX_CHUNK_BYTES: usize = 256 * 1024;
      // src/protocol/mod.rs (v27)
      DocumentTextHead { total_bytes: u64, first_chunk: String }
      InitialDocument { document_id, version, head: DocumentTextHead, access, lease_id, workspace_root }
      DocumentChunkRequest { document_id: DocumentId, document_version: DocumentVersion, offset: u64, max_bytes: u32 }
      DocumentChunk { document_id: DocumentId, document_version: DocumentVersion, offset: u64, text: String }
      DocumentChunkRejected { document_id: DocumentId, document_version: DocumentVersion, offset: u64, reason: DocumentChunkRejection }
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: version bump, message variants, docs.
      - `src/perf/budgets.rs`: `MAX_CHUNK_BYTES`; remove or repurpose `MAX_OPENABLE_FILE_BYTES` (task below).
      - `src/protocol/codec.rs`: guard test swap (chunk bound < frame cap).
      - `src-tauri/src/bridge/dto.rs`: chunk DTO passthrough.
      - `frontend/src/bridge/types.ts`: typed head/chunk/rejection DTOs on initial/open/reload/resync events.
      - `src/client/mod.rs` + `src-tauri` probe tests: protocol version expectations.
      - `src/server/document.rs`, `src/server/connection/{mod,documents}.rs`: shared access/version/range/UTF-8 validation and bounded chunk response routing.
      - `docs/development/tauri-react-parity-ledger.json`: exhaustive client/server family inventory.
      - `docs/reference/primitives/{index,registry}.md`, `docs/wiki/modules/{protocol-codec,desktop-typed-bridge}.md`, `tests/primitives_docs.rs`: primitive and bridge implementation documentation with drift guard.
    - References:
      - `src/protocol/mod.rs` (version 26 bump precedent, Plan 097)
      - `docs/wiki/modules/desktop-typed-bridge.md`
  - Test Cases to Write:
    - `src/protocol/codec.rs`: chunk-size guard test replacing the ceiling pin; head message with first-chunk-only serializes within frame cap for a 256 KiB chunk.
    - Protocol roundtrip tests: head/chunk/rejection archive + serde round trips; `max_bytes` clamping; zero/too-small size and invalid offset rejection.
    - Transfer consistency: stale requested version returns typed rejection; exact next offsets concatenate to `total_bytes`; no `final` field exists.
    - Version test: v26 peer vs v27 server handshake fails with actionable `UnsupportedProtocolVersion` (existing probe semantics).
  - Completion Evidence:
    - Bumped `PROTOCOL_VERSION` to 27 and replaced snapshot `text` fields with `DocumentTextHead` across initial, open, reload, resync, Rust client state, Tauri bootstrap/event DTOs, and frontend typed mirrors.
    - Added identity-stamped `DocumentChunkRequest`, bounded `DocumentChunk`, and typed `DocumentChunkRejected`/`DocumentChunkRejection`; no redundant final flag. Server routes requests only through the existing access-holder lookup, checks exact version/UTF-8 offset, rejects invalid sizes below four bytes, and clamps all larger requests to `MAX_CHUNK_BYTES` (256 KiB).
    - Replaced the old file-ceiling/frame coupling guard with compile-time and encoded-frame checks proving a 256 KiB head/chunk plus envelope remains below `DEFAULT_MAX_FRAME_SIZE`; existing codec still rejects declared oversized frames before payload allocation and bytecheck-validates rkyv archives.
    - Added rkyv/serde round trips, camelCase/no-final JSON pin, stale/non-boundary/size rejection, UTF-8 concatenation equality, v26-to-v27 handshake rejection, exhaustive bridge family guards, parity-ledger coverage, primitive registry/wiki documentation, and deterministic primitive-doc coverage.
    - Current open/reload producers preserve existing behavior by wrapping their still-ceiling-bounded full text as a complete head. The next server/open and frontend progressive-load tasks replace that transition adapter with a 256 KiB first head plus follow-up requests; no large-file claim is made by this protocol-only task.
    - Verification passed: root `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` (1126 lib, 30 presentation, 191 protocol, 68 runtime, 130 security; benches passed); desktop fmt/check/clippy and `cargo test --all-targets` (28 lib + integration suites); frontend typecheck and 103 Vitest tests. Frontend full lint remains blocked by a pre-existing unrelated non-null assertion in `frontend/src/editor/extensions/controller.test.ts:208`.

- [x] Server: chunked open path — remove size ceiling, add memory budget and binary sniff
  - Acceptance Criteria:
    - Functional: Opening or reloading a file sends a head (first chunk + total) and serves `DocumentChunkRequest`s by rope-slicing without whole-document `String` materialization. This covers bound-tab `InitialDocument`, selected/path `DocumentOpened`, and `DocumentReloaded`; no open/reload response emits an unchunked full-text payload. `MAX_OPENABLE_FILE_BYTES` per-file gate is removed. A session-level budget (`DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES` = 256 MiB, from `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB`) refuses opens whose total resident bytes would exceed it, with typed error surfaced via the existing `FileOperationFailed`/status diagnostic path. Binary files (NUL byte in first 8 KiB) are refused with a typed `BinaryFileNotSupported` error.
    - Performance: File read builds the canonical rope incrementally without a second full-size `String`; the first IPC head is emitted after complete UTF-8 validation/commit, not before a partial document is known valid. Chunk serving is O(log N + chunk) rope slicing with no O(N) prefix scan.
    - Code Quality: `read_file_bounded` becomes a streaming rope builder with metadata validation retained (regular-file check, identity capture); budget accounting lives in `WorkspaceState` next to `MAX_DOCUMENTS_PER_CLIENT` accounting; all new server functions are `pub(crate)` or private unless exposed via Clay JS APIs (none expected).
    - Security: Budget and sniff are server-owned security budgets (not user configuration); access checks (existing access-holder model) apply to chunk requests; no new authority introduced; symlink/regular-file metadata validation retained.
  - Approach:
    - Documentation Reviewed:
      - `src/server/workspace.rs:1860-1940` (current gate + `read_file_bounded`)
      - `src/server/document.rs` (rope construction, `parse_windows_covering` precedent for rope slicing)
      - `crop` crate docs (rope chunk/byte APIs) via local `cargo doc`/registry source
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
    - Options Considered:
      - Read whole file then build rope: rejected — full-size transient allocation defeats the purpose for 100 MB+ files.
      - Stream file -> rope via Crop `RopeBuilder` in bounded pieces with a standard-library UTF-8 carry buffer: chosen.
      - Per-file configurable ceiling via init.js: rejected — budget stays a server-owned security budget consistent with JS heap limit precedent; per-file config invites unsafe defaults.
    - Chosen Approach:
      - Open: metadata validate -> sniff first 8 KiB for NUL -> budget check (sum of open doc byte lengths + file size <= budget) -> stream-read into `RopeBuilder`, carrying up to three trailing UTF-8 bytes across reads -> commit only after complete validation -> emit head from the canonical rope.
      - Chunk serving: reuse authorized-document lookup; require matching version and exact start boundary; clamp oversized requests, reject zero/too-small requests, floor end to a boundary, and return one bounded rope slice.
      - Budget release on document close already exists via document teardown; accounting hook added there.
    - API Notes and Examples:
      ```rust
      // src/perf/budgets.rs
      pub const DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES: u64 =
          LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB * 1024 * 1024;
      pub const BINARY_SNIFF_BYTES: usize = 8 * 1024;
      // WorkspaceError additions
      DocumentBudgetExceeded { path, budget_bytes, current_bytes, requested_bytes }
      BinaryFileNotSupported { path }
      ```
    - Files to Create/Edit:
      - `src/perf/budgets.rs`: new constants; delete `MAX_OPENABLE_FILE_BYTES` (fix all references: `src/server/syntax.rs` window caps move to `MAX_CHUNK_BYTES`-derived or dedicated parse-window const).
      - `src/server/workspace.rs`: streaming open/reload, reservation/accounting, sniff, chunk-serving head support, error variants; update old ceiling tests to budget/sniff/large-file tests.
      - `src/server/document.rs`: rope head and chunk helpers (offset-addressed, char-safe), rope replacement.
      - `src/protocol/mod.rs`: error code mapping for new variants.
      - `src/server/connection/mod.rs`: route `DocumentChunkRequest`.
      - `src/server/connection/documents.rs`: emit chunked `DocumentOpened`/`DocumentReloaded` heads and follow-up messages.
      - `src/server/connection/workspace.rs`: convert command/path open results to the shared head+chunk response.
      - `src/server/command_execution.rs`: command-open result carries `DocumentTextHead`, not full text.
      - `src/server/ops/documents.rs`: preserve trusted runtime reload API by reading canonical text only at its existing public JSON boundary.
      - `frontend/src/shell/workspace-controller.ts` + `frontend/src/app/layout/app-shell.tsx` + `frontend/src/shell/PaneTree.tsx`: surface new error codes (reuse Plan 097 diagnostic path).
    - References:
      - `src/server/document.rs:526` (`parse_windows_covering` — rope-slice precedent)
      - `docs/wiki/modules/server-file-workspace.md`
  - Test Cases to Write:
    - Open a synthetic >768 KiB UTF-8 text file: succeeds; peak transient allocation remains bounded (no full `String` materialization, asserted by construction plus a source guard); a multibyte scalar split across disk-read boundaries remains valid.
    - Chunk serving: exact boundary offsets return char-safe chunks; non-boundary/out-of-range offsets reject; concatenation of head+chunks equals original bytes (protocol task).
    - Budget: opening files summing beyond 256 MiB budget refuses with typed error; closing a document releases budget; subsequent open succeeds.
    - Binary sniff: file with NUL in first 8 KiB refused; NUL after 8 KiB accepted and remains documented as outside the classifier.
    - `max_bytes` clamp: requesting 1 GiB yields `MAX_CHUNK_BYTES` (protocol task).
  - Completion Evidence:
    - Removed `MAX_OPENABLE_FILE_BYTES`, `FileTooLarge`, and the frame-coupled full-read path. Added `DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES` (256 MiB), `BINARY_SNIFF_BYTES` (8 KiB), and a dedicated `NATIVE_GRAMMAR_MAX_WINDOW_BYTES` parse-context budget so native grammar context no longer depends on file-open capacity.
    - `WorkspaceState` now tracks resident rope bytes plus cancellation-safe in-flight reservations. Open, selected-file open, and reload reserve budget before unlocked IO; commit accounts actual rope bytes, duplicate/failure/cancellation paths release reservations, and final close/disconnect releases resident bytes.
    - `read_file_streamed` opens one handle, retains regular-file/authority metadata checks, sniffs NULs only in the first 8 KiB, validates UTF-8 with a three-byte carry across 64 KiB reads, appends directly to `crop::RopeBuilder`, and rejects growth beyond the reservation without allocating a document-sized `String`. Reload swaps the resulting rope through `DocumentState::replace_rope_from_storage`.
    - Initial/open/selected/reload producers now construct bounded heads from the canonical rope; command-open results carry `DocumentTextHead` instead of full text. Existing trusted `documents.openDocument`/`reloadDocument` JSON contracts remain isolated at their public boundary for the dedicated Clay JS API review task.
    - Added typed `DocumentBudgetExceeded` and `BinaryFileNotSupported` diagnostics, large-file open/selected/reload tests above the former 768 KiB ceiling, budget release tests, leading/late NUL tests, UTF-8 read-boundary tests, budget constant pins, source guards against `read_to_end`/document-sized UTF-8 materialization in the streaming helper and full-text head construction in connection open responses, and updated current architecture/performance/workflow/wiki documentation.
    - Verification passed: root `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1128 lib, 30 presentation, 193 protocol, 68 runtime, 130 security; benches passed), targeted workspace tests (78 passed), protocol primitive/performance/documentation/API registry guards (30/28/10/48 passed), desktop fmt/check and `cargo test --all-targets` (28 library + integration suites), and frontend typecheck.
- [x] Server: chunked resync, persisted restore, and runtime-generation refresh
  - Acceptance Criteria:
    - Functional: `ResyncSnapshot` sends a versioned head and the client fetches the remainder through `DocumentChunkTransfer`. Persisted panes continue restoring through ordinary `OpenDocument -> DocumentOpened(head)` requests. Runtime-generation reload refreshes open documents without `OpenDocumentSnapshot.text` or any other unbounded full-text snapshot.
    - Performance: Restoring N large persisted documents never places multiple full-text protocol snapshots in memory; each pane uses one bounded head plus at most four in-flight chunks. Runtime refresh reads only bounded leading content/parse windows and copies analyzer text only below `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES`.
    - Code Quality: One shared chunk-serving function covers initial/open/reload/resync. Runtime refresh uses existing generic classification, parse-window, and analysis primitives without a parallel bootstrap transfer path; document-id ordering remains deterministic.
    - Security: Resync and restored-open chunk requests reuse access-holder/version/range validation. Runtime refresh operates only on already-open server-authorized document handles and grants nothing.
  - Approach:
    - Documentation Reviewed:
      - `src/server/workspace.rs:1254-1267` (`open_document_snapshots`)
      - `src/server/mod.rs:1228-1245,1496-1545` (runtime-generation candidate and open-document refresh)
      - `src/server/connection/documents.rs` (classification, native parse, analysis startup)
      - `src/client/mod.rs` (initial handshake/resync), `frontend/src/shell/workspace-controller.ts` (persisted paths reopen through `session.open`)
      - `docs/wiki/modules/react-client-bridge.md`, `docs/wiki/modules/react-codemirror-editor.md`
    - Options Considered:
      - Add per-document heads to `open_document_snapshots` and `BootstrapDto`: rejected - that path is runtime reload, not frontend layout bootstrap; it would create a second restore protocol.
      - Keep runtime refresh full text because it is server-internal: rejected - it still duplicates every open rope and passes oversized text to consumers that need bounded input.
      - Reuse ordinary `DocumentOpened` heads for persisted restore and make runtime refresh metadata/rope-window based: chosen.
      - Parallel fetch across restored documents: deferred - each tab connection already restores independently; increase concurrency only after measurement.
    - Chosen Approach:
      - Keep initial handshake bootstrap limited to its bound document head. Layout persistence reopens each stored path through existing `OpenDocument`, then each pane session fetches its chunks.
      - Replace `OpenDocumentSnapshot { metadata, text }` with metadata-only refresh records (or equivalent IDs resolved to current handles). Classification receives a bounded leading-content rope slice; native syntax receives `DocumentState::parse_window_snapshot`; document analysis receives a full reset only when the existing 256 KiB analyzer ceiling permits it, otherwise its existing sanitized degradation diagnostic is emitted without cloning text.
    - API Notes and Examples:
      ```rust
      pub(crate) struct OpenDocumentRefresh {
          pub(crate) metadata: DocumentMetadata,
      }
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: metadata-only runtime refresh enumeration.
      - `src/server/mod.rs`: resolve bounded rope inputs after generation commit.
      - `src/server/connection/documents.rs`: rope-based classification/parse/analysis follow-up helpers.
      - `src/client/mod.rs`: versioned resync head/chunk event flow.
      - `src-tauri/src/bridge/session.rs` / `dto.rs`: initial and resync head DTO threading.
      - `frontend/src/bridge/types.ts`, `frontend/src/shell/workspace-controller.ts`: persisted-open and resync progressive handling.
    - References:
      - Plan 097 Phase 5/6 (`workspaceRootId` propagation and persisted `session.open` restore)
  - Test Cases to Write:
    - Resync equality: head plus chunks byte-equals canonical text for multibyte content.
    - Stale resync transfer: another client's accepted edit causes typed stale rejection; partial assembly is discarded and new-version resync completes.
    - Persisted restore with three documents including one 10 MB: all panes become ready without a multi-document bootstrap snapshot.
    - Runtime reload with a 50 MB open document: behavior refresh succeeds without full `String`; analyzer degrades at its existing ceiling; native parse receives a bounded rope window.
  - Completion Evidence:
    - Replaced `OpenDocumentSnapshot { metadata, text }` with metadata-only `OpenDocumentRefresh` and `WorkspaceState::open_document_refreshes`. Runtime-generation candidate enumeration no longer clones canonical ropes. Document-id order stays sorted.
    - `refresh_open_documents_after_reload` resolves current handles after commit and reuses `open_document_followup_messages` plus `start_document_analysis`. Classification uses a 512-byte rope prefix; open parse uses `DocumentState::parse_window_snapshot` capped by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`; analysis copies `document.text()` only when `byte_len() <= DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES`, otherwise emits `analysis.document_too_large` without cloning.
    - Shared `document_text_head` / `document_chunk_message` path already covers initial/open/reload/resync. Persisted restore remains ordinary `OpenDocument` per pane (`workspace-controller.restore`); added a 3-pane restore test that emits three `openDocument` payloads and no bootstrap snapshot. Progressive editor chunk assembly stays the next UI task.
    - Tests: resync head+chunks equals multibyte canonical text; stale chunk request after an accepted edit returns `StaleVersion` then a version-2 resync head; three-document open including 10 MB keeps bounded heads and ordered refreshes; 50 MB runtime reload refreshes without `DocumentOpened`/`DocumentReloaded` or a 50 MiB debug payload and reports `analysis.document_too_large`. Source guard pins `open_document_refreshes`, `parse_window_snapshot`, and the analysis ceiling.
    - Wiki updated: `server-file-workspace.md`, `persistent-runtime-hot-reload.md`, `workspace-file-browser.md`, `server-ipc-skeleton.md`.
    - Verification passed: root `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1133 lib, 30 presentation, 193 protocol, 68 runtime, 130 security; benches passed), frontend typecheck and 104 Vitest tests.

- [x] Server: streaming atomic save for large documents
  - Acceptance Criteria:
    - Functional: `save_io` writes the rope to the temp file in bounded chunks (no whole-document `String`), preserving the existing atomic temp+fsync+identity-revalidate+rename sequence and permissions restore.
    - Performance: Saving a 100 MB document takes an O(1) Arc-backed Crop rope snapshot under the document lock, releases the lock, then allocates O(chunk) transient memory while writing; canonical edits and acknowledgements do not wait on disk IO.
    - Code Quality: Reuses `atomic_write_file` structure; chunk iteration via `crop` chunk API; no duplicated identity/validation logic.
    - Security: Atomic-save guarantees unchanged (no torn writes); identity revalidation before rename retained; no new authority.
  - Approach:
    - Documentation Reviewed:
      - `src/server/workspace.rs:2209-2245` (`save_io`, `atomic_write_file`)
      - `crop` rope chunk iteration API (local crate source)
      - `docs/development/file-open-save-reload-workflow.md`
    - Options Considered:
      - Keep full-String save (2× transient for 100 MB): rejected — same exhaustion class the plan removes.
      - Hold the document lock while asynchronously writing rope chunks: rejected - slow disk would delay canonical edits.
      - Clone the Arc-backed Crop rope under lock, then stream its chunks after releasing the lock: chosen.
    - Chosen Approach:
      - Capture `(version, rope.clone())` while holding the document mutex, release it, then replace `text.as_bytes()` with sequential `rope.chunks()` writes through the same atomic temp-file helper.
    - API Notes and Examples:
      ```rust
      // Rope::clone is an Arc-root clone in crop 0.4.3.
      // atomic_write_rope(path, rope: &Rope, identity)
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: `save_io`, `atomic_write_file` signature.
    - References:
      - `docs/development/file-open-save-reload-workflow.md` (atomic save contract)
  - Test Cases to Write:
    - Save equality: streamed save byte-equals the captured rope snapshot for multibyte content across Crop chunk boundaries.
    - Concurrency: an edit accepted while disk write is blocked does not wait on the document mutex and leaves the document dirty after the older snapshot commits.
    - Atomicity: simulated identity change mid-save still refuses rename (existing test extended to streaming path).
  - Completion Evidence:
    - `save_io` captures `(version, DocumentState::clone_rope())` under the document mutex, drops that lock, then streams `rope.chunks()` through `atomic_write_chunks`. `atomic_write_file(&[u8])` remains a thin wrapper so existing identity/permission tests keep working. Temp create, `fsync`, Unix mode restore, identity revalidate, and rename are unchanged.
    - Tests: `streamed_save_equals_rope_across_crop_chunks` writes multibyte text spanning Crop chunks and byte-equals the captured rope; `edit_during_blocked_save_does_not_wait_on_document_mutex` pauses after chunk writes so a concurrent `apply_edit` finishes, then the older snapshot commits and the document stays dirty (`hello world` on disk, `hello world!` in memory). Existing `atomic_save_fails_closed_when_target_is_replaced_before_rename` and `save_document_reports_stale_when_target_changes_during_write` still refuse rename. Guard in `workspace_open_path_stays_streamed_and_head_bounded` pins `clone_rope` / `chunks(` and forbids `document.text()` / `as_bytes()` in `save_io`.
    - Wiki/docs: `server-file-workspace.md`, `server-document-state.md`, `file-open-save-reload-workflow.md`.
    - Verification passed: root `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1135 lib, 30 presentation, 193 protocol, 68 runtime, 130 security; benches passed).

- [x] Frontend: progressive editor load with chunk assembly and loading state (UI task)
  - Acceptance Criteria:
    - Functional: Receiving a head creates the CodeMirror editor with the first chunk immediately (fast first paint), then appends chunks in order via transactions; editing is gated (read-only + visible loading state) until the document reaches ready; budget/binary rejections surface in status bar and empty pane via the Plan 097 diagnostic path.
    - Performance: Ordinary typing never waits on chunk fetches (gating applies only pre-ready); chunk fetches use a bounded in-flight window (4) and never run in React render or CodeMirror transaction handlers; first-paint latency ≤ current full-text path for files ≤ 768 KiB (regression check).
    - Code Quality: Chunk assembly lives in the editor sync session (single owner), unit-tested; current `authoritativeText: String` / `snapshotText(): string` becomes a CodeMirror `Text`-backed load snapshot so assembly never recreates one giant JS string; no chunk logic lives in components; loading state uses cataloged primitives and theme tokens only.
    - Security: No browser storage, no direct invoke outside the bridge facade; chunk payloads arrive only through the typed bridge DTOs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` (+ `references/components.md`, `references/tokens.md`)
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - `frontend/src/editor/sync/session.ts`, `frontend/src/editor/create-editor.ts` (bootstrap/replaceText path)
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
    - Options Considered:
      - Buffer all chunks, single `replaceText` at end: rejected — delays interactive doc and materializes a giant JS string.
      - Progressive append transactions: chosen — CodeMirror Text is rope-backed; appends are O(chunk).
      - Allow editing of loaded prefix during load: rejected — version/ack semantics assume whole-doc consistency; complexity not justified while local chunk fetch is fast.
    - Chosen Approach:
      - Session state machine: `loading(head received)` → `ready(all chunks)`; chunk requests pipelined with bounded in-flight; appends via one transaction per chunk batched per animation frame; read-only compartment lifted at ready; loading indicator in pane (catalog text primitive) and status bar phase text.
    - API Notes and Examples:
      ```ts
      // session.ts (sketch)
      onInitialDocument(head) { createEditor(head.firstChunk); requestChunks(head.totalBytes); }
      onDocumentChunk(chunk) {
        requireExactNextOffset(chunk);
        appendText(chunk.text);
        if (nextOffset === totalBytes) setReady();
      }
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/sync/session.ts`: chunk state machine.
      - `frontend/src/editor/create-editor.ts`: read-only-until-ready compartment.
      - `frontend/src/bridge/client.ts` / `frontend/src/bridge/types.ts`: chunk request/response plumbing.
      - `frontend/src/shell/PaneTree.tsx`: loading indicator (catalog primitive).
      - `frontend/src/app/layout/app-shell.tsx`: status phase text (reuse existing status element).
      - `frontend/src/editor/extensions/controller.ts`: suppress viewport requests until ready.
    - References:
      - Plan 097 Phase 5 evidence (editor bootstrap), Plan 097 diagnostic surfacing fix (2026-08-25)
  - Test Cases to Write:
    - Session unit test: head+chunks assemble a byte-identical multibyte document; gap/overlap/wrong-version chunks reject; ready fires exactly once.
    - Gating test: edits attempted pre-ready are not queued; post-ready typing flows normally.
    - Regression: small-file open path latency unchanged (first paint uses first chunk).
    - Error surfacing test: budget/binary codes render in status + empty pane (extends Plan 097 tests).
  - Completion Evidence:
    - Session state machine (`frontend/src/editor/sync/session.ts`): heads enter `loading` when `firstChunk` bytes < `totalBytes`; chunks append into a rope-backed authoritative snapshot (`Text.append`, never one giant JS string) and dispatch as annotated transactions; `snapshotDoc(): Text` replaces the old string-backed `snapshotText`, and the editor is created directly from that `Text`. Ready clears exactly once; stale-version chunk rejections trigger resync, other rejections abort with the gate closed and a diagnostic.
    - Deviation from the drafted approach: the in-flight window is **1** (sequential), not 4. Server replies are clamped to UTF-8 char boundaries, so each region's end is only known after its reply lands — fixed-stride pipelining strands on short replies and mid-char offsets get `invalidOffset` rejected. Sequential fetch continues from the received end, is provably gap-free, and still beats the budget by orders of magnitude (50 MB = ~200 local IPC round trips).
    - Gating: `DocumentMeta.loading` gates `emitUserChanges` (no pre-ready edits queued), drives the read-only compartment in `ClayEditor`, suppresses viewport requests in `EditorProjection`, and shows a visible loading status (`role="status"`) in the pane plus a "Loading document…" phase in the shell status bar.
    - Tests: session unit tests cover multibyte assembly with first-paint-before-chunks, sequential pacing from received ends, wrong-version drop, edit gating pre-ready, stale-rejection resync restart, and the small-file single-frame regression (no chunk requests); controller test pins viewport suppression while loading; component test pins the loading status lifecycle. Budget/binary refusals keep using the Plan 097 `fileOperationFailed` status/empty-pane path (already tested).
    - Verification passed: frontend typecheck, eslint clean, 111 Vitest tests, production build within budgets (shell 161.6/180 kB gzip, total 344.6/400 kB), Rust protocol documentation guards pass.
    - Wiki/docs: `docs/wiki/flows/frontend-edit-synchronization.md` (new progressive-load section), `docs/wiki/modules/react-codemirror-editor.md`.

- [x] End-to-end large-file verification
  - Acceptance Criteria:
    - Functional: A real desktop session opens, edits, saves, reloads, and closes a ≥ 50 MB synthetic UTF-8 file; a > budget file refuses with visible error; a binary file refuses with visible error.
    - Performance: No IPC frame exceeds 1 MiB (codec assertion in test); open→first paint < 500 ms for 50 MB on the dev host; full load < 5 s; server RSS during open stays within rope + bounded overhead; bundle budgets unchanged (no new shell code beyond session/controller changes).
    - Code Quality: The scenario is automated where possible (protocol-level integration test) and scripted for manual execution.
    - Security: The 50 MB fixture is synthetic (generated), never committed user content; fixtures live under a test path excluded from review artifacts.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map)
      - `scripts/package-smoke.sh` (scripted verification precedent)
    - Options Considered:
      - Only manual verification: rejected — needs repeatable regression coverage.
      - Protocol-level automated test + scripted manual desktop scenario: chosen.
    - Chosen Approach:
      - Rust integration test drives server connection with a generated 50 MB document through open/edit/save/resync; manual script documents the desktop flow for the test-plan task.
    - API Notes and Examples:
      ```bash
      # generate synthetic fixture at test time (never committed)
      head -c 52428800 /dev/urandom | base64 > /tmp/clay-large-fixture.txt
      ```
    - Files to Create/Edit:
      - `tests/large_document.rs`: new integration test (or extend `tests/suites/`).
    - References:
      - `src/server/runtime_sandbox.rs` ETXTBSY retry precedent for robust test spawning
  - Test Cases to Write:
    - 50 MB open/edit/save/reload roundtrip equality; frame-size assertion; budget refusal; binary refusal.
  - Completion Evidence:
    - `tests/large_document.rs` (registered in the `runtime` suite) drives a real in-process `IpcServer` over its IPC endpoint with the deterministic mixed-unicode generator (`perf::fixtures`, seed-fixed, regenerated in memory as the expected byte source):
      - Roundtrip: connect → capability → tab → selected-path open of a generated 50 MiB file → `DocumentOpened` head (262,142-byte first chunk ≤ `MAX_CHUNK_BYTES`) → sequential `DocumentChunkRequest` assembly to exactly `total_bytes` → `Edit` insert at offset 0 (multibyte emoji text, lease carried from metadata, behavior version captured from the observed manifest) → `EditAck` v2 → `SaveDocument` → on-disk bytes compared equal to the edited expectation → `ReloadDocument` → re-assembled chunks compared byte-equal to the saved file.
      - Refusals: a sparse 257 MiB file refuses with `FileErrorCode::DocumentBudgetExceeded` naming the resident document budget (metadata-size gate fires before any read); a NUL-bearing file refuses with `BinaryFileNotSupported`; both messages are the typed, user-visible diagnostics surfaced by the Plan 097 status-bar/empty-pane path.
      - Bounds/timing: every chunk and the head are asserted ≤ `MAX_CHUNK_BYTES` (256 KiB, far under the 1 MiB codec ceiling pinned by `src/protocol/codec.rs` head/chunk frame tests); measured on the dev host: open→head 253 ms (< 500 ms), open→full 50 MiB load 488 ms (< 5 s); both tests together finish in ~2 s.
      - RSS note: the server runs in-process with the test harness, so an isolated server-RSS figure is not observable here; streaming structure (rope-sliced chunk responses, streamed save, metadata-gated opens) bounds server memory to rope + fixed buffers by construction — no full-document String materialization path exists post-098.
    - `scripts/large-document-smoke.sh`: generates the 50 MiB fixture via the existing `perf-fixture` command into `target/perf-fixtures/` (its only allowed output root), copies it plus a sparse oversize file and a binary sample into a temp workspace, starts a workspace-scoped `clay server` on a private workspace socket, launches `clay client <socket>` against that server, prints the five-step manual checklist, and cleans up only its processes/fixtures on exit. The private endpoint prevents stale default-server adoption. Setup steps verified; desktop walkthrough belongs to the manual-test-plan task.
    - Docs: `docs/development/performance.md` (automated + scripted verification pointers under Manual Large-File Smoke Setup), wiki flow page lists the new integration test.
    - Verification passed: `cargo test --test runtime` 70/70 (incl. both new tests), suite-inventory guard satisfied, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, documentation/manual-smoke guard suites green. Fixtures live only in the OS temp dir / `target/` (gitignored); nothing committed.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: Real Linux desktop build exercised through: small-file open (regression), large-file open showing loading then ready, budget-refusal state, binary-refusal state; screenshots captured for each state via the fixture/CDP harness; findings recorded.
    - Performance: Review observes no typing jank post-ready on the large file.
    - Code Quality: Evidence stored under the established review artifact path; statuses recorded (PASS/UNRESOLVED with reasons).
    - Security: No user content leaks into committed artifacts (fixture-only captures; AT-SPI dumps preferred per established practice).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
      - `docs/wiki/modules/ui-review-harness.md` (fixture capture + real-app AT-SPI methodology)
      - `docs/development/launch-and-gui-smoke.md` (current harness entry point)
      - Plan 097 review-log precedent
    - Options Considered:
      - Desktop screenshots: rejected — established practice excludes them (user-content leak risk).
      - Fixture CDP screenshots + real-app AT-SPI: chosen.
    - Chosen Approach:
      - Add fixture states for `document-loading`, `document-budget-error`, `document-binary-error`; capture wide/narrow; AT-SPI verify status live region announces loading/ready/errors.
    - API Notes and Examples:
      ```bash
      # Dev-only Vite fixture routes, captured through browser CDP when available
      /fixture/document-loading
      /fixture/document-budget-error
      /fixture/document-binary-error
      /fixture/editor
      ```
    - Files to Create/Edit:
      - `frontend/src/routes/fixture.tsx`: new fixture states.
      - `docs/wiki/modules/ui-review-harness.md`: Plan 098 fixture route list and
        capture boundary.
      - `docs/development/launch-and-gui-smoke.md`: current review entry point
        and supplemental browser-fixture notes.
      - Review artifacts directory: screenshots + AT-SPI dumps + review-log entry.
    - References:
      - Plan 097 visual review methodology (2026-08-24)
  - Test Cases to Write:
    - Manual review record per state with pass/fail; keyboard focus reaches editor post-ready.
  - Completion Evidence:
    - Added development-only Vite fixture routes in `frontend/src/routes/fixture.tsx` for `document-loading`, `document-budget-error`, and `document-binary-error`; `/fixture/editor` remains the ready comparison. The loading fixture uses a real `DocumentSession` head with pending chunks, while refusal fixtures use the real empty-pane recovery surface and token-backed status bar.
    - Captured wide and narrow fixture screenshots and semantic dumps under `code-reviews/screenshots/2026-08-25-plan098-ui-review/`. Loading evidence shows first-paint head text, read-only editor state, disabled Save, loading status, and bounded CodeMirror content. Budget and binary evidence shows a polite status region, empty-tab recovery actions, and an alert with the server-style diagnostic. Ready editor evidence shows Save enabled after load.
    - Visual result: PASS for all four fixture states at 1440x900 and 780x900; no clipping, overlap, or horizontal overflow observed. The long budget diagnostic wraps naturally at narrow width.
    - Accessibility result: PASS for fixture DOM semantics: named `status` live regions, editor region, CodeMirror textbox, `alert`, empty-tab group, and named recovery buttons. Real Tauri/WebKitGTK AT-SPI for these dev-only routes is `UNRESOLVED`; the route is not present in the production Tauri asset and the available browser session had no Chromium CDP endpoint, so browser evidence is not mislabeled as native accessibility evidence. A real Linux `scripts/large-document-smoke.sh` run used a private workspace socket and its targeted app-only screenshot was inspected; `real-app-atspi.txt` records that only the native frame/title controls were exposed.
    - Finding fixed during review: `ClayEditor` now includes `!meta.loading` in its editability gate, disables Save, and reports read-only metadata until the final chunk arrives. `frontend/src/test/editor.test.tsx` covers both pre-ready and ready button states.
    - Security: screenshots and dumps contain fixture-only text and names; no ambient workspace data, credentials, or absolute host paths were retained. Review method and unresolved native-a11y boundary are recorded in `code-reviews/screenshots/2026-08-25-plan098-ui-review/review-log.md`; route documentation is synchronized in the wiki and launch guide.
    - Verification passed: `npm --prefix frontend run typecheck`, `npm --prefix frontend run lint`, `npm --prefix frontend test` (111 tests), `npm --prefix frontend run build`, `npm --prefix frontend run check:budget` (161.6 kB shell / 344.6 kB total gzip), `git diff --check`, and the Impeccable detector over changed UI targets.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Review covers existing public `documents.serverOpenDocument` and `documents.serverReloadDocument` full-text JSON contracts, plus any new chunk helpers. The implementation either gives those APIs an explicitly bounded/chunk-aware contract or retains a separate trusted-runtime payload guard; no API silently emits an unbounded string after the editor ceiling is removed. Any new server functions are `pub(crate)`/private unless intentionally exposed.
    - Code Quality: `tests/clay_js_api_inventory.rs` guards pass; Markdown docs and generated registry are updated if the existing API contract changes.
    - Security: No new deno_core authority; facade allowlists unchanged; the existing 128 MiB trusted-runtime heap ceiling remains effective.
    - Performance: N/A (verification task).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (Clay JS API task)
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `src/server/ops/documents.rs` (trusted-runtime open/reload ops return full `document.text()` as JSON)
    - Options Considered:
      - Expose document size/budget info to packages: rejected — no current package need; server-owned budget stays internal.
      - Convert trusted-runtime open/reload to chunked contract: rejected — these are first-party-only facades behind the 128 MiB trusted JS heap; the protocol chunked path is the Tauri/client bridge.
    - Chosen Approach:
      - Verify-only task; `documents.serverOpenDocument` and `documents.serverReloadDocument` retain their full-text JSON contract through the trusted-runtime facade. No chunk helpers exposed to JS. The trusted-runtime payload guard (128 MiB heap + first-party-only facade allowlist) remains the effective bound.
      - Updated Markdown docs to explicitly document the full-text JSON contract and distinguish it from the protocol chunked path.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js_api
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/documents/server-open-document.md`: added chunking note clarifying the full-text trusted-runtime contract vs protocol chunked path.
      - `docs/reference/clay-js-api/documents/server-reload-document.md`: same chunking note.
    - References:
      - Plan 097 Clay JS API verification evidence (2026-08-24)
  - Test Cases to Write:
    - Existing inventory/registry guards pass unchanged.
  - Completion Evidence:
    - Reviewed `src/server/ops/documents.rs` — `op_clay_documents_open_document` and `op_clay_documents_reload_document` return `"text": document.text()` (full rope text as JSON). These are trusted-runtime-only (first-party facade allowlist, 128 MiB heap ceiling), not protocol chunked path. No new chunk helpers exposed to JS. No new deno_core authority; facade allowlists unchanged.
    - Updated `docs/reference/clay-js-api/documents/server-open-document.md` and `server-reload-document.md` with explicit chunking notes distinguishing the trusted-runtime full-text JSON contract from the protocol `DocumentTextHead`/chunk transfer path.
    - Verification passed: `cargo test --test protocol clay_js_api` (13 passed), `cargo test --test protocol documentation_coverage` (10 passed), `cargo test --test protocol` (193 passed).

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review confirms the memory budget and chunk size remain server-owned security budgets, deliberately not init.js-configurable; `docs/reference/clay-js-api/configuration.md` and `examples/init.js` make no false claims about file size limits.
    - Code Quality: Any documentation mentioning the old 768 KiB ceiling is updated to describe the budget model.
    - Security: Configuration surface unchanged; no new authority.
    - Performance: N/A.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (configuration task)
      - `docs/reference/clay-js-api/configuration.md`
      - `src/perf/budgets.rs` (security-budget precedent comments)
    - Options Considered:
      - Make budget configurable via init.js: rejected — consistent with JS heap limit being non-configurable; a user-configurable budget is a separate decision if requested later.
    - Chosen Approach:
      - Verify-only; update prose wherever the ceiling is documented (performance.md, budgets.rs comments, wiki).
    - API Notes and Examples:
      ```bash
      rg -n '768|MAX_OPENABLE_FILE_BYTES|file size' docs/ examples/ src/ --include-zero || true
      ```
    - Files to Create/Edit:
      - `docs/development/performance.md`: budget-model description.
      - `docs/wiki/modules/server-file-workspace.md`: updated in wiki task (cross-reference here).
    - References:
      - Decision log from task 1.
  - Test Cases to Write:
    - Guard test: no doc claims a per-file open ceiling exists (grep-based documentation test if pattern exists).
  - Completion Evidence:
    - Reviewed `docs/reference/clay-js-api/configuration.md` — the only 768 KiB mentions are about the independent `NATIVE_GRAMMAR_MAX_WINDOW_BYTES` parse context ceiling, `RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES` review threshold, and the diff-upgrade p95 trigger table. None are about file-open caps. No false claims about configurable file size limits.
    - Reviewed `examples/init.js` — no mention of file size limits, budgets, or the old ceiling.
    - Confirmed budget/chunk size are server-owned security budgets in `src/perf/budgets.rs`, not init.js-configurable. Configuration surface is unchanged; no new authority.
    - Verification passed: `cargo test --test protocol` configuration-related tests (8 passed including `configuration_surface_is_closed_and_security_controls_are_not_properties`, `configuration_api_no_authority_grant`, `configuration_entrypoint_is_documented_and_indexed`, etc.).

- [x] Update the canonical example configuration (examples/init.js)
  - Acceptance Criteria:
    - Functional: Verified: no new configuration surface exists, so `examples/init.js` needs no new section; file remains `node --check` clean; package-smoke gate passes.
    - Code Quality: No stale references to file-size limits in comments.
    - Security: No change to active (copy-safe) configuration.
    - Performance: N/A.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (example configuration task)
      - `examples/init.js`
    - Options Considered:
      - Add a commented "document budget" section: rejected — budget is not user configuration; documenting it as such would mislead.
    - Chosen Approach:
      - Verify-only pass.
    - API Notes and Examples:
      ```bash
      node --check examples/init.js && scripts/package-smoke.sh
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `scripts/package-smoke.sh`
  - Test Cases to Write:
    - `node --check` + package-smoke pass.
  - Completion Evidence:
    - Reviewed `examples/init.js` — no stale references to file-size limits, budgets, or the old 768 KiB ceiling. No new configuration surface emerged.
    - All three example files (`init.js`, `packages/first-party.js`, `packages/third-party.js`) pass `node --check`.
    - `scripts/package-smoke.sh` passes (8/8 tests).
    - Guard tests pass: `cargo test --test protocol "canonical_example"` — 5 passed including `canonical_example_active_configuration_is_copy_safe`, `canonical_example_cross_checks_remaining_configuration_options_against_inventory`.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Affected modules executed on a real Linux build: module 01 (launch/connection — protocol v27 handshake), module 03 (file open/save/reload — large file, budget refusal, binary refusal), module 11 (performance — open latency, frame bounds); new numbered steps added for chunked loading states; records updated.
    - Performance: Fresh measurements recorded for large-file open/save on the dev host.
    - Code Quality: `test-plan/index.md` coverage matrix updated; stale references fixed if encountered.
    - Security: No user content in recorded evidence.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`, `test-plan/01-launch-and-connection.md`, `test-plan/03-files-and-workspace.md`, `test-plan/11-performance.md`
    - Options Considered:
      - Fold into end-to-end task: rejected — project convention requires the dedicated test-plan task.
    - Chosen Approach:
      - Execute, extend, and record per convention. The plan's stale `module 02`/`test-plan/02-file-workflow.md` reference was corrected to the actual module 03 file/workspace module.
    - API Notes and Examples:
      ```text
      test-plan/03-files-and-workspace.md F48–F52: open 50 MiB file → loading indicator → ready → edit → save → reload equality; budget/binary refusals
      ```
    - Files to Create/Edit:
      - `test-plan/01-launch-and-connection.md`: L23/L24 protocol-v27 and mixed-version steps/record.
      - `test-plan/03-files-and-workspace.md`: F48–F52 chunked open/save/reload/refusal steps/record.
      - `test-plan/11-performance.md`: Q34–Q37 timing/frame-budget steps/record.
      - `test-plan/index.md`: Plan 098 coverage row and execution record.
      - `scripts/large-document-smoke.sh`: manual fixture uses `large.md` so the default Markdown file filter exposes it; cleanup terminates the desktop child.
      - `tests/large_document.rs`: prints save acknowledgement timing alongside existing open timings.
    - References:
      - Plan 097 manual-test-plan execution record (2026-08-24).
      - `code-reviews/screenshots/2026-08-26-plan098-manual/manual-test-plan.md`.
  - Test Cases to Write:
    - New numbered steps for: large-file open, budget refusal, binary refusal, protocol v27 mixed-version behavior.
  - Completion Evidence:
    - Added L23/L24, F48–F52, and Q34–Q37 without deleting or weakening existing steps; added the Plan 098 row to `test-plan/index.md`.
    - Real Linux smoke path built the current Tauri desktop, started a workspace-private server socket, opened the native chooser, and selected a synthetic 52.4 MiB Markdown fixture. Live loaded-editor/error interaction remains explicitly `UNRESOLVED` because the compositor moved the target off-screen and AT-SPI exposed only the native frame; no GUI pass was inferred.
    - Real server/protocol execution passed `cargo test --test runtime large_document:: -- --nocapture` (2 tests). Fresh final debug measurements: open→head 297.339689 ms, open→full 589.273483 ms, save→ack 423.454128 ms for 52,428,815 bytes. Three isolated reruns measured 287.225620–301.800012 ms; one host-scheduled combined run reached 538.188757 ms and tripped the existing 500 ms guard before the next combined run passed. The same run asserted 256 KiB chunks, UTF-8 equality, edit/save/reload equality, 257 MiB resident-budget refusal, and binary refusal; the timing outlier is recorded as host variance, not hidden.
    - `cargo test --lib protocol_v26_client_is_rejected_by_v27_server` passed; no separate v26 server executable was available for live mixed-version testing.
    - Sanitized evidence is under `code-reviews/screenshots/2026-08-26-plan098-manual/`; no user content, host paths, or secrets retained. `node --check`/package smoke was unaffected; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets --quiet`, `bash -n scripts/large-document-smoke.sh`, `cargo test --test protocol documentation_coverage`, and `git diff --check` pass.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Primitive reference and wiki are updated after implementation: `DocumentChunkTransfer` records every initial/open/reload/resync/persisted-restore consumer; server-file-workspace explains the budget and runtime-refresh model; react-codemirror-editor explains progressive assembly; master indexes link current pages.
    - Performance: Wiki documents the bounded-memory invariants (no full-String open/save, chunk clamp, in-flight window) without adding runtime work.
    - Code Quality: Pages explain responsibilities, invariants, tradeoffs, source/test paths; every page linked from `docs/wiki/index.md`.
    - Security: Pages document the budget as server-owned security budget and binary-sniff boundary without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
    - Options Considered:
      - Update after each task: noisier; chosen single update after tests pass per template.
    - Chosen Approach:
      - One wiki pass after verification, per template.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/flows/document-chunked-loading.md (new)
      docs/wiki/modules/server-file-workspace.md
      docs/wiki/modules/react-codemirror-editor.md
      docs/reference/primitives/registry.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/flows/document-chunked-loading.md`, `docs/wiki/modules/server-file-workspace.md`, `docs/wiki/modules/react-codemirror-editor.md`, `docs/wiki/modules/desktop-typed-bridge.md`.
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`: add the internal `DocumentChunkTransfer` category, ownership, access/version/range validation, budget, and hot-path policy.
      - `tests/primitives_docs.rs`: deterministic reference/wiki/index coverage for the new protocol primitive.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Wiki navigation/source-path guard test passes (existing `tests/documentation_coverage.rs` wiki check).
    - `tests/primitives_docs.rs` fails if `DocumentChunkTransfer`, its budget, implementation flow, or wiki/index links drift.
  - Completion Evidence:
    - Created `docs/wiki/flows/document-chunked-loading.md` tracing the full head→chunk→assembly flow: every consumer (open, reload, resync, bootstrap, selected-file open), server budget/binary guards, one-outstanding-request window, versioned requests, typed rejections, and assembly outside hot paths.
    - Updated `docs/wiki/index.md` with the new flow page entry linking the Plan 098 protocol description.
    - Verified existing wiki pages already document chunked loading adequately:
      - `server-file-workspace.md` — budget/binary/head/chunk details already present.
      - `react-codemirror-editor.md` — step 1 already describes progressive chunk assembly.
      - `desktop-typed-bridge.md` — request path already mentions DocumentChunkRequest/Chunk/Rejected.
      - `docs/reference/primitives/index.md` — Plan 098 section already present.
      - `docs/reference/primitives/registry.md` — DocumentChunkTransfer row already present.
    - Updated `tests/primitives_docs.rs` `document_chunk_transfer_primitive_is_bounded_and_documented` guard to assert the new flow page content and in-flight window invariant.
    - All guard tests pass: `cargo test --test protocol primitives_docs::document_chunk_transfer_primitive_is_bounded_and_documented` (1/1), `cargo test --test protocol primitives_docs::` (30/30), `cargo test --test protocol documentation_coverage::` (10/10).
