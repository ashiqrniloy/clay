---
id: editor.clientSelectNextMatch
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSelectNextMatch
js_facade: runtime/js/editor.js::clientSelectNextMatch
backing_rust: src/editor/surface/mod.rs::EditorSurface::select_next_match
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientSelectNextMatch
user_facing_name: Select Next Match
summary: Select the next occurrence of the current selection or word as a new primary caret.
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: ["Ctrl+D"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.clientSelectNextMatch` only as a documented command ID for `bindKey`; do not expose raw Rust calls, protocol DTOs, or `Deno.core.ops`.
lookup_tags: [editor, js-api, multi-cursor, selection]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientSelectNextMatch

## Summary

Select the next occurrence of the current selection or word as a new primary caret.

## Description

`clientSelectNextMatch` returns the stable bindable command ID `editor.clientSelectNextMatch` for **Select Next Match** (Plan 071 task 9). On a collapsed caret the first press selects the word under the caret; each further press adds the next occurrence as a new caret. Search wraps once around the document and stops when every occurrence is selected. The command is allowlisted, routed `ClientUiCommand`, and dispatched client-local in `EditorWidget`; it is client-local view state and grants no authority.

## When to use

Use this API when JavaScript configuration or extensions need to bind or reference the `Select Next Match` command. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSelectNextMatch, bindKey } from "clay:editor";

bindKey("ctrl+k ctrl+n", clientSelectNextMatch());
```

## Example

```ts
const commandId = clientSelectNextMatch(); // "editor.clientSelectNextMatch"
```

## Options

None. The facade takes no arguments and returns the stable command ID string.

## Key bindings

Default key bindings: "Ctrl+D".

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

None.

## Return and async behavior

Returns the stable command ID string `editor.clientSelectNextMatch` synchronously. The facade is synchronous, side-effect free, and local.

## Errors

None for the facade itself. `bindKey` rejects command IDs outside the registered allowlist.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Bindable client UI command ID only; after explicit user routing it changes transient client selection state, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientSelectNextMatch` when the user asks for Select Next Match through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientSelectNextMatch`
- Backing Rust/current owner: `src/editor/surface/mod.rs::EditorSurface::select_next_match`
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `editor.clientSelectNextMatch`
- User-facing name: Select Next Match
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSelectNextMatch`
- Default key bindings: "Ctrl+D"
- Tags: editor,  js-api,  multi-cursor,  selection
