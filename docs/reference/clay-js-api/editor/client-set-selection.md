---
id: clay.editor.clientSetSelection
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSetSelection
js_facade: runtime/js/editor.ts::clientSetSelection
backing_rust: src/editor/selection.rs::SelectionState
deno_op: op_clay_editor_set_selection
deno_op_path: src/server/ops/editor.rs::op_clay_editor_set_selection
name: clientSetSelection
user_facing_name: Set Selection
summary: Set Selection through the planned `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: [Shift+ArrowLeft, Shift+ArrowRight, PrimaryPointerDrag]
custom_properties: []
security: Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientSetSelection` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, selection]
app_visible: true
help_visible: true
stability: planned
async: false
---

# clientSetSelection

## Summary

Set Selection through the planned `clay:editor` Clay JavaScript facade.

## Description

`clientSetSelection` is the planned public API for **Set Selection**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `client-local-ui-state`. Runtime path: `client-local-hot-path`. Shift-arrow and pointer-drag selection update local state and are not serialized unless followed by a document edit.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Set Selection` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSetSelection } from "clay:editor";

clientSetSelection({ documentId: "current", anchor: 0, focus: 5 });
```

## Example

```ts
clientSetSelection({ documentId: "current", anchor: 0, focus: 5 });
```

## Options

- `documentId` (`string`): Target editor/document surface.
- `anchor` (`number`): Anchor offset for the selection.
- `focus` (`number`): Focus offset for the selection.

## Key bindings

Default key bindings:

- `Shift+ArrowLeft`
- `Shift+ArrowRight`
- `PrimaryPointerDrag`

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns client-local selection state when runtime wiring exists; the planned facade is synchronous and local.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes only transient client selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.editor.clientSetSelection` when the user asks for set selection through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientSetSelection`
- Future Deno op: `src/server/ops/editor.rs::op_clay_editor_set_selection` (`op_clay_editor_set_selection`)
- Backing Rust/current owner: `src/editor/selection.rs::SelectionState`
- Current implementation audit path: `src/editor/surface.rs::EditorSurface::extend_selection_to_point; src/editor/surface.rs::EditorSurface::select_left`

## Lookup metadata

- Stable ID: `clay.editor.clientSetSelection`
- User-facing name: Set Selection
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSetSelection`
- Default key bindings: `Shift+ArrowLeft`, `Shift+ArrowRight`, `PrimaryPointerDrag`
- Custom properties: none
- Tags: `editor`, `js-api`, `selection`
