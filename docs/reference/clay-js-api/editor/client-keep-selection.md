---
id: clay.editor.clientKeepSelection
kind: clay-js-api
js_module: "clay:editor"
js_export: clientKeepSelection
js_facade: runtime/js/editor.js::clientKeepSelection
backing_rust: src/editor/surface.rs::EditorSurface::keep_selection
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientKeepSelection
user_facing_name: Keep Selection
summary: Keep only the primary selection, dropping every other caret.
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientKeepSelection` only as a documented command ID for `bindKey`; do not expose raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, js-api, multi-cursor, selection]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientKeepSelection

## Summary

Keep only the primary selection, dropping every other caret.

## Description

`clientKeepSelection` returns the stable bindable command ID `clay.editor.clientKeepSelection` for **Keep Selection** (Plan 071 task 9). Helix keep_primary_selection semantics: the primary selection keeps its range; all secondary carets are removed. The command is allowlisted, routed `ClientUiCommand`, and dispatched client-local in `EditorWidget`; it is client-local view state and grants no authority.

## When to use

Use this API when JavaScript configuration or extensions need to bind or reference the `Keep Selection` command. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientKeepSelection, bindKey } from "clay:editor";

bindKey("ctrl+k ctrl+n", clientKeepSelection());
```

## Example

```ts
const commandId = clientKeepSelection(); // "clay.editor.clientKeepSelection"
```

## Options

None. The facade takes no arguments and returns the stable command ID string.

## Key bindings

Default key bindings: none (bindable via `bindKey`).

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

None.

## Return and async behavior

Returns the stable command ID string `clay.editor.clientKeepSelection` synchronously. The facade is synchronous, side-effect free, and local.

## Errors

None for the facade itself. `bindKey` rejects command IDs outside the registered allowlist.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.clientKeepSelection` when the user asks for Keep Selection through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientKeepSelection`
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::keep_selection`
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `clay.editor.clientKeepSelection`
- User-facing name: Keep Selection
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientKeepSelection`
- Default key bindings: none (bindable via `bindKey`)
- Tags: editor,  js-api,  multi-cursor,  selection
