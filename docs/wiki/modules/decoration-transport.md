# Decoration Transport

## Source

- `src/protocol/decorations.rs` — inert `DecorationSet` / `DecorationSpan` shapes and provenance.
- `src/protocol/diagnostics.rs`, `src/protocol/folding.rs`, `src/protocol/parse.rs` — adjacent diagnostic, fold, and parse-update shapes.
- `src/server/decorations.rs`, `src/server/diagnostics.rs`, `src/server/folding.rs` — validation and bounded server caches.
- `src/server/ops/{decorations,diagnostics,folding}.rs` and `runtime/js/{decorations,diagnostics,folding}.js` — package publication boundaries.
- `src/server/parse_coordinator.rs` and `src/server/connection/{documents,mod}.rs` — background publication and viewport aggregation.
- `src/client/mod.rs` — renderer-neutral connection event projection.
- `src-tauri/src/bridge/{dto,forwarder,session}.rs` — typed envelope, identity stamping, and delivery.
- `frontend/src/editor/extensions/{controller,render-patch,decorations,diagnostics,folding}.ts` — CodeMirror render projection.
- `frontend/src/editor/{position-index,position-map}.ts` — UTF-16/UTF-8 conversion.
- Tests: `src/protocol/codec.rs`, `src/server/{decorations,diagnostics,folding}.rs`, `src/server/connection/mod.rs`, `src-tauri/src/bridge/forwarder.rs`, `frontend/src/editor/extensions/{controller,render-patch,extensions,performance}.test.ts`, `tests/{editor_performance,decoration_intent_authority,performance_budgets}.rs`.

## Overview

Decoration transport carries package or native syntax output as bounded,
validated, inert data. The server remains responsible for package provenance,
permissions, document/version checks, UTF-8 ranges, and payload limits. The
React client projects accepted data into CodeMirror state fields; it never runs
package code while applying or painting decorations.

For a visible viewport, `DecorationSet`, `DiagnosticSet`, and
`FoldingRangeSet` members travel together in one protocol-v29
`ViewportRenderPatch`. The patch carries request identity and exact
`coveredRanges`, so one response can replace the corresponding render slice
atomically. Edit-driven parse output keeps the existing member events and does
not acknowledge a viewport request.

## Responsibilities

- Define bounded protocol shapes and package provenance for syntax, semantic,
  link, and inlay decoration spans.
- Validate publication permission, document/version identity, viewport bounds,
  known token values, UTF-8-safe ranges, provenance, and serialized size.
- Retain server parse/decor data under `SYNTAX_CACHE_BUDGET_BYTES`; no client
  cache is used as a second source of truth.
- Route validated output through the parse coordinator, connection subscription,
  typed Tauri bridge, and CodeMirror fields.
- Keep projected marks, inlays, links, diagnostics, and folds independent by
  authority so one package or layer cannot erase another.

## Primitive Coverage

- **Decoration output:** `DecorationSet` / `DecorationSpan` is the generic
  inert output primitive for native Tree-sitter, web-tree-sitter, Markdown, and
  package-JavaScript adapters.
- **Atomic viewport output:** `ViewportRenderPatch` groups ordered decoration,
  diagnostic, and fold members with request ID, document/version, status, and
  authoritative covered ranges. It is internal transport, not a package API.
- **Client projection:** `applyRenderPatch` is one generic CodeMirror
  `StateEffect`; `decorationField`, `diagnosticField`, and `foldField` own their
  projected UTF-16 items. `EditorProjection` only orchestrates requests and
  builds effects.
- **Position conversion:** every byte boundary is converted through the shared
  `BytePositionIndex`; dense spans use `utf8ToUtf16Batch` rather than one scan
  per span.
- **Reuse rule:** future modes and analyzers publish bounded inert values via
  the existing server boundaries. They do not add renderer callbacks, raw CSS,
  client parser execution, Tauri calls, or language-specific client branches.

## How It Works

1. A native handler or authorized package produces `DecorationSet` values on the
   server. Package JavaScript reaches `serverPublishDecorations`, which routes
   through the typed `clay:decorations` facade and host package context. Caller
   JSON cannot supply executable callbacks or forge provenance.
2. Server validators check `render-decorations`, current document/version,
   package/mode identity, positive and ordered UTF-8 ranges, known vocabulary,
   viewport containment, sanitized metadata, and
   `DECORATION_PAYLOAD_BUDGET_BYTES` before publication or cache insertion.
   Diagnostics and folds use their own validators and budgets while retaining
   the same authority boundary.
3. `ParseCoordinator` validates decoration and optional diagnostic/fold side
   channels together. A bad member rejects the update instead of publishing a
   decoration-only half-state. Native syntax work is background and
   request-scoped work carries its client/request identity.
