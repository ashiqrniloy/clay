---
id: editor.clientSetSelection
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSetSelection
js_facade: runtime/js/editor.js::clientSetSelection
backing_rust: src/editor/surface.rs::EditorSurface::select_word
deno_op: op_clay_editor_set_selection
deno_op_path: src/server/ops/editor.rs::op_clay_editor_set_selection
name: clientSetSelection
user_facing_name: Set Selection
summary: Set the selection through the `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: [Shift+ArrowLeft, Shift+ArrowRight, PrimaryPointerDrag, Ctrl+L, Ctrl+D]
custom_properties:
  - name: action
    type: enum
    default: none
    description: Selection action (selectWord, selectLine, selectParagraph).
  - name: extend
    type: boolean
    default: false
    description: Whether the action extends the current selection.
  - name: direction
    type: enum
    default: current
    description: Selection direction (current, next, prev).
security: Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `editor.clientSetSelection` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, selection]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientSetSelection

## Summary

Set the selection through the `clay:editor` Clay JavaScript facade.

## Description

`clientSetSelection` is the public API for **Set Selection**. The `op_clay_editor_set_selection` deno op validates typed arguments (deny-by-default enum) and returns the validated command descriptor. Key-driven selection is served client-local by the direction-specific `editor.clientSetSelection.*` command IDs (allowlisted, routed `ClientUiCommand`, dispatched in `EditorWidget`).

Authority: `client-local-ui-state`. Runtime path: `client-local-hot-path`. Shift-arrow, pointer-drag, Ctrl+L (line), and Ctrl+D (word) selection update local state and are not serialized unless followed by a document edit.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Set Selection` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSetSelection } from "clay:editor";

clientSetSelection({ action: "selectLine" });
```

## Example

```ts
clientSetSelection({ action: "selectWord", extend: true });
```

## Options

- `documentId` (`string`, optional): Target editor/document surface.
- `action` (`enum`): `selectWord` | `selectLine` | `selectParagraph`.
- `extend` (`boolean`): Whether the action extends the current selection; defaults to `false`.
- `direction` (`enum`, optional): `current` | `next` | `prev`.

## Key bindings

Default key bindings:

- `Shift+ArrowLeft`, `Shift+ArrowRight`
- `PrimaryPointerDrag`
- `Ctrl+L` (select current line), `Ctrl+D` (select word at caret)

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js` using the direction-specific command IDs (e.g. `editor.clientSetSelection.selectLine`).

## Custom properties

- `action` (`enum`): Selection action (see Options).
- `extend` (`boolean`, default `false`): Extend the current selection.
- `direction` (`enum`, default `current`): Selection direction.

## Return and async behavior

Returns the validated command descriptor (`{ commandId, action, extend, direction }`) synchronously. The facade is synchronous and local.

## Errors

The op fails (deny-by-default) if `action` is missing or not one of the documented values, if `direction` is present but unknown, or if the options are not valid JSON.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientSetSelection` when the user asks for set selection through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientSetSelection`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_set_selection` (`op_clay_editor_set_selection`)
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::select_word` (and `select_line`, `select_paragraph`)
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `editor.clientSetSelection`
- User-facing name: Set Selection
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSetSelection`
- Default key bindings: `Shift+ArrowLeft`, `Shift+ArrowRight`, `PrimaryPointerDrag`, `Ctrl+L`, `Ctrl+D`
- Custom properties: `action`, `extend`, `direction`
- Tags: `editor`, `js-api`, `selection`
