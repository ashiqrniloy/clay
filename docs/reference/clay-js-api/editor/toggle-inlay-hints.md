---
id: editor.toggleInlayHints
kind: clay-js-api
js_module: "clay:editor"
js_export: toggleInlayHints
js_facade: runtime/js/editor.js::toggleInlayHints
backing_rust: src/client_commands.rs::EditorClientCommand
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: toggleInlayHints
user_facing_name: Toggle Inlay Hints
summary: Return the stable client command ID for showing or hiding inlay-hint overlays.
owner: client
phase: Phase 28.5
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; changes the client-local inlay visibility override for inert decoration data and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.toggleInlayHints` through `bindKey`; inlay data is published by the existing decoration/LSP path and painted by Clay as an overlay. Do not expose LSP clients, decoration handles, or raw ops.
lookup_tags: [editor, inlay-hints, decorations, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# toggleInlayHints

## Summary

Returns the stable command ID `editor.toggleInlayHints` for showing or hiding inlay-hint overlays in the active editor.

## Description

The facade only returns a command ID. After explicit routing, the CodeMirror decorations extension (`frontend/src/editor/extensions/decorations.ts`) flips a client-local visibility override. Inlay labels remain inert decoration data; CodeMirror owns overlay paint and wrapping. Code-mode chrome defaults inlays on, while prose-mode chrome defaults them off, unless the local override is set.

## When to use

Use this API to bind an alternate inlay visibility chord or to expose the command in help and agent discovery. It is not an API for requesting LSP hints or publishing arbitrary overlay text.

## JavaScript usage

```ts
import { toggleInlayHints } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+I", toggleInlayHints(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Alt+I", "editor.toggleInlayHints", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { toggleInlayHints } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+I", toggleInlayHints(), { scope: "editor" });
```

## Options

No options are accepted. The command toggles only the active editor's local visibility override.

## Key bindings

No core default key binding is assigned. The command can be bound with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined.

## Return and async behavior

Returns `"editor.toggleInlayHints"` synchronously. Calling the helper does not request LSP data, call the server, execute package JavaScript, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` rejects malformed chords, unsupported scopes, or unknown command IDs. With no inlay decorations, the routed command still only changes the local visibility override and produces no network or LSP request.

## Permissions and security

No additional permission is required to name or bind this command. LSP/decoration publication remains behind existing server/package validation and `render-decorations` permission. This command does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Prefer this command for user visibility control. Do not add a new LSP capability API, expose decoration spans as mutable objects, or paint from package JavaScript.

## Backing implementation

- JS facade: `runtime/js/editor.js::toggleInlayHints`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key`
- Command mapping: `src/client_commands.rs::EditorClientCommand`
- Local state: `src/client_commands.rs::EditorClientCommand (client-local; executed by the React/CodeMirror controller, frontend/src/editor/extensions/controller.ts)` and `src/client_commands.rs::EditorClientCommand`
- Inlay publication/paint: `packages/lsp-shared/bridge.js`, `src/protocol/decorations.rs`, and the React/CodeMirror decorations extension `frontend/src/editor/extensions/decorations.ts`

## Lookup metadata

- Stable ID: `editor.toggleInlayHints`
- User-facing name: Toggle Inlay Hints
- Module/export: `clay:editor` / `toggleInlayHints`
- Default key bindings: none
- Custom properties: none
- Tags: `[editor, inlay-hints, decorations, keybindings, js-api]`
