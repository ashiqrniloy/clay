---
id: clay.commands.serverListCommands
kind: clay-js-api
js_module: "clay:commands"
js_export: serverListCommands
js_facade: runtime/js/commands.ts::serverListCommands
backing_rust: src/packages/commands.rs::CommandRegistry::list
deno_op: op_clay_commands_list_commands
deno_op_path: src/server/ops/commands.rs::op_clay_commands_list_commands
name: serverListCommands
user_facing_name: List Commands
summary: List Commands through the runtime-backed `clay:commands` Clay JavaScript facade.
owner: server
phase: Phase 16.5
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: scope
    type: enum
    default: all
    description: Behavior-changing setting `scope` for this primitive gate API.
  - name: includePackageCommands
    type: boolean
    default: true
    description: Behavior-changing setting `includePackageCommands` for this primitive gate API.
security: Returns command metadata only after server validation of registry visibility; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, package installation, enable/disable, command execution, or command handler authority.
agent_guidance: Use `clay.commands.serverListCommands` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, commandregistryquery, commands]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverListCommands

## Summary

List Commands through the runtime-backed `clay:commands` Clay JavaScript facade.

## Description

`serverListCommands` is the runtime-backed public primitive gate API for **List Commands**. It is documented so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or `Deno.core.ops` bindings.

Authority: `server-owned-command-query`. Runtime path: `server-first-query`. Command listing is a help/configuration/agent metadata query and never participates in ordinary keypress, layout, paint, or edit acknowledgement hot paths.

## When to use

Use this API when server-side Clay JavaScript package/configuration code needs the documented `List Commands` behavior. Do not use lower-level Rust functions, protocol structures, or raw `Deno.core.ops` names for this capability.

## JavaScript usage

```ts
import { serverListCommands } from "clay:commands";

const commands = serverListCommands();
```

## Example

```ts
const commands = serverListCommands();
```

## Options

- `scope` (`enum`, default `all`): Behavior-changing setting `scope` for this API.
- `includePackageCommands` (`boolean`, default `true`): Behavior-changing setting `includePackageCommands` for this API.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.commands.serverListCommands` in `~/.config/clay/init.js`.

## Custom properties

- `scope` (`enum`, default `all`): Behavior-changing setting `scope` for this API.
- `includePackageCommands` (`boolean`, default `true`): Behavior-changing setting `includePackageCommands` for this API.

## Return and async behavior

Returns JSON-serializable primitive gate metadata from the server-owned validator or registry. The facade is synchronous in the controlled server runtime and is intended for load-time, configuration-time, document-open, or activation-time work only.

The Phase 16.5 facade/runtime status is `runtime-backed`; the `deno_core` op wiring is executable during server-side configuration evaluation for runtime-backed entries.

## Errors

The runtime fails with actionable Clay error codes when arguments are malformed, package metadata fails server validation, required permissions are absent, duplicate prefixes/modes/commands are detected, ambiguous key bindings are found, or the requested primitive is intentionally unavailable.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Returns command metadata only after server validation of registry visibility; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, package installation, enable/disable, command execution, or command handler authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.commands.serverListCommands` when the user asks for List Commands through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/commands.ts::serverListCommands`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_list_commands` (`op_clay_commands_list_commands`)
- Backing Rust/current owner: `src/packages/commands.rs::CommandRegistry::list`
- Current implementation audit path: `src/packages/commands.rs::CommandRegistry; src/packages/commands.rs::RegisteredCommand`

## Lookup metadata

- Stable ID: `clay.commands.serverListCommands`
- User-facing name: List Commands
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverListCommands`
- Default key bindings: none
- Custom properties: `scope`, `includePackageCommands`
- Tags: `js-api`, `commandregistryquery`, `commands`
