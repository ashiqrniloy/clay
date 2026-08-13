---
id: commands.serverRegisterCommand
kind: clay-js-api
js_module: "clay:commands"
js_export: serverRegisterCommand
js_facade: runtime/js/commands.js::serverRegisterCommand
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
agent_guidance: Use `commands.serverRegisterCommand` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
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

No default key binding is assigned. Users may bind a key to `commands.serverRegisterCommand` in `~/.config/clay/init.js`.

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

Use `commands.serverRegisterCommand` when the user asks for Register Command through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/commands.js::serverRegisterCommand`
- Deno op: `src/server/ops/commands.rs::op_clay_commands_register_command` (`op_clay_commands_register_command`)
- Backing Rust/current owner: `src/packages/commands.rs::CommandRegistry::register_command`
- Current implementation audit path: `src/packages/commands.rs::CommandRegistry; src/packages/commands.rs::PackageCommandDeclaration`

## Phase 18.8 command execution boundary

Phase 18.8 added the server-owned `CommandExecutor` (`src/server/command_execution.rs`) as the single normalization boundary for command execution. The following surfaces are intentionally **not** public Clay JS APIs and have no JavaScript facade, `Deno.core.ops` op, or inventory entry:

- **Command execution from JavaScript** — Phase 18.12 exposes `commands.serverExecuteCommand` for server-side controlled runtime code and first-party workspace surfaces. It still routes through `CommandExecutor` (command id, routing policy, package provenance, declared permissions, target context, argument budget, and session/action freshness) before any side effect. Workspace file-browser commands additionally route through `CommandExecutor::execute_workspace`, which re-checks roots and selected-file grants server-side. Packages still cannot bypass this with callbacks, raw paths, or raw `Deno.core.ops` names.
- **Transient menu sessions** — `TransientMenuSession` and related types (`src/shell/transient_menu.rs`) are Clay-owned session state (prompt, query, bounded items, selection, status, focus policy, inert activation actions) projected over the wire as bounded protocol DTOs (`src/protocol/menu.rs`); they are internal Rust types with no JavaScript facade. There is no `ui.serverOpenTransientMenu` facade/op; transient menu requests differ from fixed `PanelContribution`/`TransientOverlayContribution` panels because the menu owns dynamic query/selection state and is projected onto existing shell overlay primitives.
- **Control Center** — the Control Center (`src/server/control_center.rs`, `pub(crate)`) is a built-in command-palette workflow reached through the built-in server-first command `controlCenter.open` — shipped with the default `Ctrl+X Ctrl+P` chord (Global, `ServerFirst`) and overridable via [`keybindings.bindKey`](../keybindings/bind-key.md); it has no callable Clay JS facade of its own.
- **Path Browser** — the Phase 24.3 Path Browser (`PathBrowserSession` in `src/shell/path_browser.rs`, `BuiltInUserBrowseListing` in `src/server/workspace.rs`, both `pub(crate)`) is a built-in dired-style browse workflow reached through the built-in server-first command `controlCenter.openPath` — shipped with the Phase 24.5 `Ctrl+X Ctrl+F` sequence default (Global, `ServerFirst`, rebindable/removable via `bindKey`/`unbindKey` without changing the id); it has no callable Clay JS facade, no raw arbitrary-path op, no browse/grant/tab-id selector API, and packages cannot open, drive, intercept, or receive paths from it. Browse authority is ephemeral; activation converts it into exactly one `SingleFile` or `Directory` grant.

Command registration through this API declares metadata only — routing policy, permissions, key bindings, custom properties, and lookup tags — at package-load time. It grants **no execution authority**: execution authority is re-derived per activation through `CommandExecutor` from the registered routing policy, declared permissions, and provenance. Command execution and transient menu query/filter/selection updates are server-first or UI-reactive (local bounded filtering) and are not part of the ordinary keypress-to-paint, layout, scroll, text-event, edit acknowledgement, parse-result publication, or decoration rendering hot paths.

See `docs/reference/clay-js-api/configuration.md` (Phase 18.8 configuration review) and `docs/reference/primitives/package-security.md` for the full validation checklist and denied-authority list.

## Phase 19 built-in reload command boundary

`runtime.reloadConfiguration` (**Reload Configuration and Packages**) is a Clay-owned built-in global command registered through `builtin_server_command`. It is intentionally **not** a Clay JS API facade: there is no `clay:runtime`, no `clay:configuration.reloadConfiguration` export, no `commands.serverExecuteCommand("runtime.reloadConfiguration")` path, and no `Deno.core.ops` op for direct JavaScript invocation.

### Boundary

| Surface | Status |
|---|---|
| Clay JS facade | None — not callable from any `clay:*` module |
| `serverListCommands` output | Not listed (built-in commands are separate from package commands) |
| `builtin_server_command_ids` | Included — `pub(crate)` Rust-only lookup |
| Control Center | Discoverable (`RuntimeGenerationStore::command_catalogue_snapshot` builds the generation-stamped catalogue; built-ins merged with `shell.client*` and package registrations) |
| `bindKey` | Bindable with shipped global `Ctrl+Shift+R` default; override or restore with `bindKey`, remove with `unbindKey` |
| SDUI action | Routable — `SduiActionIntent { commandId: "runtime.reloadConfiguration" }` |
| Package JS via `serverExecuteCommand` | Rejected with `UnauthorizedTarget` ("runtime reload requires a user command intent") |
| Direct Rust call (`IpcServer::reload_runtime_generation`) | `pub(crate)` — exposed only to the command execution path and tests; `trigger_developer_hot_reload` is `#[doc(hidden)]` |

