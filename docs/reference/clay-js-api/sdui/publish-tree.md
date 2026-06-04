---
id: clay.sdui.publishTree
kind: clay-js-api
js_module: "clay:sdui"
js_export: publishTree
js_facade: runtime/js/sdui.ts::publishTree
backing_rust: src/server/sdui.rs::validate_runtime_tree
deno_op: op_clay_sdui_publish_tree
deno_op_path: src/server/ops/sdui.rs::op_clay_sdui_publish_tree
name: publishTree
user_facing_name: Publish SDUI Tree
summary: Publish a runtime-built inert server-driven UI tree through the `clay:sdui` server-side facade.
owner: server
phase: Phase 13
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: tree
    type: SduiNodeDefinition
    default: "required"
    description: Root node definition previously built with `clay:sdui` helpers.
security: Publishes inert declarative UI metadata only after server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.sdui.publishTree` only from server-side configuration or extension code after building inert SDUI nodes with documented helpers; never call raw ops or embed executable client script.
lookup_tags: [sdui, server-driven-ui, js-api, phase13, runtime-backed, configuration]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# publishTree

## Summary

Publish a runtime-built inert server-driven UI tree through the `clay:sdui` server-side facade.

## Description

`publishTree` is the public Phase 13 Clay JS API for committing an SDUI node graph built with `definePanel`, `defineFlex`, `defineEditorView`, and related helpers. The server converts the JSON-compatible node definition into typed Rust SDUI protocol structures, validates IDs, editor bindings, node kinds, and action intents, then stores the validated tree for publication through the existing SDUI snapshot/update path.

The API publishes native UI metadata only. It does not execute JavaScript on the client, mutate documents, open files, contact the network, or bypass server validation.

## When to use

Use this API from `~/.config/clay/init.js` or a local configuration module when configuration should replace or augment the server-driven UI tree with documented native UI nodes.

## JavaScript usage

```ts
import { defineFlex, definePanel, defineEditorView, publishTree } from "clay:sdui";

await publishTree(
  defineFlex({
    direction: "row",
    children: [
      definePanel({ title: "Workspace", children: [] }),
      defineEditorView({ documentId: 1 }),
    ],
  }),
);
```

## Example

```ts
const root = definePanel({
  title: "Configured UI",
  children: [defineEditorView({ documentId: 1 })],
});

await publishTree(root);
```

## Options

- `tree` (`SduiNodeDefinition`, required): Root node definition returned by a documented SDUI helper. Children may contain supported panel, label, button, list, editor view, flex, and stack node definitions.

## Key bindings

No default key binding is assigned. Users should not bind keys directly to `publishTree`; use configuration modules to publish UI during startup or explicit reload work.

## Custom properties

- `tree` (`SduiNodeDefinition`, default `required`): Root node definition validated and published by the server.

## Return and async behavior

Returns `Promise<void>`. The current facade calls the server op synchronously and exposes an async API so future publication acknowledgements can remain source compatible.

## Errors

Fails when the tree is not JSON-serializable, node options are malformed, node IDs are invalid, children are not arrays, editor bindings reference an invalid document, action intents contain unknown commands, action arguments are non-primitive, or server SDUI validation rejects the resulting tree.

## Permissions and security

No additional permission is required to publish inert UI metadata from already-authorized server-side configuration code. The server validates all node data before it reaches the client.

Publishes inert declarative UI metadata only after server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.sdui.publishTree` only with documented SDUI node helpers and only in server-side configuration/runtime contexts. Avoid raw `Deno.core.ops`, raw Rust/protocol construction, executable action payloads, arbitrary command IDs, filesystem effects, network effects, shell commands, extension loading, AI mutation, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/sdui.ts::publishTree`
- Deno op: `src/server/ops/sdui.rs::op_clay_sdui_publish_tree` (`op_clay_sdui_publish_tree`)
- Rust function: `src/server/sdui.rs::validate_runtime_tree`
- Runtime conversion path: `src/server/ops/sdui.rs::runtime_tree_from_json`

## Lookup metadata

- Stable ID: `clay.sdui.publishTree`
- User-facing name: Publish SDUI Tree
- Kind: `clay-js-api`
- Module/export: `clay:sdui` / `publishTree`
- Default key bindings: none
- Custom properties: `tree`
- Tags: `sdui`, `server-driven-ui`, `js-api`, `phase13`, `runtime-backed`, `configuration`
