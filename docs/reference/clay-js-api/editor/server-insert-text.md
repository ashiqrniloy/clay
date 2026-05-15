---
id: clay.editor.serverInsertText
kind: clay-js-api
js_module: "clay:editor"
js_export: serverInsertText
js_facade: runtime/js/editor.ts::serverInsertText
backing_rust: src/server/document.rs::DocumentState::apply_edit
deno_op: op_clay_editor_insert_text
deno_op_path: src/server/ops/editor.rs::op_clay_editor_insert_text
name: serverInsertText
user_facing_name: Insert Text
summary: Insert Text through the planned `clay:editor` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: [document-edit]
key_bindings: []
custom_properties:
  - name: normalizeLineEndings
    type: boolean
    default: true
    description: Behavior-changing setting `normalizeLineEndings` for this API.
security: Requires edit authority for the target document and a valid lease once runtime permission checks exist; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.serverInsertText` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, textinsertion]
app_visible: true
help_visible: true
stability: planned
async: true
---

# serverInsertText

## Summary

Insert Text through the planned `clay:editor` Clay JavaScript facade.

## Description

`serverInsertText` is the planned public API for **Insert Text**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `server-authoritative-document-mutation`. Runtime path: `server-first-op-wrapper`. Ordinary typed characters are currently client-first predictable through the behavior manifest and are enqueued asynchronously; this API is the future authoritative programmatic mutation path.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Insert Text` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverInsertText } from "clay:editor";

await serverInsertText({ documentId: "current", offset: 0, text: "hello" });
```

## Example

```ts
await serverInsertText({ documentId: "current", offset: 0, text: "hello" });
```

## Options

- `documentId` (`string`): Target document identifier such as `"current"` once runtime document IDs exist.
- `offset` (`number`): Protocol-defined insertion offset.
- `text` (`string`): Inert text to insert.
- `normalizeLineEndings` (`boolean`): Optional override for Clay line-ending normalization; defaults to `true`.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.editor.serverInsertText` in `~/.config/clay/init.js`.

## Custom properties

- `normalizeLineEndings` (`boolean`, default `true`): Behavior-changing setting `normalizeLineEndings` for this API.

## Return and async behavior

Returns a promise for an edit result after the server accepts or rejects the insert request.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

Requires: `document-edit`.

Requires edit authority for the target document and a valid lease once runtime permission checks exist; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.serverInsertText` when the user asks for insert text through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::serverInsertText`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_insert_text` (`op_clay_editor_insert_text`)
- Backing Rust/current owner: `src/server/document.rs::DocumentState::apply_edit`
- Current implementation audit path: `src/editor/surface.rs::EditorSurface::insert_text_with_event; src/server/document.rs::DocumentState::apply_edit`

## Lookup metadata

- Stable ID: `clay.editor.serverInsertText`
- User-facing name: Insert Text
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `serverInsertText`
- Default key bindings: none
- Custom properties: `normalizeLineEndings`
- Tags: `editor`, `js-api`, `textinsertion`
