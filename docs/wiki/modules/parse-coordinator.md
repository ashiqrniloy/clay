# Parse Coordinator

## Source

- `src/server/parse_coordinator.rs` — handler registry, session lifecycle, scheduling, validation, and publication.
- `src/server/syntax_session.rs` — per-document latest-wins mailbox and bounded executor.
- `src/server/syntax.rs` — native Tree-sitter parser/tree state and window parsing.
- `src/server/connection/{documents,mod}.rs` — open/edit/viewport scheduling and request aggregation.
- `src/server/document.rs` — canonical rope snapshots and parse-window slicing.
- `src/server/js_runtime/{mod,validation}.rs` and `src/server/ops/parse.rs` — runtime-backed package handler boundary.
- `src/protocol/parse.rs` — parse notifications, updates, request/client identity, and trace metadata.
- `src/perf/budgets.rs` — executor, tree-cache, window, payload, and memory bounds.
- `runtime/js/parse.js` — `clay:parse` facade.
- Tests: `src/server/{parse_coordinator,syntax,syntax_session}.rs`, `src/server/connection/mod.rs`, `src/server/js_runtime/mod.rs`, `tests/editor_performance.rs`, `tests/performance_budgets.rs`.
- Reference contracts: `docs/reference/primitives/parse-update-strategy.md` and `docs/reference/primitives/registry.md`.

## Overview

The parse coordinator turns accepted open/edit/viewport metadata into validated,
server-side background syntax work. It is the handoff between canonical
`DocumentState`, registered native/package handlers, per-document `SyntaxSession`
workers, and inert decoration/diagnostic/fold output.

The coordinator never sends parser code to the browser and never waits for parse
completion before local text paint or edit acknowledgement. Plan 099 replaced
per-request native task fan-out with a persistent session keyed by runtime
generation, document, and selected grammar.

## Responsibilities

- Register native and package handlers only after `parse-document` permission,
  provenance, mode, and runtime-generation validation.
- Schedule compact `ParseEditNotification` values with bounded viewport/window
  metadata; keep connection dispatch separate from parser execution.
- Own session creation, latest-wins scheduling, cancellation on close/revoke/
  generation replacement, stale-result rejection, and output routing.
- Validate every returned decoration, diagnostic, fold, version, viewport,
  provenance, and payload budget before publication.
- Stamp trace/request/client metadata on internal updates so atomic viewport
  completion can be correlated without trusting package output.
- Publish through bounded internal/test receivers and per-client document
  subscriptions without allowing connections to compete for one global drain.

## How It Works

1. A package registers through the typed `parse.serverRegisterParseHandler`
   facade or a native first-party grammar registration. The coordinator records
   the validated package/mode handler and generation. JavaScript callbacks stay
   behind a server-issued runtime token; raw executable callback fields are not
   accepted from package payloads.
2. Open/edit/viewport code builds a `ParseScheduleRequest` containing document
   and behavior versions, package/mode identity, viewport and invalidated byte
   ranges, optional accepted edit, bounded parse windows, and optional trace,
   request, and client IDs. Scheduling validates metadata and returns without
   running parser CPU on the connection task.
3. `schedule_parse_with_windows` looks up the handler and session key. The first
   schedule creates one `SessionMailbox` and worker; later jobs enter the same
   mailbox. The job carries the current notification/window identity rather than
   creating a separate parser per decoration destination.
4. The mailbox is latest-wins for pending compatible work. Request-scoped jobs
   are idempotent per request ID; edit/viewport schedules without a request ID
   still enqueue even when version/window metadata matches. Newer document
   versions supersede older work for that document/handler while other
   documents and grammars remain independent.
5. The session worker acquires one permit from `SyntaxExecutor` and runs native
   `ParseHandler::parse_blocking` inside `spawn_blocking`. Native parser CPU is
   therefore off Tokio workers and at most `SYNTAX_EXECUTOR_MAX_JOBS` jobs run
   concurrently. Package-JavaScript handlers continue through the persistent
   runtime worker and its registered timeout.
6. A running job is not interrupted mid-parse. When it finishes, `finish_task`
   checks session sequence, document version, handler generation, provenance,
   and request state. Superseded/stale output is discarded. A superseded,
   failed, or closed request-scoped job still emits one terminal empty completion
   so the connection's pending-patch counter cannot leak.
7. Successful output is validated as one logical update. Decoration sets,
   optional diagnostics, optional folds, window identity, current version,
   package provenance, UTF-8 ranges, serialized size, and syntax-cache/memory
   bounds must all pass. A side-channel failure publishes no decoration-only
   half-state.
8. Edit-driven updates route through bounded document subscriptions and retain
   their `DecorationSet`/`DecorationBatch`, `DiagnosticSet`, and
   `FoldingRangeSet` event shapes. Request-scoped viewport updates carry IDs to
   `connection/mod.rs`, which aggregates them into one `ViewportRenderPatch`.

## Parse shapes and window policy

`ParseEditNotification` is compact server-side data:

```text
ParseEditNotification {
  document_id, document_version, behavior_version,
  package_prefix, mode_id, viewport,
  accepted_edit?, invalidated_ranges[], parse_windows[],
  memory_budget?, trace_id?, request_id?
}
```

