---
id: editor.clientSetCursorStyle
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSetCursorStyle
js_facade: runtime/js/editor.js::clientSetCursorStyle
backing_rust: src/editor/surface.rs::EditorSurface::set_caret_style_override
deno_op: op_clay_editor_set_cursor_style
deno_op_path: src/server/ops/editor.rs::op_clay_editor_set_cursor_style
name: clientSetCursorStyle
user_facing_name: Set Cursor Style
summary: Set the caret shape and blink through the `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 8
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: shape
    type: enum
    default: bar
    description: Caret glyph shape; allowed values are bar, line, block, and underline; the default is bar. Colour stays theme-owned (base.caret).
  - name: blink
    type: enum
    default: solid
    description: Blink behaviour; allowed values are solid, blink, phase, and smooth; the default is solid (never hides, reduced-motion friendly).
  - name: widthPx
    type: number
    default: "1.5"
    description: Stroke thickness for bar/line/underline in pixels; defaults to 1.5.
  - name: heightPct
    type: number
    default: "1"
    description: Caret height as a fraction of the line height; defaults to 1 (full line).
  - name: hollow
    type: boolean
    default: false
    description: Render the block caret as an outline; defaults to false.
  - name: stopBlinkOnTyping
    type: boolean
    default: true
    description: Restart the blink to visible on typing; defaults to true.
security: Configuration-only UI customization; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, or document mutation authority.
agent_guidance: Use `editor.clientSetCursorStyle` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [cursorstylecustomization, editor, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientSetCursorStyle

## Summary

Set the caret shape and blink through the `clay:editor` Clay JavaScript facade.

## Description

`clientSetCursorStyle` is the public API for **Set Cursor Style**. The `op_clay_editor_set_cursor_style` deno op validates typed arguments (deny-by-default enum) and returns the validated command descriptor. The client applies the style through `EditorSurface::set_caret_style_override`, which takes precedence over the per-mode manifest `caret_style` and the editor `StyleRegistry` default.

Authority: `configuration-driven-client-ui-state`. Runtime path: `configuration-api-to-client-ui`. Cursor styling is paint-time UI metadata; changing it does not route ordinary keypresses through JavaScript or block paint/input on server work. Caret **colour** stays theme-owned (`base.caret`); this API owns shape and blink only.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Set Cursor Style` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSetCursorStyle } from "clay:editor";

clientSetCursorStyle({ shape: "block", blink: "solid" });
```

## Example

```ts
clientSetCursorStyle({ shape: "underline", widthPx: 2, blink: "blink" });
```

## Options

- `shape` (`"bar" | "line" | "block" | "underline"`): Caret glyph shape; allowed values are `"bar"`, `"line"`, `"block"`, and `"underline"`; default `"bar"`.
- `blink` (`"solid" | "blink" | "phase" | "smooth"`): Blink behaviour; allowed values are `"solid"`, `"blink"`, `"phase"`, and `"smooth"`; default `"solid"` (never hides).
- `widthPx` (`number`): Stroke thickness for bar/line/underline in pixels; default `1.5`.
- `heightPct` (`number`): Caret height as a fraction of the line height; default `1` (full line).
- `hollow` (`boolean`): Render the block caret as an outline; default `false`.
- `stopBlinkOnTyping` (`boolean`): Restart the blink to visible on typing; default `true`.

## Key bindings

No default key binding is assigned. `clientSetCursorStyle` is a programmatic, argument-bearing API; per-mode defaults are set through the behavior manifest `editorRules.caretStyle`, and the caret colour stays theme-owned.

## Custom properties

- `shape` (`enum`, default `bar`): Caret glyph shape; allowed values are `bar`, `line`, `block`, and `underline`.
- `blink` (`enum`, default `solid`): Blink behaviour; allowed values are `solid`, `blink`, `phase`, and `smooth`. `solid` never hides (reduced-motion friendly).
- `widthPx` (`number`, default `1.5`): Stroke thickness for bar/line/underline in pixels.
- `heightPct` (`number`, default `1`): Caret height as a fraction of the line height.
- `hollow` (`boolean`, default `false`): Render the block caret as an outline.
- `stopBlinkOnTyping` (`boolean`, default `true`): Restart the blink to visible on typing.

## Return and async behavior

Returns the validated command descriptor (`{ commandId, shape, blink, widthPx, heightPct, hollow, stopBlinkOnTyping }`) synchronously. The facade is synchronous and local.

## Errors

The op fails (deny-by-default) if a present `shape` or `blink` value is not one of the documented values, or if the options are not valid JSON. Absent fields fall back to the active style.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Configuration-only UI customization; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, or document mutation authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientSetCursorStyle` when the user asks for set cursor style through the Clay JS API or `~/.config/clay/init.js` customization. Avoid inventing direct Rust calls, raw op names, document mutation, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientSetCursorStyle`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_set_cursor_style` (`op_clay_editor_set_cursor_style`)
- Backing Rust/current owner: `src/editor/surface.rs::EditorSurface::set_caret_style_override`
- Paint: `src/editor/surface.rs::EditorSurface::paint_caret` (shape-aware), `src/editor/layout.rs::caret_cell_for_visible_byte_offset`

## Lookup metadata

- Stable ID: `editor.clientSetCursorStyle`
- User-facing name: Set Cursor Style
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSetCursorStyle`
- Default key bindings: none
- Custom properties: `shape`, `blink`, `widthPx`, `heightPct`, `hollow`, `stopBlinkOnTyping`
- Tags: `cursorstylecustomization`, `editor`, `js-api`
