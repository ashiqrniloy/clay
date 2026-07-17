---
id: clay.editor.clientPasteClipboard
kind: clay-js-api
js_module: "clay:editor"
js_export: clientPasteClipboard
js_facade: runtime/js/editor.ts::clientPasteClipboard
backing_rust: src/masonry_editor.rs::EditorWidget::paste_from_system_clipboard; src/client/clipboard.rs::SystemClipboard; src/editor/surface.rs::EditorSurface::paste_text_with_event
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientPasteClipboard
user_facing_name: Paste Clipboard
summary: Return the stable bindable command ID for pasting OS clipboard UTF-8 text into the native editor as an ordinary local edit.
owner: client
phase: Phase 20
visibility: public
permissions: []
key_bindings: ["Ctrl+V", "Cmd+V"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it reads OS clipboard text once and inserts or replaces through the ordinary local edit path, and this API does not grant filesystem/workspace authority, package/configuration/AI clipboard-contents inspection APIs, arbitrary clipboard writes, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientPasteClipboard` only as a documented command ID for `bindKey`; do not expose raw clipboard text return values, server/package clipboard access, raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, clipboard, paste, selection, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientPasteClipboard

## Summary

Return the stable bindable command ID for pasting OS clipboard UTF-8 text into the native editor as an ordinary local edit.

## Description

`clientPasteClipboard` is the public Clay JS API descriptor for **Paste Clipboard**. It returns the stable command ID `clay.editor.clientPasteClipboard` so configuration, help, key-binding discovery, and agents can name the paste route without hard-coding Rust shortcuts or raw clipboard operations.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Paste happens later only after an explicit user key/command route reaches the native editor widget. The command reads OS clipboard text through the client-owned clipboard sink, normalizes line endings, and inserts at the caret or replaces the current selection through the ordinary local edit enqueue path. Empty clipboard text is a no-op. Clipboard read failures become sanitized runtime diagnostics that never include full clipboard contents.

## When to use

Use this API when a user wants to bind an alternate paste chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientPasteClipboard } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+V", clientPasteClipboard(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+V", "clay.editor.clientPasteClipboard", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientPasteClipboard } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+V", clientPasteClipboard(), { scope: "editor" });
```

Native `Ctrl+V` on Linux/Windows and `Cmd+V` on macOS are handled directly by the editor. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. Clipboard source, primary selection, rich text, HTML, image data, and clipboard-contents return values are not configurable through this API.

## Key bindings

Native default shortcuts: `Ctrl+V` on Linux/Windows and `Cmd+V` on macOS. Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"clay.editor.clientPasteClipboard"` synchronously. The helper does not touch the clipboard, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, unsupported `when` clauses, or undocumented command IDs. The native command path can report a sanitized `clay.client.clipboard.read_failed` diagnostic if the OS clipboard backend fails.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it reads OS clipboard text once and inserts or replaces through the ordinary local edit path, and this API does not grant filesystem/workspace authority, package/configuration/AI clipboard-contents inspection APIs, arbitrary clipboard writes, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Clipboard reads happen only on the explicit paste command path and stay off Masonry paint/layout/scroll and ordinary key-insertion work. Paste does not wait on IPC acknowledgement before applying the local edit. Diagnostics never include full clipboard contents. Broader package/configuration/AI clipboard authority remains deferred.

## Agent guidance

Use `clay.editor.clientPasteClipboard` only as a documented command ID for `bindKey`. Avoid raw clipboard APIs that return clipboard text to JavaScript, server/package clipboard authority, shell commands, network effects, WASM, AI mutation, raw ops, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientPasteClipboard`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_editor.rs::EditorWidget::paste_from_system_clipboard`; `src/client/clipboard.rs::SystemClipboard`; `src/editor/surface.rs::EditorSurface::paste_text_with_event`

## Lookup metadata

- Stable ID: `clay.editor.clientPasteClipboard`
- User-facing name: Paste Clipboard
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientPasteClipboard`
- Default key bindings: `Ctrl+V`, `Cmd+V`
- Custom properties: none
- Tags: `[editor, clipboard, paste, selection, keybindings, js-api]`
