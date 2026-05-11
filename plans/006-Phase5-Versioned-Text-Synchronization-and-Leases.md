# Phase 5 Versioned Text Synchronization and Leases

## Objectives
- Implement Clay's server-authoritative canonical text model with optimistic client shadows.
- Enforce document versions, transaction IDs, behavior versions, editable leases, read-only observer access, and stale-edit handling across the Phase 4 IPC boundary.
- Replace the Phase 4 server `String` placeholder with canonical `crop::Rope` document state while preserving byte-offset edit semantics.
- Add basic region-lock data structures and enforcement for future AI/extension-driven mutations without introducing AI execution yet.
- Preserve immediate local typing for server-issued manifest-declared client-first behavior while making server acknowledgements authoritative.
- Keep synchronization semantics isolated from `rkyv` codec details and Masonry rendering/input hot paths.

## Expected Outcome
- The server owns a canonical per-document `crop::Rope`, document version, ordered transaction log or recent transaction metadata, editable lease, connected client access state, and region locks.
- The client owns a local shadow buffer, pending edit queue, confirmed server version, optimistic local version state, and resync/correction handling.
- Every client edit carries document ID, client ID or lease ID where required, base document version, behavior version, transaction ID, and delta edit operation.
- Server acknowledgements advance confirmed versions and transaction IDs; stale/conflicting edits are rejected with actionable protocol messages.
- A stale client can request or receive a bounded resync snapshot/transaction response and return to a consistent shadow state.
- Only one client can hold an editable lease for a document; additional clients receive read-only observer access and cannot mutate the document.
- Region locks can be created in server state and block overlapping user edits with deterministic conflict errors.
- Ordinary manifest-declared text edits remain locally immediate and use asynchronous bounded IPC; no keypress waits for server, JavaScript, AI, file IO, or full-document serialization.
- `cargo fmt`, `cargo test`, and `cargo check` pass, including synchronization, stale-edit, lease, read-only observer, region-lock, and non-blocking client tests.
- No arbitrary JavaScript execution in the client, file/workspace authority, remote network listener, AI mutation, SDUI expansion, or package system is introduced in this phase.

## Tasks

- [x] Extend the protocol with version, lease, rejection, resync, and region-lock messages
  - Acceptance Criteria:
    - Functional: Protocol types represent editable lease IDs, client access state, edit base versions, behavior versions, server-confirmed versions, stale-edit rejection, resync snapshot/transaction responses, read-only denial, and region-lock conflict errors.
    - Performance: Ordinary edits remain delta messages; full text snapshots are used only for initial load or explicit resync. `rkyv` remains behind the existing bounded codec boundary with validation before access.
    - Code Quality: Message semantics live in `src/protocol/mod.rs` while serialization/framing stays in `src/protocol/codec.rs`; protocol enums distinguish recoverable synchronization errors from malformed-message errors.
    - Security: All received metadata is validated as fallible input; lease IDs, document IDs, versions, and ranges are checked server-side before mutation authority is granted.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/rkyv/rkyv`: derive `Archive`, `Serialize`, and `Deserialize`; use validated access/deserialization for untrusted bytes with bytecheck-enabled validation.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: keep semantics separate from codec, include document/client/access/lease/version/behavior metadata, validate frames, and bound frame sizes.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: server owns canonical documents, versions, transactions, leases, and locks; client owns shadows and pending edit queues.
      - `roadmap.md` Phase 5: add enforced document version numbers, base versions, acknowledgements, stale rejection, resync, leases, read-only observers, and region-lock enforcement.
    - Options Considered:
      - Reuse Phase 4 `Error` messages for all sync failures: simple, but too coarse for client resync and conflict UI.
      - Add explicit synchronization response variants: clearer state machine and better tests, with slightly more protocol surface.
      - Introduce operational-transform/CRDT messages now: too broad for Phase 5's single-writer lease model.
    - Chosen Approach:
      - Add explicit protocol variants for lease metadata and synchronization outcomes while keeping edit operations byte-range based. Use error codes such as stale version, lease required/expired, read-only document, and region locked so the client can distinguish resync from fatal protocol failure.
    - API Notes and Examples:
      ```rust
      pub type LeaseId = u64;

      pub enum ClientMessage {
          Edit {
              document_id: DocumentId,
              client_id: ClientId,
              lease_id: Option<LeaseId>,
              base_version: DocumentVersion,
              behavior_version: BehaviorVersion,
              transaction_id: TransactionId,
              operation: EditOperation,
          },
          RequestResync { document_id: DocumentId, known_version: DocumentVersion },
      }

      pub enum ServerMessage {
          InitialDocument { document_id: DocumentId, version: DocumentVersion, text: String, access: DocumentAccess, lease_id: Option<LeaseId> },
          EditAck { document_id: DocumentId, confirmed_version: DocumentVersion, transaction_id: TransactionId },
          EditRejected { document_id: DocumentId, transaction_id: TransactionId, reason: EditRejection },
          ResyncSnapshot { document_id: DocumentId, version: DocumentVersion, text: String, access: DocumentAccess, lease_id: Option<LeaseId> },
      }
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: Add lease IDs, sync rejection reasons, resync messages, lock conflict metadata, and updated edit metadata.
      - `src/protocol/codec.rs`: Preserve bounded validated framing for the changed messages.
      - `src/client/mod.rs`: Update client message construction and server event handling for new sync variants.
      - `src/server/connection.rs`: Update dispatch for lease/version-aware edit messages and resync requests.
      - `src/editor/surface.rs`: Update editable-access checks for lease-carrying `DocumentAccess::Editable`.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
  - Test Cases to Write:
    - `protocol_round_trips_edit_with_lease_and_versions`: Encodes/decodes an edit carrying client, lease, base version, behavior version, and transaction ID.
    - `protocol_round_trips_stale_edit_rejection`: Encodes/decodes a stale-version rejection with the server's current version.
    - `protocol_round_trips_resync_snapshot`: Encodes/decodes a resync snapshot containing Unicode text and lease/access metadata.
    - `codec_rejects_invalid_phase5_archive_bytes`: Invalid bytes remain rejected after protocol changes.
    - `codec_rejects_oversized_phase5_frame`: Oversized frames still fail before payload allocation.
    - `protocol_round_trips_region_lock_rejection`: Encodes/decodes region-lock conflict metadata for future lock UI/AI explanation.

