---
id: shell.clientSplitPaneDown
kind: clay-js-api
js_module: "clay:shell"
js_export: clientSplitPaneDown
js_facade: runtime/js/shell.js::clientSplitPaneDown
backing_rust: src/masonry_shell/mod.rs::ClayShellWidget::apply_shell_client_command; src/shell/layout.rs::PaneSplitTree
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientSplitPaneDown
user_facing_name: "Split Pane Down"
summary: "Return the stable bindable command ID for the down-split alias: resolves to the same handler as splitting the focused pane stacked (horizontal divider line)."
owner: client
phase: Phase 22.7
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no server round-trip, no package JavaScript, no IPC). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Panes are generic content hosts; this command does not open files or grant document authority.
agent_guidance: "Use `shell.clientSplitPaneDown` only as a documented command ID for `bindKey` to name the down-split direction. It is an alias of `shell.clientSplitPaneHorizontal` (canonical ID, unchanged default `Ctrl+-` chord); prefer the canonical ID in new configuration. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, panes, splits, aliases, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientSplitPaneDown

## Summary

Return the stable bindable command ID for the down-split alias: resolves to the same handler as splitting the focused pane stacked (horizontal divider line).

## Description

`clientSplitPaneDown` is the public Clay JS API descriptor for **Split Pane Down** (Phase 22.7 alias). It returns the stable command ID `shell.clientSplitPaneDown` so configuration, help, key-binding discovery, and agents can name the direction without hard-coding Rust shortcuts.

The alias resolves to the existing `SplitPaneHorizontal` handler — the same stacked layout (`SplitOrientation::Vertical`) as `shell.clientSplitPaneHorizontal`: the focused pane keeps its top half and a new pane occupies the bottom half. The pane cap is `MAX_PANES_PER_TAB = 4`; the split is a no-op at cap.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Pane topology mutation happens later only after an explicit user key/command route reaches `ClayShellWidget::apply_shell_client_command`. The command operates purely client-side: bounded `PaneSplitTree` rebuild + `reconcile_pane_hosts`, no server round-trip, no package JavaScript, no IPC.

## When to use

Use this API when a user wants to bind a direction-named pane-management chord in `~/.config/clay/init.js`. New configuration should prefer the canonical `shell.clientSplitPaneHorizontal`; the alias exists for direction-named bindings and help discovery.

## JavaScript usage

```ts
import { clientSplitPaneDown } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+Down", clientSplitPaneDown(), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+Down", "shell.clientSplitPaneDown", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientSplitPaneDown } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+Down", clientSplitPaneDown(), { scope: "global" });
```

The alias carries no default chord; the canonical `Ctrl+-` binding for `shell.clientSplitPaneHorizontal` is unchanged.

## Options

No options are accepted. The command takes no arguments; pane topology is Clay-owned state.

## Key bindings

Default: none (alias — the canonical `shell.clientSplitPaneHorizontal` keeps its `Ctrl+-` default). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"shell.clientSplitPaneDown"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a no-op if the pane tree cannot accommodate the operation (e.g. cap reached, single pane, no bordering divider).

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no server round-trip, no package JavaScript, no IPC). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Panes are generic content hosts; this command does not open files or grant document authority.

## Agent guidance

Use `shell.clientSplitPaneDown` only as a documented command ID for `bindKey` to name the down-split direction. It is an alias of `shell.clientSplitPaneHorizontal` (canonical ID, unchanged default `Ctrl+-` chord); prefer the canonical ID in new configuration. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientSplitPaneDown`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_shell/mod.rs::ClayShellWidget::apply_shell_client_command` (alias routed in `ShellClientCommand::from_command_id`); `src/shell/layout.rs::PaneSplitTree`

## Lookup metadata

- Stable ID: `shell.clientSplitPaneDown`
- User-facing name: Split Pane Down
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientSplitPaneDown`
- Default key bindings: none (alias)
- Custom properties: none
- Tags: `[shell, panes, splits, aliases, keybindings, js-api]`
