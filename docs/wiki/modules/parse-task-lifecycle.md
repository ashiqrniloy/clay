# Parse Task Lifecycle

## Source

- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/markdown-mode-requirements.md`
- `src/server/document.rs`
- `src/server/js_runtime.rs`
- `src/server/parse_coordinator.rs`
- `src/protocol/parse.rs`
- `src/perf/budgets.rs`
- `tests/primitives_docs.rs`
- `tests/parse_coordinator.rs`

## Overview

Phase 16 defines package parsing as server-side, cancellable background work. Package parsers may analyze document text and return inert syntax, decoration, folding, or diagnostic data, but they never run in the Rust client and never block local typing or paint.

The canonical public design is `docs/reference/primitives/parse-update-strategy.md`; this wiki page explains the internal lifecycle and implemented coordinator boundary.

## Responsibilities

- Explain how accepted edits become compact background parse notifications.
- Describe spawn, cancellation, timeout, stale-result rejection, viewport priority, and fallback behavior.
- Identify where the parse coordinator attaches without overloading document mutation or JavaScript runtime modules.
- Record why parse output must be validated before becoming decoration/rendering data.

## How It Works

The implemented parse flow operates after the server accepts an edit/open, not before local paint:

1. `src/server/document.rs::DocumentState::apply_edit_with_parse_input` derives an exact `ParseInputEdit` from the validated canonical rope, then updates the rope and increments the server document version. Open/resync/viewport work has no fabricated edit.
2. `src/server/parse_coordinator.rs` receives compact accepted-edit, open-time, or viewport metadata. It does not receive or send full-document snapshots for ordinary edits.
3. The coordinator enqueues a `ParseEditNotification` for the active `(document_id, package_prefix, mode)` stream with the current document version, optional exact accepted edit, invalidated byte ranges, latest viewport range, and bounded `ParseWindowSnapshot`s carrying a stable `window_id` when parse text is needed.
4. The coordinator start-gates a background task that invokes a registered native or runtime-backed package parse handler. JS-backed handlers are looked up by a server-issued token stored during package load; the public op payload still rejects executable callback fields. Native task identity includes runtime generation, document, package/mode grammar stream, and stable parse-window identity—not decoration destination ranges. Duplicate same-version/window requests coalesce before spawn.
5. If a newer edit, viewport request, runtime generation replacement, or package-scoped revocation arrives, the coordinator aborts superseded tasks for the affected stream and keeps only the latest active package/generation authoritative. Newer versions supersede older work even when stable window identity changes; other documents and grammars remain independent. `ParseCoordinator::cancel_older_generations` is the post-commit reload cleanup path; `ParseCoordinator::cancel_package` withdraws package-owned handlers and active tasks through the same abort path as `cancel_generation`. Queued updates are drained so late old-generation results cannot publish.
6. If the handler exceeds its timeout, `RuntimeCommand::Parse` uses the smaller of the runtime service timeout and the handler's registered `timeoutMs`, terminates the isolate, returns `clay.runtime.timeout`, increments `ParseCoordinatorStats.failed_tasks`, and publishes no partial update.
7. Returned parse data is validated for active runtime generation, package provenance, declared permission, version, byte ranges, known schema values, payload size, viewport filtering, and parse-produced decoration payload budgets.
8. The server publishes validated inert results through implemented decoration publication (`DecorationSet`) or future folding, diagnostic, or related protocol messages. The client applies those updates outside paint/text-event handlers.

## Parse Units

Packages declare the coarsest unit they can update incrementally:

- **File-level**: useful for open/reload or small documents, but not the default after every edit in large files.
- **Region-level**: useful for Markdown fenced code blocks, sections, and diagnostics around changed blocks.
- **Line-group-level**: useful for Markdown syntax highlighting and list/heading-oriented line context.

For the Phase 18 Markdown POC, line-group-level parsing is the preferred default for inline syntax spans, with region-level invalidation for fenced code blocks and heading/list sections.

## Notification and Result Shapes

The coordinator now implements compact `rkyv`-serializable shapes in `src/protocol/parse.rs`:

```text
ParseEditNotification {
  document_id,
  package_prefix,
  active_mode_id,
  document_version,
  viewport,
  accepted_edit: Option<ParseInputEdit>,
  invalidated_ranges,
  parse_windows: Vec<ParseWindowSnapshot { window_id, text, ... }>,
}
```

```text
IncrementalParseUpdate {
  document_id,
  document_version,
  package_prefix,
  mode_id,
  viewport,
  invalidated_ranges,
  syntax_tree_delta,
  decoration_updates: Vec<DecorationSet>,
}
```

`ParseInputEdit` holds canonical old/new byte and point endpoints. A consecutive matching stable window permits `Tree::edit` plus one incremental parse; its old/new changed ranges union explicit invalidations, then `replacement_ranges` converts them into a shared 128-byte UTF-8-safe replacement-chunk grid, and the handler queries the full envelope covering every touched chunk once — so query coverage and replacement coverage are identical. One capture result fans out into 128-byte `DecorationSet` members built from the same grid, but member count never adds parser jobs.

The client should receive only validated rendering/folding/diagnostic declarations it knows how to apply. Syntax tree deltas are server/cache metadata unless a later primitive explicitly exposes them.

## Background Scheduling Policy

Parsing is `Background` work:

- It must not participate in the `ClientFirstPredictable` keypress-to-local-paint path.
- Queues are bounded per document and per package.
- Visible viewport ranges are prioritized first, adjacent ranges second, and off-viewport cache refresh last.
- Newer document versions supersede older tasks in the same document/package/mode stream; duplicate same-version/stable-window work coalesces.
- Newer runtime generations replace handler tokens and cancel old-generation in-flight tasks; package disable/revoke removes package-owned handlers and cancels in-flight tasks for that package prefix.
- Slow parse handlers degrade decoration freshness only; they do not prevent local text from appearing.

Relevant budgets:

- `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`: parse scheduling/results must not block this path.
- `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`: compact parse notifications/results.
- `DECORATION_PAYLOAD_BUDGET_BYTES`: parse-produced decoration payloads after validation and viewport filtering.
- `SYNTAX_CACHE_BUDGET_BYTES`: retained parse-window text across snapshots.
- Per-handler `timeoutMs`: package-declared parse timeout, capped by the runtime service timeout and surfaced as `clay.runtime.timeout` on expiration.

## Fallback Behavior

When package parsing lags:

- The server may send a `no-decoration-update` acknowledgement for the current version.
- The client retains last validated decorations for unaffected ranges.
- Edited syntax may remain provisionally styled only where generic inert-span interpolation is safe; structural or non-syntax overlap waits for authoritative current-version output.
- Diagnostics and semantic spans may be stale temporarily, then replaced or cleared when a current result arrives.
- No fallback path executes package JavaScript in the client.

## Invariants and Constraints

- Parse handlers run server-side through constrained `deno_core`, not in the Rust client.
- Package parse primitives require declared permissions such as `parse-document` and cannot access filesystem outside already-open document content, network, shell, AI mutation, WASM execution, remote listeners, raw `Deno.core.ops`, native widget mutation, or client-side JavaScript by default.
- `DocumentState` remains focused on canonical mutation; `ClayJsRuntimeService` remains the JavaScript boundary; `ParseCoordinator` owns scheduling, cancellation, validation, and server-side publication.
- Stale parse results are discarded before client publication, including results from old runtime generations after hot reload.
- Validation failures produce diagnostics or package errors, not server/client panics.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- `tests/parse_coordinator.rs`: covers permission-gated registration, superseded task cancellation, runtime-generation handler replacement/cancellation, package-scoped cancellation with handler withdrawal, stale-result discard, payload bounds, failed-task instrumentation, and proof that parse delays do not block edit acknowledgement.
- `src/server/js_runtime.rs::js_parse_handler_bridge_runs_registered_markdown_handler`: verifies `loadPackage("@clay/markdown")` registers a live JS parse handler, `ParseCoordinator::schedule_parse_with_windows` invokes it, and `next_update` receives validated decoration output.
- `src/server/js_runtime.rs::parse_registration_rejects_executable_callbacks_and_missing_permissions`: verifies executable callback fields and missing `parse-document` permissions are rejected.
- `src/server/js_runtime.rs::js_parse_handler_timeout_uses_registered_budget`: verifies a looping JS handler is bounded by registered `timeoutMs` instead of the larger service timeout.
- `cargo test --test protocol primitives_docs::`: runs the Phase 16 primitive documentation coverage suite.
- `cargo test --test runtime parse_coordinator::`: runs the implemented coordinator coverage.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Rendering Primitives](rendering-primitives.md)
- [Parse Coordinator](parse-coordinator.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Server Document State](server-document-state.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/markdown-mode-requirements.md`
