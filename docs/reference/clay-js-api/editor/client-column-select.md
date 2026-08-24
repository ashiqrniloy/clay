---
id: editor.clientColumnSelect
kind: clay-js-api
js_module: "clay:editor"
js_export: clientColumnSelect
js_facade: runtime/js/editor.js::clientColumnSelect
backing_rust: src/client_commands.rs::EditorClientCommand
deno_op: op_clay_editor_column_select
deno_op_path: src/server/ops/editor.rs::op_clay_editor_column_select
name: clientColumnSelect
user_facing_name: Column Select
summary: Grow a column/box selection one line (down/up) or move every caret one scalar (left/right).
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: ["Shift+Alt+Down", "Shift+Alt+Up", "Shift+Alt+Left", "Shift+Alt+Right"]
custom_properties:
  - name: direction
    type: enum
    default: none
    description: Column-select direction (down, up, left, right).
security: Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `editor.clientColumnSelect` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, multi-cursor, column-select, selection]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientColumnSelect

## Summary

Grow a column/box selection one line (down/up) or move every caret one scalar (left/right).

## Description

`clientColumnSelect` is the public API for **Column Select** (Plan 071 task 9, VSCode `cursorColumnSelect*`). The `op_clay_editor_column_select` deno op validates the `direction` argument (deny-by-default enum) and returns the direction-specific command descriptor (`editor.clientColumnSelect.down|up|left|right`). Down/up add a caret one line below/above the primary at the same column (growing the box); left/right move every caret one scalar character. Key-driven execution is served client-local by those command IDs (allowlisted, routed `ClientUiCommand`, dispatched client-local by the React/CodeMirror controller).

Authority: `client-local-ui-state`. Runtime path: `client-local-hot-path`.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need box/column editing. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientColumnSelect } from "clay:editor";

clientColumnSelect({ direction: "down" });
```

## Example

```ts
clientColumnSelect({ direction: "right" });
```

## Options

- `direction` (`enum`): `down` | `up` | `left` | `right`.

## Key bindings

Default key bindings:

- `Shift+Alt+Down`, `Shift+Alt+Up` (grow the box one line)
- `Shift+Alt+Left`, `Shift+Alt+Right` (move all carets)

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js` using the direction-specific command IDs.

## Custom properties

- `direction` (`enum`): Column-select direction (see Options).

## Return and async behavior

Returns the validated command descriptor (`{ commandId, direction }`) synchronously. The facade is synchronous and local.

## Errors

The op fails (deny-by-default) if `direction` is missing or not one of the documented values, or if the options are not valid JSON.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientColumnSelect` when the user asks for column/box selection through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientColumnSelect`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_column_select`
- Backing Rust/current owner: `src/client_commands.rs::EditorClientCommand` (down/up), `src/client_commands.rs::EditorClientCommand` (left/right)
- Key-driven dispatch: `src/client_commands.rs::EditorClientCommand (client-local; executed by the React/CodeMirror controller, frontend/src/editor/extensions/controller.ts)`

## Lookup metadata

- Stable ID: `editor.clientColumnSelect`
- User-facing name: Column Select
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientColumnSelect`
- Default key bindings: `Shift+Alt+Down`, `Shift+Alt+Up`, `Shift+Alt+Left`, `Shift+Alt+Right`
- Custom properties: `direction`
- Tags: `editor`, `js-api`, `multi-cursor`, `column-select`, `selection`
