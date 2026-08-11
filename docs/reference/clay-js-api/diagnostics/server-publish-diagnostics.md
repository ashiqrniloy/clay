---
id: diagnostics.serverPublishDiagnostics
kind: clay-js-api
js_module: "clay:diagnostics"
js_export: serverPublishDiagnostics
js_facade: runtime/js/diagnostics.js::serverPublishDiagnostics
backing_rust: src/server/diagnostics.rs::validate_diagnostic_publication
deno_op: op_clay_diagnostics_publish_diagnostics
deno_op_path: src/server/ops/diagnostics.rs::op_clay_diagnostics_publish_diagnostics
name: serverPublishDiagnostics
user_facing_name: Publish Diagnostics
summary: Publish viewport-bounded, inert range diagnostics from server-side package code.
owner: server
phase: Phase 18.17
visibility: public
permissions: ['render-decorations']
key_bindings: []
custom_properties:
  - name: documentId
    type: number
    default: required
    description: Target open document ID.
  - name: documentVersion
    type: number
    default: required
    description: Server document version the diagnostics were produced for.
  - name: viewport
    type: byte-range
    default: required
    description: Visible byte range `{ byteStart, byteEnd }`; ordinary publications must be viewport-bounded.
  - name: source
    type: string
    default: required
    description: Source key for chunk replacement (for example `tree-sitter` or a package analyzer id).
  - name: spans
    type: DiagnosticSpan[]
    default: []
    description: Bounded inert byte-range diagnostics with severity, code, and message; empty clears the source chunk.
  - name: packagePrefix
    type: string
    default: package context
    description: Package API prefix retained as provenance.
security: Requires render-decorations permission plus server validation of package provenance, current document version, byte ranges, viewport bounds, source/code/message bounds, and DIAGNOSTIC_PAYLOAD_BUDGET_BYTES; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, language-server process, client-side JavaScript, raw Deno ops, arbitrary CSS, draw callbacks, GPU commands, or native widget mutation authority.
agent_guidance: Use `diagnostics.serverPublishDiagnostics` from server-side package code only. Publish inert DiagnosticSpan records; never invent renderer callbacks, CSS, raw ops, LSP process spawning, or client-side JavaScript hooks. Prefer this over stuffing diagnostic metadata into `serverPublishDecorations`.
lookup_tags: [js-api, diagnostics, syntax-error, range-diagnostic, lsp-ready, phase18.17]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverPublishDiagnostics

## Summary

Publishes validated `DiagnosticSet` data for an open document version. Clay filters and paints these inert ranges locally in Rust as theme-owned severity squiggles; package JavaScript never runs in paint, layout, keypress, scroll, or text-event handlers.

## Description

`serverPublishDiagnostics` is the public Clay JS API for the Phase 18.17 range-diagnostic primitive. It accepts a viewport-bounded set of byte-range diagnostics from server-side package code, validates the `render-decorations` permission and package provenance, checks version/range/field safety, enforces `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, and stores the validated set for server-to-client delivery.

Replacement is source-keyed: publishing an empty `spans` array for the same document/version/source/viewport clears prior diagnostics from that source without touching other sources. Future LSP packages can map LSP diagnostic fields onto this same inert contract without a second renderer path.

## When to use

Use this API when a package analyzer, parser bridge, or future language-server adapter has produced range diagnostics for the current document viewport. Do not use `serverPublishDecorations` for diagnostic message/severity metadata. Do not use this API for status-only session failures; those remain `RuntimeDiagnostic`.

## JavaScript usage

```ts
import { serverPublishDiagnostics } from "clay:diagnostics";

serverPublishDiagnostics({
  packageName: "@clay/rust",
  packageVersion: "0.1.0",
  packagePrefix: "rust",
  permissions: ["render-decorations"],
  documentId,
  documentVersion,
  viewport: { byteStart, byteEnd },
  source: "my-parser",
  spans: [{
    byteStart,
    byteEnd,
    severity: "error",
    code: "parser.syntax-error",
    message: "Syntax error",
  }],
});
```

## Example

```ts
serverPublishDiagnostics({
  packageName: "@clay/rust",
  packagePrefix: "rust",
  permissions: ["render-decorations"],
  documentId: 1,
  documentVersion: 4,
  viewport: { byteStart: 0, byteEnd: 80 },
  source: "my-parser",
  spans: [{
    byteStart: 12,
    byteEnd: 13,
    severity: "error",
    code: "parser.syntax-error",
    message: "Syntax error",
  }],
});
```

## Options

- `packageManifest` or package context fields: package identity and declared `render-decorations` permission.
- `documentId` (`number`, required): Target document.
- `documentVersion` (`number`, required): Version used by the producer.
- `currentDocumentVersion` (`number`, optional): Validation override used by tests/server integration; defaults to `documentVersion`.
- `viewport` (`{ byteStart: number; byteEnd: number }`, required): Viewport byte range.
- `source` (`string`, required): Source key for chunk replacement/clearing.
- `spans` (`DiagnosticSpan[]`, required): Known inert diagnostic records; empty clears the current source chunk.

Known severities are `error`, `warning`, and `info`. Span `source` defaults to the set-level `source` when omitted.

## Key bindings

No default key binding is assigned.

## Custom properties

- `documentId`: target open document.
- `documentVersion`: stale-version guard.
- `viewport`: publication range and IPC bound.
- `source`: source-keyed replacement identity.
- `spans`: bounded inert diagnostics.
- `packagePrefix`: provenance and conflict identity.

## Return and async behavior

Returns JSON-serializable publication metadata synchronously from the server runtime (`documentId`, `documentVersion`, `packagePrefix`, `source`, `publishedSpanCount`). The operation is intended for background parse/analyze work and must not be called from ordinary typing, paint, layout, scroll, pointer, or text-event paths.

Large-file callers should publish only the current viewport. Clay may evict off-viewport chunks to keep retained diagnostic cache overhead within `DIAGNOSTIC_CACHE_BUDGET_BYTES` (8 MiB), so callers must treat off-viewport diagnostics as refreshable background state.

## Errors

Fails with Clay error codes when permissions are missing, options are malformed, the document version is stale, ranges are invalid or outside the viewport, fields are empty/oversized/control-containing, package provenance mismatches, executable payloads are attempted, or payload size exceeds `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`.

## Permissions and security

Requires: `render-decorations`.

The API accepts inert data only. It rejects arbitrary CSS, callbacks, draw functions, raw `Deno.core.ops`, client-side JavaScript hooks, language-server process spawning, and unknown native rendering authority.

## Agent guidance

Prefer this facade over raw ops or Rust internals. Keep publications viewport-bounded. Map future LSP diagnostics onto `DiagnosticSpan` fields rather than inventing a second paint path. Do not expose or call internal chunk-cache helpers from packages.

## Backing implementation

- JS facade: `runtime/js/diagnostics.js::serverPublishDiagnostics`
- Runtime facade: `src/server/facades.rs`
- Op wrapper: `src/server/ops/diagnostics.rs::op_clay_diagnostics_publish_diagnostics`
- Validator: `src/server/diagnostics.rs::validate_diagnostic_publication`
- Protocol type: `src/protocol/diagnostics.rs::DiagnosticSet`

## Lookup metadata

- Stable ID: `diagnostics.serverPublishDiagnostics`
- User-facing name: Publish Diagnostics
- Lookup tags: `js-api`, `diagnostics`, `syntax-error`, `range-diagnostic`, `lsp-ready`, `phase18.17`
- App/help visible: true