- [x] Replace server document text with versioned canonical `crop::Rope` state
  - Acceptance Criteria:
    - Functional: Server document state stores canonical text in `crop::Rope`, applies insert/delete/replace deltas by UTF-8 byte offsets, increments versions exactly once per accepted mutation, and can produce initial/resync snapshots.
    - Performance: Insert/delete/replace use `crop` rope mutation APIs rather than rebuilding a full `String`; snapshots clone/extract text only for initial load or resync. Per-edit validation does not scan the whole document unnecessarily.
    - Code Quality: Document state owns version advancement, UTF-8 boundary checks, transaction metadata, and snapshot creation in one focused module; panicking `crop` operations are preceded by explicit validation.
    - Security: Invalid document IDs, non-boundary offsets, out-of-range spans, reversed ranges, stale versions, missing leases, and read-only mutations return protocol errors without panics.
  - Approach:
    - Documentation Reviewed:
      - crop 0.4.3 docs: `Rope` is a UTF-8 B-tree rope for frequent edits; `insert`, `delete`, and `replace` take byte offsets/ranges and panic if offsets are out of bounds or not code point boundaries.
      - crop docs/code examples: `Rope` uses byte offsets like Rust `String`; `byte_slice`, `byte_of_line`, and `replace` support range-based editor workflows.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: server owns canonical document ropes/state and document versions.
      - `concept.md`: canonical state is server-held `crop` rope; client holds a lightweight shadow copy for optimistic typing.
    - Options Considered:
      - Keep `String` until file/workspace phase: easy, but Phase 5 specifically requires canonical `crop` ropes and versioned text sync.
      - Use `crop::Rope` only in client/editor buffer: violates server-authoritative canonical state.
      - Replace server state with `crop::Rope` now and keep snapshots as `String` at protocol edges: chosen because it proves canonical rope mutation while keeping Phase 5 protocol simple.
    - Chosen Approach:
      - Convert `DocumentState` to hold a `crop::Rope`, add safe validation helpers before calling `insert`, `delete`, or `replace`, and centralize accepted-version advancement with transaction metadata.
    - API Notes and Examples:
      ```rust
      let mut rope = crop::Rope::from("Hello 🌎");
      rope.insert(6, "Clay ");
      rope.replace(0..5, "Hi");
      rope.delete(2..3);
      ```
    - Files to Create/Edit:
      - `src/server/document.rs`: Replace `String` storage with `crop::Rope`, add versioned apply functions, validation, and snapshot extraction.
      - `src/server/connection.rs`: Use version-aware document APIs and return explicit sync responses.
      - `Cargo.toml`: No new dependency expected; `crop = "0.4.3"` already exists.
      - `docs/wiki/modules/server-document-state.md`: Updated now to reflect the canonical rope implementation; final Phase 5 wiki task will verify the complete synchronization set.
    - References:
      - crop docs.rs `Rope::insert`, `Rope::delete`, `Rope::replace`, `Rope::byte_slice`.
      - `concept.md` section 4.
  - Test Cases to Write:
    - `server_document_uses_rope_for_insert_delete_replace`: Accepted mutations update canonical text and version.
    - `server_document_rejects_non_boundary_rope_edit_without_panic`: Multi-byte Unicode boundary violations return errors.
    - `server_document_rejects_out_of_range_rope_edit`: Range validation prevents panics.
    - `server_document_snapshot_preserves_unicode`: Snapshot text from the rope matches expected Unicode content.
    - `server_document_version_advances_once_per_accepted_edit`: Rejected edits do not increment version.
  - Verification Completed:
    - `cargo fmt`
    - `cargo test --quiet`
    - `cargo check --quiet`

