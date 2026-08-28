# Range Diagnostics

## Source

- `src/protocol/diagnostics.rs` — `DiagnosticSpan`, `DiagnosticSet`, provenance, and bounded fields.
- `src/protocol/parse.rs` — `IncrementalParseUpdate::diagnostic_update` and parse-side diagnostic metadata.
- `src/protocol/mod.rs` — `RuntimeDiagnostic`, severity, and event variants.
- `src/server/diagnostics.rs` — publication validation and bounded diagnostic cache.
- `src/server/ops/diagnostics.rs`, `runtime/js/diagnostics.js` — package publication boundary.
- `src/server/parse_coordinator.rs`, `src/server/connection/mod.rs` — side-channel validation and delivery.
- `src/client/mod.rs` and `src-tauri/src/bridge/{dto,forwarder}.rs` — typed event/bridge projection.
- `frontend/src/editor/extensions/{diagnostics,render-patch,controller}.ts` — CodeMirror diagnostic field and lint projection.
- `frontend/src/editor/position-index.ts`, `frontend/src/editor/position-map.ts` — byte conversion.
- Tests: `src/server/diagnostics.rs`, `src/protocol/codec.rs`, `src/server/connection/mod.rs`, `frontend/src/editor/extensions/{controller,render-patch,performance}.test.ts`, `tests/{editor_performance,decoration_intent_authority,performance_budgets}.rs`.
- Authoritative public API: [`diagnostics.serverPublishDiagnostics`](../../reference/clay-js-api/diagnostics/server-publish-diagnostics.md).
- Authoritative primitive contract: [Diagnostics](../../reference/primitives/diagnostics.md).

## Overview

Range diagnostics are an additive, source-associated editor layer. Authorized
server analyzers publish bounded `DiagnosticSet` values; Clay validates and
routes them beside syntax decorations. The React client stores projected
diagnostic items in a CodeMirror state field and mirrors them into CodeMirror's
lint extension for gutter/marker presentation. Status failures remain
`RuntimeDiagnostic` values and are not converted into inline squiggles.

A viewport response carries diagnostics as members of the atomic
`ViewportRenderPatch`. Edit-driven parse output may carry a standalone
`DiagnosticSet`. Both paths remain asynchronous and never block local text
input or paint.

## Responsibilities

- Validate document/version, source, viewport, span ranges, provenance, bounded
  messages, span count, serialized size, and cache budget.
- Keep analyzer diagnostics distinct from syntax highlighting and parser
  recovery details.
- Replace only the matching source/provenance authority inside a covered range.
- Map retained diagnostic ranges through local edits and prune outside the
  shared near-viewport guard.
- Suppress overlapping Tree-sitter recovery details under analyzer Error or
  Warning spans without an O(N²) comparison.
- Paint severity using theme-owned CodeMirror lint styles; no package code runs
  in the client.

## Primitive Coverage

- **Wire:** `DiagnosticSet` is document/version/viewport/source scoped and
  contains bounded `DiagnosticSpan` values with severity, message, code, and
  provenance.
- **Publication:** `diagnostics.serverPublishDiagnostics` reaches the typed
  `clay:diagnostics` facade and requires the existing render permission. Host
  context supplies package provenance.
- **Client:** `diagnosticField` owns projected `DiagnosticItem[]`; its
  `diagnosticPatch` effect and `setDiagnostics` lint effect are dispatched in
  one transaction. `applyRenderPatch` is shared with decorations and folds.
- **Reuse rule:** future analyzers and LSP bridges publish the same inert
  `DiagnosticSet`; they do not add parser authority, process spawning, client
  callbacks, or diagnostic-specific Rust branches.

## How It Works

1. A server analyzer or package publishes a diagnostic set through the typed
   facade. `validate_diagnostic_publication` checks permission and delegates to
   `validate_diagnostic_set` for version, viewport, provenance, sanitized
   fields, span/range limits, serialization, and `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`.
