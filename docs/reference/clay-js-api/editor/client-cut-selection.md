---
id: clay.editor.clientCutSelection
kind: clay-js-api
js_module: "clay:editor"
js_export: clientCutSelection
js_facade: runtime/js/editor.js::clientCutSelection
backing_rust: src/masonry_editor.rs::EditorWidget::cut_selection_to_system_clipboard; src/client/clipboard.rs::SystemClipboard; src/editor/surface.rs::EditorSurface::selected_text
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientCutSelection
user_facing_name: Cut Selection
summary: Return the stable bindable command ID for cutting the current native editor selection to the OS clipboard and deleting it as an ordinary local edit.
owner: client
phase: Phase 20
visibility: public
permissions: []
key_bindings: ["Ctrl+X", "Cmd+X"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it writes only the current non-empty native editor selection to the OS clipboard and then deletes that selection through the ordinary local edit path, and this API does not grant filesystem/workspace authority, arbitrary clipboard text writes, clipboard inspection APIs for packages/configuration/AI, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientCutSelection` only as a documented command ID for `bindKey`; do not expose raw clipboard text APIs, server/package clipboard access, raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, clipboard, cut, selection, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientCutSelection

## Summary

Return the stable bindable command ID for cutting the current native editor selection to the OS clipboard and deleting it as an ordinary local edit.

## Description

`clientCutSelection` is the public Clay JS API descriptor for **Cut Selection**. It returns the stable command ID `clay.editor.clientCutSelection` so configuration, help, key-binding discovery, and agents can name the cut-selection route without hard-coding Rust shortcuts or raw clipboard operations.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Cut happens later only after an explicit user key/command route reaches the native editor widget. The command reads only `EditorSurface::selected_text()`, writes that text to the OS clipboard, and then deletes the selection through the ordinary local edit enqueue path. Collapsed selections are a no-op. If the clipboard write fails, the selection is not deleted and a sanitized runtime diagnostic is reported.

## When to use

Use this API when a user wants to bind an alternate cut chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientCutSelection } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+X", clientCutSelection(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+X", "clay.editor.clientCutSelection", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientCutSelection } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+X", clientCutSelection(), { scope: "editor" });
```

Native `Ctrl+X` on Linux/Windows and `Cmd+X` on macOS are handled directly by the editor. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. Clipboard target, primary selection, rich text, HTML, image data, and arbitrary text writes are not configurable through this API.

## Key bindings

Native default shortcuts: `Ctrl+X` on Linux/Windows and `Cmd+X` on macOS. Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"clay.editor.clientCutSelection"` synchronously. The helper does not touch the clipboard, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, unsupported `when` clauses, or undocumented command IDs. The native command path can report a sanitized `clay.client.clipboard.write_failed` diagnostic if the OS clipboard backend fails before deletion.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it writes only the current non-empty native editor selection to the OS clipboard and then deletes that selection through the ordinary local edit path, and this API does not grant filesystem/workspace authority, arbitrary clipboard text writes, clipboard inspection APIs for packages/configuration/AI, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Cut selection is client-local UI work. Clipboard writes stay off server command execution, workspace APIs, filesystem paths, package loading, JS evaluation, Masonry paint/layout, and ordinary key-insertion paths. Deletion reuses the ordinary optimistic local edit path.

## Agent guidance

Use `clay.editor.clientCutSelection` only as a documented command ID for `bindKey`. Avoid raw clipboard APIs, arbitrary strings, server/package clipboard authority, shell commands, network effects, WASM, AI mutation, raw ops, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientCutSelection`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_editor.rs::EditorWidget::cut_selection_to_system_clipboard`; `src/client/clipboard.rs::SystemClipboard`; `src/editor/surface.rs::EditorSurface::selected_text`

## Lookup metadata

- Stable ID: `clay.editor.clientCutSelection`
- User-facing name: Cut Selection
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientCutSelection`
- Default key bindings: `Ctrl+X`, `Cmd+X`
- Custom properties: none
- Tags: `[editor, clipboard, cut, selection, keybindings, js-api]`
