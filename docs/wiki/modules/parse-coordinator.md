# Parse Coordinator

## Source

- `src/server/parse_coordinator.rs`
- `src/protocol/parse.rs`
- `src/server/ops/parse.rs`
- `runtime/js/parse.js`
- `src/server/syntax.rs`
- `src/server/connection.rs`
- `src/server/mod.rs`
- `tests/parse_coordinator.rs`
- `docs/reference/primitives/parse-update-strategy.md`

## Overview

The parse coordinator implements the Phase 18 `IncrementalParseUpdate` handoff as server-side background work. Package parse handlers register through a typed Rust boundary or the runtime-backed `clay.parse.serverRegisterParseHandler` facade/op contract, are permission checked for `parse-document`, and run asynchronously after document edits or viewport changes have already been accepted.

The coordinator never sends parser code to the Rust client and never waits for parse completion in the ordinary typing/local-paint path.

## Responsibilities

- Register package parse handlers only when the owning package declares `parse-document`.
- Provide the public `clay:parse` facade and explicit `op_clay_parse_register_parse_handler` wrapper for package registration metadata.
- Schedule per-document parse work for `(document_id, package_prefix, mode_id)`, including the bounded initial parse used by selected-file open-time activation.
- Schedule bounded parse-window work with `ParseWindowSnapshot`, `ParsePolicy`, and `SyntaxMemoryBudget` metadata for large-file modes and grammar-only syntax packages.
- Coalesce duplicate work for one document/package/mode/stable-window/version and abort older versions in that stream without affecting other documents or grammars.
- Replace parse handlers by runtime generation during hot reload and cancel old-generation parse tasks before they can publish.
- Sort invalidated ranges so viewport-intersecting work is handled first, using only generic byte-range metadata that token-stream adapters for Markdown, Python, or other modes can consume.
- Validate parse-window document/version/provenance metadata, byte lengths, per-window limits, `SYNTAX_CACHE_BUDGET_BYTES`, stale versions, ranges, every parse-produced decoration chunk and optional range-diagnostic metadata, their viewport/provenance identity, component payload budgets, and per-member `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` before publishing updates to downstream consumers.
- Instrument scheduled, cancelled, published, stale, and failed parse tasks through `ParseCoordinatorStats`.
- Dual-publish updates/diagnostics to bounded internal test/tooling receivers and bounded access-scoped connection subscriptions; production connections never compete to drain a global receiver.

## How It Works

`ParseCoordinator::register_handler` accepts a validated `PackageRecord`, mode ID, and `ParseHandler`. The package record must include `PackagePermission::ParseDocument`; otherwise registration returns `ParseCoordinatorError::MissingPermission` with package-prefix provenance. Runtime-backed registrations use `register_handler_for_generation` / `register_handler_meta_for_generation`, so each handler key stores the owning runtime generation ID with the handler. Re-registering the same package/mode for a newer generation replaces the old handler and aborts old-generation active tasks for that handler. The internal `replace_handler_meta_for_generation` variant permits generation-scoped handler refresh under an exact key; ordinary package registration still rejects same-generation duplicates. Native syntax handlers use document-selected grammar IDs as coordinator mode keys, while Tier 3 fallbacks keep package mode keys, so TypeScript/TSX can coexist and native selection does not overwrite fallback registration. The Phase 18 JavaScript-facing registration path calls `serverRegisterParseHandler`, which routes through `op_clay_parse_register_parse_handler`, validates package identity/permission, parse unit, viewport-priority metadata, timeout bounds, records a stable runtime handler token, and rejects executable callback fields in the public op payload.

The live JS bridge is deliberately split across the facade and the op. Package load code imports its parser module and passes the module object/export name to `serverRegisterParseHandler`; the facade stores that function in `globalThis.__clayParseHandlers[token]` after the op accepts the package metadata. Rust never receives a JS function value. `ClayJsRuntimeService::register_parse_handlers` adapts each accepted `JsParseHandlerRegistration` into the existing `ParseHandler` trait with `ParseCoordinator::register_handler_meta`, so the coordinator still sees only a normal Rust handler.

