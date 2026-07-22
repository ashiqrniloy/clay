---
id: clay.commands.serverRevealInTree
kind: clay-js-api
js_module: "clay:commands"
js_export: serverRevealInTree
js_facade: runtime/js/commands.js::serverRevealInTree
backing_rust: src/server/command_execution.rs::CommandExecutor::execute_workspace
deno_op: op_clay_commands_execute_command
deno_op_path: src/server/ops/commands.rs::op_clay_commands_execute_command
name: serverRevealInTree
user_facing_name: Reveal In Tree
summary: Validate a document reveal request for the Clay-owned file tree through CommandExecution.
owner: server
phase: Phase 18.12
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Validates the document id against open server workspace metadata and only affects Clay-owned file-browser focus state; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.commands.serverRevealInTree` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [commands, workspace, file-browser, reveal, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverRevealInTree

## Summary

Validate a document reveal request for the Clay-owned file tree through CommandExecution.

## Description

`serverRevealInTree` is the Phase 18.12 runtime-backed Clay JS API for **Reveal In Tree**. It is exposed through the curated `clay:commands` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs reveal in tree behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverRevealInTree } from "clay:commands";

await serverRevealInTree({ documentId });
```

## Example

```ts
await serverRevealInTree({ documentId });
```

## Options

`documentId` (`string`): an already-open document id.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.commands.serverRevealInTree` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise that resolves when the reveal command is accepted. The facade throws if the workspace status is not `revealed`.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots/documents, unauthorized targets, traversal escapes, oversize arguments, stale or missing files, unsupported file types, workspace limits, cancellation, and permission-denied filesystem conditions as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: [].

Validates the document id against open server workspace metadata and only affects Clay-owned file-browser focus state; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.commands.serverRevealInTree` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/commands.js::serverRevealInTree`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_execute_command` (`op_clay_commands_execute_command`)
- Backing Rust/current owner: `src/server/command_execution.rs::CommandExecutor::execute_workspace`

## Lookup metadata

- Stable ID: `clay.commands.serverRevealInTree`
- User-facing name: Reveal In Tree
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverRevealInTree`
- Default key bindings: none
- Tags: `[commands, workspace, file-browser, reveal, phase18.12, js-api]`
