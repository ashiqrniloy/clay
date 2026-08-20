---
id: shell.clientTabActivate
kind: clay-js-api
js_module: "clay:shell"
js_export: clientTabActivate
js_facade: runtime/js/shell.js::clientTabActivate
backing_rust: src/driver/restore.rs::Driver::apply_tab_command (tab-order policy resolvers + execution); src/masonry_shell/window_tabs.rs::ShellClientCommand (command mapping)
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientTabActivate
user_facing_name: Activate Tab
summary: "Return the stable bindable command ID for activating a specific tab by number."
owner: client
phase: Phase 22.4
visibility: public
permissions: []
key_bindings: ["Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4", "Ctrl+5", "Ctrl+6", "Ctrl+7", "Ctrl+8", "Ctrl+9"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned tab state via TabCommand::Activate with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.
agent_guidance: "Use `shell.clientTabActivate` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, tabs, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientTabActivate

## Summary

Return the stable bindable command ID for activating a specific tab by number.

## Description

`clientTabActivate` is the public Clay JS API descriptor for **Activate Tab**. It returns the stable command ID `shell.clientTabActivate` so configuration, help, key-binding discovery, and agents can name the route without hard-coding Rust shortcuts.

Activate Tab The numbered family `shell.clientTabActivate.N` exists for `N` in `1..=9`; each variant activates the `N`-th tab in the user-visible card order (the server-authoritative `TabRegistry` order, entry-less mounted tabs appended). Positions beyond the tab count are silent no-ops; positions beyond 9 do not exist as command IDs (10+ tabs are reachable by next/prev or card click). The tab switch is optimistic client-side and reconciles against the server's pushed `TabRegistrySnapshot`.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Tab switching happens later only after an explicit user key/command route reaches the driver's tab-command dispatcher.

## When to use

Use this API when a user wants to bind an alternate next-tab chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientTabActivate } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+1", clientTabActivate(1), { scope: "global" });

bindKey("Ctrl+9", clientTabActivate(9), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+1", "shell.clientTabActivate.1", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientTabActivate } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+1", clientTabActivate(1), { scope: "global" });

bindKey("Ctrl+9", clientTabActivate(9), { scope: "global" });
```

The default `Ctrl+1`..`Ctrl+9` chords ship in Clay's `default_keymaps()` with `Global` context. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

`position` is required: an integer tab position `1..=9`. The helper returns the dotted variant ID `shell.clientTabActivate.<position>`; any other value throws `RangeError` (`shell.invalid_tab_position`).

## Key bindings

Default: `Ctrl+1`..`Ctrl+9` (Global context). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"shell.clientTabActivate"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper throws `RangeError` (`shell.invalid_tab_position`) for non-integer positions or values outside `1..=9`. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a silent no-op when the position exceeds the tab count.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it switches the active tab via `TabCommand::Activate` with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.

## Agent guidance

Use `shell.clientTabActivate` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientTabActivate`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/driver/restore.rs::Driver::apply_tab_command` (tab-order policy resolvers + execution); `src/masonry_shell/window_tabs.rs::ShellClientCommand` (command mapping)

## Lookup metadata

- Stable ID: `shell.clientTabActivate`
- User-facing name: Activate Tab
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientTabActivate`
- Default key bindings: `Ctrl+1`..`Ctrl+9`
- Custom properties: none
- Tags: `[shell, tabs, keybindings, js-api]`