- [x] Enforce base document versions and confirmed-version tracking
  - Acceptance Criteria:
    - Functional: The server accepts an edit only when the edit's base version matches the current canonical document version and rejects stale/future versions with explicit synchronization outcomes. The client updates confirmed document version from `EditAck` and keeps pending transactions until confirmed or rejected.
    - Performance: Version checks are constant-time metadata checks; they do not require full-document comparison or synchronous UI waits.
    - Code Quality: Client state separates server-confirmed version, optimistic local edit state, and pending transaction queue; server state separates canonical version from client-provided base versions.
    - Security: Clients cannot advance server versions by sending forged or future base versions; malformed version values produce recoverable protocol errors.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: edit messages should carry base document version, server version, client transaction ID, and behavior version.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: server owns document versions and transaction ordering.
      - `plans/005-Phase4-IPC-Client-Server-Skeleton.md`: Phase 4 carried versions but intentionally did not enforce them.
    - Options Considered:
      - Server accepts stale edits and transforms them: more collaborative, but not needed for one editable lease in Phase 5.
      - Server rejects any base-version mismatch and asks client to resync: simpler, deterministic, and aligned with one-writer leases.
      - Client blocks local typing until ack: rejected because hot-path typing must remain immediate.
    - Chosen Approach:
      - Add strict server base-version equality checks and client pending-edit bookkeeping. Local edits remain optimistic; acknowledgements advance confirmed versions, while rejection triggers resync/correction handling in later tasks.
    - API Notes and Examples:
      ```rust
      if edit.base_version != document.version() {
          return ServerMessage::EditRejected {
              document_id,
              transaction_id,
              reason: EditRejection::StaleVersion { client_base_version: edit.base_version, server_version: document.version() },
          };
      }
      ```
    - Files to Create/Edit:
      - `src/server/document.rs`: Add base-version validation and confirmed-version response data.
      - `src/server/connection.rs`: Pass base version and transaction metadata to document state.
      - `src/client/mod.rs`: Track pending transactions, confirmed version, optimistic version, and ack/rejection events.
      - `src/editor/surface.rs`: Existing edit events still carry document metadata; outgoing base-version policy is selected by `ClientEditQueue` to avoid blocking editor input.
      - `src/masonry_editor.rs`: Existing non-blocking event forwarding remains unchanged while `ClientEditQueue` handles updated sync metadata.
      - `docs/wiki/modules/server-document-state.md`: Document strict base-version enforcement.
      - `docs/wiki/flows/client-edit-emission.md`: Document optimistic base-version assignment and pending reservations.
      - `docs/wiki/flows/client-server-edit-ack.md`: Document confirmed-version advancement and rejection event handling.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `roadmap.md` Phase 5.
  - Test Cases to Write:
    - `server_accepts_edit_at_current_base_version`: Matching base version mutates and acknowledges.
    - `server_rejects_stale_base_version`: Lower base version returns stale rejection and current server version.
    - `server_rejects_future_base_version`: Future base version returns explicit invalid/stale sync outcome without mutation.
    - `client_ack_advances_confirmed_version`: Client confirmed state updates after ack.
    - `client_keeps_pending_edit_until_ack_or_rejection`: Pending transaction bookkeeping is deterministic.
  - Verification Completed:
    - `cargo fmt`
    - `cargo test --quiet`
    - `cargo check --quiet`

