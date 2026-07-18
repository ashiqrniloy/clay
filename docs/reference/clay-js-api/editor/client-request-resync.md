---
id: clay.editor.clientRequestResync
kind: clay-js-api
js_module: "clay:editor"
js_export: clientRequestResync
js_facade: runtime/js/editor.ts::clientRequestResync
backing_rust: src/masonry_editor.rs::EditorWidget::request_resync_active_document; src/client/mod.rs::ClientEditQueue::enqueue_request_resync
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientRequestResync
user_facing_name: Request Resync
summary: Return the stable bindable command ID for requesting a canonical document resync snapshot for the active document.
owner: client
phase: Phase 20
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it enqueues ClientMessage::RequestResync for the active document under an ordinary inverse edit recovery path and does not grant filesystem/workspace expansion, package/configuration/AI mutation authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.editor.clientRequestResync` only as a documented command ID for `bindKey` or recovery menus; do not invent client reconnect sockets, package-owned resync loops, or raw Deno ops.
lookup_tags: [editor, resync, recovery, sync, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientRequestResync

## Summary

Return the stable bindable command ID for requesting a canonical document resync snapshot for the active document.

## Description

`clientRequestResync` is the public Clay JS API descriptor for **Request Resync**. It returns the stable command ID `clay.editor.clientRequestResync` so configuration and recovery menus can name the explicit resync route without hard-coding protocol messages.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. After an explicit user key/command route, the native editor enqueues `ClientMessage::RequestResync` for the active document's known version. Stale/lease/read-only/region-lock rejections may already auto-request resync in the connection task; this command is the explicit user-facing affordance when recovery UX offers Resync or when `init.js` binds a chord.

Disconnected/local-fallback editors refuse the request with a sanitized runtime diagnostic instead of inventing a reconnect socket.

## When to use

Use this API when binding an explicit resync chord or when recovery menus need a stable command ID.

## JavaScript usage

```ts
import { clientRequestResync } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+R", clientRequestResync(), { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientRequestResync, clientDismissRecovery } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+R", clientRequestResync(), { scope: "editor" });
bindKey("Escape", clientDismissRecovery(), { scope: "editor" });
```

## Options

No options are accepted.

## Key bindings

No native default shortcut is assigned. Bind with `bindKey` when desired. Recovery menus also invoke this command ID directly.

## Custom properties

None.

## Return and async behavior

Returns the string `"clay.editor.clientRequestResync"`. Synchronous. The later `RequestResync` IPC is non-blocking for paint.

## Errors

This helper does not throw. Runtime diagnostics appear when there is no edit queue, the editor is disconnected/local-fallback, or the outbound queue is full.

## Permissions and security

Bindable client UI command ID only. After explicit user routing it enqueues `ClientMessage::RequestResync` for the active document under an ordinary inverse edit recovery path. Does not grant filesystem/workspace expansion, package/configuration/AI mutation authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.editor.clientRequestResync` only as a documented command ID for `bindKey` or recovery menus. Do not invent client reconnect sockets, package-owned resync loops, or raw Deno ops.

## Backing implementation

- JS facade: `runtime/js/editor.ts::clientRequestResync`
- Editor: `src/masonry_editor.rs::EditorWidget::request_resync_active_document`
- Queue: `src/client/mod.rs::ClientEditQueue::enqueue_request_resync`
- Keybinding allowlist: `src/server/ops/keybindings.rs`

## Stability notes

Runtime-backed Phase 20 client UI command for sync recovery.

## Lookup metadata

- Stable ID: `clay.editor.clientRequestResync`
- User-facing name: Request Resync
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientRequestResync`
- Default key bindings: none
- Custom properties: none
- Tags: `[editor, resync, recovery, sync, keybindings, js-api]`