`IncrementalParseUpdate` carries document/version/package/mode/viewport,
invalidated ranges, decoration members, an optional diagnostic set, optional
folding set, and trace/request/client identity. It remains an internal
coordination shape; ordinary edit acknowledgements do not carry parse output.

The connection prepares bounded rope windows before enqueueing in the current
implementation. This leaves O(window) snapshot work on the connection task,
but parser/query work is off the runtime hot path; move slicing into the worker
only if profiling proves the remaining cost material.

Each window must match document/version/package/mode metadata, have byte length
equal to its UTF-8 text, fit the grammar's `ParsePolicy::max_window_bytes`, and
fit both `SyntaxMemoryBudget` and `SYNTAX_CACHE_BUDGET_BYTES`. Native syntax
uses the cached per-document tree/parser where possible. The request's wider
parse context is not automatically authoritative render coverage; the viewport
patch derives coverage from validated output members.

## Output routing and lifecycle

`ParseCoordinator` has bounded legacy/test `next_update` and `next_diagnostic`
lanes plus document/access-scoped output subscriptions. Live connection
subscriptions are removed on client disconnect, document close, package revoke,
or runtime-generation replacement. Saturated output drops rather than blocks
edit acknowledgement or grows memory.

`close_sessions` replaces the old per-task abort lifecycle. It closes mailboxes,
returns never-delivered pending request jobs for terminal completion, and lets a
running worker finish before discarding its output. `remove_document`,
`cancel_package`, `cancel_generation`, `cancel_older_generations`, and handler
replacement all use this cleanup path.

For mode activation, `classify_open_document` has a per-generation cache keyed by
path probes (extension/name, shebang, bounded leading-content hash). A cached
native classification republishes the identical behavior manifest without
re-evaluating the generated module in V8; cold or third-party paths retain the
runtime activation flow. `MODE_ACTIVATION_CACHE_ENTRIES` bounds this cache.

Runtime and parse failures become sanitized `parse.open_failed` diagnostics
containing package/mode/document identifiers and a reason category only. They
do not expose handler messages, source text, query text, paths, or parser
internals.

## Primitive Coverage

- **Parse handler:** existing typed `ParseHandler` boundary; package JavaScript
  receives only token-backed server operations and cannot select executor state.
- **Syntax session:** `SessionMailbox` + `SyntaxExecutor` in
  `src/server/syntax_session.rs`, one per generation/document/grammar.
- **Parser state:** `TreeSitterSyntaxHandler` owns per-document cached parser/tree
  state; immutable grammar/query definitions remain registry-owned.
- **Output:** `IncrementalParseUpdate` is validated inert data. Viewport output
  becomes `ViewportRenderPatch`; edit-driven output keeps existing member frames.
- **Reuse rule:** new modes consume the same bounded notification/session/
  validation path. They do not add parser branches to connection dispatch,
  client parser authority, raw ops, or synchronous input work.

## Invariants and Constraints

- Parse work is `Background`; it cannot block local CodeMirror updates, edit
  acknowledgement, React commits, bridge input, or paint.
- At most one pending job is retained per session and at most four native jobs
  run concurrently (`SYNTAX_EXECUTOR_MAX_JOBS = 4`).
- Each native document owns parser/tree state; a grammar-global parser mutex is
  not used to serialize unrelated documents.
- `SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES = 64`, syntax cache/window budgets, and
  handler/runtime timeouts remain host-owned limits.
- Stale versions, old generations, closed documents, superseded request IDs,
  invalid provenance, malformed windows, and over-budget output fail closed.
- Package authority remains server-side and permission-gated. No parser handle,
  scheduler, executor, document rope, trace contents, or completion control is
  exposed to package JavaScript or the webview.

## Tests

- `src/server/syntax_session.rs` — mailbox latest-wins/close behavior and
  executor permit bound.
- `src/server/parse_coordinator.rs` — bounded-session scheduling, Tokio timer
  starvation, latest-wins coalescing, independent document progress, and
  request-scoped completion.
- `src/server/syntax.rs` — per-document parser/tree cache and window-byte bound.
- `src/server/connection/mod.rs::open_document_renders_before_background_parse_completes` —
  open response before parse completion.
- `src/server/connection/mod.rs::viewport_render_requests_answer_one_patch_per_request_id` —
  request aggregation and one patch terminal response.
- `src/server/connection/mod.rs::mode_activation_cache_hit_skips_generated_module_evaluation` —
  activation parity and zero repeat V8 evaluation.
- `tests/editor_performance.rs` — 30-cell protocol matrix, mode identity,
  exact edit/version accounting, patch completion, and close retirement.
- `tests/performance_budgets.rs` — syntax/window/cache budget documentation.

Run focused coverage with:

```bash
cargo test --lib server::syntax_session
cargo test --lib server::parse_coordinator
cargo test --test runtime editor_performance_matrix_holds_deterministic_invariants -- --exact
```

## Related

- [Syntax Sessions](syntax-sessions.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Folding Ranges](folding-ranges.md)
- [Protocol Codec](protocol-codec.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- `docs/reference/primitives/parse-update-strategy.md`