`ParseCoordinator::schedule_parse` takes a `ParseScheduleRequest` containing document/version metadata, behavior version, package prefix, mode ID, viewport byte range, and invalidated ranges. Scheduling is intentionally cheap: it validates range shape, snapshots the registered handler generation and stable parse-window identity into the task key, records the latest document version, and returns immediately. Duplicate requests for the same stable window/version coalesce. A newer version aborts older work for the same document/package/mode even when window identity changed, while other documents and grammars remain independent. A start gate ensures task execution cannot finish before its active-task record is installed, and completion verifies the active version before removing or publishing work.

Native connection scheduling submits one full bounded window instead of one task per decoration destination. Open, edit, and viewport requests therefore enter the parser once per missing document/version/window. The handler returns one `IncrementalParseUpdate` containing visible/changed-first `decoration_updates`; the coordinator validates every member before publishing the batch, and the connection drains members as ordinary `DecorationSet` messages.

`ParseCoordinator::schedule_parse_with_windows` is the Phase 18.5 large-file path. The caller prepares bounded server-canonical snapshots from already-open document text, then the coordinator validates each snapshot before any package handler can observe it: document ID and version must match the request, package prefix and mode ID must match handler provenance, `byte_end - byte_start` must equal the UTF-8 byte length of `text`, every window must fit `ParsePolicy::max_window_bytes`, and total retained window text must fit `ParsePolicy::memory_budget_bytes` and `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB). Valid windows are delivered in `ParseEditNotification::parse_windows` with `SyntaxMemoryBudget` metadata.

The spawned task calls the handler with a compact `ParseEditNotification`. For metadata-only requests `parse_windows` is empty; for windowed requests it contains only the bounded snapshots selected for the viewport/invalidated ranges. JS-backed handlers are invoked on the persistent runtime worker by token; the runtime calls the registered package function under the smaller of the service timeout and the handler's registered `timeoutMs`, stores the returned update through `op_clay_parse_store_update`, converts it into `IncrementalParseUpdate`, then returns it to the coordinator task. Phase 18.10 Tree-sitter highlighting uses the same handler trait through `TreeSitterSyntaxHandler`: package grammar metadata is validated before registration, the handler receives only bounded server-prepared windows, compiles/caches its query and tree server-side, and returns an inert `IncrementalParseUpdate::decoration_updates` batch for normal coordinator validation. When the handler resolves, `finish_task` first verifies that the task generation still matches the active handler generation for that package/mode; old-generation results are counted as stale and never published. It then validates all decoration batch members and the optional diagnostic side channel together before sending one `IncrementalParseUpdate` on the coordinator's internal update channel. Diagnostic document/version/viewport and package-prefix provenance must match the enclosing update, and `validate_diagnostic_set` enforces bounded sanitized metadata. Any side-channel failure rejects the whole update, so consumers never observe decoration-only half-state. If the result document version no longer matches the latest recorded document version, the coordinator drops it and increments stale-result stats instead of publishing. Handler errors, parse timeouts, invalid updates, and payload-budget failures increment failed-task stats, publish no half-updated result, and enqueue sanitized `RuntimeDiagnostic` values on the diagnostic receiver (`clay.parse.open_failed`) without leaking handler messages, file paths, or source text.

`src/protocol/parse.rs` defines the inert parse shapes:

- `ParseByteRange`
- `ParseUnit`
- `ParseEditNotification`
- `ParseWindowSnapshot`
- `ParseWindowRequest`
- `ParsePolicy`
- `SyntaxMemoryBudget`
- `SyntaxDiagnosticCapture` / `SyntaxDiagnosticKind`
- `IncrementalParseUpdate`, including `decoration_updates: Vec<DecorationSet>` and optional `diagnostic_update`

`SyntaxDiagnosticCapture` is the engine-neutral local recovery shape (`byte_start`, `byte_end`, and `Error`/`Missing`) used before a syntax engine translates captures into a source-associated `DiagnosticSet`. JavaScript handlers return inert `{ diagnostics: { source, spans } }` records; Rust supplies package provenance from the accepted registration rather than trusting handler JSON.

These types are `rkyv`-serializable for future protocol/cache use, but the current coordinator keeps parse updates server-side for downstream decoration/folding consumption rather than adding them to hot edit-ack IPC messages.

## Output Routing and Runtime Diagnostics

`ParseCoordinator` dual-publishes through two bounded paths. Legacy internal/test receivers (`next_update` / `next_diagnostic`) use capacity-4096 `mpsc` lanes with non-blocking `try_send`; live connections use `OutputRouter` lanes (capacity 64 per client). `subscribe_document` adds a client only after workspace access is established, updates route by `document_id`, diagnostics broadcast only to registered clients, and `unsubscribe_client` / `remove_document` withdraw routes on disconnect/final close. Saturation drops output rather than growing memory or blocking edit acknowledgement.

`finish_task` publishes a sanitized `clay.parse.open_failed` diagnostic for handler failures, invalid decoration/update validation, parse-window or payload-budget failures, and stale-result rejection paths that must be visible to the runtime/UI without publishing a partial update. `parse_failure_diagnostic` reports only package prefix, mode ID, document ID, and a bounded reason category such as `handler failed` or `payload budget exceeded`; it does not forward handler text, paths, source text, query text, or parser internals.

## Open-Time Flow

`open_document_followup_messages` classifies and activates the document, schedules the grammar policy's bounded opening window (4 KiB for first-party native grammars), and returns the behavior manifest immediately. `schedule_open_parse` is enqueue-only, so initial text and mode state render before background syntax/parse work completes. When scrolling changes the visible byte range, the client sends a deduplicated metadata-only `DecorationViewportRequest`; `schedule_parse_window` validates the current document/version and schedules the already-registered document-selected native handler over a UTF-8-safe nonzero window. Later decorations and failures arrive through that connection's bounded document subscription; internal tests/tooling may still use `next_update()` / `next_diagnostic()`. Scheduling errors use sanitized runtime diagnostics.

## Invariants and Constraints

- Parsing is `Background` work and must not block edit acknowledgement, client shadow updates, Masonry text-event handlers, or paint.
- Handler registration requires `parse-document`; install/enable alone grants no parser authority.
- Parser execution stays server-side through the typed handler/runtime boundary; Rust never accepts executable callback fields in the op payload.
- The client receives only later validated inert render/folding/diagnostic data; it does not receive parser functions or package JavaScript.
- Stale results are discarded before publication, including old-runtime-generation task results after hot reload.
- Live parse output is document/access scoped. Per-client lanes and legacy test lanes are bounded and use non-blocking publication; final close/disconnect removes routes and document task/version state.
- Incremental parse updates are bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`; over-budget updates increment failed-task stats, emit sanitized diagnostics, and are not published.
- Windowed parser input is bounded by `ParsePolicy::max_window_bytes` per snapshot and `SYNTAX_CACHE_BUDGET_BYTES`/`SyntaxMemoryBudget` across retained syntax windows.
- Parse-produced decoration batches and diagnostics are validated atomically. Every decoration member must match the enclosing update's document/version/package and stay inside its viewport; each member independently satisfies decoration and incremental-update payload ceilings. Diagnostics match the enclosing viewport exactly and additionally use centralized field/count/range/payload sanitization.
- The Markdown parser adapter and the Tree-sitter syntax handler publish decoration updates as generic `DecorationSet` values; the coordinator validates them without knowing markdown-it tokens, Markdown syntax, Rust syntax, TypeScript syntax, or JavaScript syntax.
- Token-stream adapters receive package-neutral parse metadata (`document_id`, versions, package prefix, mode ID, viewport, invalidated byte ranges, and optional bounded parse windows). If a future adapter needs line-start tables beyond `base_line`, those must be added as generic parse-input primitives, not mode-specific Rust parser branches.

