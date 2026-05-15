---
id: clay.editor.serverInsertNewline
kind: clay-js-api
js_module: "clay:editor"
js_export: serverInsertNewline
js_facade: runtime/js/editor.ts::serverInsertNewline
backing_rust: src/editor/surface.rs::EditorSurface::insert_newline_with_event; src/server/document.rs::DocumentState::apply_edit
deno_op: op_clay_editor_insert_newline
deno_op_path: src/server/ops/editor.rs::op_clay_editor_insert_newline
name: serverInsertNewline
user_facing_name: Insert Newline
summary: Insert Newline through the planned `clay:editor` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: [document-edit]
key_bindings: [Enter]
custom_properties:
  - name: enterRule
    type: manifest
    default: PreserveLeadingWhitespace
    description: Behavior-changing setting `enterRule` for this API.
  - name: commentContinuation
    type: manifest
    default: //
    description: Behavior-changing setting `commentContinuation` for this API.
security: Uses inert behavior manifest rules for hot-path newline shaping and still requires document edit authority; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.serverInsertNewline` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, newline]
app_visible: true
help_visible: true
stability: planned
async: true
---

# serverInsertNewline

## Summary

Insert Newline through the planned `clay:editor` Clay JavaScript facade.

## Description

`serverInsertNewline` is the planned public API for **Insert Newline**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `server-authoritative-document-mutation-with-behavior-context`. Runtime path: `client-first-predictable-hot-path-and-server-ack`. Enter is routed locally through the active behavior manifest for indentation/comment continuation, then emitted asynchronously as an edit transaction.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Insert Newline` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverInsertNewline } from "clay:editor";

await serverInsertNewline({ documentId: "current", offset: 12 });
```

## Example

```ts
await serverInsertNewline({ documentId: "current", offset: 12 });
```

## Options

- `documentId` (`string`): Target document identifier.
- `offset` (`number`): Insertion offset for the newline.
- `behaviorContext` (`object`): Optional future context used by inert manifest rules such as leading-whitespace preservation.

## Key bindings

Default key bindings:

- `Enter`

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

- `enterRule` (`manifest`, default `PreserveLeadingWhitespace`): Behavior-changing setting `enterRule` for this API.
- `commentContinuation` (`manifest`, default `//`): Behavior-changing setting `commentContinuation` for this API.

## Return and async behavior

Returns a promise for an edit result after the server accepts or rejects the newline edit.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

Requires: `document-edit`.

Uses inert behavior manifest rules for hot-path newline shaping and still requires document edit authority; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.serverInsertNewline` when the user asks for insert newline through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::serverInsertNewline`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_insert_newline` (`op_clay_editor_insert_newline`)
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::insert_newline_with_event; src/server/document.rs::DocumentState::apply_edit`
- Current implementation audit path: `src/editor/surface.rs::newline_text_at; src/client/behavior.rs::ClientBehaviorState::route_key`

## Lookup metadata

- Stable ID: `clay.editor.serverInsertNewline`
- User-facing name: Insert Newline
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `serverInsertNewline`
- Default key bindings: `Enter`
- Custom properties: `enterRule`, `commentContinuation`
- Tags: `editor`, `js-api`, `newline`
