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

A future parse flow should operate after the server accepts an edit, not before local paint:

1. `src/server/document.rs::DocumentState::apply_edit` accepts an edit, updates the canonical rope, and increments the server document version.
2. `src/server/parse_coordinator.rs` receives compact accepted-edit or viewport metadata. It does not receive or send full-document snapshots for ordinary edits.
3. The coordinator enqueues a `ParseEditNotification` for the active `(document_id, package_prefix, mode)` stream with the new document version, behavior version, invalidated byte ranges, and the latest viewport range.
4. The coordinator spawns or reuses a background task that invokes the package parse handler through `src/server/js_runtime.rs`, the constrained `deno_core` boundary.
5. If a newer edit arrives, the coordinator cancels or supersedes stale work by cancellation token or generation counter.
6. If the handler exceeds its timeout, the coordinator cancels the task and emits a sanitized runtime diagnostic.
7. Returned parse data is validated for package provenance, declared permission, version, byte ranges, known schema values, payload size, and viewport filtering.
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
  client_id,
  package_prefix,
  active_mode_id,
  base_version,
  document_version,
  behavior_version,
  edit_range_before,
  edit_range_after,
  inserted_text_preview,
  invalidated_byte_ranges,
  viewport_byte_start,
  viewport_byte_end,
}
```

```text
ParseResult {
  document_id,
  document_version,
  behavior_version,
  package_prefix,
  parse_unit,
  invalidated_byte_ranges,
  syntax_tree_delta,
  decoration_update,
  folding_ranges,
  diagnostics,
}
```

The client should receive only validated rendering/folding/diagnostic declarations it knows how to apply. Syntax tree deltas are server/cache metadata unless a later primitive explicitly exposes them.

## Background Scheduling Policy

Parsing is `Background` work:

- It must not participate in the `ClientFirstPredictable` keypress-to-local-paint path.
- Queues are bounded per document and per package.
- Visible viewport ranges are prioritized first, adjacent ranges second, and off-viewport cache refresh last.
- Newer document versions supersede older overlapping tasks.
- Slow parse handlers degrade decoration freshness only; they do not prevent local text from appearing.

Relevant budgets:

- `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`: parse scheduling/results must not block this path.
- `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`: compact parse notifications/results.
- `DECORATION_PAYLOAD_BUDGET_BYTES`: parse-produced decoration payloads after validation and viewport filtering.

## Fallback Behavior

When package parsing lags:

- The server may send a `no-decoration-update` acknowledgement for the current version.
- The client retains last validated decorations for unaffected ranges.
- Edited regions may temporarily fall back to plain/default mode styling.
- Diagnostics and semantic spans may be stale temporarily, then replaced or cleared when a current result arrives.
- No fallback path executes package JavaScript in the client.

## Invariants and Constraints

- Parse handlers run server-side through constrained `deno_core`, not in the Rust client.
- Package parse primitives require declared permissions such as `parse-document` and cannot access filesystem outside already-open document content, network, shell, AI mutation, WASM execution, remote listeners, raw `Deno.core.ops`, native widget mutation, or client-side JavaScript by default.
- `DocumentState` remains focused on canonical mutation; `ClayJsRuntimeService` remains the JavaScript boundary; `ParseCoordinator` owns scheduling, cancellation, validation, and server-side publication.
- Stale parse results are discarded before client publication.
- Validation failures produce diagnostics or package errors, not server/client panics.

## Tests

- `tests/primitives_docs.rs::parse_strategy_doc_linked_from_index`: verifies the parse strategy is linked from `docs/index.md`.
- `tests/primitives_docs.rs::incremental_parse_budget_constant_exists`: verifies `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` compiles through `src/perf/budgets.rs`.
- `tests/parse_coordinator.rs`: covers permission-gated registration, superseded task cancellation, stale-result discard, payload bounds, and proof that parse delays do not block edit acknowledgement.
- `cargo test --test primitives_docs`: runs the Phase 16 primitive documentation coverage suite.
- `cargo test --test parse_coordinator`: runs the implemented coordinator coverage.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Rendering Primitives](rendering-primitives.md)
- [Parse Coordinator](parse-coordinator.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Server Document State](server-document-state.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/markdown-mode-requirements.md`
