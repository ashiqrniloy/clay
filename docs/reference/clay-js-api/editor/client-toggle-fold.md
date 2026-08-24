---
id: editor.clientToggleFold
kind: clay-js-api
js_module: "clay:editor"
js_export: clientToggleFold
js_facade: runtime/js/editor.js::clientToggleFold
backing_rust: src/client_commands.rs::EditorClientCommand
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientToggleFold
user_facing_name: Toggle Fold
summary: Return the stable client command ID for collapsing or expanding the fold containing the caret.
owner: client
phase: Phase 28.3
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; changes client-local collapse state for validated inert fold ranges and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.clientToggleFold` through `bindKey`; packages publish inert ranges through `folding.serverPublishFoldingRanges`, while Clay owns collapse state and painting. Do not expose fold handles or raw ops.
lookup_tags: [editor, folding, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientToggleFold

## Summary

Returns the stable command ID `editor.clientToggleFold` for collapsing or expanding the validated fold containing the caret.

## Description

The facade does not publish or discover ranges. It only names the client-local command. After explicit routing, The CodeMirror folding extension (`frontend/src/editor/extensions/folding.ts`) updates collapse state for the caret's containing range; fold range transport and hidden-line semantics stay in Clay-owned Rust code.

## When to use

Use this API to bind an alternate fold chord. Packages that need additional ranges should use `folding.serverPublishFoldingRanges`; they must not attempt to paint or mutate fold state from JavaScript.

## JavaScript usage

```ts
import { clientToggleFold } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+F", clientToggleFold(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+F", "editor.clientToggleFold", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientToggleFold } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+F", clientToggleFold(), { scope: "editor" });
```

## Options

No options are accepted. The active caret and already validated fold ranges determine the target.

## Key bindings

No core default key binding is assigned. The command can be bound with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined.

## Return and async behavior

Returns `"editor.clientToggleFold"` synchronously. Calling the helper does not query the server, execute package JavaScript, publish ranges, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` rejects malformed chords, unsupported scopes, or unknown command IDs. With no containing fold, the routed command is a local no-op.

## Permissions and security

No additional permission is required. Fold ranges are inert and server-validated before reaching the client; collapse state is client-local and document-scoped. This command does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use the editor command only for user routing. Use `serverPublishFoldingRanges` for package publication and keep all range validation, layout invalidation, chevrons, and hidden-line behavior host-owned.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientToggleFold`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key`
- Command mapping: `src/client_commands.rs::EditorClientCommand`
- Local state: `src/client_commands.rs::EditorClientCommand (client-local; executed by the React/CodeMirror controller, frontend/src/editor/extensions/controller.ts)` and `src/client_commands.rs::EditorClientCommand`
- Range publication API: [`folding.serverPublishFoldingRanges`](../folding/server-publish-folding-ranges.md)

## Lookup metadata

- Stable ID: `editor.clientToggleFold`
- User-facing name: Toggle Fold
- Module/export: `clay:editor` / `clientToggleFold`
- Default key bindings: none
- Custom properties: none
- Tags: `[editor, folding, keybindings, js-api]`
