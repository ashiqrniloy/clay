# Phase 9 File and Workspace Server

## Objectives
- Move Clay from the in-memory document prototype to server-owned file loading, saving, workspace roots, open-document registry, dirty-state tracking, and file IO diagnostics.
- Preserve the locked authority model: the server owns filesystem/workspace authority and canonical documents; the client remains a canvas/view/input layer with optimistic shadows.
- Add protocol, documentation, Clay JS API, configuration, and test coverage for file/workspace behavior without introducing client filesystem authority or synchronous file IO in editor hot paths.

## Expected Outcome
- Clay can open, edit, save, and reload ordinary text files through the authoritative server model.
- Workspace roots are validated server-side, open documents are keyed by stable document IDs and canonicalized paths, and duplicate opens share the existing lease/read-only observer behavior.
- Dirty state reflects accepted edits and successful saves/reloads; file IO failures return clear protocol/app errors without panics or silent data loss.
- Container/toolbox/distrobox workflows are supported by explicit workspace-root and permission diagnostics while the native client receives metadata only.
- File/workspace Clay JS APIs and any configuration hooks are documented, indexed, generated, lookup-visible, and tested.
- `cargo fmt --check`, `cargo test`, and `cargo check` pass.

## Tasks

- [x] Define Phase 9 authority, workspace, and file state model
  - Acceptance Criteria:
    - Functional: The plan execution starts with a concrete model for workspace roots, canonical paths, open document identity, dirty state, save/reload transitions, and duplicate opens.
    - Performance: The model confirms ordinary typing/rendering never waits synchronously on file IO, workspace scans, JavaScript, AI, or full-document serialization.
    - Code Quality: The model identifies Rust ownership boundaries before implementation and avoids a global document lock where per-document state is sufficient.
    - Security: The model states that only the server may read/write workspace files and that the client receives document snapshots/errors, not filesystem authority.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 9 and locked authority/performance rules.
      - `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`: Server owns documents, file/workspace authority, leases, and locks.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Authority, hot-path, documentation, configuration, security, and performance checks.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server/client ownership split.
    - Options Considered:
      - Keep a single in-memory `DocumentState`: simple, but cannot represent file paths, dirty state, reload, or multiple open files.
      - Add ad hoc file fields to `DocumentState`: quick, but mixes path/workspace registry concerns into text-edit logic.
      - Add a server workspace/open-document layer above `DocumentState`: preserves existing edit/lease logic and makes file metadata explicit.
    - Chosen Approach:
      - Introduce a `WorkspaceState`/open-document registry layer that owns workspace roots, path validation, document ID allocation, path-to-document mapping, dirty metadata, and per-document `DocumentState` handles.
    - API Notes and Examples:
      ```rust
      // Intended shape, names may change during implementation.
      let document = workspace.open_document(&workspace_root, "src/main.rs").await?;
      workspace.save_document(document.document_id).await?;
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: New workspace root and open-document registry model.
      - `src/server/document.rs`: Add file/dirty metadata hooks while preserving edit validation responsibilities.
      - `src/server/mod.rs`: Register the workspace module; replacing the single default document field with workspace/open-document state remains for protocol/dispatch integration.
      - `docs/wiki/modules/server-file-workspace.md`: Document final implementation after execution.
      - `docs/wiki/index.md`: Link the server file workspace model wiki page.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `roadmap.md` Phase 9
  - Test Cases to Write:
    - `duplicate_open_reuses_document_and_preserves_lease_policy`: Opening the same canonical file twice returns the same document ID and preserves one editable lease/read-only observer behavior.
    - `file_backed_document_dirty_state_tracks_accepted_edits_and_clean_marking`: New file-backed documents start clean, accepted edits mark dirty, and explicit clean marking is available for successful saves/reloads.

- [x] Implement server-side workspace root and path validation
  - Acceptance Criteria:
    - Functional: The server accepts configured workspace roots, canonicalizes ordinary file paths, rejects traversal outside allowed roots, rejects directories/special files for document opens, and reports actionable errors.
    - Performance: Path validation uses metadata/canonicalization only at open/save/reload boundaries and never runs inside Masonry paint/input handlers or ordinary edit application.
    - Code Quality: Path validation and error mapping are isolated, deterministic, and covered by unit tests using temporary directories.
    - Security: Symlink/traversal handling prevents opening or saving outside authorized roots, and no workspace authority is delegated to the client.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/websites/rs_tokio_1_49_0` `tokio::fs`: Tokio filesystem APIs run blocking operations on the `spawn_blocking` pool and are intended for ordinary files.
      - Rust `std::path`/`std::fs` semantics as used by existing socket-path validation in `src/server/mod.rs`.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns workspace authority.
    - Options Considered:
      - Trust client-sent absolute paths: flexible, but violates authority boundaries and enables traversal.
      - Store only a single process working directory: simple, but weak for container/remote roots and future multi-workspace use.
      - Validate all opens against explicit server workspace roots and canonical paths: safer and compatible with container/remote execution.
    - Chosen Approach:
      - Add explicit `WorkspaceRoot` records and canonical path checks. Allow only regular files under a registered root for Phase 9 document IO; defer directories, globbing, watchers, and remote roots.
    - API Notes and Examples:
      ```rust
      let canonical_root = tokio::fs::canonicalize(root).await?;
      let canonical_file = tokio::fs::canonicalize(root.join(relative_path)).await?;
      if !canonical_file.starts_with(&canonical_root) { return Err(WorkspaceError::OutsideRoot); }
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: Root registration, path canonicalization, special-file rejection, workspace errors.
      - `src/server/mod.rs`: Server config gained initial workspace roots plus startup validation through internal server construction.
      - `src/server/workspace.rs`: Temporary-directory validation tests cover traversal, directory/special-file rejection, and symlink canonicalization.
      - `docs/wiki/modules/server-file-workspace.md`: Updated server workspace validation notes and test references.
      - `tests/rust_visibility_api_mapping.rs`: Allowlisted `IpcServer::try_new` as server infrastructure rather than a Clay JS API surface.
    - References:
      - Context7 `/websites/rs_tokio_1_49_0` `tokio::fs` docs.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `workspace_rejects_path_traversal_outside_root`: `../outside.txt` cannot be opened through a workspace root.
    - `workspace_rejects_directory_and_special_file_open`: Directories and non-ordinary files return typed errors.
    - `workspace_canonicalizes_symlink_before_authorization`: Symlinks escaping the root are denied; symlinks staying in-root are handled consistently.

- [x] Implement file-backed open document registry and loading
  - Acceptance Criteria:
    - Functional: The server can open an existing UTF-8 text file, create a canonical `DocumentState` from its contents, assign/stabilize a document ID, and reuse the existing document entry for duplicate opens.
    - Performance: Initial open may read a full file, but ordinary edits continue to use deltas and do not trigger full-document IPC except initial load or resync.
    - Code Quality: File IO, registry lookup, and text-edit mutation responsibilities remain separated; public Rust items are intentional and covered by visibility/API mapping.
    - Security: Loading is limited to authorized regular files and reports invalid UTF-8 or unsupported content clearly without exposing host paths beyond approved metadata.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/websites/rs_tokio_1_49_0` `tokio::fs::read`: async convenience read implemented through Tokio's blocking pool.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Initial snapshots/resync may use snapshots; ordinary edits must use deltas.
      - `src/server/document.rs`: Existing canonical rope, leases, versions, and region-lock validation.
    - Options Considered:
      - Load files directly in connection handlers: quick, but duplicates registry logic and risks per-connection ownership bugs.
      - Create a document actor per open file now: scalable, but more concurrency infrastructure than Phase 9 needs.
      - Use a central registry with per-document mutex/state handles: aligns with current server shape while allowing later actor migration.
    - Chosen Approach:
      - Add an open-document registry that maps canonical file path to document ID and document ID to file-backed state. Keep per-document ordering local to each document state and avoid global serialization for normal edits.
    - API Notes and Examples:
      ```rust
      let bytes = tokio::fs::read(&canonical_path).await?;
      let text = String::from_utf8(bytes).map_err(WorkspaceError::InvalidUtf8)?;
      let document = DocumentState::new(document_id, text, DocumentAccess::ReadOnly);
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: Added `WorkspaceState::open_existing_file`, async `tokio::fs::read` loading, duplicate-open reuse before disk re-read, invalid UTF-8 mapping, and registry-loading tests.
      - `docs/wiki/modules/server-file-workspace.md`: Updated implementation notes for server-side file loading, duplicate-open read avoidance, invalid UTF-8 behavior, and test coverage.
      - `src/server/document.rs`: No edit needed; existing `initial_document_message`, `is_dirty`, and lease hooks covered this task.
      - `src/server/connection.rs`: Deferred to the protocol/dispatch task; no protocol messages exist yet.
      - `src/protocol/mod.rs`: Deferred to the protocol/dispatch task; no message shape changes were needed for registry loading.
    - References:
      - Context7 `/websites/rs_tokio_1_49_0` `tokio::fs` docs.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `open_existing_file_loads_utf8_text`: Server registry returns a document snapshot with file contents and version 1.
    - `duplicate_open_reuses_loaded_document_and_lease_policy`: Two opens for the same canonical file share document ID, enforce editable/read-only access, and do not re-read changed disk contents after the first open.
    - `open_invalid_utf8_reports_file_io_error_without_document_entry`: Invalid text is rejected clearly and does not poison the registry.
  - Verification:
    - `cargo fmt --check`
    - `cargo test workspace:: --lib`
    - `cargo test server:: --lib`
    - `cargo test --test rust_visibility_api_mapping`
    - `cargo check`

- [ ] Implement dirty-state tracking, save, and reload behavior
  - Acceptance Criteria:
    - Functional: Accepted edits mark file-backed documents dirty; successful saves write the current canonical text and mark clean; reload can refresh a clean document from disk and rejects or requires force for dirty documents.
    - Performance: Save/reload run on server-side async file IO boundaries and never block ordinary typing, rendering, or edit acknowledgement paths longer than the specific server-first command requires.
    - Code Quality: Save/reload transitions are explicit state-machine operations with typed errors and tests for success, dirty conflict, missing file, permission denied, and stale metadata.
    - Security: Save writes only to the document's authorized canonical path or an explicitly validated save-as target; reload does not bypass workspace authorization.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/websites/rs_tokio_1_49_0` `tokio::fs::write`: async convenience write overwrites contents and uses Tokio's blocking pool.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns persistence and validation.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: File/workspace side effects are server-first, not behavior-manifest hot-path actions.
    - Options Considered:
      - Save after every accepted edit: simple persistence, but bad latency and disk churn.
      - Manual save only in Phase 9: predictable and enough for initial real-file editing.
      - Add file watchers/autosave now: useful, but belongs to product hardening/hot reload phases.
    - Chosen Approach:
      - Implement explicit server-first `saveDocument` and `reloadDocument` operations. Keep autosave/watchers deferred. Use current canonical rope text as save source after validating the document path/root.
    - API Notes and Examples:
      ```rust
      if document.is_dirty() {
          tokio::fs::write(document.path(), document.text().as_bytes()).await?;
          document.mark_clean_after_save();
      }
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: Save/reload operations and dirty-state metadata.
      - `src/server/document.rs`: Dirty flag updates after accepted edits and clean marking after save/reload.
      - `src/protocol/mod.rs`: Add save/reload request/result/error messages or command intents.
      - `src/server/connection.rs`: Dispatch save/reload as server-first operations.
    - References:
      - Context7 `/websites/rs_tokio_1_49_0` `tokio::fs` docs.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `accepted_edit_marks_file_document_dirty_and_save_marks_clean`: Dirty flag changes only on accepted edits and successful save.
    - `save_writes_canonical_rope_text_to_disk`: Disk content matches server canonical text after save.
    - `reload_dirty_document_requires_force_or_rejects`: Dirty documents are not silently overwritten by disk contents.
    - `save_permission_error_returns_typed_protocol_error`: IO failures are visible and leave the document dirty.

- [ ] Extend IPC protocol and connection dispatch for file/workspace commands and errors
  - Acceptance Criteria:
    - Functional: Clients can request workspace open, document save, document reload, document metadata/dirty state, and document list through versioned protocol messages; responses include document IDs, versions, access/lease metadata, dirty state, workspace-relative display paths, and typed errors.
    - Performance: Protocol messages avoid full-document payloads except initial open snapshots and explicit resync/reload snapshots; edit acknowledgements remain per-document ordered.
    - Code Quality: Protocol message shapes remain codec-isolated, `rkyv` round-trip tested, and error enums are actionable without string matching.
    - Security: Error responses avoid leaking unauthorized host paths while still explaining access denied, outside root, not found, invalid UTF-8, permission denied, and unsupported file type.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/websites/rs_rkyv` validation docs: use safe `from_bytes`/validation rather than unchecked archive access for IPC bytes.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Final-compatible metadata and fallible IPC input.
      - `src/protocol/codec.rs`: Existing length-prefixed `rkyv` codec with max frame and validation tests.
    - Options Considered:
      - Encode file operations as generic string commands: flexible, but hard to validate and document.
      - Add typed protocol variants: more explicit and testable.
      - Wait for Clay JS runtime ops before protocol changes: blocks real file editing unnecessarily.
    - Chosen Approach:
      - Add typed Phase 9 protocol variants for server-first file/workspace commands and results. Keep raw paths server-validated and use stable document/workspace IDs in normal messages.
    - API Notes and Examples:
      ```rust
      ClientMessage::OpenDocument { client_id, workspace_id, path }
      ClientMessage::SaveDocument { client_id, document_id, known_version }
      ServerMessage::DocumentOpened { document_id, version, text, access, dirty, path }
      ServerMessage::FileOperationFailed { code, message, document_id }
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: Add file/workspace request/result/error structs and enums.
      - `src/protocol/codec.rs`: Add round-trip and invalid-frame tests if needed for new variants.
      - `src/server/connection.rs`: Dispatch new messages to workspace state.
      - `src/client/**` or `src/main.rs`: Minimal client wiring only if needed to open an initial file through protocol.
    - References:
      - Context7 `/websites/rs_rkyv` validation docs.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `protocol_round_trips_open_save_reload_messages`: New message variants encode/decode safely.
    - `connection_open_document_sends_snapshot_and_manifest_without_full_document_on_edit_ack`: Open uses snapshot; edits continue as edit acks/transactions.
    - `file_io_errors_are_typed_protocol_failures`: Not-found/access-denied/invalid-UTF8 errors map to stable protocol codes.

- [ ] Add container/toolbox/distrobox-friendly workspace diagnostics
  - Acceptance Criteria:
    - Functional: Startup/open errors distinguish missing workspace roots, inaccessible mounts, permission denied, outside-root paths, and unsupported special files with messages suitable for host-client/container-server workflows.
    - Performance: Diagnostics run on workspace/open/save boundaries and do not add background scanning or blocking client startup beyond explicit server initialization checks.
    - Code Quality: Diagnostics are centralized, tested, and reusable by protocol errors, logs, and future UI surfaces.
    - Security: Diagnostics do not grant extra authority, shell access, network access, or unrestricted host path discovery.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 9: Container/toolbox/distrobox-friendly environment and permission handling.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server is the only component needing workspace filesystem permissions.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep background work from delaying input/UI-reactive paths.
    - Options Considered:
      - Add shell-based environment probing: rich but unsafe and out of scope.
      - Rely only on raw `io::Error`: simple, but poor UX for container mount/permission cases.
      - Map `io::ErrorKind` plus workspace context to typed Clay errors: useful and safe for Phase 9.
    - Chosen Approach:
      - Add typed workspace diagnostic errors with sanitized display paths and hints. Do not run shell commands or inspect outside configured roots.
    - API Notes and Examples:
      ```text
      Workspace root is not accessible from the Clay server process. If the server runs in toolbox/distrobox, mount or choose a root visible inside that environment.
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: Diagnostic/error types and display-path sanitization.
      - `src/server/mod.rs`: Startup workspace-root validation errors.
      - `docs/wiki/modules/server-file-workspace.md`: Document container/root assumptions after implementation.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `workspace_diagnostic_for_missing_root_is_actionable`: Missing root returns a stable code and helpful hint.
    - `workspace_diagnostic_sanitizes_unauthorized_paths`: Outside-root errors do not reveal extra path details.
    - `workspace_permission_denied_keeps_document_dirty`: Failed save reports permission denied and preserves dirty state.

- [ ] Run Phase 9 implementation verification
  - Acceptance Criteria:
    - Functional: Workspace roots, open registry, file load/save/reload, dirty state, protocol messages, and diagnostics are complete and consistent.
    - Performance: Verification confirms no ordinary typing/rendering path synchronously waits on file IO, workspace validation, registry generation, JavaScript, AI, or full-document IPC.
    - Code Quality: `cargo fmt --check`, `cargo test`, and `cargo check` pass with deterministic tests and current generated artifacts.
    - Security: Verification confirms no direct client filesystem authority, no path traversal, no unrestricted workspace access, and no arbitrary JavaScript/client-side JS execution was introduced.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Deterministic checks and actionable failures.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Protocol/performance validation guidance.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Final server/client ownership check.
    - Options Considered:
      - Run only new workspace tests: faster, but may miss synchronization or behavior-manifest regressions.
      - Run full phase verification: slower, but appropriate for a persistence boundary.
    - Chosen Approach:
      - Run focused tests during implementation, then final `cargo fmt --check`, `cargo test`, and `cargo check`. Regenerate doc registry only through `cargo run --bin update-doc-registry` when docs change, then rely on stale-check tests.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo test
      cargo check
      ```
    - Files to Create/Edit:
      - No new files expected unless verification exposes stale generated docs, tests, or wiki links.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - Full verification command set: `cargo fmt --check`, `cargo test`, and `cargo check` pass.
    - Manual phase-boundary review: Confirm file/workspace side effects are server-first and client hot-path editing remains client-first/asynchronous.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Any Phase 9 behavior-changing or user-configurable workspace/file settings are represented as Clay JS APIs with docs, index links, generated registry entries, lookup coverage, and tests; if no runtime configuration is introduced, this is explicitly verified.
    - Performance: Configuration verification confirms workspace/file configuration is not loaded or executed synchronously in editor input/rendering hot paths.
    - Code Quality: Configuration is not implemented as undocumented environment variables or ad hoc settings; raw ops and Rust internals remain implementation details.
    - Security: Configuration cannot implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace expansion, package, WASM, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration-as-Clay-JS-API rule and no-authority default.
      - `docs/reference/clay-js-api/configuration.md`: Existing `~/.config/clay/init.js` contract and Phase 11 runtime boundary.
    - Options Considered:
      - Add workspace roots as implicit config keys: rejected because Clay configuration must be API-documented and permissioned.
      - Defer all workspace configuration to process/server launch flags: acceptable for Phase 9 if documented as server startup authority, not user `init.js` runtime configuration.
      - Add planned documented workspace configuration APIs now: useful if Phase 9 introduces stable user-facing settings.
    - Chosen Approach:
      - Audit implementation. If workspace roots, default open behavior, save/reload prompts, or path display settings are user-configurable, add/verify documented Clay JS APIs; otherwise record that Phase 9 uses server startup/workspace authority only and no new `init.js` configuration authority.
    - API Notes and Examples:
      ```text
      Verify each configuration-relevant API:
      stable ID -> JS module/export -> docs path -> docs/index.md link -> generated registry entry -> lookup by ID/tag/custom property -> tests.
      ```
    - Files to Create/Edit:
      - `runtime/js/workspace.ts` or `runtime/js/documents.ts`: Add planned configuration-related facades only if needed.
      - `docs/reference/clay-js-api/workspace/**` or `documents/**`: Add docs only for actual public configuration surfaces.
      - `docs/index.md`: Link new API docs when added.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after docs changes.
      - `tests/clay_js_doc_registry.rs`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_facade_layout.rs`: Add coverage for any new configuration APIs.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - Configuration API audit: Confirm no undocumented Phase 9 behavior-changing settings exist.
    - No-authority validation: Any workspace/file configuration docs deny implicit filesystem/workspace expansion beyond explicit server-side validation.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Public Phase 9 programmatic surfaces such as opening, saving, reloading, listing documents, querying dirty state, and workspace metadata are exposed or planned through stable Clay JS/TS facades, explicit future op names, Markdown docs, index links, generated registry entries, and lookup tests.
    - Performance: Clay JS API verification confirms file/workspace operations are server-first and never part of ordinary keypress-to-paint latency.
    - Code Quality: Server-side Rust functions introduced for file/workspace internals are private/`pub(crate)` unless promoted through documented Clay JS APIs; visibility tests are updated.
    - Security: File/workspace APIs document permissions, workspace root validation, path traversal rejection, no raw `Deno.core.ops.op_*` user calls, and no client filesystem authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API verification task.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Rust public functions are not the public API.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Use domain modules and server/client authority markers.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown/index/generated registry/lookup contract.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Stale generated registry and lookup coverage gates.
    - Options Considered:
      - Put all file APIs in `clay:documents`: coherent with document lifecycle, but may become crowded as workspace APIs grow.
      - Add a new `clay:workspace` module for roots and workspace metadata while keeping document lifecycle in `clay:documents`: clearer domain split.
      - Keep APIs internal until `deno_core` runtime exists: safe but hurts discoverability and violates documentation-as-code for public behavior.
    - Chosen Approach:
      - Document planned Phase 9 APIs even before runtime op wiring, likely `serverOpenDocument`, `serverSaveDocument`, `serverReloadDocument`, `serverListDocuments`, `serverGetDocumentStatus`, and workspace-root metadata APIs if user-facing. Keep actual Rust implementation internal unless exposed through future op wrappers.
    - API Notes and Examples:
      ```ts
      import { serverOpenDocument, serverSaveDocument } from "clay:documents";

      const doc = await serverOpenDocument({ workspaceRootId: "default", path: "src/main.rs" });
      await serverSaveDocument({ documentId: doc.documentId });
      ```
    - Files to Create/Edit:
      - `runtime/js/documents.ts`: Add/verify planned document lifecycle exports and types.
      - `runtime/js/workspace.ts` and `runtime/js/mod.ts`: Add workspace facade if workspace metadata is public.
      - `docs/reference/clay-js-api/documents/*.md`: Add docs for open/save/reload/status/list APIs.
      - `docs/reference/clay-js-api/workspace/*.md`: Add docs for workspace APIs if introduced.
      - `docs/reference/clay-js-api/api-inventory.toml`: Add Phase 9 API metadata.
      - `docs/index.md`: Link all new API docs.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after docs changes.
      - `tests/rust_visibility_api_mapping.rs`: Update internal allowlist or API mapping for new Rust public items.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - `phase9_file_workspace_apis_are_documented_indexed_and_generated`: New public APIs have Markdown docs, index links, registry entries, and lookup coverage.
    - `server_public_items_have_api_inventory_entries_or_are_internal`: New Rust public items are either mapped to Clay JS APIs or made private/`pub(crate)`.
    - `file_workspace_api_security_notes_cover_permissions_and_path_validation`: Docs include workspace authorization, path traversal rejection, and no client filesystem authority.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/server-file-workspace.md
      docs/wiki/flows/file-open-save-reload.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/modules/server-file-workspace.md`: Explain workspace roots, open-document registry, file IO, dirty state, errors, and authority boundaries.
      - `docs/wiki/flows/file-open-save-reload.md`: Explain open/save/reload protocol and state transitions if useful after implementation.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
