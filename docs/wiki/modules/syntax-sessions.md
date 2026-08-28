# Syntax Sessions

## Source

- `src/server/syntax_session.rs` — `SessionMailbox`, `SessionReceiver`, and `SyntaxExecutor`.
- `src/server/parse_coordinator.rs` — session registry, enqueue/close lifecycle, and result publication.
- `src/server/syntax.rs` — per-document parser/tree state and bounded window/cache operations.
- `src/server/connection/{documents,mod}.rs` — validated scheduling and request-scoped patch aggregation.
- `src/server/js_runtime/{mod,validation}.rs` — mode activation and package handler boundary.
- `src/perf/budgets.rs` — executor, tree-cache, mode-cache, and parse-memory bounds.
- Tests: `src/server/syntax_session.rs`, `src/server/parse_coordinator.rs`, `src/server/syntax.rs`, `src/server/connection/mod.rs`.

## Overview

Plan 099 moved native syntax CPU and mode activation out of connection/Tokio
hot paths. Each runtime-generation/document/grammar stream owns one persistent
session with a latest-wins mailbox and one worker. Native Tree-sitter work runs
on a shared bounded blocking executor; package JavaScript handlers retain their
server runtime-worker path.

Sessions publish only validated inert parse results. They do not expose parser
handles, source text, scheduling controls, or trace contents to the webview or
package JavaScript.

## Responsibilities

- Retain at most one pending compatible job per session.
- Coalesce newer edit/viewport work without creating one task per decoration
  destination.
- Preserve request-scoped terminal completion when jobs are superseded, closed,
  stale, or failed.
- Bound native parser concurrency and per-document parser/tree cache state.
- Close sessions on document removal, package cancellation, runtime replacement,
  and connection/document lifecycle teardown.

## How It Works

1. `ParseCoordinator::schedule_parse_with_windows` validates a compact
   `ParseEditNotification`, resolves the current handler/generation, and finds
   the `(generation, document, grammar)` session key.
2. The first schedule creates a `SessionMailbox`, subscribes a worker, and
   stores the session. Later schedules replace only the mailbox's undelivered
   pending job. A request-scoped schedule is idempotent per request ID; normal
   edit/viewport schedules without a request ID still enqueue distinct work.
3. `SessionReceiver` tracks a monotonic `JobSeq` and an observed watermark. This
   matters because `watch` retains its last value after delivery: close/push
   must not treat an already-drained job as pending or complete it twice.
4. The worker acquires one `SyntaxExecutor` permit. Native handlers run
   `parse_blocking` inside `spawn_blocking`; package handlers await the normal
   runtime path. Queue wait, start, and end are recorded under the optional
   numeric trace ID.
5. A running job is never aborted mid-parse. `finish_task` checks generation,
   document version, session sequence, provenance, and request identity before
   publishing. Superseded output is discarded; request-scoped jobs receive an
   empty terminal completion when needed.
6. `close_sessions` closes mailboxes and returns undelivered request jobs for
   completion. Workers exit after their current job; late results cannot publish
   after generation/document cleanup.

## Parser state and mode activation

`TreeSitterSyntaxHandler` owns a `CachedSyntaxState` per document, including its
parser and latest tree. Immutable grammar/query definitions remain registry
owned, so same-language documents can parse independently. The tree cache is
bounded to `SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES` (64); eviction beyond that
ceiling is intentionally unspecified.

`classify_open_document` caches completed native mode activations per runtime
generation. Its key contains the path extension/name, shebang line, and a hash
of bounded leading content. A hit republishes the cached behavior manifest
under the new document scope and avoids another generated-module V8 evaluation;
cold and third-party modes retain the regular activation path. The cache is
bounded to `MODE_ACTIVATION_CACHE_ENTRIES` (64).

## Bounds and invariants

- `SYNTAX_EXECUTOR_MAX_JOBS = 4` native parses may hold permits concurrently.
- One session has one undelivered pending job and one worker.
- Newer document versions supersede older results; other documents remain
  independent.
- Parse windows are validated against grammar max-window, syntax-memory, and
  `SYNTAX_CACHE_BUDGET_BYTES` limits before a handler observes them.
- Session cleanup never grants package authority and never blocks edit
  acknowledgement on parser completion.
- Trace data is numeric and source-free. A running native parser can finish
  after supersession, but its result is discarded before publication.

## Tests

- `src/server/syntax_session.rs` — latest-wins replacement, close drain, jobs
  after delivery, and executor permit count.
- `src/server/parse_coordinator.rs` — timer starvation, 100-update latest-wins,
  same-language independent documents, superseded/closed request completion,
  and session cleanup.
- `src/server/syntax.rs::document_tree_cache_is_bounded_and_windows_respect_byte_budget` —
  window refusal and bounded per-document cache.
- `src/server/connection/mod.rs::mode_activation_cache_hit_skips_generated_module_evaluation` —
  activation identity parity and no repeat generated-module evaluation.
- `src/server/connection/mod.rs::open_document_renders_before_background_parse_completes` —
  open response does not wait for syntax work.

Run focused coverage with:

```bash
cargo test --lib server::syntax_session
cargo test --lib server::parse_coordinator
cargo test --lib mode_activation_cache_hit_skips_generated_module_evaluation
```

## Related

- [Parse Coordinator](parse-coordinator.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Decoration Transport](decoration-transport.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- `docs/reference/primitives/parse-update-strategy.md`
- `plans/099-Clay-Editor-Performance-Overhaul.md`