### Execution

Activating the command routes through `CommandExecutor` with `ServerFirstWithLock { lock_scope: Behavior }`. The executor validates command id, routing policy, provenance, declared permissions, target context, argument budget, and session/action freshness. On success, `IpcServer::execute_reload_command` acquires a reload-attempt guard (concurrent triggers return `ReloadInProgress`), then delegates to `reload_runtime_generation` which performs the full candidate prepare/commit cycle.

### Diagnostics

| Diagnostic code | Condition |
|---|---|
| `runtime.reload_succeeded` | Commit succeeded; G2 is active |
| `ReloadInProgress` | A concurrent reload is already evaluating/committing |
| `runtime.behavior_locked` | Behavior lock acquisition failed (another mutation in progress) |
| `runtime.snapshot_too_large` | Candidate snapshot exceeds the 1 MiB IPC frame ceiling |
| `packages.not_installed` | A configured package is not installed |
| `runtime.evaluation_failed` | Candidate JS evaluation threw an error |
| `runtime.incomplete_candidate` | Candidate evaluation produced no `ClayRuntimeEvaluation` |

All diagnostics are sanitized — they contain no raw source text, file paths, package internals, or token payloads.

### Authority

Reload does not broaden package source trust, process grants, filesystem access, network access, shell authority, extension loading, AI mutation, workspace expansion, WASM, raw-op, native-widget, client-side JavaScript, or package-manager authority. It reruns the same `~/.config/clay/init.js` in a fresh generation with an empty `globalThis.__clayLoadedPackages` cache. See `docs/reference/clay-js-api/configuration.md#phase-19-persistent-runtime-hot-reload-configuration-review` for the compiled budget table and rejected hidden keys.

## Lookup metadata

- Stable ID: `commands.serverRegisterCommand`
- User-facing name: Register Command
- Kind: `clay-js-api`
- Module/export: `clay:commands` / `serverRegisterCommand`
- Default key bindings: none
- Custom properties: `commandId`, `displayName`, `routingPolicy`, `defaultKeyBindings`, `requiredPermissions`
- Tags: `js-api`, `commandregistry`, `commands`
