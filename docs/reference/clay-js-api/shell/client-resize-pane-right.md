---
id: clay.shell.clientResizePaneRight
kind: clay-js-api
js_module: "clay:shell"
js_export: clientResizePaneRight
js_facade: runtime/js/shell.js::clientResizePaneRight
backing_rust: src/masonry_shell.rs::ClayShellWidget::apply_shell_client_command; src/shell/layout.rs::PaneSplitTree
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientResizePaneRight
user_facing_name: "Resize Pane Right"
summary: "Return the stable bindable command ID for growing the focused pane toward the right."
owner: client
phase: Phase 22.1
visibility: public
permissions: []
key_bindings: ["Ctrl+Alt+Shift+Right"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no server round-trip, no package JavaScript, no IPC). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Panes are generic content hosts; this command does not open files or grant document authority.
agent_guidance: "Use `clay.shell.clientResizePaneRight` only as a documented command ID for `bindKey` to remap the default Phase 22.1 pane-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, panes, splits, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientResizePaneRight

## Summary

Return the stable bindable command ID for growing the focused pane toward the right.

## Description

`clientResizePaneRight` is the public Clay JS API descriptor for **Resize Pane Right**. It returns the stable command ID `clay.shell.clientResizePaneRight` so configuration, help, key-binding discovery, and agents can name the route without hard-coding Rust shortcuts.

Resize Right adjusts the ratio of the deepest ancestor Horizontal split whose first child contains the focused pane, shrinking the right neighbor. Clamped to `MIN_SPLIT_RATIO`/`MAX_SPLIT_RATIO` in `KEYBOARD_RESIZE_STEP` increments. No-op if no bordering divider exists.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Pane topology mutation happens later only after an explicit user key/command route reaches `ClayShellWidget::apply_shell_client_command`. The command operates purely client-side: bounded `PaneSplitTree` rebuild + `reconcile_pane_hosts`, no server round-trip, no package JavaScript, no IPC.

## When to use

Use this API when a user wants to bind an alternate pane-management chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientResizePaneRight } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+Shift+Right", clientResizePaneRight(), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Alt+Shift+Right", "clay.shell.clientResizePaneRight", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientResizePaneRight } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+Shift+Right", clientResizePaneRight(), { scope: "global" });
```

The default `Ctrl+Alt+Shift+Right` chord ships in Clay's `default_keymaps()` with `Global` context. This API exists for documented keybinding/configuration metadata and alternate chords.

## Options

No options are accepted. The command takes no arguments; pane topology is Clay-owned state.

## Key bindings

Default: `Ctrl+Alt+Shift+Right` (Global context). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"clay.shell.clientResizePaneRight"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a no-op if the pane tree cannot accommodate the operation (e.g. cap reached, single pane, no bordering divider).

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no server round-trip, no package JavaScript, no IPC). Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. Panes are generic content hosts; this command does not open files or grant document authority.

## Agent guidance

Use `clay.shell.clientResizePaneRight` only as a documented command ID for `bindKey` to remap the default Phase 22.1 pane-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientResizePaneRight`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/masonry_shell.rs::ClayShellWidget::apply_shell_client_command`; `src/shell/layout.rs::PaneSplitTree`

## Lookup metadata

- Stable ID: `clay.shell.clientResizePaneRight`
- User-facing name: Resize Pane Right
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientResizePaneRight`
- Default key bindings: `Ctrl+Alt+Shift+Right`
- Custom properties: none
- Tags: `[shell, panes, splits, keybindings, js-api]`
