---
id: clay.editor.clientRedo
kind: clay-js-api
js_module: "clay:editor"
js_export: clientRedo
js_facade: runtime/js/editor.ts::clientRedo
backing_rust: src/masonry_editor.rs::EditorWidget::redo; src/editor/surface.rs::EditorSurface::redo_with_event; src/editor/history.rs::EditHistory
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientRedo
user_facing_name: Redo
summary: Return the stable bindable command ID for redoing the latest undone local edit on the active editable document as an ordinary edit.
owner: client
phase: Phase 20
visibility: public
permissions: []
key_bindings: ["Ctrl+Shift+Z", "Cmd+Shift+Z", "Ctrl+Y"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it reapplies a client-local ordinary inverse edit under the editable lease through the ordinary local edit path, and this API does not grant filesystem/workspace authority, package/configuration/AI history mutation APIs, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientRedo` only as a documented command ID for `bindKey`; do not expose raw history stacks, server undo protocols, raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, redo, history, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientRedo

## Summary

Return the stable bindable command ID for redoing the latest undone local edit on the active editable document as an ordinary edit.

## Description

`clientRedo` is the public Clay JS API descriptor for **Redo**. It returns the stable command ID `clay.editor.clientRedo` so configuration, help, key-binding discovery, and agents can name the redo route without hard-coding Rust shortcuts or inventing a server undo protocol.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Redo happens later only after an explicit user key/command route reaches the native editor widget. The command pops the per-document client redo stack, reapplies the forward insert/delete/replace locally, restores caret/selection, and enqueues a normal optimistic `Edit` under the editable lease. Read-only observers are a no-op. Empty redo stacks are a no-op. Any new divergent user edit clears the redo stack. Rejected redo edits recover through the existing resync path.

## When to use

Use this API when a user wants to bind an alternate redo chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientRedo } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Y", clientRedo(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Y", "clay.editor.clientRedo", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientRedo } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Y", clientRedo(), { scope: "editor" });
```

Native `Ctrl+Shift+Z` / `Cmd+Shift+Z`, and `Ctrl+Y` on non-macOS platforms, are handled directly by the editor. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. Undo depth ceilings, coalescing, and multi-document history retention are not configurable through this API in Phase 20.

## Key bindings

Native default shortcuts: `Ctrl+Shift+Z` on Linux/Windows, `Cmd+Shift+Z` on macOS, and `Ctrl+Y` on non-macOS platforms. Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"clay.editor.clientRedo"` synchronously. The helper does not mutate history, call the server, execute package code, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, unsupported `when` clauses, or undocumented command IDs. Native redo against an empty stack or read-only observer is a silent no-op.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it applies a client-local ordinary inverse edit under the editable lease, and this API does not grant filesystem/workspace authority, package/configuration/AI history mutation APIs, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Redo applies locally first and enqueues through the existing bounded edit queue. It does not invent a server undo protocol and cannot bypass leases, region locks, or server validation.

## Agent guidance

Use `clay.editor.clientRedo` only as a documented command ID for `bindKey`. Avoid raw history inspection APIs, server undo protocols, shell commands, network effects, WASM, AI mutation, raw ops, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientRedo`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_editor.rs::EditorWidget::redo`; `src/editor/surface.rs::EditorSurface::redo_with_event`; `src/editor/history.rs::EditHistory`

## Lookup metadata

- Stable ID: `clay.editor.clientRedo`
- User-facing name: Redo
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientRedo`
- Default key bindings: `Ctrl+Shift+Z`, `Cmd+Shift+Z`, `Ctrl+Y`
- Custom properties: none
- Tags: `[editor, redo, history, keybindings, js-api]`
