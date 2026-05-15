---
id: clay.editor.clientSetCursorStyle
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSetCursorStyle
js_facade: runtime/js/editor.ts::clientSetCursorStyle
backing_rust: src/editor/surface.rs::EditorSurface::paint_caret
deno_op: op_clay_editor_set_cursor_style
deno_op_path: src/server/ops/editor.rs::op_clay_editor_set_cursor_style
name: clientSetCursorStyle
user_facing_name: Set Cursor Style
summary: Set Cursor Style through the planned `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: color
    type: string
    default: inherited
    description: Behavior-changing setting `color` for this API.
  - name: blinking
    type: boolean
    default: true
    description: Behavior-changing setting `blinking` for this API.
  - name: type
    type: enum
    default: bar
    description: Behavior-changing setting `type` for this API.
security: Configuration-only UI customization; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientSetCursorStyle` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [cursorstylecustomization, editor, js-api]
app_visible: true
help_visible: true
stability: planned
async: false
---

# clientSetCursorStyle

## Summary

Set Cursor Style through the planned `clay:editor` Clay JavaScript facade.

## Description

`clientSetCursorStyle` is the planned public API for **Set Cursor Style**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `configuration-driven-client-ui-state`. Runtime path: `configuration-api-to-client-ui`. Cursor styling is paint-time UI metadata; changing it must not route ordinary keypresses through JavaScript.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Set Cursor Style` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSetCursorStyle } from "clay:editor";

clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
```

## Example

```ts
clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
```

## Options

- `color` (`string`): Optional CSS-like color value or inherited theme value.
- `blinking` (`boolean`): Whether the caret blinks; defaults to `true`.
- `type` (`"block" | "bar" | "underline"`): Caret shape; defaults to `"bar"`.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.editor.clientSetCursorStyle` in `~/.config/clay/init.js`.

## Custom properties

- `color` (`string`, default `inherited`): Behavior-changing setting `color` for this API.
- `blinking` (`boolean`, default `true`): Behavior-changing setting `blinking` for this API.
- `type` (`enum`, default `bar`): Behavior-changing setting `type` for this API.

## Return and async behavior

Returns client-local cursor style state when runtime wiring exists; the planned facade is synchronous and local.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Configuration-only UI customization; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.clientSetCursorStyle` when the user asks for set cursor style through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientSetCursorStyle`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_set_cursor_style` (`op_clay_editor_set_cursor_style`)
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::paint_caret`
- Current implementation audit path: `src/editor/surface.rs::CARET_COLOR; src/editor/surface.rs::CARET_WIDTH`

## Lookup metadata

- Stable ID: `clay.editor.clientSetCursorStyle`
- User-facing name: Set Cursor Style
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSetCursorStyle`
- Default key bindings: none
- Custom properties: `color`, `blinking`, `type`
- Tags: `cursorstylecustomization`, `editor`, `js-api`