4. A viewport request is handled by
   `handle_viewport_render_request`. The server validates access, version, and
   range, clamps to canonical document bytes, prepares bounded parse windows,
   and schedules the selected grammar through the per-document `SyntaxSession`.
   The connection aggregates terminal window updates into one
   `ViewportRenderPatch`; `coveredRanges` comes from member output ranges, not
   the wider parser context.
5. `src/client` projects the server event into a typed connection event. Tauri's
   forwarder coalesces only obsolete whole viewport patches per document in its
   latest-wins lane. Member events, edit acknowledgements, and other live data
   retain strict FIFO ordering, so sibling ranges and layers stay intact.
6. `EditorProjection.handleEnvelope` drops stale request IDs and prepares every
   accepted member effect. Complete patch members dispatch together in one
   CodeMirror transaction. Empty/rejected patches dispatch no render data but
   still release the viewport request slot immediately.
7. `decorationPatch` converts all span boundaries once using the position index,
   maps token types to the closed `TOKEN_CLASSES` table, and emits mark/inlay/link
   items tagged by `packagePrefix:kind`. No server string reaches a CSS class.
8. `diagnosticPatch` replaces only the source/provenance authority inside the
   covered range and includes the CodeMirror lint effect in the same transaction.
   `foldPatch` replaces the package's sorted ranges and the fold service uses
   binary search for visible-line lookup. All fields map retained items through
   local edits and prune outside the covered range plus bounded overscan.

## Code Example

```text
ViewportRenderPatch {
  requestId,
  documentId,
  documentVersion,
  status: complete | empty | rejected,
  coveredRanges,
  decorations[],
  diagnostics[],
  folds[]
}
```

A complete patch is not a full-document replacement. It replaces only the
same-authority items intersecting each member's declared coverage; other
packages, layers, and ranges remain untouched.

## Invariants and Constraints

- Decorations are inert data: no callbacks, raw CSS, HTML, URLs, native handles,
  raw ops, or client-side package JavaScript.
- Stale document versions, unknown token values, malformed ranges, mismatched
  provenance, and over-budget payloads fail closed.
- `ViewportRenderPatch` has one terminal status per request ID. Its parse context
  may be wider than its authoritative output coverage.
- `VIEWPORT_OVERSCAN` is 4,096 UTF-16 positions, widened to the covered range
  for small viewports. Server syntax windows remain bounded by grammar policy,
  `MAX_VIEWPORT_PARSE_WINDOWS`, and syntax-cache budgets.
- A complete patch is one CodeMirror transaction. React does not hold or clone
  document text, and bridge slots do not own render state.
- Syntax highlighting remains separate from analyzer diagnostics. Tree-sitter
  recovery details are not promoted to correctness diagnostics.
- `PerformanceTraceId` is optional numeric metadata only; traces contain no
  source, paths, package code, credentials, or raw diagnostic content.

## Tests

- `src/server/decorations.rs` — publication validation, payload limits, and
  bounded syntax chunk cache.
- `src/server/diagnostics.rs` and `src/server/folding.rs` — source/range
  validation, cache bounds, permission separation, and fold derivation.
- `src/protocol/codec.rs::protocol_round_trips_viewport_render_patches` —
  complete, empty, rejected, and split-member patch round trips.
- `src/server/connection/mod.rs::viewport_render_requests_answer_one_patch_per_request_id` —
  request validation, range clamping, ordered aggregation, and one terminal
  response.
- `src-tauri/src/bridge/forwarder.rs` — whole-patch latest-wins and intact
  mixed-member delivery.
- `frontend/src/editor/extensions/render-patch.test.ts` — exact covered-range
  replacement, authority isolation, edit mapping, and retention guard.
- `frontend/src/editor/extensions/{controller,extensions,performance}.test.ts` —
  stale/empty completion, projection, and constant-size/four-pane invariants.
- `tests/editor_performance.rs`, `tests/decoration_intent_authority.rs`, and
  `tests/performance_budgets.rs` — protocol matrix, authority denial, and named
  budget coverage.

Run focused coverage with:

```bash
cargo test --lib protocol_round_trips_viewport_render_patches
cargo test --lib viewport_render_requests_answer_one_patch_per_request_id
cd frontend && npm test -- --run src/editor/extensions/render-patch.test.ts
```

## Related

- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- [Range Diagnostics](range-diagnostics.md)
- [Folding Ranges](folding-ranges.md)
- [Parse Coordinator](parse-coordinator.md)
- [Syntax Sessions](syntax-sessions.md)
- [Desktop Typed Bridge](desktop-typed-bridge.md)
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/registry.md#viewportrenderpatch`
- `docs/reference/packages/creating-packages.md`
