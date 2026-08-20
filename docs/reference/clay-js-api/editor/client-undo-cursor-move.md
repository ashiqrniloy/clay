---
id: editor.clientUndoCursorMove
kind: clay-js-api
js_module: "clay:editor"
js_export: clientUndoCursorMove
js_facade: runtime/js/editor.js::clientUndoCursorMove
backing_rust: src/editor/surface/mod.rs::EditorSurface::undo_cursor_move
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientUndoCursorMove
user_facing_name: Undo Cursor Move
summary: Restore the previous selection set from the cursor-undo stack.
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: ["Ctrl+U"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.clientUndoCursorMove` only as a documented command ID for `bindKey`; do not expose raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, js-api, multi-cursor, selection, undo]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientUndoCursorMove

## Summary

Restore the previous selection set from the cursor-undo stack.

## Description

`clientUndoCursorMove` returns the stable bindable command ID `editor.clientUndoCursorMove` for **Undo Cursor Move** (Plan 071 task 9). VSCode cursorUndo semantics: walks the selection-set snapshots taken before each caret-moving command. Cursor movements only; edits have their own undo history. The command is allowlisted, routed `ClientUiCommand`, and dispatched client-local in `EditorWidget`; it is client-local view state and grants no authority.

## When to use

Use this API when JavaScript configuration or extensions need to bind or reference the `Undo Cursor Move` command. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientUndoCursorMove, bindKey } from "clay:editor";

bindKey("ctrl+k ctrl+n", clientUndoCursorMove());
```

## Example

```ts
const commandId = clientUndoCursorMove(); // "editor.clientUndoCursorMove"
```

## Options

None. The facade takes no arguments and returns the stable command ID string.

## Key bindings

Default key bindings: "Ctrl+U".

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

None.

## Return and async behavior

Returns the stable command ID string `editor.clientUndoCursorMove` synchronously. The facade is synchronous, side-effect free, and local.

## Errors

None for the facade itself. `bindKey` rejects command IDs outside the registered allowlist.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientUndoCursorMove` when the user asks for Undo Cursor Move through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientUndoCursorMove`
- Backing Rust/current owner: `src/editor/surface/mod.rs::EditorSurface::undo_cursor_move`
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `editor.clientUndoCursorMove`
- User-facing name: Undo Cursor Move
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientUndoCursorMove`
- Default key bindings: "Ctrl+U"
- Tags: editor,  js-api,  multi-cursor,  selection,  undo
