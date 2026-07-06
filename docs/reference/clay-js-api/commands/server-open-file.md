---
id: clay.commands.serverOpenFile
kind: clay-js-api
js_module: "clay:commands"
js_export: serverOpenFile
js_facade: runtime/js/commands.ts::serverOpenFile
backing_rust: src/server/command_execution.rs::CommandExecutor::execute_workspace; src/server/workspace.rs::WorkspaceState::open_existing_file; src/server/workspace.rs::WorkspaceState::open_selected_file
deno_op: op_clay_commands_execute_command
deno_op_path: src/server/ops/commands.rs::op_clay_commands_execute_command
name: serverOpenFile
user_facing_name: Open File
summary: Open a workspace-root-relative file or selected-file grant through the server command boundary.
owner: server
phase: Phase 18.12
visibility: public
permissions: ["workspace-read"]
key_bindings: []
custom_properties: []
security: Routes through clay.workspace.openFile and server workspace APIs; in-root paths are root-relative and out-of-root paths use selected-file single-file grants; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.commands.serverOpenFile` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [commands, workspace, open-file, selected-file-grant, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverOpenFile

## Summary

Open a workspace-root-relative file or selected-file grant through the server command boundary.

## Description

`serverOpenFile` is the Phase 18.12 runtime-backed Clay JS API for **Open File**. It is exposed through the curated `clay:commands` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs open file behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverOpenFile } from "clay:commands";

const handle = await serverOpenFile({ workspaceRootId: rootId, relativePath: "src/main.rs" });
```

## Example

```ts
const handle = await serverOpenFile({ workspaceRootId: rootId, relativePath: "src/main.rs" });
```

## Options

Either `workspaceRootId` + `relativePath` for in-root opens, or `absolutePath` for explicit selected-file fallback.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.commands.serverOpenFile` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a `DocumentHandle` with document id, version, and sanitized path.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots/documents, unauthorized targets, traversal escapes, oversize arguments, stale or missing files, unsupported file types, workspace limits, cancellation, and permission-denied filesystem conditions as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: ["workspace-read"].

Routes through clay.workspace.openFile and server workspace APIs; in-root paths are root-relative and out-of-root paths use selected-file single-file grants; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.commands.serverOpenFile` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/commands.ts::serverOpenFile`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_execute_command` (`op_clay_commands_execute_command`)
- Backing Rust/current owner: `src/server/command_execution.rs::CommandExecutor::execute_workspace; src/server/workspace.rs::WorkspaceState::open_existing_file; src/server/workspace.rs::WorkspaceState::open_selected_file`

## Lookup metadata

- Stable ID: `clay.commands.serverOpenFile`
- User-facing name: Open File
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverOpenFile`
- Default key bindings: none
- Tags: `[commands, workspace, open-file, selected-file-grant, phase18.12, js-api]`
