---
id: editor.clientRemoveSelection
kind: clay-js-api
js_module: "clay:editor"
js_export: clientRemoveSelection
js_facade: runtime/js/editor.js::clientRemoveSelection
backing_rust: src/editor/surface.rs::EditorSurface::remove_selection
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientRemoveSelection
user_facing_name: Remove Selection
summary: Remove the primary selection, keeping the rest.
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.clientRemoveSelection` only as a documented command ID for `bindKey`; do not expose raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, js-api, multi-cursor, selection]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientRemoveSelection

## Summary

Remove the primary selection, keeping the rest.

## Description

`clientRemoveSelection` returns the stable bindable command ID `editor.clientRemoveSelection` for **Remove Selection** (Plan 071 task 9). Helix remove_primary_selection semantics: the primary is dropped and a remaining selection becomes primary; a no-op for a single selection. The command is allowlisted, routed `ClientUiCommand`, and dispatched client-local in `EditorWidget`; it is client-local view state and grants no authority.

## When to use

Use this API when JavaScript configuration or extensions need to bind or reference the `Remove Selection` command. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientRemoveSelection, bindKey } from "clay:editor";

bindKey("ctrl+k ctrl+n", clientRemoveSelection());
```

## Example

```ts
const commandId = clientRemoveSelection(); // "editor.clientRemoveSelection"
```

## Options

None. The facade takes no arguments and returns the stable command ID string.

## Key bindings

Default key bindings: none (bindable via `bindKey`).

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

None.

## Return and async behavior

Returns the stable command ID string `editor.clientRemoveSelection` synchronously. The facade is synchronous, side-effect free, and local.

## Errors

None for the facade itself. `bindKey` rejects command IDs outside the registered allowlist.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientRemoveSelection` when the user asks for Remove Selection through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientRemoveSelection`
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::remove_selection`
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `editor.clientRemoveSelection`
- User-facing name: Remove Selection
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientRemoveSelection`
- Default key bindings: none (bindable via `bindKey`)
- Tags: editor,  js-api,  multi-cursor,  selection