- [x] Add stale-edit rejection handling and simple client resync
  - Acceptance Criteria:
    - Functional: When the server rejects an edit due to stale version, lease loss, read-only access, or region lock, the client surfaces a connection/sync event, requests or receives a resync snapshot when appropriate, replaces its shadow text safely, clears or reconciles pending edits according to the chosen Phase 5 policy, and resumes from the server-confirmed version.
    - Performance: Resync uses full text only on explicit rejection/recovery paths; normal edit acknowledgements remain delta-only.
    - Code Quality: Resync logic is isolated from Masonry paint/input handlers and from protocol codec internals; editor snapshot replacement continues to reset caret/selection/viewport only when a real resync snapshot is applied.
    - Security: Rejected server messages and resync snapshots are treated as fallible input; resync does not grant new authority beyond the access/lease metadata returned by the server.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: snapshots are acceptable for initial load or resync, ordinary edits must use deltas.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: client owns shadow state, pending edit queue, and transient UI; server owns canonical state and corrections.
      - `roadmap.md` Phase 5: add stale-edit rejection and simple resync behavior.
    - Options Considered:
      - Attempt to replay pending edits automatically after every resync: user-friendly but risks subtle offset conflicts before transaction logs/OT are mature.
      - Clear pending edits on resync and replace the shadow with canonical text: deterministic and safer for Phase 5.
      - Apply correction transactions instead of snapshots: efficient, but more complex than needed for first resync.
    - Chosen Approach:
      - Implement a simple deterministic resync policy: stale/conflict rejection causes the client to request or accept `ResyncSnapshot`, replace local shadow state with the server snapshot, clear acknowledged/rejected pending edits, and expose a sync event. Defer automatic pending-edit replay unless it is trivial and covered by tests.
    - API Notes and Examples:
      ```rust
      match event {
          ClientConnectionEvent::EditRejected { reason: EditRejection::StaleVersion { .. }, .. } => {
              edit_queue.request_resync(document_id, known_version)?;
          }
          ClientConnectionEvent::ResyncSnapshot(snapshot) => editor.load_snapshot(snapshot.document_id, snapshot.version, snapshot.text, snapshot.access),
      }
      ```
    - Files to Create/Edit:
      - `src/client/mod.rs`: Add rejection/resync events, resync request sending, pending queue cleanup, and snapshot handling data structures.
      - `src/editor/surface.rs`: Reuse or extend snapshot load APIs to apply server resync safely.
      - `src/masonry_editor.rs`: Apply resync events to the editor through a narrow UI-safe boundary if available, or document a temporary polling/logging compromise.
      - `src/server/connection.rs`: Handle `RequestResync` and send canonical snapshots.
    - References:
      - `plans/005-Phase4-IPC-Client-Server-Skeleton.md` Phase 4 acknowledged that resync was deferred.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `server_sends_resync_snapshot_after_request`: A stale client can request the current canonical text/version.
    - `client_requests_resync_after_stale_rejection`: Stale rejection enqueues a resync request without blocking local UI code.
    - `client_applies_resync_snapshot_and_clears_pending_edits`: Shadow state returns to server-confirmed text/version.
    - `resync_snapshot_replaces_unicode_text_safely`: Unicode and byte boundary state remain valid after resync.
    - `ordinary_ack_flow_does_not_send_full_snapshot`: Non-stale edit acknowledgement remains delta/metadata only.
  - Verification Completed:
    - `cargo fmt`
    - `cargo test --quiet`
    - `cargo check --quiet`

