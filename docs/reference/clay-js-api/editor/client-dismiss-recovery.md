---
id: clay.editor.clientDismissRecovery
kind: clay-js-api
js_module: "clay:editor"
js_export: clientDismissRecovery
js_facade: runtime/js/editor.js::clientDismissRecovery
backing_rust: src/masonry_editor.rs::EditorWidget::dismiss_recovery
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientDismissRecovery
user_facing_name: Dismiss Recovery
summary: Return the stable bindable command ID for dismissing pending-edit / disconnect / resync recovery chrome without mutating document text.
owner: client
phase: Phase 20
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it clears recovery menus and sanitized runtime diagnostics only and does not grant filesystem/workspace expansion, package/configuration/AI mutation authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientDismissRecovery` only as a documented command ID for `bindKey` or recovery menus; do not invent package-owned modal stacks or raw Deno ops.
lookup_tags: [editor, recovery, dismiss, sync, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientDismissRecovery

## Summary

Return the stable bindable command ID for dismissing pending-edit / disconnect / resync recovery chrome without mutating document text.

## Description

`clientDismissRecovery` is the public Clay JS API descriptor for **Dismiss Recovery**. It returns the stable command ID `clay.editor.clientDismissRecovery` so configuration and recovery menus can clear actionable sync chrome without touching the rope or server state.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. After an explicit user route, the editor clears the active recovery transient menu (when present) and drops the current runtime diagnostic used for disconnect/rejection/server-error recovery summaries. It does not reconnect, resync, save, reload, or discard local edits.

## When to use

Use this API for recovery-menu Dismiss actions or an optional bindable dismiss chord.

## JavaScript usage

```ts
import { clientDismissRecovery } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+D", clientDismissRecovery(), { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientDismissRecovery, clientRequestResync } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+R", clientRequestResync(), { scope: "editor" });
bindKey("Ctrl+Shift+D", clientDismissRecovery(), { scope: "editor" });
```

## Options

No options are accepted.

## Key bindings

No native default shortcut is assigned. Recovery menus invoke this command ID for Dismiss.

## Custom properties

None.

## Return and async behavior

Returns the string `"clay.editor.clientDismissRecovery"`. Synchronous. No IPC.

## Errors

This helper does not throw. If no recovery chrome is active, the later command is effectively a no-op.

## Permissions and security

Bindable client UI command ID only. Clears recovery menus and sanitized runtime diagnostics only. Does not grant filesystem/workspace expansion, package/configuration/AI mutation authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.editor.clientDismissRecovery` only as a documented command ID for `bindKey` or recovery menus. Do not invent package-owned modal stacks or raw Deno ops.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientDismissRecovery`
- Editor: `src/masonry_editor.rs::EditorWidget::dismiss_recovery`
- Keybinding allowlist: `src/server/ops/keybindings.rs`

## Stability notes

Runtime-backed Phase 20 client UI command for sync recovery chrome.

## Lookup metadata

- Stable ID: `clay.editor.clientDismissRecovery`
- User-facing name: Dismiss Recovery
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientDismissRecovery`
- Default key bindings: none
- Custom properties: none
- Tags: `[editor, recovery, dismiss, sync, keybindings, js-api]`
