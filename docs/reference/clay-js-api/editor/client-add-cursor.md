---
id: editor.clientAddCursor
kind: clay-js-api
js_module: "clay:editor"
js_export: clientAddCursor
js_facade: runtime/js/editor.js::clientAddCursor
backing_rust: src/editor/surface/mod.rs::EditorSurface::add_cursor_line
deno_op: op_clay_editor_add_cursor
deno_op_path: src/server/ops/editor.rs::op_clay_editor_add_cursor
name: clientAddCursor
user_facing_name: Add Cursor
summary: Add a collapsed caret one line below or above the primary caret at the same column.
owner: client
phase: Phase 21
visibility: public
permissions: []
key_bindings: ["Ctrl+Alt+Down", "Ctrl+Alt+Up"]
custom_properties:
  - name: direction
    type: enum
    default: none
    description: Where the new caret is added relative to the primary (below, above).
security: Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `editor.clientAddCursor` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, multi-cursor, selection]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientAddCursor

## Summary

Add a collapsed caret one line below or above the primary caret at the same column.

## Description

`clientAddCursor` is the public API for **Add Cursor** (Plan 071 task 9, VSCode `insertCursorBelow`/`insertCursorAbove`). The `op_clay_editor_add_cursor` deno op validates the `direction` argument (deny-by-default enum) and returns the direction-specific command descriptor (`editor.clientAddCursor.below` or `.above`). Key-driven execution is served client-local by those command IDs (allowlisted, routed `ClientUiCommand`, dispatched in `EditorWidget`). The new caret is placed at the same scalar column on the target line, clamped to the line end, and becomes the primary. A caret is never stacked twice on one line.

Authority: `client-local-ui-state`. Runtime path: `client-local-hot-path`.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need to add an extra caret above or below the primary caret. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientAddCursor } from "clay:editor";

clientAddCursor({ direction: "below" });
```

## Example

```ts
clientAddCursor({ direction: "above" });
```

## Options

- `direction` (`enum`): `below` | `above`.

## Key bindings

Default key bindings:

- `Ctrl+Alt+Down` (add cursor below), `Ctrl+Alt+Up` (add cursor above)

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js` using the direction-specific command IDs (`editor.clientAddCursor.below`, `editor.clientAddCursor.above`).

## Custom properties

- `direction` (`enum`): Where the new caret is added relative to the primary (see Options).

## Return and async behavior

Returns the validated command descriptor (`{ commandId, direction }`) synchronously. The facade is synchronous and local.

## Errors

The op fails (deny-by-default) if `direction` is missing or not one of the documented values, or if the options are not valid JSON.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientAddCursor` when the user asks for add-cursor multi-editing through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientAddCursor`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_add_cursor`
- Backing Rust/current owner: `src/editor/surface/mod.rs::EditorSurface::add_cursor_line`
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `editor.clientAddCursor`
- User-facing name: Add Cursor
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientAddCursor`
- Default key bindings: `Ctrl+Alt+Down`, `Ctrl+Alt+Up`
- Custom properties: `direction`
- Tags: `editor`, `js-api`, `multi-cursor`, `selection`
