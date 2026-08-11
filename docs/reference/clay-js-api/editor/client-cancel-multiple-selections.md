---
id: editor.clientCancelMultipleSelections
kind: clay-js-api
js_module: "clay:editor"
js_export: clientCancelMultipleSelections
js_facade: runtime/js/editor.js::clientCancelMultipleSelections
backing_rust: src/editor/surface.rs::EditorSurface::cancel_multiple_selections
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientCancelMultipleSelections
user_facing_name: Cancel Multiple Selections
summary: Collapse the selection set to the primary caret.
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: ["Escape"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.clientCancelMultipleSelections` only as a documented command ID for `bindKey`; do not expose raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, js-api, multi-cursor, selection]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientCancelMultipleSelections

## Summary

Collapse the selection set to the primary caret.

## Description

`clientCancelMultipleSelections` returns the stable bindable command ID `editor.clientCancelMultipleSelections` for **Cancel Multiple Selections** (Plan 071 task 9). Escape drops every secondary caret and collapses the primary selection to its caret. The command is allowlisted, routed `ClientUiCommand`, and dispatched client-local in `EditorWidget`; it is client-local view state and grants no authority.

## When to use

Use this API when JavaScript configuration or extensions need to bind or reference the `Cancel Multiple Selections` command. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientCancelMultipleSelections, bindKey } from "clay:editor";

bindKey("ctrl+k ctrl+n", clientCancelMultipleSelections());
```

## Example

```ts
const commandId = clientCancelMultipleSelections(); // "editor.clientCancelMultipleSelections"
```

## Options

None. The facade takes no arguments and returns the stable command ID string.

## Key bindings

Default key bindings: "Escape".

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

None.

## Return and async behavior

Returns the stable command ID string `editor.clientCancelMultipleSelections` synchronously. The facade is synchronous, side-effect free, and local.

## Errors

None for the facade itself. `bindKey` rejects command IDs outside the registered allowlist.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientCancelMultipleSelections` when the user asks for Cancel Multiple Selections through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientCancelMultipleSelections`
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::cancel_multiple_selections`
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `editor.clientCancelMultipleSelections`
- User-facing name: Cancel Multiple Selections
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientCancelMultipleSelections`
- Default key bindings: "Escape"
- Tags: editor,  js-api,  multi-cursor,  selection
