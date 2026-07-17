---
id: clay.editor.clientUndo
kind: clay-js-api
js_module: "clay:editor"
js_export: clientUndo
js_facade: runtime/js/editor.ts::clientUndo
backing_rust: src/masonry_editor.rs::EditorWidget::undo; src/editor/surface.rs::EditorSurface::undo_with_event; src/editor/history.rs::EditHistory
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientUndo
user_facing_name: Undo
summary: Return the stable bindable command ID for undoing the latest local edit on the active editable document as an ordinary inverse edit.
owner: client
phase: Phase 20
visibility: public
permissions: []
key_bindings: ["Ctrl+Z", "Cmd+Z"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it applies a client-local ordinary inverse edit under the editable lease through the ordinary local edit path, and this API does not grant filesystem/workspace authority, package/configuration/AI history mutation APIs, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientUndo` only as a documented command ID for `bindKey`; do not expose raw history stacks, server undo protocols, raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, undo, history, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientUndo

## Summary

Return the stable bindable command ID for undoing the latest local edit on the active editable document as an ordinary inverse edit.

## Description

`clientUndo` is the public Clay JS API descriptor for **Undo**. It returns the stable command ID `clay.editor.clientUndo` so configuration, help, key-binding discovery, and agents can name the undo route without hard-coding Rust shortcuts or inventing a server undo protocol.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Undo happens later only after an explicit user key/command route reaches the native editor widget. The command pops the per-document client history stack, applies the inverse insert/delete/replace locally, restores caret/selection, and enqueues a normal optimistic `Edit` under the editable lease. Read-only observers are a no-op. Empty undo stacks are a no-op. Rejected inverse edits recover through the existing resync path.

## When to use

Use this API when a user wants to bind an alternate undo chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientUndo } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+Z", clientUndo(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Alt+Backspace", "clay.editor.clientUndo", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientUndo } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Alt+Backspace", clientUndo(), { scope: "editor" });
```

Native `Ctrl+Z` on Linux/Windows and `Cmd+Z` on macOS are handled directly by the editor. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. Undo depth ceilings, coalescing, and multi-document history retention are not configurable through this API in Phase 20.

## Key bindings

Native default shortcuts: `Ctrl+Z` on Linux/Windows and `Cmd+Z` on macOS. Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"clay.editor.clientUndo"` synchronously. The helper does not mutate history, call the server, execute package code, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, unsupported `when` clauses, or undocumented command IDs. Native undo against an empty stack or read-only observer is a silent no-op.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it applies a client-local ordinary inverse edit under the editable lease, and this API does not grant filesystem/workspace authority, package/configuration/AI history mutation APIs, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Undo applies locally first and enqueues through the existing bounded edit queue. It does not invent a server undo protocol and cannot bypass leases, region locks, or server validation.

## Agent guidance

Use `clay.editor.clientUndo` only as a documented command ID for `bindKey`. Avoid raw history inspection APIs, server undo protocols, shell commands, network effects, WASM, AI mutation, raw ops, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientUndo`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_editor.rs::EditorWidget::undo`; `src/editor/surface.rs::EditorSurface::undo_with_event`; `src/editor/history.rs::EditHistory`

## Lookup metadata

- Stable ID: `clay.editor.clientUndo`
- User-facing name: Undo
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientUndo`
- Default key bindings: `Ctrl+Z`, `Cmd+Z`
- Custom properties: none
- Tags: `[editor, undo, history, keybindings, js-api]`