2. `ParseCoordinator` validates an optional diagnostic side channel together
   with decorations and folds. Document/version/viewport/provenance must match;
   a failed side channel prevents a partial update from being published.
3. For a visible viewport, `handle_viewport_render_request` aggregates the
   validated diagnostic member into one `ViewportRenderPatch`. The connection
   uses the request ID and client ID to ensure completion belongs to the
   requesting pane. Edit-driven diagnostic events continue through the bounded
   document subscription.
4. `diagnosticPatch` converts byte boundaries through `BytePositionIndex` and
   the batch converter, tags items with `source:packagePrefix`, replaces only
   that authority's covered range, and prunes outside the guard. It returns a
   transaction spec containing both `applyRenderPatch` and the lint effect.
5. `diagnosticField` maps items through local edits. `visibleDiagnostics` first
   sorts and merges suppressor intervals, then binary-searches those intervals
   for each Tree-sitter recovery item. Analyzer Error/Warning spans suppress
   overlapping recovery details; Info and non-overlapping items remain additive.
6. CodeMirror's `lintGutter` consumes the synchronized lint list. The client
   does not run the analyzer, parser, LSP process, filesystem, or Tauri bridge
   during paint/layout/input.
7. A new document snapshot or reset clears projected diagnostics. Stale
   document/version events are ignored before creating an effect.

## Code Example

```js
import { serverPublishDiagnostics } from "clay:diagnostics";

serverPublishDiagnostics({
  documentId,
  documentVersion,
  viewport: { byteStart, byteEnd },
  source: "my-analyzer",
  spans: [{ byteStart, byteEnd, severity: "error", message: "Invalid value" }],
});
```

The example is a server-side package publication. It does not provide a client
callback or a way to bypass host validation.

## Invariants and Constraints

- `RuntimeDiagnostic` is status-only; `DiagnosticSet` is source-associated
  inline data. Neither grants authority.
- Tree-sitter `ERROR`/`MISSING` recovery nodes are not correctness diagnostics;
  native highlighting leaves `diagnostic_update` empty.
- Empty source chunks clear only that source/provenance authority. Other
  analyzers and syntax layers remain intact.
- Diagnostics match current document/version and remain inside validated
  viewport bounds. Stale, malformed, over-budget, or unsanitized data fails
  closed.
- `DIAGNOSTIC_CACHE_BUDGET_BYTES` bounds server retention; client retention uses
  the shared 4,096-position viewport guard.
- Diagnostic severity colors are resolved by the active theme. Package values
  cannot inject CSS, raw colors, HTML, URLs, callbacks, or native handles.
- No package JavaScript or LSP process executes in CodeMirror state updates,
  paint, layout, scrolling, or keypress handling.

## Tests

- `src/server/diagnostics.rs` — validation, source-keyed cache replacement,
  empty clear, eviction, and generic composition rules.
- `src/protocol/codec.rs` and `src/server/connection/mod.rs` — wire and
  request-scoped delivery coverage.
- `frontend/src/editor/extensions/render-patch.test.ts` — covered-range and
  authority replacement.
- `frontend/src/editor/extensions/controller.test.ts` — patch delivery and
  stale identity handling.
- `frontend/src/editor/extensions/performance.test.ts` — bounded retention and
  four-pane projection invariants.
- `tests/editor_performance.rs`, `tests/decoration_intent_authority.rs`, and
  `tests/performance_budgets.rs` — matrix, authority, and budget guards.

Run focused coverage with:

```bash
cargo test --lib server::diagnostics
cargo test --test protocol performance_budgets::
cd frontend && npm test -- --run src/editor/extensions/render-patch.test.ts
```

## Related

- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Decoration Transport](decoration-transport.md)
- [Folding Ranges](folding-ranges.md)
- [Parse Coordinator](parse-coordinator.md)
- [Syntax Sessions](syntax-sessions.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- [First-Party LSP Bridge Packages](first-party-lsp-bridge-packages.md)
- `docs/reference/primitives/diagnostics.md`
- `docs/reference/clay-js-api/diagnostics/server-publish-diagnostics.md`
