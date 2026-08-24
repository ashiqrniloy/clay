---
id: editor.toggleListMarker
kind: clay-js-api
js_module: "clay:editor"
js_export: toggleListMarker
js_facade: runtime/js/editor.js::toggleListMarker
backing_rust: src/client_commands.rs::EditorClientCommand
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: toggleListMarker
user_facing_name: Toggle List Marker
summary: Return the stable command ID for the manifest-driven list-marker toggle.
owner: client
phase: Phase 28.2
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client-first editor command ID only; applies inert line-prefix changes locally after explicit user routing and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.toggleListMarker` through `bindKey`; list markers come from the active behavior manifest. Do not add Markdown-specific Rust branches or raw Deno ops.
lookup_tags: [editor, list, text-transform, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# toggleListMarker

## Summary

Returns the stable command ID `editor.toggleListMarker` for toggling the first configured list marker on active editor lines.

## Description

The facade only returns a command ID. Once routed, Clay reads `EnterRule::ContinueLineMarkers` from the active behavior manifest and applies the generic line-prefix transform to every line touched by the caret or selection. The transform uses the manifest's first marker and preserves ordinary local history/selection remapping.

## When to use

Use this API to bind a list-marker command for a mode that declares continuation markers. Markdown's package-owned alias `markdown.toggleList` uses the same client transform when that package is loaded.

## JavaScript usage

```ts
import { toggleListMarker } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+8", toggleListMarker(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+8", "editor.toggleListMarker", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { toggleListMarker } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+8", toggleListMarker(), { scope: "editor" });
```

## Options

No options are accepted. Marker vocabulary and indentation behavior come from the active mode manifest.

## Key bindings

No core default key binding is assigned. A package alias may provide a mode-specific default; `bindKey` can assign any additional editor chord.

## Custom properties

No behavior-changing custom properties are defined.

## Return and async behavior

Returns `"editor.toggleListMarker"` synchronously. Calling the helper does not mutate a document, call the server, execute package code, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` rejects malformed chords, unsupported scopes, or unknown command IDs. A mode without continuation markers reports a sanitized no-op diagnostic instead of inventing a marker.

## Permissions and security

No additional permission is required. The routed operation is client-local ordinary editor behavior using inert manifest data. It does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Prefer this core command over package-specific transform logic. Configure marker behavior in the mode manifest; do not add callbacks, executable transform fields, or language-specific Rust branches.

## Backing implementation

- JS facade: `runtime/js/editor.js::toggleListMarker`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key`
- Command mapping: `src/client_commands.rs::EditorClientCommand`
- Local transform: `src/client_commands.rs::EditorClientCommand (client-local; executed by the React/CodeMirror controller, frontend/src/editor/extensions/controller.ts)` and `src/client_commands.rs::EditorClientCommand`

## Lookup metadata

- Stable ID: `editor.toggleListMarker`
- User-facing name: Toggle List Marker
- Module/export: `clay:editor` / `toggleListMarker`
- Default key bindings: none
- Custom properties: none
- Tags: `[editor, list, text-transform, keybindings, js-api]`
