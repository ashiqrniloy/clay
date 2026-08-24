---
id: shell.clientTabMoveTo
kind: clay-js-api
js_module: "clay:shell"
js_export: clientTabMoveTo
js_facade: runtime/js/shell.js::clientTabMoveTo
backing_rust: src/client_commands.rs::EditorClientCommand
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientTabMoveTo
user_facing_name: Move Tab to Position
summary: "Return the stable bindable command ID for moving the active tab to a specific position."
owner: client
phase: Phase 22.4
visibility: public
permissions: []
key_bindings: ["Ctrl+Shift+1", "Ctrl+Shift+2", "Ctrl+Shift+3", "Ctrl+Shift+4", "Ctrl+Shift+5", "Ctrl+Shift+6", "Ctrl+Shift+7", "Ctrl+Shift+8", "Ctrl+Shift+9"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned tab order via TabCommand::MoveTo with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.
agent_guidance: "Use `shell.clientTabMoveTo` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, tabs, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientTabMoveTo

## Summary

Return the stable bindable command ID for moving the active tab to a specific position.

## Description

`clientTabMoveTo` is the public Clay JS API descriptor for **Move Tab to Position**. It returns the stable command ID `shell.clientTabMoveTo` so configuration, help, key-binding discovery, and agents can name the route without hard-coding Rust shortcuts.

Move Tab to Position The numbered family `shell.clientTabMoveTo.N` exists for `N` in `1..=9`; each variant moves the active tab to the `N`-th card position via the server-validated `TabCommand::MoveTo`. The server's `TabRegistry` reorder preserves the active-tab status by `TabId`, rejects out-of-range positions (a pushed snapshot reconciles), and every mutation broadcasts a fresh snapshot. Positions beyond the tab count are silent client-side no-ops; positions beyond 9 do not exist as command IDs.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Tab switching happens later only after an explicit user key/command route reaches the React workspace controller's tab-command handler `frontend/src/shell/workspace-controller.ts` (React workspace controller).

## When to use

Use this API when a user wants to bind an alternate next-tab chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientTabMoveTo } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+1", clientTabMoveTo(1), { scope: "global" });

bindKey("Ctrl+Shift+9", clientTabMoveTo(9), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+1", "shell.clientTabMoveTo.1", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientTabMoveTo } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+1", clientTabMoveTo(1), { scope: "global" });

bindKey("Ctrl+Shift+9", clientTabMoveTo(9), { scope: "global" });
```

The default `Ctrl+Shift+1`..`Ctrl+Shift+9` chords ship in Clay's `default_keymaps()` with `Global` context. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

`position` is required: an integer tab position `1..=9`. The helper returns the dotted variant ID `shell.clientTabMoveTo.<position>`; any other value throws `RangeError` (`shell.invalid_tab_position`).

## Key bindings

Default: `Ctrl+Shift+1`..`Ctrl+Shift+9` (Global context). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"shell.clientTabMoveTo"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper throws `RangeError` (`shell.invalid_tab_position`) for non-integer positions or values outside `1..=9`. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a silent no-op when the position exceeds the tab count.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it switches the active tab via `TabCommand::Activate` with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.

## Agent guidance

Use `shell.clientTabMoveTo` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientTabMoveTo`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/client_commands.rs::ShellClientCommand (client-local; React tab bar, frontend/src/app/layout/tab-bar.tsx)` (tab-order policy resolvers + execution); `src/client_commands.rs::ShellClientCommand` (command mapping)

## Lookup metadata

- Stable ID: `shell.clientTabMoveTo`
- User-facing name: Move Tab to Position
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientTabMoveTo`
- Default key bindings: `Ctrl+Shift+1`..`Ctrl+Shift+9`
- Custom properties: none
- Tags: `[shell, tabs, keybindings, js-api]`
