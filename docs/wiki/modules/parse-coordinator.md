# Parse Coordinator

## Source

- `src/server/parse_coordinator.rs`
- `src/protocol/parse.rs`
- `src/server/ops/parse.rs`
- `runtime/js/parse.ts`
- `src/server/mod.rs`
- `tests/parse_coordinator.rs`
- `docs/reference/primitives/parse-update-strategy.md`

## Overview

The parse coordinator implements the Phase 18 `IncrementalParseUpdate` handoff as server-side background work. Package parse handlers register through a typed Rust boundary or the runtime-backed `clay.parse.serverRegisterParseHandler` facade/op contract, are permission checked for `parse-document`, and run asynchronously after document edits or viewport changes have already been accepted.

The coordinator never sends parser code to the Rust client and never waits for parse completion in the ordinary typing/local-paint path.

## Responsibilities

- Register package parse handlers only when the owning package declares `parse-document`.
- Provide the public `clay:parse` facade and explicit `op_clay_parse_register_parse_handler` wrapper for package registration metadata.
- Schedule per-document parse work for `(document_id, package_prefix, mode_id)`.
- Schedule bounded parse-window work with `ParseWindowSnapshot`, `ParsePolicy`, and `SyntaxMemoryBudget` metadata for large-file modes.
- Abort superseded in-flight tasks when a newer version for the same document/package/mode is scheduled.
- Sort invalidated ranges so viewport-intersecting work is handled first, using only generic byte-range metadata that token-stream adapters for Markdown, Python, or other modes can consume.
- Validate parse-window document/version/provenance metadata, byte lengths, per-window limits, `SYNTAX_CACHE_BUDGET_BYTES`, stale versions, ranges, optional parse-produced decoration metadata, known decoration style tokens, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` before publishing updates to downstream consumers.
- Expose an internal update receiver for later decoration/folding/diagnostic publication paths.

## How It Works

`ParseCoordinator::register_handler` accepts a validated `PackageRecord`, mode ID, and `ParseHandler`. The package record must include `PackagePermission::ParseDocument`; otherwise registration returns `ParseCoordinatorError::MissingPermission` with package-prefix provenance. The Phase 18 JavaScript-facing registration path calls `serverRegisterParseHandler`, which routes through `op_clay_parse_register_parse_handler`, validates package identity/permission, parse unit, viewport-priority metadata, timeout bounds, and rejects executable callback fields in the public registration payload.

`ParseCoordinator::schedule_parse` takes a `ParseScheduleRequest` containing document/version metadata, behavior version, package prefix, mode ID, viewport byte range, and invalidated ranges. Scheduling is intentionally cheap: it validates range shape, records the latest document version, aborts any active task for the same document/package/mode, spawns a Tokio background task, and returns immediately.

`ParseCoordinator::schedule_parse_with_windows` is the Phase 18.5 large-file path. The caller prepares bounded server-canonical snapshots from already-open document text, then the coordinator validates each snapshot before any package handler can observe it: document ID and version must match the request, package prefix and mode ID must match handler provenance, `byte_end - byte_start` must equal the UTF-8 byte length of `text`, every window must fit `ParsePolicy::max_window_bytes`, and total retained window text must fit `ParsePolicy::memory_budget_bytes` and `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB). Valid windows are delivered in `ParseEditNotification::parse_windows` with `SyntaxMemoryBudget` metadata.

The spawned task calls the handler with a compact `ParseEditNotification`. For metadata-only requests `parse_windows` is empty; for windowed requests it contains only the bounded snapshots selected for the viewport/invalidated ranges. When the handler resolves, `finish_task` runs validation before sending an `IncrementalParseUpdate` on the coordinator's internal update channel. If the result version no longer matches the latest recorded document version, the coordinator drops it and increments stale-result stats instead of publishing.

`src/protocol/parse.rs` defines the inert parse shapes:

- `ParseByteRange`
- `ParseUnit`
- `ParseEditNotification`
- `ParseWindowSnapshot`
- `ParseWindowRequest`
- `ParsePolicy`
- `SyntaxMemoryBudget`
- `IncrementalParseUpdate`

These types are `rkyv`-serializable for future protocol/cache use, but the current coordinator keeps parse updates server-side for downstream decoration/folding consumption rather than adding them to hot edit-ack IPC messages.

## Invariants and Constraints

- Parsing is `Background` work and must not block edit acknowledgement, client shadow updates, Masonry text-event handlers, or paint.
- Handler registration requires `parse-document`; install/enable alone grants no parser authority.
- Parser execution stays server-side through the typed handler/runtime boundary.
- The client receives only later validated inert render/folding/diagnostic data; it does not receive parser functions or package JavaScript.
- Stale results are discarded before publication.
- Incremental parse updates are bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.
- Windowed parser input is bounded by `ParsePolicy::max_window_bytes` per snapshot and `SYNTAX_CACHE_BUDGET_BYTES`/`SyntaxMemoryBudget` across retained syntax windows.
- Optional parse-produced decorations are validated through the shared decoration validation path before client publication, including range/style-token, document-version, viewport, and decoration payload-budget checks.
- The Markdown parser adapter publishes decoration updates as generic `DecorationSet` values; the coordinator validates them without knowing markdown-it tokens or Markdown syntax.
- Token-stream adapters receive package-neutral parse metadata (`document_id`, versions, package prefix, mode ID, viewport, invalidated byte ranges, and optional bounded parse windows). If a future adapter needs line-start tables beyond `base_line`, those must be added as generic parse-input primitives, not mode-specific Rust parser branches.

## Tests

- `tests/parse_coordinator.rs::parse_handler_registration_requires_parse_permission`
- `tests/parse_coordinator.rs::superseded_parse_task_is_cancelled`
- `tests/parse_coordinator.rs::parse_result_rejected_for_stale_version_and_oversized_payload`
- `tests/parse_coordinator.rs::generic_parse_request_metadata_supports_token_stream_adapters`
- `tests/parse_coordinator.rs::rust_code_has_no_markdown_specific_parser_branch`
- `tests/parse_coordinator.rs::markdown_parse_update_accepts_valid_decoration_payload`
- `tests/parse_coordinator.rs::markdown_parse_update_rejects_decoration_version_mismatch`
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
- `tests/parse_coordinator.rs::parsing_does_not_block_edit_acknowledgement`
- `src/server/js_runtime.rs::phase18_parse_and_decoration_facades_are_runtime_backed`

Run with:

```bash
cargo test --test parse_coordinator
cargo test phase18_parse_and_decoration_facades_are_runtime_backed --lib
```

## Related

- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Decoration Transport](decoration-transport.md)
- [Protocol Codec](protocol-codec.md)
- `docs/reference/primitives/parse-update-strategy.md`
- `plans/019-Phase17-Package-System-and-Mode-Loading-Foundation.md`
