---
id: folding.serverPublishFoldingRanges
kind: clay-js-api
js_module: "clay:folding"
js_export: serverPublishFoldingRanges
js_facade: runtime/js/folding.js::serverPublishFoldingRanges
backing_rust: src/server/folding.rs::FoldingRangeRegistry::publish_ranges
deno_op: op_clay_folding_publish_ranges
deno_op_path: src/server/ops/folding.rs::op_clay_folding_publish_ranges
name: serverPublishFoldingRanges
user_facing_name: Publish Folding Ranges
summary: Publish validated, inert folding ranges from server-side package code.
owner: server
phase: Phase 28
visibility: public
permissions: ['render-folding']
key_bindings: []
custom_properties:
  - name: documentId
    type: number
    default: required
    description: Target open document ID.
  - name: documentVersion
    type: number
    default: required
    description: Server document version the ranges were produced for.
  - name: ranges
    type: FoldingRange[]
    default: []
    description: Bounded inert byte ranges with optional labels.
  - name: packagePrefix
    type: string
    default: package context
    description: Package API prefix retained as provenance.
security: Requires render-folding permission plus server validation of package provenance, current document version, byte ranges, nesting, and FOLDING_RANGE_PAYLOAD_BUDGET_BYTES; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, arbitrary GPU draw calls, or native widget mutation authority.
agent_guidance: Use `folding.serverPublishFoldingRanges` from server-side package code only. Publish inert ranges; never invent renderer callbacks or client-side JavaScript hooks. Collapse state is client-local.
lookup_tags: [js-api, folding, foldingrange]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverPublishFoldingRanges

## Summary

Publishes validated `FoldingRange` data for an open document version. Clay paints fold chevrons and hides collapsed interiors locally in Rust; package JavaScript never runs in paint, layout, keypress, scroll, or text-event handlers.

## Description

`serverPublishFoldingRanges` is the public Clay JS API for the `FoldingRange` primitive. It accepts a bounded set of byte ranges from server-side package code, validates the package permission and provenance, checks version/range/nesting safety, enforces `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`, and stores the validated set for server-to-client delivery. Stale document versions drop without mutating stored ranges. Oversize publications deny rather than truncate.

Clay also computes generic core folds from accepted syntax trees (named multiline nodes). Package ranges merge by provenance with those core folds.

## When to use

Use this API when a package has extra fold ranges that the generic tree walk does not cover. Do not use it to hide lines from paint by itself; publish ranges and let Clay own collapse.

## JavaScript usage

```ts
import { serverPublishFoldingRanges } from "clay:folding";

await serverPublishFoldingRanges({
  documentId,
  documentVersion,
  packagePrefix: "markdown",
  ranges: [{ byteStart: 0, byteEnd: 24, label: "section" }],
});
```

## Example

```ts
serverPublishFoldingRanges({
  documentId: 1,
  documentVersion: 4,
  ranges: [{ byteStart: 0, byteEnd: 80 }],
});
```

## Options

- `documentId` (`number`, required): Target document.
- `documentVersion` (`number`, required): Version used by the producer.
- `currentDocumentVersion` (`number`, optional): Validation override; defaults to `documentVersion`. A mismatch drops the publication.
- `ranges` (`FoldingRange[]`, required): Ordered, properly nested inert ranges.
- `packagePrefix` (`string`, optional): Ignored if supplied; provenance comes from the executing package.

## Key bindings

No default key binding is assigned. `editor.clientToggleFold` toggles the range containing the caret.

## Custom properties

- `documentId`: target open document.
- `documentVersion`: stale-version guard.
- `ranges`: bounded inert folds.
- `packagePrefix`: provenance and merge identity.

## Return and async behavior

Returns JSON-serializable publication metadata synchronously from the server runtime. Stale versions return `{ dropped: true }` without an error. The operation is intended for background parse/render work and must not be called from ordinary typing, paint, layout, scroll, pointer, or text-event paths.

## Errors

Fails with Clay error codes when permissions are missing, options are malformed, ranges are invalid or improperly nested, package provenance mismatches, or payload size exceeds `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`.

## Permissions and security

Requires: `render-folding`.

The API accepts inert data only. Collapse state is not a grant and does not leak across documents. Core tree folds are Clay-owned and do not require this permission.

## Agent guidance

Prefer this facade over raw ops. Keep ranges ordered and nested. Do not add language-specific fold queries in Rust.

## Backing implementation

- JS facade: `runtime/js/folding.js::serverPublishFoldingRanges`
- Runtime facade: `src/server/facades.rs`
- Op wrapper: `src/server/ops/folding.rs::op_clay_folding_publish_ranges`
- Validator: `src/server/folding.rs::validate_folding_publication`
- Protocol type: `src/protocol/folding.rs::FoldingRangeSet`

## Lookup metadata

- Stable ID: `folding.serverPublishFoldingRanges`
- User-facing name: Publish Folding Ranges
- Lookup tags: `js-api`, `folding`, `foldingrange`
- App/help visible: true
