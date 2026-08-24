---
id: editor.clientSelectTextobject
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSelectTextobject
js_facade: runtime/js/editor.js::clientSelectTextobject
backing_rust: src/client_commands.rs::EditorClientCommand; src/server/syntax.rs::TreeSitterSyntaxHandler::selection_query_ranges
deno_op: op_clay_editor_select_textobject
deno_op_path: src/server/ops/editor.rs::op_clay_editor_select_textobject
name: clientSelectTextobject
user_facing_name: Select Textobject
summary: Select the tree-sitter text object (function, class, argument, comment, ...) around, after, or before each caret.
owner: client
phase: Phase 22
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: object
    type: enum
    default: none
    description: Which text object to select (function, class, argument, comment, loop, conditional, call, statement).
  - name: around
    type: boolean
    default: "false"
    description: true selects the around capture (whole node); false selects the inner capture (falling back to around when the grammar defines no inner).
  - name: direction
    type: enum
    default: current
    description: current selects the innermost object at the caret; next/previous walk to the nearest object after/before the caret (no wrap).
security: Changes only transient client selection state from a read-only server grammar query; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `editor.clientSelectTextobject` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, textobjects, tree-sitter, selection]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientSelectTextobject

## Summary

Select the tree-sitter text object (function, class, argument, comment, ...) around, after, or before each caret.

## Description

`clientSelectTextobject` is the public API for **Select Textobject** (Plan 071 task 10, Helix-style `select_textobject`). The `op_clay_editor_select_textobject` deno op validates the `object`/`around`/`direction` arguments (deny-by-default enums) and returns the direction-specific command ID (`editor.clientSelectTextobject.<object>.<inner|around>[.next|.previous]`). Key-driven execution captures the client selection set locally, sends one bounded read-only request to the server, and the server runs the active grammar's `textobjects.scm` (captures named `textobject.<kind>.<inner|around>`) against the parsed tree. Returned byte ranges are applied as selections — one per requested caret, multi-cursor aware; carets with no matching object keep their selection. Grammars without a textobject query (or documents without a grammar) degrade to "no ranges" without error. Built-in query files ship for Rust, TypeScript/TSX, and JavaScript under `packages/*/queries/textobjects.scm`.

Authority: `client-local-ui-state` (result data is server-computed and read-only). Runtime path: `ui-reactive-server-query`.

## When to use

Use this API when JavaScript configuration or packages need structural selections (e.g. bind `]f`/`[f` style next/previous function navigation or `mif`/`maf` style inner/around selections). Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientSelectTextobject } from "clay:editor";

clientSelectTextobject({ object: "function", around: false, direction: "current" });
```

## Example

```ts
// Bind Ctrl+] to jump to the next function (package init.js):
bindKey("Ctrl+]", clientSelectTextobject({ object: "function", around: true, direction: "next" }).commandId);
```

## Options

- `object` (`enum`): `function` | `class` | `argument` | `comment` | `loop` | `conditional` | `call` | `statement`. Required.
- `around` (`boolean`): `true` selects the `around` capture; `false` (default) the `inner` capture.
- `direction` (`enum`): `current` (default) | `next` | `previous`.

## Key bindings

No default key bindings. The command IDs are bindable through documented key binding APIs in `~/.config/clay/init.js` (for example `editor.clientSelectTextobject.function.inner`, `editor.clientSelectTextobject.function.around`, `editor.clientSelectTextobject.function.around.next`).

## Custom properties

- `object` (`enum`): Which text object to select (see Options).
- `around` (`boolean`): Whole node vs interior capture (see Options).
- `direction` (`enum`): Which occurrence relative to the caret (see Options).

## Return and async behavior

Returns the validated command descriptor (`{ commandId, object, around, direction }`) synchronously. The facade is synchronous and local; the key-driven selection query itself is a one-round-trip UI-reactive server request.

## Errors

The op fails (deny-by-default) if `object` is missing or unknown, if `direction` is present but unknown, or if the options are not valid JSON.

## Permissions and security

No additional permission is required beyond access to the running editor session. The server query is read-only: it never mutates the document, spawns processes, or loads artifacts.

Changes only transient client selection state from a read-only server grammar query; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientSelectTextobject` when the user asks for structural/text-object selections through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientSelectTextobject`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_select_textobject`
- Server query runner: `src/server/syntax.rs::TreeSitterSyntaxHandler::selection_query_ranges`
- Client application: `src/server/syntax.rs::TreeSitterSyntaxHandler::selection_query_ranges`
- Query files: `packages/rust/queries/textobjects.scm`, `packages/typescript/queries/textobjects.scm`, `packages/javascript/queries/textobjects.scm`

## Lookup metadata

- Stable ID: `editor.clientSelectTextobject`
- User-facing name: Select Textobject
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSelectTextobject`
- Default key bindings: none
- Custom properties: `object`, `around`, `direction`
- Tags: `editor`, `js-api`, `textobjects`, `tree-sitter`, `selection`
