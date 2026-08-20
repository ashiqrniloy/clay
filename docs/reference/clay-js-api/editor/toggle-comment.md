---
id: editor.toggleComment
kind: clay-js-api
js_module: "clay:editor"
js_export: toggleComment
js_facade: runtime/js/editor.js::toggleComment
backing_rust: src/masonry_pane_document.rs::PaneDocumentView::apply_editor_client_command; src/editor/surface/mod.rs::EditorSurface::command(EditorCommand::ToggleComment)
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: toggleComment
user_facing_name: Toggle Comment
summary: Return the stable command ID for the manifest-driven line-prefix comment toggle.
owner: client
phase: Phase 28.2
visibility: public
permissions: []
key_bindings: ["Ctrl+/"]
custom_properties: []
security: Bindable client-first editor command ID only; applies line-prefix edits locally after explicit user routing and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.toggleComment` through `bindKey`; comment syntax comes from the active behavior manifest. Do not add language-specific Rust branches, raw edit operations, or raw Deno ops.
lookup_tags: [editor, comment, text-transform, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# toggleComment

## Summary

Returns the stable command ID `editor.toggleComment` for toggling line-prefix comments on the active editor lines.

## Description

The facade is a synchronous command-ID helper. It does not edit text when called. After a key binding or command route reaches the client, Clay reads the active mode's `CommentContinuationRule`, applies an indent-aware add-or-strip transform to lines touched by each caret or selection, and records the result through the normal local edit/history path.

## When to use

Use this API to bind an alternate comment-toggle chord in `~/.config/clay/init.js` or to expose the command to help and agent discovery.

## JavaScript usage

```ts
import { toggleComment } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+C", toggleComment(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+C", "editor.toggleComment", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { toggleComment } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+C", toggleComment(), { scope: "editor" });
```

## Options

No options are accepted. Prefix text is supplied by the active mode manifest; the command never accepts a language name or arbitrary comment syntax.

## Key bindings

Default: `Ctrl+/` in editor text focus. The default can be replaced or supplemented with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined.

## Return and async behavior

Returns the string literal command ID `"editor.toggleComment"` synchronously. Calling the helper does not access the document, call the server, execute package code, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` rejects malformed chords, unsupported scopes, or unknown command IDs. If the active mode has no comment rule, the routed command is a no-op with a sanitized status diagnostic rather than inserting a guessed prefix.

## Permissions and security

No additional permission is required to name or bind this command ID. The routed operation is a client-local ordinary edit under the active editable lease and behavior manifest. It does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Prefer `toggleComment()` over spelling the command ID manually. Reuse manifest comment rules and keep the transform generic; do not add language-specific client/server branches or callback fields.

## Backing implementation

- JS facade: `runtime/js/editor.js::toggleComment`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key`
- Command mapping: `src/masonry_editor.rs::EditorClientCommand::from_command_id`
- Local transform: `src/masonry_pane_document.rs::PaneDocumentView::apply_editor_client_command` and `src/editor/surface/mod.rs::EditorSurface::command`

## Lookup metadata

- Stable ID: `editor.toggleComment`
- User-facing name: Toggle Comment
- Module/export: `clay:editor` / `toggleComment`
- Default key bindings: `Ctrl+/`
- Custom properties: none
- Tags: `[editor, comment, text-transform, keybindings, js-api]`
