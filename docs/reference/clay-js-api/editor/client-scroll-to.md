---
id: clay.editor.clientScrollTo
kind: clay-js-api
js_module: "clay:editor"
js_export: clientScrollTo
js_facade: runtime/js/editor.ts::clientScrollTo
backing_rust: src/editor/viewport.rs::Viewport::scroll_lines
deno_op: op_clay_editor_scroll_to
deno_op_path: src/server/ops/editor.rs::op_clay_editor_scroll_to
name: clientScrollTo
user_facing_name: Scroll Editor
summary: Scroll Editor through the planned `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: [PointerScroll]
custom_properties:
  - name: line
    type: number
    default: none
    description: Behavior-changing setting `line` for this API.
  - name: column
    type: number
    default: none
    description: Behavior-changing setting `column` for this API.
  - name: revealCursor
    type: boolean
    default: false
    description: Behavior-changing setting `revealCursor` for this API.
security: Changes only client viewport/visual scroll state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientScrollTo` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, scrolling]
app_visible: true
help_visible: true
stability: planned
async: false
---

# clientScrollTo

## Summary

Scroll Editor through the planned `clay:editor` Clay JavaScript facade.

## Description

`clientScrollTo` is the planned public API for **Scroll Editor**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `client-local-ui-state`. Runtime path: `client-local-hot-path`. Wheel/page/line scrolling updates viewport and visual overflow locally during input handling.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Scroll Editor` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientScrollTo } from "clay:editor";

clientScrollTo({ documentId: "current", line: 42, column: 0 });
```

## Example

```ts
clientScrollTo({ documentId: "current", line: 42, column: 0 });
```

## Options

- `documentId` (`string`): Target editor/document surface.
- `line` (`number`): Optional target line.
- `column` (`number`): Optional target column.
- `revealCursor` (`boolean`): Whether to scroll enough to reveal the current cursor; defaults to `false`.

## Key bindings

Default key bindings:

- `PointerScroll`

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

- `line` (`number`, default `none`): Behavior-changing setting `line` for this API.
- `column` (`number`, default `none`): Behavior-changing setting `column` for this API.
- `revealCursor` (`boolean`, default `false`): Behavior-changing setting `revealCursor` for this API.

## Return and async behavior

Returns client-local scroll state when runtime wiring exists; the planned facade is synchronous and local.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes only client viewport/visual scroll state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.clientScrollTo` when the user asks for scroll editor through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientScrollTo`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_scroll_to` (`op_clay_editor_scroll_to`)
- Backing Rust/current owner: `src/editor/viewport.rs::Viewport::scroll_lines`
- Current implementation audit path: `src/editor/surface.rs::EditorSurface::scroll_lines; src/editor/surface.rs::EditorSurface::scroll_vertical_pixels`

## Lookup metadata

- Stable ID: `clay.editor.clientScrollTo`
- User-facing name: Scroll Editor
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientScrollTo`
- Default key bindings: `PointerScroll`
- Custom properties: `line`, `column`, `revealCursor`
- Tags: `editor`, `js-api`, `scrolling`
