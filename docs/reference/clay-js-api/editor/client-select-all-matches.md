---
id: clay.editor.clientSelectAllMatches
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSelectAllMatches
js_facade: runtime/js/editor.js::clientSelectAllMatches
backing_rust: src/editor/surface.rs::EditorSurface::select_all_matches
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientSelectAllMatches
user_facing_name: Select All Matches
summary: Replace the selection set with every occurrence of the current selection or word.
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: ["Ctrl+Shift+L"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientSelectAllMatches` only as a documented command ID for `bindKey`; do not expose raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, js-api, multi-cursor, selection]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientSelectAllMatches

## Summary

Replace the selection set with every occurrence of the current selection or word.

## Description

`clientSelectAllMatches` returns the stable bindable command ID `clay.editor.clientSelectAllMatches` for **Select All Matches** (Plan 071 task 9). All occurrences become selections; the occurrence containing the original caret stays primary. Copy unions every range in document order. The command is allowlisted, routed `ClientUiCommand`, and dispatched client-local in `EditorWidget`; it is client-local view state and grants no authority.

## When to use

Use this API when JavaScript configuration or extensions need to bind or reference the `Select All Matches` command. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSelectAllMatches, bindKey } from "clay:editor";

bindKey("ctrl+k ctrl+n", clientSelectAllMatches());
```

## Example

```ts
const commandId = clientSelectAllMatches(); // "clay.editor.clientSelectAllMatches"
```

## Options

None. The facade takes no arguments and returns the stable command ID string.

## Key bindings

Default key bindings: "Ctrl+Shift+L".

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

None.

## Return and async behavior

Returns the stable command ID string `clay.editor.clientSelectAllMatches` synchronously. The facade is synchronous, side-effect free, and local.

## Errors

None for the facade itself. `bindKey` rejects command IDs outside the registered allowlist.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.clientSelectAllMatches` when the user asks for Select All Matches through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientSelectAllMatches`
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::select_all_matches`
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `clay.editor.clientSelectAllMatches`
- User-facing name: Select All Matches
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSelectAllMatches`
- Default key bindings: "Ctrl+Shift+L"
- Tags: editor,  js-api,  multi-cursor,  selection
