---
id: shell.clientTabNew
kind: clay-js-api
js_module: "clay:shell"
js_export: clientTabNew
js_facade: runtime/js/shell.js::clientTabNew
backing_rust: src/main.rs::Driver::apply_tab_command (tab-order policy resolvers + execution); src/masonry_shell.rs::ShellClientCommand (command mapping)
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientTabNew
user_facing_name: New Tab
summary: "Return the stable bindable command ID for opening a new tab."
owner: client
phase: Phase 22.4
visibility: public
permissions: []
key_bindings: ["Ctrl+T"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it switches the active tab via TabCommand::Activate with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.
agent_guidance: "Use `shell.clientTabNew` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, tabs, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientTabNew

## Summary

Return the stable bindable command ID for opening a new tab.

## Description

`clientTabNew` is the public Clay JS API descriptor for **New Tab**. It returns the stable command ID `shell.clientTabNew` so configuration, help, key-binding discovery, and agents can name the route without hard-coding Rust shortcuts.

New Tab New Tab runs the same flow as the tab bar's `+` affordance: the native folder picker opens, the picked folder connects as a new independent client view, and the server registers the tab (server-validated `TabCommand::New`). A second new-tab request while one is in flight is ignored. Phase 22.8 binds the picked folder during the connection handshake with `TabCommand::New`; the server then owns that tab's workspace and welcome document. This API does not expose a `TabId`, workspace handle, or arbitrary-tab selector.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Tab switching happens later only after an explicit user key/command route reaches the driver's tab-command dispatcher.

## When to use

Use this API when a user wants to bind an alternate next-tab chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientTabNew } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+T", clientTabNew(), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+T", "shell.clientTabNew", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientTabNew } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+T", clientTabNew(), { scope: "global" });
```

The default `Ctrl+T` chord ships in Clay's `default_keymaps()` with `Global` context. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. The command takes no arguments; tab state is Clay-owned.

## Key bindings

Default: `Ctrl+T` (Global context). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"shell.clientTabNew"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a silent no-op with fewer than two tabs.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it switches the active tab via `TabCommand::Activate` with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.

## Agent guidance

Use `shell.clientTabNew` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientTabNew`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/main.rs::Driver::apply_tab_command` (tab-order policy resolvers + execution); `src/masonry_shell.rs::ShellClientCommand` (command mapping)

## Lookup metadata

- Stable ID: `shell.clientTabNew`
- User-facing name: New Tab
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientTabNew`
- Default key bindings: `Ctrl+T`
- Custom properties: none
- Tags: `[shell, tabs, keybindings, js-api]`
