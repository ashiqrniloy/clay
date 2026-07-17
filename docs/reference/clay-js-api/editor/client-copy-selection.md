---
id: clay.editor.clientCopySelection
kind: clay-js-api
js_module: "clay:editor"
js_export: clientCopySelection
js_facade: runtime/js/editor.ts::clientCopySelection
backing_rust: src/masonry_editor.rs::EditorWidget::copy_selection_to_system_clipboard; src/client/clipboard.rs::SystemClipboard; src/editor/surface.rs::EditorSurface::selected_text
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientCopySelection
user_facing_name: Copy Selection
summary: Return the stable bindable command ID for copying the current native editor selection to the OS clipboard.
owner: client
phase: Phase 19
visibility: public
permissions: []
key_bindings: ["Ctrl+C", "Cmd+C"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it writes only the current non-empty native editor selection to the OS clipboard, and this API does not grant filesystem/workspace authority, arbitrary clipboard text writes, package/configuration/AI clipboard-contents APIs, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority. Cut and paste are separate documented command IDs.
agent_guidance: Use `clay.editor.clientCopySelection` only as a documented command ID for `bindKey`; do not expose raw clipboard text APIs, server/package clipboard access, raw Rust calls, protocol DTOs, or `Deno.core.ops`. Prefer `clientCutSelection` / `clientPasteClipboard` for cut/paste chords.
lookup_tags: [editor, clipboard, copy, selection, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientCopySelection

## Summary

Return the stable bindable command ID for copying the current native editor selection to the OS clipboard.

## Description

`clientCopySelection` is the public Clay JS API descriptor for **Copy Selection**. It returns the stable command ID `clay.editor.clientCopySelection` so configuration, help, key-binding discovery, and agents can name the copy-selection route without hard-coding Rust shortcuts or raw clipboard operations.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Clipboard writing happens later only after an explicit user key/command route reaches the native editor widget. The command reads only `EditorSurface::selected_text()` and writes only that text to the OS clipboard; collapsed selections are a no-op.

## When to use

Use this API when a user wants to bind an alternate copy chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientCopySelection } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+C", "clay.editor.clientCopySelection", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientCopySelection } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
```

Native `Ctrl+C` on Linux/Windows and `Cmd+C` on macOS are handled directly by the editor. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. Clipboard target, clipboard readback, primary selection, rich text, HTML, image data, and arbitrary text writes are not configurable through this API. Cut and paste use separate command IDs.

## Key bindings

Native default shortcuts: `Ctrl+C` on Linux/Windows and `Cmd+C` on macOS. Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"clay.editor.clientCopySelection"` synchronously. The helper does not touch the clipboard, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, unsupported `when` clauses, or undocumented command IDs. The native command path can report a sanitized `clay.client.clipboard.write_failed` diagnostic if the OS clipboard backend fails.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it writes only the current non-empty native editor selection to the OS clipboard, and this API does not grant filesystem/workspace authority, arbitrary clipboard text writes, package/configuration/AI clipboard-contents APIs, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority. Cut and paste are separate documented command IDs.

The server, packages, and configuration JavaScript cannot read clipboard contents or set arbitrary clipboard text. Copy selection is client-local UI work and stays off server command execution, workspace APIs, filesystem paths, package loading, JS evaluation, Masonry paint/layout, and ordinary edit IPC.

## Agent guidance

Use `clay.editor.clientCopySelection` only as a documented command ID for `bindKey`. Avoid raw clipboard APIs, arbitrary strings, server/package clipboard authority, shell commands, network effects, WASM, AI mutation, raw ops, or client-side JavaScript execution. Use `clientCutSelection` and `clientPasteClipboard` for cut/paste.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientCopySelection`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_editor.rs::EditorWidget::copy_selection_to_system_clipboard`; `src/client/clipboard.rs::SystemClipboard`; `src/editor/surface.rs::EditorSurface::selected_text`

## Lookup metadata

- Stable ID: `clay.editor.clientCopySelection`
- User-facing name: Copy Selection
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientCopySelection`
- Default key bindings: `Ctrl+C`, `Cmd+C`
- Custom properties: none
- Tags: `[editor, clipboard, copy, selection, keybindings, js-api]`
