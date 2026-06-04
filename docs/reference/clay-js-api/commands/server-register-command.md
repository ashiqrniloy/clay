---
id: clay.commands.serverRegisterCommand
kind: clay-js-api
js_module: "clay:commands"
js_export: serverRegisterCommand
js_facade: runtime/js/commands.ts::serverRegisterCommand
backing_rust: src/packages/commands.rs::CommandRegistry::register_command
deno_op: op_clay_commands_register_command
deno_op_path: src/server/ops/commands.rs::op_clay_commands_register_command
name: serverRegisterCommand
user_facing_name: Register Command
summary: Register Command through the runtime-backed `clay:commands` Clay JavaScript facade.
owner: server
phase: Phase 16.5
visibility: public
permissions: ['command-registration']
key_bindings: []
custom_properties:
  - name: commandId
    type: string
    default: package-prefixed
    description: Behavior-changing setting `commandId` for this primitive gate API.
  - name: displayName
    type: string
    default: required
    description: Behavior-changing setting `displayName` for this primitive gate API.
  - name: routingPolicy
    type: enum
    default: server-first
    description: Behavior-changing setting `routingPolicy` for this primitive gate API.
  - name: defaultKeyBindings
    type: string[]
    default: []
    description: Behavior-changing setting `defaultKeyBindings` for this primitive gate API.
  - name: requiredPermissions
    type: string[]
    default: []
    description: Behavior-changing setting `requiredPermissions` for this primitive gate API.
security: Requires command-registration permission and server validation of package-prefixed command IDs, routing policy, key binding metadata, and declared handler permissions; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, package installation, enable/disable, or command handler authority by registration alone.
agent_guidance: Use `clay.commands.serverRegisterCommand` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, commandregistry, commands]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterCommand

## Summary

Register Command through the runtime-backed `clay:commands` Clay JavaScript facade.

## Description

`serverRegisterCommand` is the runtime-backed public primitive gate API for **Register Command**. It is documented so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or `Deno.core.ops` bindings.

Authority: `server-first-command-registration`. Runtime path: `server-first-op-wrapper`. Command registration occurs at package load time; command execution later follows the registered routing policy and is not part of local keypress-to-paint unless represented by an inert manifest route.

## When to use

Use this API when server-side Clay JavaScript package/configuration code needs the documented `Register Command` behavior. Do not use lower-level Rust functions, protocol structures, or raw `Deno.core.ops` names for this capability.

## JavaScript usage

```ts
import { serverRegisterCommand } from "clay:commands";

const command = serverRegisterCommand(manifest, { commandId: "markdown.togglePreview", displayName: "Toggle Markdown Preview", routingPolicy: "server-first" });
```

## Example

```ts
const command = serverRegisterCommand(manifest, { commandId: "markdown.togglePreview", displayName: "Toggle Markdown Preview", routingPolicy: "server-first" });
```

## Options

- `commandId` (`string`, default `package-prefixed`): Behavior-changing setting `commandId` for this API.
- `displayName` (`string`, default `required`): Behavior-changing setting `displayName` for this API.
- `routingPolicy` (`enum`, default `server-first`): Behavior-changing setting `routingPolicy` for this API.
- `defaultKeyBindings` (`string[]`, default `[]`): Behavior-changing setting `defaultKeyBindings` for this API.
- `requiredPermissions` (`string[]`, default `[]`): Behavior-changing setting `requiredPermissions` for this API.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.commands.serverRegisterCommand` in `~/.config/clay/init.js`.

## Custom properties

- `commandId` (`string`, default `package-prefixed`): Behavior-changing setting `commandId` for this API.
- `displayName` (`string`, default `required`): Behavior-changing setting `displayName` for this API.
- `routingPolicy` (`enum`, default `server-first`): Behavior-changing setting `routingPolicy` for this API.
- `defaultKeyBindings` (`string[]`, default `[]`): Behavior-changing setting `defaultKeyBindings` for this API.
- `requiredPermissions` (`string[]`, default `[]`): Behavior-changing setting `requiredPermissions` for this API.

## Return and async behavior

Returns JSON-serializable primitive gate metadata from the server-owned validator or registry. The facade is synchronous in the controlled server runtime and is intended for load-time, configuration-time, document-open, or activation-time work only.

The Phase 16.5 facade/runtime status is `runtime-backed`; the `deno_core` op wiring is executable during server-side configuration evaluation for runtime-backed entries.

## Errors

The runtime fails with actionable Clay error codes when arguments are malformed, package metadata fails server validation, required permissions are absent, duplicate prefixes/modes/commands are detected, ambiguous key bindings are found, or the requested primitive is intentionally unavailable.

## Permissions and security

Requires: command-registration

Requires command-registration permission and server validation of package-prefixed command IDs, routing policy, key binding metadata, and declared handler permissions; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, package installation, enable/disable, or command handler authority by registration alone.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.commands.serverRegisterCommand` when the user asks for Register Command through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/commands.ts::serverRegisterCommand`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_register_command` (`op_clay_commands_register_command`)
- Backing Rust/current owner: `src/packages/commands.rs::CommandRegistry::register_command`
- Current implementation audit path: `src/packages/commands.rs::CommandRegistry; src/packages/commands.rs::PackageCommandDeclaration`

## Lookup metadata

- Stable ID: `clay.commands.serverRegisterCommand`
- User-facing name: Register Command
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverRegisterCommand`
- Default key bindings: none
- Custom properties: `commandId`, `displayName`, `routingPolicy`, `defaultKeyBindings`, `requiredPermissions`
- Tags: `js-api`, `commandregistry`, `commands`
