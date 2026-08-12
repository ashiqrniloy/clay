---
id: commands.serverListCommands
kind: clay-js-api
js_module: "clay:commands"
js_export: serverListCommands
js_facade: runtime/js/commands.js::serverListCommands
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
agent_guidance: Use `commands.serverListCommands` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
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

No default key binding is assigned. Users may bind a key to `commands.serverListCommands` in `~/.config/clay/init.js`.

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

Use `commands.serverListCommands` when the user asks for List Commands through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/commands.js::serverListCommands`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_list_commands` (`op_clay_commands_list_commands`)
- Backing Rust/current owner: `src/packages/commands.rs::CommandRegistry::list`
- Current implementation audit path: `src/packages/commands.rs::CommandRegistry; src/packages/commands.rs::RegisteredCommand`

## Phase 24.2 control-center catalogue note

Phase 24.2 replaced the Control Center's earlier `serverListCommands`-style reuse with a dedicated generation-safe catalogue: the internal `ControlCenter` workflow (`src/server/control_center.rs`, `pub(crate)`) is fed by `RuntimeGenerationStore::command_catalogue_snapshot`, which merges built-in server commands (`builtin_server_command_ids`), the declared `shell.client*` pane/tab surface, and trusted/third-party package registry snapshots into one deterministically sorted, duplicate-fail-closed `CommandCatalogue` stamped with the runtime generation id. `shell.client*` entries (`ClientUiCommand` routing) are listed and, on activation, bridged back to the client shell driver through the server-approved `ShellClientCommandRequest` frame; only client-first edit commands stay excluded. Because the catalogue carries only validated command metadata, it grants no execution authority; activating a listed command dispatches through the same server-owned `CommandExecutor` boundary (or the narrow shell bridge), which re-validates before any side effect. There is no public `commands.serverExecuteCommand` JS facade/op — see [`serverRegisterCommand`](server-register-command.md#phase-188-command-execution-boundary) for the full command execution and transient menu boundary.

## Phase 19 built-in command discovery note

`serverListCommands` returns package-registered commands only. Built-in Clay-owned commands (such as `runtime.reloadConfiguration`, `controlCenter.open`, `workspace.openFuzzyFile`, `workspace.toggleFileBrowser`) are not listed by this API. The Control Center (`controlCenter.open`) builds its catalogue from the Rust `builtin_server_command_ids` table, the `shell.client*` surface, and the trusted/third-party package registry snapshots (see the Phase 24.2 note above). User configuration can bind built-in commands through [`bindKey`](../keybindings/bind-key.md) (a default `Ctrl+Shift+P` chord ships for `controlCenter.open`) or invoke them through SDUI actions; discovery is through documentation and the Control Center, not through this listing API.

## Lookup metadata

- Stable ID: `commands.serverListCommands`
- User-facing name: List Commands
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverListCommands`
- Default key bindings: none
- Custom properties: `scope`, `includePackageCommands`
- Tags: `js-api`, `commandregistryquery`, `commands`
