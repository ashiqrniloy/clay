---
id: clay.commands.serverOpenDirectory
kind: clay-js-api
js_module: "clay:commands"
js_export: serverOpenDirectory
js_facade: runtime/js/commands.ts::serverOpenDirectory
backing_rust: src/server/command_execution.rs::CommandExecutor::execute_workspace; src/server/workspace.rs::WorkspaceState::list_directory; src/server/connection.rs::file_browser_snapshot_message
deno_op: op_clay_commands_execute_command
deno_op_path: src/server/ops/commands.rs::op_clay_commands_execute_command
name: serverOpenDirectory
user_facing_name: Open Directory
summary: Navigate the Clay-owned workspace file browser to a root-relative directory through the server command boundary.
owner: server
phase: Phase 19
visibility: public
permissions: ["workspace-read"]
key_bindings: []
custom_properties: []
security: Routes through clay.workspace.openDirectory and server workspace APIs; directory paths are root-relative and validated inside a known workspace root, and this API does not grant filesystem/workspace authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, clipboard authority, or client-side JavaScript authority.
agent_guidance: Use `clay.commands.serverOpenDirectory` only through the documented Clay JS facade or Clay-owned SDUI file-browser actions; do not bind it as a global key without root-relative arguments, call raw Rust/protocol/ops, or invent broader filesystem/workspace authority.
lookup_tags: [commands, workspace, open-directory, file-browser, navigation, phase19, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverOpenDirectory

## Summary

Navigate the Clay-owned workspace file browser to a root-relative directory through the server command boundary.

## Description

`serverOpenDirectory` is the runtime-backed Clay JS API for **Open Directory**. It wraps the built-in `clay.workspace.openDirectory` command and returns the validated navigation target. The same command ID is used by Clay-owned file-browser SDUI directory rows, which include the required `workspaceRootId` and `relativePath` arguments.

This API is server-first background/action work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths. The server validates the directory with `WorkspaceState::list_directory` bounds and refreshes file-browser SDUI when invoked from the live file-browser action path.

## When to use

Use this API when server-side Clay JavaScript or future help/automation needs documented directory navigation through the command boundary. For ordinary UI, prefer the built-in file-browser directory rows; they already provide bounded root-relative arguments.

## JavaScript usage

```ts
import { serverOpenDirectory } from "clay:commands";

const location = await serverOpenDirectory({
  workspaceRootId: rootId,
  relativePath: "src",
});
```

## Example

```ts
const location = await serverOpenDirectory({ workspaceRootId: rootId, relativePath: "src" });
console.log(location.workspaceRootId, location.relativePath);
```

## Options

- `workspaceRootId` (`string`, required): Known workspace root ID.
- `relativePath` (`string`, optional): Directory path relative to the workspace root. Empty or omitted means the root directory.

Absolute paths, traversal escapes, glob patterns, ignore-rule changes, listing budgets, and native folder-picker behavior are not accepted options.

## Key bindings

No default key binding is assigned. This command needs root-relative arguments, so it is normally invoked by Clay-owned SDUI file-browser rows rather than a global key chord.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise resolving to `{ workspaceRootId, relativePath }` after server validation. The facade is asynchronous and uses the documented command execution op behind the curated `clay:commands` facade.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots, traversal escapes, non-directories, permission-denied filesystem conditions, listing bounds failures, and stale workspace state as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: `["workspace-read"]`.

Routes through `clay.workspace.openDirectory` and server workspace APIs; directory paths are root-relative and validated inside a known workspace root, and this API does not grant filesystem/workspace authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, clipboard authority, or client-side JavaScript authority.

The file browser remains Clay-owned SDUI, not a package/native widget. Packages/configuration cannot override marker files, ignore lists, listing budgets, directory traversal validation, or SDUI action validation through this API.

## Agent guidance

Use `clay.commands.serverOpenDirectory` only through the documented Clay JS facade or Clay-owned SDUI file-browser actions. Do not call raw Rust functions, protocol DTOs, raw `Deno.core.ops`, shell commands, arbitrary absolute paths, hidden workspace grants, package-manager actions, WASM, AI mutation, clipboard APIs, native widgets, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/commands.ts::serverOpenDirectory`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_execute_command` (`op_clay_commands_execute_command`)
- Backing Rust/current owner: `src/server/command_execution.rs::CommandExecutor::execute_workspace`; `src/server/workspace.rs::WorkspaceState::list_directory`; `src/server/connection.rs::file_browser_snapshot_message`

## Lookup metadata

- Stable ID: `clay.commands.serverOpenDirectory`
- User-facing name: Open Directory
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverOpenDirectory`
- Default key bindings: none
- Custom properties: none
- Tags: `[commands, workspace, open-directory, file-browser, navigation, phase19, js-api]`