- [x] Implement editable document leases and read-only observers
  - Acceptance Criteria:
    - Functional: The first eligible client opening the document receives editable access and a lease ID; additional clients receive read-only observer access. Only the current lease holder can mutate; read-only clients cannot emit or successfully apply edits.
    - Performance: Lease checks are per-document metadata checks and do not serialize unrelated future documents globally.
    - Code Quality: Lease state is owned by server document/session state and is explicit in protocol snapshots; client editor state stores access and lease metadata separately from rendering/layout state.
    - Security: A client cannot gain edit authority by omitting, guessing, or replaying lease IDs; disconnect/release behavior is deterministic and does not grant filesystem, extension, shell, AI, or workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: one editable lease per document; other clients are read-only observers; lease transfer/release is explicit.
      - `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`: server owns leases/locks and clients execute only server-issued behavior manifests.
      - `roadmap.md` Phase 5: add one-editable-client document leases and read-only observer clients.
    - Options Considered:
      - Allow all local clients to edit and rely on stale-version rejection: insufficient because duplicate clients must not edit simultaneously.
      - Assign the first connected client the editable lease and all others read-only: simple and matches current one-document server placeholder.
      - Implement full lease transfer UI now: useful later, but beyond Phase 5 skeleton needs.
    - Chosen Approach:
      - Add per-document lease state with generated lease IDs. On connection, grant editable access only when no active lease exists; otherwise send read-only access. Defer explicit user-facing lease transfer UX but include protocol fields that can support it later.
    - API Notes and Examples:
      ```rust
      pub enum DocumentAccess {
          ReadOnly,
          Editable { lease_id: LeaseId },
      }

      if edit.lease_id != document.current_lease_for(client_id) {
          return EditRejection::LeaseRequired;
      }
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: Represent lease metadata in access/snapshot/edit messages.
      - `src/server/document.rs`: Track active editable lease, observer access, disconnect/release policy, and lease validation.
      - `src/server/connection.rs`: Assign access during handshake and validate edits against connection client ID plus lease ID.
      - `src/client/mod.rs`: Store lease/access in initial state and edit messages.
      - `src/editor/surface.rs`: Prevent local edit events when access is read-only while preserving navigation/selection.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `roadmap.md` Phase 5.
  - Test Cases to Write:
    - `first_client_receives_editable_lease`: First connection snapshot includes editable access and lease ID.
    - `second_client_receives_read_only_access`: Duplicate client snapshot is read-only.
    - `server_rejects_edit_without_current_lease`: Missing or wrong lease ID cannot mutate canonical text.
    - `read_only_editor_allows_navigation_but_not_mutation`: Client UI does not create edit events in read-only access.
    - `lease_released_or_retained_on_disconnect_matches_policy`: Disconnect behavior is deterministic and tested.
  - Verification Completed:
    - `cargo fmt`
    - `cargo test --quiet`
    - `cargo check --quiet`
  - Implementation Notes:
    - `DocumentState` now owns per-document lease metadata and grants the first connected client editable access via `acquire_access`; duplicate clients receive read-only snapshots.
    - Server edit validation now requires the message client ID and lease ID to match the active lease, rejecting missing or replayed leases without mutating the canonical rope.
    - Connection tasks release the editable lease when the lease-holding client disconnects; existing observers remain read-only until reconnect or future transfer UX.
    - `EditorSurface` blocks read-only text mutation while preserving navigation/selection, and `ClientEditQueue` refuses to emit edits without a lease.
    - Wiki pages updated: `docs/wiki/modules/server-document-state.md`, `docs/wiki/flows/client-edit-emission.md`, and `docs/wiki/flows/client-server-edit-ack.md`.

- [x] Add region-lock data structures and basic enforcement
  - Acceptance Criteria:
    - Functional: Server document state can register region locks with byte ranges, owner/type metadata, and version context; accepted user edits are rejected when their affected range overlaps an active lock.
    - Performance: Lock checks are bounded by the number of active locks and avoid whole-document scanning. Normal documents with no locks pay only a small metadata check.
    - Code Quality: Region-lock logic is isolated from text mutation mechanics and uses validated UTF-8 byte ranges. Lock structs are protocol-ready but do not expose AI mutation authority yet.
    - Security: Locks cannot be bypassed by malformed ranges, empty/reversed ranges, or operation shape changes; no AI, shell, file, network, extension, or workspace permission is introduced.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: server owns region/document/behavior/workspace locks and AI/tool mutation authority.
      - `concept.md`: stale AI edits are rejected and heavy AI rewrites use region locks to prevent conflicting user input.
      - `roadmap.md` Phase 5: introduce region lock data structures and basic enforcement.
    - Options Considered:
      - Defer all locks until AI phase: simpler, but Phase 5 explicitly needs lock data structures and enforcement boundaries.
      - Implement only in-memory region locks with tests: chosen; enough to prove conflict handling without AI sessions.
      - Build full lock lifecycle/UI now: too broad before AI-safe mutation phases.
    - Chosen Approach:
      - Add in-memory region-lock structs and overlap checks in server document state. Tests can create locks directly through internal helpers; public lock-management APIs can wait until AI/extension phases unless a Phase 5 public surface is intentionally introduced and documented.
    - API Notes and Examples:
      ```rust
      pub struct RegionLock {
          pub lock_id: RegionLockId,
          pub document_id: DocumentId,
          pub start: u64,
          pub end: u64,
          pub owner: LockOwner,
          pub created_at_version: DocumentVersion,
      }

      fn overlaps_edit(lock: &RegionLock, edit_start: u64, edit_end: u64) -> bool {
          edit_start < lock.end && edit_end > lock.start
      }
      ```
    - Files to Create/Edit:
      - `src/server/document.rs`: Add region lock structs, validation, overlap checks, and test-only/internal lock registration helpers.
      - `src/protocol/mod.rs`: Add lock conflict rejection metadata if client needs to display the locked range/reason.
      - `docs/wiki/modules/server-document-state.md`: Update after implementation in the final wiki task.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `concept.md` section 4.
  - Test Cases to Write:
    - `server_rejects_insert_inside_region_lock`: Insert at a locked offset is rejected.
    - `server_rejects_delete_overlapping_region_lock`: Delete spanning a lock is rejected.
    - `server_accepts_edit_outside_region_lock`: Non-overlapping edit succeeds and increments version.
    - `region_lock_range_validation_rejects_invalid_boundaries`: Invalid lock ranges cannot be registered.
    - `region_lock_conflict_reports_range_metadata`: Rejection includes enough lock metadata for future UI/AI explanation.
  - Verification Completed:
    - `cargo fmt`
    - `cargo test --quiet`
    - `cargo check --quiet`
  - Implementation Notes:
    - `DocumentState` now stores active in-memory region locks plus a future-facing internal registration helper that validates non-empty, in-bounds UTF-8 byte ranges.
    - Server edit validation now computes the edit's affected range before mutation and rejects inserts inside locks or delete/replace spans overlapping locks with `EditRejection::RegionLocked` metadata, including empty replace ranges treated as insert-shaped edits.
    - Lock conflicts preserve canonical text, document version, and transaction metadata; non-overlapping edits continue to acknowledge normally.
    - Region locks remain server-owned internal metadata only; no AI, extension, file/workspace, shell, network, or public lock-management authority was introduced.
    - Wiki page updated: `docs/wiki/modules/server-document-state.md`.

- [x] Verify end-to-end synchronization, leases, and hot-path regressions
  - Acceptance Criteria:
    - Functional: Integration tests cover initial editable connection, duplicate read-only connection, successful acknowledged edit, stale edit rejection, resync recovery, and region-lock conflict rejection across the Unix socket IPC path.
    - Performance: Masonry input handlers still apply manifest-declared client-first edits locally and only use bounded non-blocking queue forwarding; no full-document IPC occurs for ordinary acknowledged edits.
    - Code Quality: `cargo fmt`, `cargo test`, and `cargo check` pass; tests use deterministic temporary socket cleanup and do not rely on task timing beyond bounded retries/timeouts.
    - Security: Verification confirms Phase 5 does not add arbitrary JavaScript/client execution, file/workspace authority, remote listeners, shell/network access, AI mutation, or SDUI expansion.
  - Approach:
    - Documentation Reviewed:
      - Tokio 1.49 docs: bounded `mpsc::channel` provides inter-task backpressure, `try_send` avoids blocking UI callers, and `UnixStream` can be split for concurrent async read/write when needed.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: no IPC work in Masonry paint/text-event handlers; use bounded queues and per-document ordering.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: prefer deterministic checks for workflow-maintained artifacts.
    - Options Considered:
      - Rely only on unit tests for document state: insufficient because protocol/client/server interactions are the phase goal.
      - Add end-to-end IPC tests plus targeted editor hot-path tests: chosen for regression confidence.
      - Add benchmarks now: useful, but Phase 5 correctness and non-regression tests are higher priority.
    - Chosen Approach:
      - Add focused unit tests for server document state and client sync state plus end-to-end Unix socket tests using the existing Phase 4 integration-test style. Preserve existing Phase 2/4 tests and run full Cargo verification.
    - API Notes and Examples:
      ```bash
      cargo fmt
      cargo test
      cargo check
      ```
    - Files to Create/Edit:
      - `src/server/document.rs`: Unit tests for versioning, leases, and locks.
      - `src/server/connection.rs`: Connection-level tests for handshake/access and rejection responses.
      - `src/client/mod.rs`: Client sync-state and event-loop tests.
      - `src/editor/surface.rs`: Read-only and version metadata tests.
      - `src/masonry_editor.rs`: Non-blocking forwarding regression tests if needed.
      - `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`: Mark completed tasks and record actual compromises/follow-ups during execution.
    - References:
      - `plans/005-Phase4-IPC-Client-Server-Skeleton.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `end_to_end_first_client_edits_and_receives_confirmed_version`: Accepted edit advances server/client versions.
    - `end_to_end_second_client_is_read_only`: Duplicate client cannot mutate.
    - `end_to_end_stale_edit_rejected_then_resynced`: Stale edit triggers resync recovery.
    - `end_to_end_region_locked_edit_rejected`: Lock conflict crosses IPC boundary.
    - `phase5_regression_commands`: `cargo fmt`, `cargo test`, and `cargo check` pass.
  - Verification Completed:
    - `cargo fmt`
    - `cargo test --quiet`
    - `cargo check --quiet`
  - Implementation Notes:
    - Added `real_server_end_to_end_stale_edit_rejected_then_resynced` to verify stale-version rejection and explicit `RequestResync`/`ResyncSnapshot` recovery through `IpcServer` on a real Unix socket.
    - Added `real_server_end_to_end_region_locked_edit_rejected` to verify region-lock conflict rejection metadata crosses the real Unix socket IPC path.
    - Existing real-server tests continue to cover initial editable connection, duplicate read-only observer access, and acknowledged edit/version advancement.
    - Existing `ClientEditQueue` tests continue to verify bounded `try_send` behavior and read-only enqueue refusal, preserving the Masonry/client hot-path no-blocking guarantee without adding JavaScript, file/workspace, shell/network, extension, SDUI, remote-listener, or AI authority.
    - Updated related wiki test references in `docs/wiki/flows/client-server-edit-ack.md` and `docs/wiki/modules/server-document-state.md`.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: The Phase 5 implementation is reviewed and the Clay JS APIs needed for synchronization, leases, document access state, resync status, region-lock inspection, extensibility, configuration, customization, user search/help, key binding, AI-agent discovery, and future public programmatic use are proposed or created. All server-side Rust public functions introduced or changed by Phase 5 are inventoried; each has a stable Clay JS/TS facade API backed by an explicit `deno_core` op wrapper when it is a public programmatic capability, or is made private/`pub(crate)` when it should remain internal. Every Clay JS API has Markdown docs linked from `docs/index.md`, generated registry coverage, and lookup access.
    - Performance: Clay JS API and documentation checks do not add synchronous work to Masonry input/paint paths or ordinary edit IPC; JavaScript remains server-side and outside the keypress hot path.
    - Code Quality: Rust implementation functions, op wrappers, JS/TS facade exports, Markdown docs, generated registry entries, and lookup metadata use stable names that are easy to map in tests. Each API doc includes a searchable user-facing name, default key bindings or an empty key binding list, and custom properties for behavior-changing settings.
    - Security: Raw `Deno.core.ops.op_*` calls and arbitrary Rust functions are not user-facing APIs; lease, lock, and mutation authority remains validated at the server/API boundary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay plans require a JS API task for public programmatic surfaces and Rust public functions.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public programmatic APIs are Clay JS facades backed by explicit ops; internal server helpers should be private or `pub(crate)`.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Clay JS API docs include stable IDs, user-facing names, key binding metadata, custom properties, security notes, backing Rust paths, op names, facade paths, and lookup tags.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown plus `docs/index.md` is authoritative for Clay JS APIs and generated registries.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Tests must fail for missing APIs, docs, index links, stale registries, or lookup gaps.
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`: Approved Rust-to-JS exposure boundary.
    - Options Considered:
      - Document raw synchronization protocol types as the public API: rejected because public programmatic behavior must be Clay JS facade APIs.
      - Defer every Phase 5 API until `deno_core` execution phases: risky if public Rust surfaces expand unnoticed.
      - Inventory/narrow visibility now and add planned docs/coverage only for intentional public surfaces: chosen to preserve scope while honoring the documentation contract.
    - Chosen Approach:
      - Review Phase 5 server/client/protocol changes after implementation. Make internal sync helpers `pub(crate)` where possible. For intentional public programmatic surfaces such as document access/status, resync, and lock inspection, create or propose Clay JS facade metadata/docs without putting JavaScript on the edit hot path.
    - API Notes and Examples:
      ```text
      server Rust function -> deno_core op wrapper -> Clay JS/TS facade -> docs/reference/clay-js-api/** -> docs/index.md -> generated registry -> lookup test
      ```
    - Files to Create/Edit:
      - `src/server/**`: Narrow internal helper visibility or mark public API functions for facade coverage.
      - `src/protocol/**`: Keep internal protocol helpers out of the public JS surface unless intentionally exposed.
      - `docs/reference/clay-js-api/**/*.md`: Add/update Clay JS API docs for Phase 5 public capabilities, including user-facing names, key bindings, and custom properties.
      - `docs/index.md`: Link new Clay JS API docs.
      - `generated/**` or equivalent: Update generated registry artifacts using the project command when available.
      - `tests/docs_contract.rs` or module tests: Add Phase 5 coverage mappings when the registry/check infrastructure exists.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`
  - Test Cases to Write:
    - `phase5_server_public_rust_functions_have_clay_js_api`: Fails when a Phase 5 server-side Rust public function lacks a Clay JS API or is not made non-public.
    - `phase5_clay_js_api_docs_are_indexed_and_generated`: Fails when Phase 5 Clay JS API docs are missing from `docs/index.md`, generated registry output, or lookup APIs.
    - `phase5_clay_js_api_discovery_metadata_is_complete`: Fails when user-facing name, key binding metadata, or custom property metadata is missing or malformed.
    - `phase5_generated_doc_registry_is_current`: Fails with `cargo run --bin update-doc-registry` instructions when generated registry artifacts are stale.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The Phase 5 implementation is reviewed for user-configurable synchronization, lease, read-only observer, resync, pending-edit, IPC/client/server status, key binding, customization, and extensibility needs. Any configuration option introduced by Phase 5 is represented as a Clay JS API documented in Markdown, linked from `docs/index.md`, included in generated registry output, and lookup-accessible. The plan preserves `~/.config/clay/init.js` as the configuration entry point, with modular file loading supported when runtime configuration loading is implemented.
    - Performance: Configuration APIs and docs do not add synchronous JavaScript, IPC, or server work to Masonry input/paint paths; ordinary typing remains client-first where the behavior manifest permits it.
    - Code Quality: Configuration uses documented Clay JS APIs and `custom_properties` metadata instead of ad hoc undocumented keys.
    - Security: Configuration does not implicitly grant filesystem, network, shell, extension loading, AI mutation, remote listener, workspace authority, lease stealing, or lock bypass authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay plans require a separate configuration task when adding user-visible behavior, server APIs, protocol capabilities, or public programmatic surfaces.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration is loaded from `~/.config/clay/init.js` and each option is a Clay JS API.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Configuration APIs need user-facing names, key bindings, custom properties, security notes, and lookup tags.
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`: Approved configuration model.
    - Options Considered:
      - Add ad hoc config flags for resync or leases: rejected because every configuration option must be a Clay JS API.
      - Defer configuration review entirely: rejected because synchronization and lease behavior is user-visible.
      - Review and document planned configuration APIs without runtime execution: chosen if runtime `init.js` loading remains out of scope.
    - Chosen Approach:
      - Review Phase 5 synchronization/lease surfaces after implementation. Document any intentional configuration capabilities as Clay JS APIs with `custom_properties`; if no runtime configuration is implemented, record planned docs/metadata and tests so future runtime work follows the contract.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      import { configureSync } from "clay:client";

      configureSync({ showPendingEditStatus: true });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**/*.md`: Add/update configuration API docs for Phase 5 surfaces.
      - `docs/index.md`: Link configuration API docs.
      - `generated/**` or equivalent: Update generated registry artifacts using the project command when available.
      - `tests/docs_contract.rs` or module tests: Add configuration metadata coverage when the registry/check infrastructure exists.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - `phase5_configuration_apis_are_documented`: Fails when Phase 5 configuration APIs lack Markdown docs, index links, generated registry entries, or lookup access.
    - `phase5_configuration_custom_properties_are_complete`: Fails when behavior-changing synchronization/lease settings are absent from `custom_properties`.
    - `phase5_configuration_does_not_enter_input_hot_path`: Verifies configuration handling does not add synchronous JavaScript/IPC/server work to editor input handling.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all Phase 5 implementation tasks are complete, documenting or verifying protocol synchronization messages, canonical rope document state, version enforcement, client pending/confirmed state, resync flow, leases/read-only observers, and region-lock enforcement.
    - Performance: Wiki updates add no runtime work and document hot-path guarantees: no blocking IPC in input/paint handlers, bounded queues, delta edits for ordinary typing, snapshots only for initial load/resync, and no synchronous server/JavaScript round trip.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from `docs/wiki/index.md`; public programmatic usage links to authoritative `docs/reference/` pages instead of duplicating them.
    - Security: Wiki pages document touched security boundaries: server authority, leases, read-only access, lock enforcement, IPC validation, and absent authorities such as extension execution, SDUI, file workspace authority, remote listeners, shell/network access, and AI mutation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow, public-reference linking boundary, and quality bar.
      - `.agents/skills/create-plan/references/wiki-task.md`: Final wiki task belongs after implementation, verification, and project-specific API/documentation maintenance tasks.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic checks for wiki/docs maintenance where practical.
    - Options Considered:
      - Update wiki after each implementation task: accurate but noisy while synchronization design changes.
      - Update once after tests pass: chosen to keep implementation education aligned with final code.
    - Chosen Approach:
      - After Phase 5 implementation and verification pass, update existing Phase 4 wiki pages and add focused Phase 5 pages for synchronization, leases, and region locks. Verify every wiki page is linked from `docs/wiki/index.md`.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/protocol-codec.md
      docs/wiki/modules/server-document-state.md
      docs/wiki/flows/versioned-text-synchronization.md
      docs/wiki/flows/document-leases-and-region-locks.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for Phase 5 pages.
      - `docs/wiki/modules/protocol-codec.md`: Update protocol message descriptions if changed.
      - `docs/wiki/modules/server-document-state.md`: Update canonical rope/version/lease/lock details.
      - `docs/wiki/flows/versioned-text-synchronization.md`: Document client shadow, pending edits, acknowledgements, stale rejection, and resync.
      - `docs/wiki/flows/document-leases-and-region-locks.md`: Document editable lease/read-only observer behavior and region-lock enforcement.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-wiki/references/page-template.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm `docs/wiki/index.md` links Phase 5 pages and the pages explain implementation flow, source paths, tests, performance constraints, and security boundaries.
    - `wiki_index_links_all_pages`: Run or add a deterministic check that every `docs/wiki/**/*.md` page except the index is linked from `docs/wiki/index.md`.
  - Verification Completed:
    - Manual wiki review of Phase 5 pages and related module/flow pages.
    - `python3 - <<'PY' ... PY` deterministic check confirmed every `docs/wiki/**/*.md` page except `docs/wiki/index.md` is linked from `docs/wiki/index.md`.
  - Implementation Notes:
    - Added `docs/wiki/flows/versioned-text-synchronization.md` to document client shadow state, pending transactions, confirmed/optimistic versions, stale/future rejection, resync snapshots, hot-path constraints, and IPC validation boundaries.
    - Added `docs/wiki/flows/document-leases-and-region-locks.md` to document editable lease ownership, read-only observer behavior, queue/editor enforcement, region-lock registration, overlap checks, conflict metadata, and absent authorities.
    - Updated `docs/wiki/index.md` navigation and cross-links in protocol, server document, client edit, and acknowledgement pages so Phase 5 synchronization, lease, and lock documentation is discoverable.

## Compromises Made
- Resync recovery is intentionally snapshot-based: stale/future version, lease, read-only, and region-lock rejections request a full canonical snapshot, clear pending edits, and reset confirmed/optimistic client versions instead of replaying pending local edits or applying correction transactions. This keeps Phase 5 deterministic but can discard optimistic local edits that were not acknowledged.
- Collaboration remains a single-writer lease model. The first connected client holds the editable lease, later clients are read-only observers, and disconnect releases the lease; user-facing lease transfer/steal/renewal UX is deferred.
- Region locks are in-memory server-owned metadata with internal/test registration only. Phase 5 enforces conflicts and exposes rejection metadata, but public lock-management APIs, persistence, UI, and AI/extension ownership flows are deferred.
- Ordinary edit IPC remains delta-only, but initial load and explicit resync still serialize full document snapshots as `String` values at the protocol edge while canonical server text is stored as `crop::Rope`.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
