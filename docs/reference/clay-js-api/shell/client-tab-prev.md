---
id: clay.shell.clientTabPrev
kind: clay-js-api
js_module: "clay:shell"
js_export: clientTabPrev
js_facade: runtime/js/shell.js::clientTabPrev
backing_rust: src/main.rs::Driver::apply_tab_command (tab-order policy resolvers + execution); src/masonry_shell.rs::ShellClientCommand (command mapping)
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientTabPrev
user_facing_name: Previous Tab
summary: "Return the stable bindable command ID for switching to the previous tab."
owner: client
phase: Phase 22.4
visibility: public
permissions: []
key_bindings: ["Ctrl+Shift+Tab"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it switches the active tab via TabCommand::Activate with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.
agent_guidance: "Use `clay.shell.clientTabPrev` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, tabs, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientTabPrev

## Summary

Return the stable bindable command ID for switching to the previous tab.

## Description

`clientTabPrev` is the public Clay JS API descriptor for **Previous Tab**. It returns the stable command ID `clay.shell.clientTabPrev` so configuration, help, key-binding discovery, and agents can name the route without hard-coding Rust shortcuts.

Previous Tab Previous Tab activates the tab before the active tab in the user-visible card order (the server-authoritative `TabRegistry` order, entry-less mounted tabs appended), wrapping from the first tab back to the last. With fewer than two tabs it is a silent no-op. The tab switch is optimistic client-side and reconciles against the server's pushed `TabRegistrySnapshot`.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Tab switching happens later only after an explicit user key/command route reaches the driver's tab-command dispatcher.

## When to use

Use this API when a user wants to bind an alternate next-tab chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientTabPrev } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+Tab", clientTabPrev(), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+Tab", "clay.shell.clientTabPrev", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientTabPrev } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+Tab", clientTabPrev(), { scope: "global" });
```

The default `Ctrl+Tab` chord ships in Clay's `default_keymaps()` with `Global` context. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. The command takes no arguments; tab state is Clay-owned.

## Key bindings

Default: `Ctrl+Tab` (Global context). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"clay.shell.clientTabPrev"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a silent no-op with fewer than two tabs.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it switches the active tab via `TabCommand::Activate` with server-confirmed snapshot reconciliation and no package JavaScript. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Tabs are independent client views; this command does not open files or grant document authority.

## Agent guidance

Use `clay.shell.clientTabPrev` only as a documented command ID for `bindKey` to remap the default Phase 22.4 tab-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Tab topology mutation is Clay-owned client behavior; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientTabPrev`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/main.rs::Driver::apply_tab_command` (tab-order policy resolvers + execution); `src/masonry_shell.rs::ShellClientCommand` (command mapping)

## Lookup metadata

- Stable ID: `clay.shell.clientTabPrev`
- User-facing name: Previous Tab
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientTabPrev`
- Default key bindings: `Ctrl+Tab`
- Custom properties: none
- Tags: `[shell, tabs, keybindings, js-api]`
