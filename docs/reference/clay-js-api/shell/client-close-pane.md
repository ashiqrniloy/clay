---
id: shell.clientClosePane
kind: clay-js-api
js_module: "clay:shell"
js_export: clientClosePane
js_facade: runtime/js/shell.js::clientClosePane
backing_rust: src/masonry_shell/mod.rs::ClayShellWidget::apply_shell_client_command; src/shell/layout.rs::PaneSplitTree; src/app_driver.rs::Driver (dirty guard, document release, conflict-menu sync); src/masonry_pane_document.rs::PaneDocumentView::guard_pane_close / close_pane
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientClosePane
user_facing_name: "Close Pane"
summary: "Return the stable bindable command ID for closing the focused pane (dirty documents protected; clean panes release their document lease)."
owner: client
phase: Phase 22.2
visibility: public
permissions: []
key_bindings: ["Ctrl+Alt+W"]
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no package JavaScript, no IPC for topology). Since Phase 22.2, closing a pane with a dirty document is blocked, and closing a clean pane releases its document lease only through the server's existing capability-gated close path. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. This command does not open files or grant document authority.
agent_guidance: "Use `shell.clientClosePane` only as a documented command ID for `bindKey` to remap the default Phase 22.1 pane-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only."
lookup_tags: [shell, panes, splits, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientClosePane

## Summary

Return the stable bindable command ID for closing the focused pane. Since Phase 22.2 a pane whose document is dirty is protected (the save-conflict menu must resolve first); closing a clean pane releases its document lease.

## Description

`clientClosePane` is the public Clay JS API descriptor for **Close Pane**. It returns the stable command ID `shell.clientClosePane` so configuration, help, key-binding discovery, and agents can name the route without hard-coding Rust shortcuts.

Close Pane removes the focused leaf and promotes its sibling subtree. The last pane is protected (no-op). The active pane becomes the sibling's first leaf.

Since Phase 22.2 panes host document views of the tab's workspace, so closing a pane with an open document is document-aware:

- A **dirty** document blocks the close: the pane's save-conflict menu (the existing server-owned save/reload path) must resolve first. No lease is released and no topology changes until the conflict is resolved.
- A **clean** pane releases its document: the pane sends capability-gated close requests for its active document and every retained session in its session store (bounded by the 64-session ceiling), then closes the pane. The release goes through the same server authority as every document close; the client never drops a lease unilaterally.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. Pane topology mutation happens later only after an explicit user key/command route reaches the driver, which guards dirty documents, releases clean documents through `PaneDocumentView::close_pane`, and rebuilds the bounded `PaneSplitTree` + `reconcile_pane_hosts` in `ClayShellWidget`. Topology mutation is client-local; document release is the only server round-trip and it is capability-gated.

## When to use

Use this API when a user wants to bind an alternate pane-management chord in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientClosePane } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+W", clientClosePane(), { scope: "global" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Alt+W", "shell.clientClosePane", { scope: "global" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientClosePane } from "clay:shell";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+W", clientClosePane(), { scope: "global" });
```

The default `Ctrl+Alt+W` chord ships in Clay's `default_keymaps()` with `Global` context. This API exists for documented keybinding/configuration metadata and alternate chords. The chord is a no-op while a dirty-document conflict menu is pending in the focused pane.

## Options

No options are accepted. The command takes no arguments; pane topology is Clay-owned state.

## Key bindings

Default: `Ctrl+Alt+W` (Global context). Additional bindings may be configured with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"shell.clientClosePane"` synchronously. The helper does not touch the shell, call the server, execute package code, mutate document text, read files, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, or undocumented command IDs. The native command path is a no-op if the pane tree cannot accommodate the operation (e.g. cap reached, single pane, no bordering divider) and is blocked while the focused pane's dirty document conflict is unresolved.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; after explicit user routing it mutates only the Clay-owned pane/split tree on the client (no package JavaScript, no IPC for topology). Dirty panes are protected; clean panes release their document lease through the server's capability-gated close path. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget handles, or client-side JavaScript authority. This command does not open files or grant document authority.

## Agent guidance

Use `shell.clientClosePane` only as a documented command ID for `bindKey` to remap the default Phase 22.1 pane-management chord. Avoid raw Rust calls, protocol DTOs, or `Deno.core.ops`. Pane topology mutation is Clay-owned; packages interact through inert `serverRequestLayoutIntent` only.

## Backing implementation

- JS facade: `runtime/js/shell.js::clientClosePane`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/app_driver.rs::Driver::apply_shell_client_command` (dirty guard, document release, conflict-menu sync); `src/masonry_shell/mod.rs::ClayShellWidget::apply_shell_client_command`; `src/shell/layout.rs::PaneSplitTree`; `src/masonry_pane_document.rs::PaneDocumentView::guard_pane_close` / `close_pane`

## Lookup metadata

- Stable ID: `shell.clientClosePane`
- User-facing name: Close Pane
- Kind: `clay-js-api`
- Module/export: `clay:shell` / `clientClosePane`
- Default key bindings: `Ctrl+Alt+W`
- Custom properties: none
- Tags: `[shell, panes, splits, keybindings, js-api]`