## Tests

- `tests/parse_coordinator.rs::parse_handler_registration_requires_parse_permission`
- `tests/parse_coordinator.rs::superseded_parse_task_is_cancelled`
- `tests/parse_coordinator.rs::parse_result_rejected_for_stale_version_and_oversized_payload`
- `tests/parse_coordinator.rs::generic_parse_request_metadata_supports_token_stream_adapters`
- `tests/parse_coordinator.rs::rust_code_has_no_markdown_specific_parser_branch`
- `tests/parse_coordinator.rs::markdown_parse_update_accepts_valid_decoration_payload`
- `tests/parse_coordinator.rs::markdown_parse_update_rejects_decoration_version_mismatch`
- `tests/parse_coordinator.rs::finish_task_publishes_runtime_diagnostic_for_handler_error`
- `tests/parse_coordinator.rs::stale_parse_result_is_not_published`
- `tests/parse_coordinator.rs::parse_window_snapshot_is_bounded_and_versioned`
- `tests/parse_coordinator.rs::large_file_edit_does_not_copy_full_document_to_parser`
- `tests/parse_coordinator.rs::newer_viewport_parse_cancels_stale_window_work`
- `tests/parse_coordinator.rs::parse_window_snapshot_requires_parse_permission`
- `tests/parse_coordinator.rs::parse_window_snapshot_rejects_oversized_or_mismatched_windows`
- `src/server/document.rs::tests::parse_window_snapshot_slices_only_requested_server_range`
- `src/server/document.rs::tests::parse_window_snapshots_validate_utf8_boundaries_and_memory_budget`
- `tests/performance_protocol.rs::parse_window_policy_keeps_large_file_snapshot_budget_bounded`
- `tests/editor_performance_invariants.rs::parse_window_snapshot_primitive_uses_bounded_rope_slicing`
- `tests/parse_coordinator.rs::generation_replacement_uses_new_handler_for_subsequent_parse`
- `tests/parse_coordinator.rs::replacing_generation_cancels_old_in_flight_parse_work`
- `tests/parse_coordinator.rs::handler_failures_are_instrumented_after_generation_replacement`
- `tests/parse_coordinator.rs::handler_failures_are_instrumented_and_not_published`
- `tests/parse_coordinator.rs::parsing_does_not_block_edit_acknowledgement`
- `src/server/js_runtime.rs::phase18_parse_and_decoration_facades_are_runtime_backed`
- `src/server/js_runtime.rs::js_parse_handler_bridge_runs_registered_markdown_handler`
- `src/server/js_runtime.rs::parse_registration_rejects_executable_callbacks_and_missing_permissions`
- `src/server/js_runtime.rs::js_parse_handler_timeout_uses_registered_budget`
- `src/server/js_runtime.rs::runtime_boundary_does_not_expose_platform_authorities`
- `src/server/connection.rs::open_document_renders_before_background_parse_completes`
- `src/server/connection.rs::native_windows_schedule_once_for_each_first_party_language`
- `tests/parse_coordinator.rs::same_native_window_and_version_is_scheduled_once_across_viewports`
- `tests/parse_coordinator.rs::rapid_native_versions_cancel_superseded_work_and_publish_latest`
- `tests/parse_coordinator.rs::native_work_for_two_documents_runs_independently`
- `src/server/connection.rs::selected_markdown_file_publishes_manifest_and_decorations`
- `tests/syntax_grammar.rs::manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow`
- `tests/syntax_grammar.rs::tree_sitter_handler_publishes_through_parse_coordinator_and_rejects_stale_results`

Run with:

```bash
cargo test --test runtime parse_coordinator::
cargo test phase18_parse_and_decoration_facades_are_runtime_backed --lib
cargo test js_parse_handler_bridge_runs_registered_markdown_handler --lib
```

## Related

- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Protocol Codec](protocol-codec.md)
- `docs/reference/primitives/parse-update-strategy.md`
- `plans/019-Phase17-Package-System-and-Mode-Loading-Foundation.md`
- `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`
- `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`
