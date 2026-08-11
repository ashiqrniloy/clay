---
id: shell.clientAddEqualPane
kind: clay-js-api
js_module: "clay:shell"
js_export: clientAddEqualPane
js_facade: runtime/js/shell.js::clientAddEqualPane
backing_rust: src/masonry_shell.rs::ClayShellWidget::apply_shell_client_command; src/shell/layout.rs::PaneSplitTree
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientAddEqualPane
user_facing_name: "Add Equal Pane"
summary: "Return the stable bindable command ID for redividing the active tab’s working area into N+1 equal-area panes."
owner: client
phase: Phase 22.1
visibility: public
permissions: []
key_bindings: ["Ctrl+Shift+\\"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no server round-trip, no package JavaScript, no IPC). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Panes are generic content hosts; this command does not open files or grant document authority.
agent_guidance: "Use `shell.clientAddEqualPane` only as a documented command ID for `bindKey` to remap the default Phase 22.1 pane-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, panes, splits, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientAddEqualPane

## Summary

Return the stable bindable command ID for redividing the active tab’s working area into N+1 equal-area panes.

## Description

`clientAddEqualPane` is the public Clay JS API descriptor for **Add Equal Pane**. It returns the stable command ID `shell.clientAddEqualPane` so configuration, help, key-binding discovery, and agents can name the route without hard-coding Rust shortcuts.

Add Equal Pane builds a right-leaning comb tree with ratios `1/(N+1)`, `1/N`, ..., `1/2` so each leaf gets exactly `1/(N+1)` of the active tab’s working area. Existing panes retain their reading order; the new pane is appended. No-op at `MAX_PANES_PER_TAB = 4`.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Pane topology mutation happens later only after an explicit user key/command route reaches `ClayShellWidget::apply_shell_client_command`. The command operates purely client-side: bounded `PaneSplitTree` rebuild + `reconcile_pane_hosts`, no server round-trip, no package JavaScript, no IPC.

## When to use

Use this API when a user wants to bind an alternate pane-management chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientAddEqualPane } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+\\", clientAddEqualPane(), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+\\", "shell.clientAddEqualPane", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientAddEqualPane } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+\\", clientAddEqualPane(), { scope: "global" });
```

The default `Ctrl+Shift+\\` chord ships in Clay's `default_keymaps()` with `Global` context. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. The command takes no arguments; pane topology is Clay-owned state.

## Key bindings

Default: `Ctrl+Shift+\\` (Global context). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"shell.clientAddEqualPane"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a no-op if the pane tree cannot accommodate the operation (e.g. cap reached, single pane, no bordering divider).

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no server round-trip, no package JavaScript, no IPC). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Panes are generic content hosts; this command does not open files or grant document authority.

## Agent guidance

Use `shell.clientAddEqualPane` only as a documented command ID for `bindKey` to remap the default Phase 22.1 pane-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientAddEqualPane`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_shell.rs::ClayShellWidget::apply_shell_client_command`; `src/shell/layout.rs::PaneSplitTree`

## Lookup metadata

- Stable ID: `shell.clientAddEqualPane`
- User-facing name: Add Equal Pane
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientAddEqualPane`
- Default key bindings: `Ctrl+Shift+\\`
- Custom properties: none
- Tags: `[shell, panes, splits, keybindings, js-api]`
