---
id: editor.clientSmartSelect
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSmartSelect
js_facade: runtime/js/editor.js::clientSmartSelect
backing_rust: src/server/syntax.rs::TreeSitterSyntaxHandler::selection_query_ranges; src/masonry_editor.rs::EditorWidget::apply_selection_query_result
deno_op: op_clay_editor_smart_select
deno_op_path: src/server/ops/editor.rs::op_clay_editor_smart_select
name: clientSmartSelect
user_facing_name: Smart Select
summary: Expand or shrink the selection along the document's syntax tree (AST-aware grow/shrink).
owner: client
phase: Phase 22
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: action
    type: enum
    default: none
    description: expand grows each selection to the smallest enclosing syntax-node range; shrink returns to the largest node range strictly inside the current selection.
security: Changes only transient client selection state from a read-only server tree walk; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `editor.clientSmartSelect` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, smart-select, tree-sitter, selection]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientSmartSelect

## Summary

Expand or shrink the selection along the document's syntax tree (AST-aware grow/shrink).

## Description

`clientSmartSelect` is the public API for **Smart Select** (Plan 071 task 10, VSCode `smartSelect.expand`/`smartSelect.shrink`). The `op_clay_editor_smart_select` deno op validates the `action` argument (deny-by-default enum) and returns the action-specific command ID (`editor.clientSmartSelect.expand` or `.shrink`). Key-driven execution captures the client selection set locally, sends one bounded read-only request to the server, and the server walks the parsed tree: **expand** grows each selection to the smallest node range strictly larger than it (the parent chain, up to the whole document), **shrink** returns to the largest node range strictly contained in the current selection. Works for any grammar with a native parser even when no `textobjects.scm` ships for it (e.g. Markdown). Multi-cursor aware: every selection expands/shrinks independently. Results apply as selections on the client.

Authority: `client-local-ui-state` (result data is server-computed and read-only). Runtime path: `ui-reactive-server-query`.

## When to use

Use this API when JavaScript configuration or packages need progressive structural selection (grow to enclosing expression/statement/function, then shrink back). Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSmartSelect } from "clay:editor";

clientSmartSelect({ action: "expand" });
```

## Example

```ts
// Bind expand/shrink in package init.js:
bindKey("Ctrl+Shift+\\", clientSmartSelect({ action: "expand" }).commandId);
bindKey("Ctrl+Shift+Alt+\\", clientSmartSelect({ action: "shrink" }).commandId);
```

## Options

- `action` (`enum`): `expand` | `shrink`. Required.

## Key bindings

No default key bindings. The command IDs are bindable through documented key binding APIs in `~/.config/clay/init.js` (`editor.clientSmartSelect.expand`, `editor.clientSmartSelect.shrink`).

## Custom properties

- `action` (`enum`): Expand or shrink the selection (see Options).

## Return and async behavior

Returns the validated command descriptor (`{ commandId, action }`) synchronously. The facade is synchronous and local; the key-driven selection query itself is a one-round-trip UI-reactive server request.

## Errors

The op fails (deny-by-default) if `action` is missing or not one of the documented values, or if the options are not valid JSON.

## Permissions and security

No additional permission is required beyond access to the running editor session. The server tree walk is read-only: it never mutates the document, spawns processes, or loads artifacts.

Changes only transient client selection state from a read-only server tree walk; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientSmartSelect` when the user asks for AST-aware expand/shrink selection through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientSmartSelect`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_smart_select`
- Server tree walk: `src/server/syntax.rs::TreeSitterSyntaxHandler::selection_query_ranges`
- Client application: `src/masonry_editor.rs::EditorWidget::apply_selection_query_result`

## Lookup metadata

- Stable ID: `editor.clientSmartSelect`
- User-facing name: Smart Select
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSmartSelect`
- Default key bindings: none
- Custom properties: `action`
- Tags: `editor`, `js-api`, `smart-select`, `tree-sitter`, `selection`
