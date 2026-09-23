---
id: commands.serverExecuteCommand
kind: clay-js-api
js_module: "clay:commands"
js_export: serverExecuteCommand
js_facade: runtime/js/commands.js::serverExecuteCommand
backing_rust: src/server/ops/mod.rs::ClayOpState::execute_command; src/server/command_execution.rs::CommandExecutor::execute_workspace
deno_op: op_clay_commands_execute_command
deno_op_path: src/server/ops/commands.rs::op_clay_commands_execute_command
name: serverExecuteCommand
user_facing_name: Execute Command
summary: Execute a registered server command through the CommandExecution validation boundary.
owner: server
phase: Phase 18.12
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Revalidates command id, routing policy, provenance, permissions, target context, and argument budget before side effects; workspace commands re-check roots and selected-file grants; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `commands.serverExecuteCommand` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [commands, execution, workspace, file-browser, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverExecuteCommand

## Summary

Execute a registered server command through the CommandExecution validation boundary.

## Description

`serverExecuteCommand` is the Phase 18.12 runtime-backed Clay JS API for **Execute Command**. It is exposed through the curated `clay:commands` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It must not run in ordinary typing, client paint, client layout, pointer, scroll, keypress, or text-event hot paths. Phase 22.8 does not add a tab selector: connection-level workspace commands resolve the caller's bound `TabServerState`, and this facade exposes neither arbitrary `TabId` access nor a `TabServerState` handle.

## When to use

Use this API when server-side Clay JavaScript needs execute command behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverExecuteCommand } from "clay:commands";

const result = await serverExecuteCommand("workspace.revealInTree", { documentId });
```

## Example

```ts
const result = await serverExecuteCommand("workspace.revealInTree", { documentId });
```

## Options

`commandId` (`string`), `args` (`object`, optional), and `target` (`activeDocument`, `workspace`, or `global`, optional).

## Key bindings

No default key binding is assigned. Users may bind a key to `commands.serverExecuteCommand` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns `CommandExecutionResult` with command id, routing policy, target, and accepted/discovery/workspace status payload.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots/documents, unauthorized targets, traversal escapes, oversize arguments, stale or missing files, unsupported file types, workspace limits, cancellation, and permission-denied filesystem conditions as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: [].

Revalidates command id, routing policy, provenance, permissions, target context, and argument budget before side effects; workspace commands re-check roots and selected-file grants; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `commands.serverExecuteCommand` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/commands.js::serverExecuteCommand`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_execute_command` (`op_clay_commands_execute_command`)
- Backing Rust/current owner: `src/server/ops/mod.rs::ClayOpState::execute_command; src/server/command_execution.rs::CommandExecutor::execute_workspace`

## Lookup metadata

- Stable ID: `commands.serverExecuteCommand`
- User-facing name: Execute Command
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverExecuteCommand`
- Default key bindings: none
- Tags: `[commands, execution, workspace, file-browser, phase18.12, js-api]`
