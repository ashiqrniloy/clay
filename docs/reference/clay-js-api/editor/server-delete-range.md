---
id: clay.editor.serverDeleteRange
kind: clay-js-api
js_module: "clay:editor"
js_export: serverDeleteRange
js_facade: runtime/js/editor.ts::serverDeleteRange
backing_rust: src/server/document.rs::DocumentState::apply_edit
deno_op: op_clay_editor_delete_range
deno_op_path: src/server/ops/editor.rs::op_clay_editor_delete_range
name: serverDeleteRange
user_facing_name: Delete Text Range
summary: Delete Text Range through the planned `clay:editor` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: [document-edit]
key_bindings: [Backspace, Delete]
custom_properties: []
security: Requires document edit authority, valid byte/scalar boundaries, and an editable lease; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.serverDeleteRange` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [backspacedelete, editor, js-api]
app_visible: true
help_visible: true
stability: planned
async: true
---

# serverDeleteRange

## Summary

Delete Text Range through the planned `clay:editor` Clay JavaScript facade.

## Description

`serverDeleteRange` is the planned public API for **Delete Text Range**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `server-authoritative-document-mutation`. Runtime path: `client-first-predictable-hot-path-and-server-ack`. Backspace/Delete are applied locally for editable documents and emitted asynchronously when the manifest allows delete/replace operations.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Delete Text Range` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverDeleteRange } from "clay:editor";

await serverDeleteRange({ documentId: "current", start: 4, end: 9 });
```

## Example

```ts
await serverDeleteRange({ documentId: "current", start: 4, end: 9 });
```

## Options

- `documentId` (`string`): Target document identifier.
- `start` (`number`): Start offset of the range to delete.
- `end` (`number`): Exclusive end offset of the range to delete.

## Key bindings

Default key bindings:

- `Backspace`
- `Delete`

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for an edit result after the server accepts or rejects the deletion.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

Requires: `document-edit`.

Requires document edit authority, valid byte/scalar boundaries, and an editable lease; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.serverDeleteRange` when the user asks for delete text range through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::serverDeleteRange`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_delete_range` (`op_clay_editor_delete_range`)
- Backing Rust/current owner: `src/server/document.rs::DocumentState::apply_edit`
- Current implementation audit path: `src/editor/surface.rs::EditorSurface::backspace_with_event; src/editor/surface.rs::EditorSurface::delete_forward_with_event; src/server/document.rs::DocumentState::apply_edit`

## Lookup metadata

- Stable ID: `clay.editor.serverDeleteRange`
- User-facing name: Delete Text Range
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `serverDeleteRange`
- Default key bindings: `Backspace`, `Delete`
- Custom properties: none
- Tags: `backspacedelete`, `editor`, `js-api`
