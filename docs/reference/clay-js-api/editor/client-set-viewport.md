---
id: clay.editor.clientSetViewport
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSetViewport
js_facade: runtime/js/editor.ts::clientSetViewport
backing_rust: src/editor/surface.rs::EditorSurface::update_visible_line_count_for_height
deno_op: op_clay_editor_set_viewport
deno_op_path: src/server/ops/editor.rs::op_clay_editor_set_viewport
name: clientSetViewport
user_facing_name: Set Editor Viewport
summary: Set Editor Viewport through the planned `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: visibleLineCount
    type: number
    default: none
    description: Behavior-changing setting `visibleLineCount` for this API.
  - name: overscanLines
    type: number
    default: 4
    description: Behavior-changing setting `overscanLines` for this API.
security: Controls local viewport metadata only and does not expose document contents beyond the visible editor surface; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientSetViewport` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, resizeviewport]
app_visible: true
help_visible: true
stability: planned
async: false
---

# clientSetViewport

## Summary

Set Editor Viewport through the planned `clay:editor` Clay JavaScript facade.

## Description

`clientSetViewport` is the planned public API for **Set Editor Viewport**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `client-local-ui-state`. Runtime path: `client-local-layout-paint`. Resize recomputes visible line count and bounded visible extraction locally in layout/paint, never with full-document IPC.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Set Editor Viewport` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSetViewport } from "clay:editor";

clientSetViewport({ documentId: "current", visibleLineCount: 40, overscanLines: 4 });
```

## Example

```ts
clientSetViewport({ documentId: "current", visibleLineCount: 40, overscanLines: 4 });
```

## Options

- `documentId` (`string`): Target editor/document surface.
- `visibleLineCount` (`number`): Visible line capacity computed from the host viewport.
- `overscanLines` (`number`): Extra lines retained for smooth local paint; default `4`.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.editor.clientSetViewport` in `~/.config/clay/init.js`.

## Custom properties

- `visibleLineCount` (`number`, default `none`): Behavior-changing setting `visibleLineCount` for this API.
- `overscanLines` (`number`, default `4`): Behavior-changing setting `overscanLines` for this API.

## Return and async behavior

Returns client-local viewport state when runtime wiring exists; the planned facade is synchronous and local.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Controls local viewport metadata only and does not expose document contents beyond the visible editor surface; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.clientSetViewport` when the user asks for set editor viewport through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientSetViewport`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_set_viewport` (`op_clay_editor_set_viewport`)
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::update_visible_line_count_for_height`
- Current implementation audit path: `src/editor/viewport.rs::Viewport; src/editor/buffer.rs::EditorBuffer::visible_snapshot`

## Lookup metadata

- Stable ID: `clay.editor.clientSetViewport`
- User-facing name: Set Editor Viewport
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSetViewport`
- Default key bindings: none
- Custom properties: `visibleLineCount`, `overscanLines`
- Tags: `editor`, `js-api`, `resizeviewport`
