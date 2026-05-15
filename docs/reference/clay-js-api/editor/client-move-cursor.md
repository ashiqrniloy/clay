---
id: clay.editor.clientMoveCursor
kind: clay-js-api
js_module: "clay:editor"
js_export: clientMoveCursor
js_facade: runtime/js/editor.ts::clientMoveCursor
backing_rust: src/editor/surface.rs::EditorSurface::move_left
deno_op: op_clay_editor_move_cursor
deno_op_path: src/server/ops/editor.rs::op_clay_editor_move_cursor
name: clientMoveCursor
user_facing_name: Move Cursor
summary: Move Cursor through the planned `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: [ArrowLeft, ArrowRight, ArrowUp, ArrowDown, Home, End, Ctrl+Home, Ctrl+End]
custom_properties:
  - name: direction
    type: enum
    default: none
    description: Behavior-changing setting `direction` for this API.
  - name: extendSelection
    type: boolean
    default: false
    description: Behavior-changing setting `extendSelection` for this API.
security: Changes only client-local caret/selection/viewport state and grants no document mutation or external authority; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientMoveCursor` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [cursormovement, editor, js-api]
app_visible: true
help_visible: true
stability: planned
async: false
---

# clientMoveCursor

## Summary

Move Cursor through the planned `clay:editor` Clay JavaScript facade.

## Description

`clientMoveCursor` is the planned public API for **Move Cursor**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `client-local-ui-state`. Runtime path: `client-local-hot-path`. Arrow/Home/End movement updates local caret/viewport state without IPC, server work, or JavaScript.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Move Cursor` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientMoveCursor } from "clay:editor";

clientMoveCursor({ documentId: "current", direction: "right" });
```

## Example

```ts
clientMoveCursor({ documentId: "current", direction: "right" });
```

## Options

- `documentId` (`string`): Target editor/document surface.
- `direction` (`"left" | "right" | "up" | "down" | "start" | "end"`): Movement direction.
- `extendSelection` (`boolean`): Whether movement extends the current selection; defaults to `false`.

## Key bindings

Default key bindings:

- `ArrowLeft`
- `ArrowRight`
- `ArrowUp`
- `ArrowDown`
- `Home`
- `End`
- `Ctrl+Home`
- `Ctrl+End`

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

- `direction` (`enum`, default `none`): Behavior-changing setting `direction` for this API.
- `extendSelection` (`boolean`, default `false`): Behavior-changing setting `extendSelection` for this API.

## Return and async behavior

Returns client-local cursor state when runtime wiring exists; the planned facade is synchronous and local.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes only client-local caret/selection/viewport state and grants no document mutation or external authority; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.clientMoveCursor` when the user asks for move cursor through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientMoveCursor`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_move_cursor` (`op_clay_editor_move_cursor`)
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::move_left`
- Current implementation audit path: `src/editor/cursor.rs::CursorState; src/editor/surface.rs::EditorSurface::command_with_event`

## Lookup metadata

- Stable ID: `clay.editor.clientMoveCursor`
- User-facing name: Move Cursor
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientMoveCursor`
- Default key bindings: `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`, `Home`, `End`, `Ctrl+Home`, `Ctrl+End`
- Custom properties: `direction`, `extendSelection`
- Tags: `cursormovement`, `editor`, `js-api`
