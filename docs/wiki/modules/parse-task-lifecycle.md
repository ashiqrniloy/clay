# Parse Task Lifecycle

## Source

- `src/server/parse_coordinator.rs` — scheduling, validation, stale-result handling, and output routing.
- `src/server/syntax_session.rs` — latest-wins mailbox and bounded native executor.
- `src/server/syntax.rs` — parser/tree state and bounded windows.
- `src/server/connection/{documents,mod}.rs` — open/edit/viewport scheduling and patch aggregation.
- `src/server/document.rs` — canonical rope snapshot boundary.
- `src/protocol/parse.rs` — notification/update metadata.
- `src/perf/budgets.rs` — parse, cache, and payload limits.
- Tests: `src/server/{parse_coordinator,syntax_session,syntax}.rs`, `src/server/connection/mod.rs`, `tests/editor_performance.rs`.

## Overview

Parsing is server-side, cancellable background work. Accepted document changes
and visible viewport metadata enter a bounded per-document `SyntaxSession`; the
session runs native parser CPU off Tokio workers or invokes package JavaScript
through the existing server runtime. Validated inert results then reach either
edit-driven member events or one request-scoped `ViewportRenderPatch`.

There is no timer-based viewport acknowledgement and no parser work in the
browser's input, React, bridge, layout, or paint paths.

## Lifecycle

1. `DocumentState` validates and applies an edit to the canonical rope. It
   produces compact accepted-edit metadata; ordinary edits do not copy the full
   document into a parser message.
2. Connection code builds `ParseScheduleRequest` / bounded
   `ParseEditNotification` metadata with document/version, mode/grammar,
   viewport, invalidated ranges, optional edit, and optional parse windows.
3. `ParseCoordinator` resolves the validated handler and runtime generation,
   creates or finds one session, and enqueues without waiting for completion.
4. `SessionMailbox` retains the latest undelivered compatible job. Newer
   versions supersede older work for the same document/handler; other documents
   and grammars remain independent. Request-scoped jobs preserve request
   completion even when superseded or closed.
5. The worker acquires a bounded `SyntaxExecutor` permit and runs native
   `parse_blocking` inside `spawn_blocking`; package handlers use the runtime
   worker and registered timeout. Queue wait/start/end metrics use optional
   numeric trace IDs.
6. `finish_task` rejects stale generation/version/sequence/provenance results,
   validates all decoration/diagnostic/fold members and budgets, and publishes
   no partial result. A running parse is allowed to finish, but superseded output
   is discarded before publication.
7. Edit-driven output is sent through bounded document subscriptions as
   `DecorationSet`/`DecorationBatch`, `DiagnosticSet`, and `FoldingRangeSet`.
   Request-scoped viewport output is aggregated by document/request/client and
   completes with exactly one `ViewportRenderPatch` (complete, empty, or
   rejected).
8. Close, document removal, package revoke, or runtime-generation replacement
   closes sessions and subscriptions. Undelivered request jobs receive terminal
   completion; late worker results cannot reach the client.

## Window and fallback policy

The connection currently slices already-open canonical rope windows before
queueing. Each window is UTF-8-safe, versioned, package/mode matched, bounded by
the grammar policy, and counted against `SyntaxMemoryBudget` and
`SYNTAX_CACHE_BUDGET_BYTES`. This leaves O(window) snapshot work on the
connection task while moving parser/query CPU to the worker; move slicing later
only if profiling proves it is material.

Slow or failed parsing never blocks local text or edit acknowledgements. The
client retains unaffected render items and waits for current authoritative
ranges. A viewport request is not freed by an unrelated member event: only
its explicit complete, empty, or rejected patch is terminal.
Errors are sanitized `RuntimeDiagnostic` values and contain no source,
handler text, paths, query text, or parser internals.

## Invariants and constraints

- Parsing is `Background` work; it cannot participate in
  `ClientFirstPredictable` keypress-to-local-paint.
- Native concurrency is capped at `SYNTAX_EXECUTOR_MAX_JOBS = 4`; per-document
  parser/tree retention is capped at `SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES = 64`.
- Package registration requires `parse-document`; client/package JavaScript
  cannot access parser handles, ropes, executors, raw ops, or completion state.
- Stale versions/generations, malformed windows, invalid provenance, and
  over-budget output fail closed.
- Trace metadata is bounded numeric data and never includes document content or
  unsanitized paths.

## Tests

- `src/server/parse_coordinator.rs` — permission, cancellation, stale-result,
  budget, runtime-diagnostic, session, and non-blocking acknowledgement tests.
- `src/server/syntax_session.rs` — mailbox and executor unit tests.
- `src/server/syntax.rs` — bounded window and per-document cache tests.
- `src/server/connection/mod.rs` — open-before-parse and one-patch-per-request
  integration tests.
- `tests/editor_performance.rs` — 30-cell mode/edit/version/patch matrix.

Run focused coverage with:

```bash
cargo test --lib server::parse_coordinator
cargo test --lib server::syntax_session
cargo test --test runtime editor_performance_matrix_holds_deterministic_invariants -- --exact
```

## Related

- [Syntax Sessions](syntax-sessions.md)
- [Parse Coordinator](parse-coordinator.md)
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Folding Ranges](folding-ranges.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- `docs/reference/primitives/parse-update-strategy.md`
