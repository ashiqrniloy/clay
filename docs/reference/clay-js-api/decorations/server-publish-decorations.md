---
id: clay.decorations.serverPublishDecorations
kind: clay-js-api
js_module: "clay:decorations"
js_export: serverPublishDecorations
js_facade: runtime/js/decorations.ts::serverPublishDecorations
backing_rust: src/server/decorations.rs::validate_decoration_publication
deno_op: op_clay_decorations_publish_decorations
deno_op_path: src/server/ops/decorations.rs::op_clay_decorations_publish_decorations
name: serverPublishDecorations
user_facing_name: Publish Decorations
summary: Publish viewport-bounded, inert decoration spans from server-side package code.
owner: server
phase: Phase 18
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
    description: Server document version the spans were produced for.
  - name: viewportByteRange
    type: byte-range
    default: required
    description: Visible byte range `{ byteStart, byteEnd }`; ordinary publications must be viewport-bounded.
  - name: spans
    type: DecorationSpan[]
    default: []
    description: Bounded inert byte-range spans with known kinds and style tokens.
  - name: packagePrefix
    type: string
    default: package context
    description: Package API prefix retained as provenance.
security: Requires render-decorations permission plus server validation of package provenance, current document version, byte ranges, viewport bounds, known style tokens, known kinds, and DECORATION_PAYLOAD_BUDGET_BYTES; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, arbitrary CSS, draw callbacks, GPU commands, or native widget mutation authority.
agent_guidance: Use `clay.decorations.serverPublishDecorations` from server-side package code only. Publish inert spans; never invent renderer callbacks, CSS, raw ops, or client-side JavaScript hooks.
lookup_tags: [js-api, decorations, markdown, decorationrange, syntax-highlighting]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverPublishDecorations

## Summary

Publishes validated `DecorationSpan` data for an open document version. Clay filters and renders these inert ranges locally in Rust; package JavaScript never runs in paint, layout, keypress, scroll, or text-event handlers.

## Description

`serverPublishDecorations` is the public Clay JS API for the Phase 18 `DecorationRange` primitive. It accepts a viewport-bounded set of byte spans from server-side package code, validates the package permission and provenance, checks version/range/style-token safety, enforces `DECORATION_PAYLOAD_BUDGET_BYTES`, and stores the validated set for server-to-client delivery.

## When to use

Use this API when a package parse/render provider has produced syntax, semantic, diagnostic, search, or Markdown decoration spans for the current document viewport. Do not use it for preview panels; use `clay:sdui` for package UI.

## JavaScript usage

```ts
import { serverPublishDecorations } from "clay:decorations";

serverPublishDecorations({
  packageName: "@clay/markdown",
  packageVersion: "0.1.0",
  packagePrefix: "markdown",
  permissions: ["render-decorations"],
  documentId,
  documentVersion,
  viewport: { byteStart, byteEnd },
  spans: [{
    byteStart,
    byteEnd,
    kind: "syntax",
    styleToken: "markup.heading.1",
    priority: 10,
  }],
});
```

## Example

```ts
serverPublishDecorations({
  packageName: "@clay/markdown",
  packagePrefix: "markdown",
  permissions: ["render-decorations"],
  documentId: 1,
  documentVersion: 4,
  viewport: { byteStart: 0, byteEnd: 80 },
  spans: [{ byteStart: 0, byteEnd: 6, kind: "syntax", styleToken: "markup.heading.1" }],
});
```

## Options

- `packageManifest` or package context fields: package identity and declared `render-decorations` permission.
- `documentId` (`number`, required): Target document.
- `documentVersion` (`number`, required): Version used by the parser.
- `currentDocumentVersion` (`number`, optional): Validation override used by tests/server integration; defaults to `documentVersion`.
- `viewportByteRange` / `viewport` (`{ byteStart: number; byteEnd: number }`, required): Viewport byte range.
- `spans` (`DecorationSpan[]`, required): Known inert span records.

Known span kinds are `syntax`, `semantic`, `diagnostic`, and `search-match`. Style tokens are bounded Clay tokens such as `markup.heading.1`, `markup.strong`, `markup.emphasis`, `markup.inline-code`, `markup.code-block`, `markup.list-marker`, `keyword.control`, `string.quoted`, `comment.line`, `punctuation.definition`, `diagnostic.error`, and `search.match`.

## Key bindings

No default key binding is assigned.

## Custom properties

- `documentId`: target open document.
- `documentVersion`: stale-version guard.
- `viewportByteRange`: publication range and IPC bound.
- `spans`: bounded inert decorations.
- `packagePrefix`: provenance and conflict identity.

## Return and async behavior

Returns JSON-serializable publication metadata synchronously from the server runtime. The operation is intended for background parse/render work and must not be called from ordinary typing, paint, layout, scroll, pointer, or text-event paths.

## Errors

Fails with Clay error codes when permissions are missing, options are malformed, the document version is stale, ranges are invalid or outside the viewport, style tokens/kinds are unknown, package provenance mismatches, executable payloads are attempted, or payload size exceeds `DECORATION_PAYLOAD_BUDGET_BYTES`.

## Permissions and security

Requires: `render-decorations`.

The API accepts inert data only. It rejects arbitrary CSS, callbacks, draw functions, raw `Deno.core.ops`, client-side JavaScript hooks, and unknown native rendering authority.

## Agent guidance

Prefer this facade over raw ops or Rust internals. Keep publications viewport-bounded and use known style tokens. If a user asks for Markdown preview UI, use SDUI rather than decoration spans.

## Backing implementation

- JS facade: `runtime/js/decorations.ts::serverPublishDecorations`
- Runtime facade: `src/server/js_runtime.rs::CLAY_FACADE_DECORATIONS`
- Op wrapper: `src/server/ops/decorations.rs::op_clay_decorations_publish_decorations`
- Validator: `src/server/decorations.rs::validate_decoration_publication`
- Protocol type: `src/protocol/decorations.rs::DecorationSet`

## Lookup metadata

- Stable ID: `clay.decorations.serverPublishDecorations`
- User-facing name: Publish Decorations
- Lookup tags: `js-api`, `decorations`, `markdown`, `decorationrange`, `syntax-highlighting`
- App/help visible: true
