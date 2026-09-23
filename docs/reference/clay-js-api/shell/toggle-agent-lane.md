---
id: shell.toggleAgentLane
kind: clay-js-api
js_module: "clay:shell"
js_export: toggleAgentLane
js_facade: runtime/js/shell.js::toggleAgentLane
backing_rust: src/client_commands.rs::ShellClientCommand::ToggleAgentLane; src/protocol/mod.rs::default_keymaps; frontend/src/shell/workspace-commands.ts::dispatchClientCommand
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: toggleAgentLane
user_facing_name: Toggle Agent Lane
summary: Return the stable bindable command ID for toggling the persistent per-tab agent lane.
owner: client
phase: Plan 124
visibility: public
permissions: []
key_bindings: ["Ctrl+X Ctrl+P"]
custom_properties: []
security: Names a bounded client UI command only; after explicit user routing it changes Clay-owned per-tab lane visibility and grants no filesystem, network, process, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use toggleAgentLane only as the documented command ID for bindKey when a user wants to show or hide the persistent agent lane; do not invent a direct lane, palette, layout, or filesystem API.
lookup_tags: [shell, agent-lane, visibility, keybindings, command, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# toggleAgentLane

## Summary

Return the stable bindable command ID for toggling the persistent per-tab agent lane.

## Description

`toggleAgentLane` is a synchronous, side-effect-free Clay JS command helper. It returns the stable command ID `shell.toggleAgentLane`; it does not mutate layout while configuration JavaScript is running and it does not open a second runtime or IPC path.

After an explicit user key or command route, Clay's shell flips the active tab's lane visibility. The lane is shared by that tab's Workspace and Agent views, starts visible when no saved value exists, and preserves its composer draft and agent state when hidden. Visibility is client-owned per-tab layout state and is persisted through the existing layout-state path.

This helper is a documented user-configuration surface, not a package-owned extension point. Packages cannot claim or replace the lane. The lane's composer owns the `/` palette, but the palette itself remains a Clay-owned server command session rather than a package-callable menu API.

## When to use

Use this API from trusted `~/.clay/init.js` configuration when a user wants to replace or supplement the shipped lane-toggle chord. Use the returned ID with `keybindings.bindKey`; do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`.

## JavaScript usage

```ts
import { toggleAgentLane } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+X Ctrl+P", toggleAgentLane(), { scope: "global" });
```

The equivalent stable-ID form is:

```ts
bindKey("Alt+L", "shell.toggleAgentLane", { scope: "global" });
```

The Control Center palette uses a separate built-in server command ID and chord:

```ts
// The palette is composer-anchored; this binds its launch route only.
bindKey("Ctrl+X Ctrl+O", "controlCenter.open", { scope: "global" });
```

There is intentionally no `clay:controlCenter` facade or direct `open()` call. The palette's `controlCenter.open` ID is documented in the keybinding contract because it opens a Clay-owned transient session; packages cannot open, populate, filter, intercept, or drive that session.

## Example

```ts
// ~/.clay/init.js
import { bindKey } from "clay:keybindings";
import { toggleAgentLane } from "clay:shell";

// Keep the shipped default explicit.
bindKey({
  scope: "global",
  bindings: {
    "Ctrl+X Ctrl+P": toggleAgentLane(),
    "Ctrl+X Ctrl+O": "controlCenter.open",
  },
});
```

The table form validates both bindings before applying either one. `controlCenter.open` is the palette's fixed server-first command ID; it is not a package command and its listing does not grant command, filesystem, or workspace authority.

## Options

No options are accepted. The helper takes no arguments; lane visibility, tab identity, draft state, composer contents, and palette session state are owned by Clay's shell and are not caller-supplied selectors.

## Key bindings

Default: `Ctrl+X Ctrl+P` in Global scope, with `ServerFirst` manifest routing. The server resolves the global chord outside editor focus and returns the narrow `ShellClientCommandRequest`; the client shell then applies the local per-tab toggle. Users may replace or supplement the default with `bindKey`, or remove it with `unbindKey`.

The composer palette has a separate default: `controlCenter.open` uses `Ctrl+X Ctrl+O` in Global scope with `ServerFirst` routing. That ID is a fixed built-in command target, not an export from this facade and not a package-callable menu API.

## Custom properties

No behavior-changing custom properties are defined. The lane's saved visibility is layout state, not a hidden `init.js` option; no palette geometry, catalogue membership, query, scope, or session property is configurable through this helper.

## Return and async behavior

Returns the string literal command ID `"shell.toggleAgentLane"` synchronously. Calling the helper performs no shell mutation, server request, file access, package loading, palette opening, or JavaScript execution in the client.

## Errors

The helper itself has no runtime errors. `bindKey` rejects malformed chords, unsupported scopes, strict-prefix collisions, or undocumented command IDs. A valid routed toggle is a no-op only when the active shell has no mounted lane owner, such as a source-tree fixture outside a tab; it never changes the active document or workspace root.

## Permissions and security

No additional permission is required to name or bind the command ID from trusted Clay configuration. The command changes only Clay-owned per-tab shell visibility after explicit user routing. It does not grant filesystem, network, process, shell, extension loading, AI mutation, workspace, package, WASM, raw `Deno.core.ops`, native widget, or client-side JavaScript authority.

The palette route is equally deny-by-default: `controlCenter.open` opens only Clay's composer-anchored, server-owned transient catalogue. It carries no filesystem, network, process, shell, extension loading, AI mutation, workspace, package, WASM, raw-op, native-widget, or client-side-JavaScript authority. Palette rows are metadata; activation is revalidated through Clay's command boundary, and package code cannot open or drive the session.

## Agent guidance

Use `toggleAgentLane()` when a user asks to bind, restore, or discover the persistent agent-lane visibility command. Prefer its returned ID over spelling `shell.toggleAgentLane` manually. Use the bare string `controlCenter.open` only with `bindKey` when configuring the separate palette launch route; do not suggest a `clay:controlCenter` import, direct menu session API, raw op, filesystem path, or client-side callback.

## Backing implementation

- JS facade: `runtime/js/shell.js::toggleAgentLane`
- Type declaration: `runtime/js/shell.d.ts::toggleAgentLane`
- Deno op used by the binding API: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key`
- Stable command declaration/default: `src/protocol/mod.rs::default_commands` and `src/protocol/mod.rs::default_keymaps`
- Client command mapping: `src/client_commands.rs::ShellClientCommand::ToggleAgentLane`
- Shell execution: `frontend/src/shell/workspace-commands.ts::dispatchClientCommand` and `frontend/src/shell/layout-state.ts::agentLane`
- Palette command/session boundary: `src/server/command_execution.rs::CONTROL_CENTER_COMMAND_ID`, `src/server/connection/menus.rs::open_command_centre_session`, and `src/server/menu_sessions.rs::ServerMenuSessions::open_control_center`

The helper follows the existing command-ID facade pattern: the only operation involved is the normal `keybindings.bindKey` validation path. No new hot-path operation or direct palette facade is added.

## Lookup metadata

- Stable ID: `shell.toggleAgentLane`
- User-facing name: Toggle Agent Lane
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `toggleAgentLane`
- Default key bindings: `Ctrl+X Ctrl+P`
- Custom properties: none
- Tags: `shell`, `agent-lane`, `visibility`, `keybindings`, `command`, `js-api`
- Related fixed command ID: `controlCenter.open` — Open Control Center, default `Ctrl+X Ctrl+O`, no standalone facade
