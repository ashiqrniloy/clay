---
id: workspace.clientOpenFolderDialog
kind: clay-js-api
js_module: "clay:workspace"
js_export: clientOpenFolderDialog
js_facade: runtime/js/workspace.js::clientOpenFolderDialog
backing_rust: src/client_commands.rs::EditorClientCommand; src/client/file_dialog.rs::open_folder_dialog; src/server/connection/workspace.rs::ClientMessage::AddSelectedWorkspaceRoot
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientOpenFolderDialog
user_facing_name: Open Folder Dialog
summary: Return the stable bindable command ID for Clay's native client folder-picker route.
owner: client
phase: Phase 19
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; native folder selection requires explicit user key routing, selected folders are sent through the server selected-path capability flow before becoming workspace roots, and this API does not grant filesystem/workspace authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, clipboard read/write, or client-side JavaScript authority.
agent_guidance: Use `workspace.clientOpenFolderDialog` as a documented command ID for `bindKey`; do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`, and do not invent dialog options, broad filesystem access, workspace expansion without server validation, package loading, shell/network effects, WASM, AI mutation, clipboard access, or client-side JavaScript execution.
lookup_tags: [workspace, open-folder, folder-dialog, xdg-desktop-portal, windows, macos, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientOpenFolderDialog

## Summary

Return the stable bindable command ID for Clay's native client folder-picker route.

## Description

`clientOpenFolderDialog` is the public Clay JS API descriptor for **Open Folder Dialog**. It returns the stable command ID `workspace.clientOpenFolderDialog` so configuration, help, key-binding discovery, and agents can refer to the native folder picker without hard-coding Rust shortcuts, protocol messages, or raw `Deno.core.ops` names.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. The native dialog opens only after an explicit user command. The selected path is then sent to the server with the current selected-path capability; the server validates it before adding a workspace root and publishing a refreshed file-browser SDUI snapshot.

## When to use

Use this API from `~/.config/clay/init.js` when a user wants a key binding for selecting a workspace folder.

## JavaScript usage

```ts
import { bindKey } from "clay:keybindings";
import { clientOpenFolderDialog } from "clay:workspace";

bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Shift+O", "workspace.clientOpenFolderDialog", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { bindKey } from "clay:keybindings";
import { clientOpenFolderDialog } from "clay:workspace";

bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
```

On Linux, Clay uses the xdg-desktop-portal FileChooser over D-Bus with `directory=true`. On Windows, Clay uses COM `IFileOpenDialog` with `FOS_PICKFOLDERS`. On macOS, Clay uses `NSOpenPanel` in directory-chooser mode. Other platforms report a sanitized unsupported diagnostic instead of panicking. Cancellation is a non-error no-op.

## Options

No options are accepted. Dialog backend, filters, start directory, multi-select, hidden-file policy, and workspace-grant behavior are not configurable through this API.

## Key bindings

No default key binding is assigned. Users may bind any supported chord with `bindKey`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"workspace.clientOpenFolderDialog"` synchronously. The helper does not open a dialog, scan files, call the server, mutate workspace state, or execute client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, unsupported `when` clauses, or undocumented command IDs. The native command path can report sanitized diagnostics for unsupported platforms or dialog failures. The server can reject stale/missing capabilities, nonexistent directories, traversal attempts, permission failures, and paths that do not validate as the explicit selected folder.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; native folder selection requires explicit user key routing, selected folders are sent through the server selected-path capability flow before becoming workspace roots, and this API does not grant filesystem/workspace authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, clipboard read/write, or client-side JavaScript authority.

The client owns the native prompt. The server owns workspace roots, canonicalizes/validates selected paths, rejects stale capabilities, and refreshes the Clay-owned file browser through SDUI. Package/configuration JavaScript cannot receive arbitrary native path handles or bypass workspace validation.

## Agent guidance

Use `workspace.clientOpenFolderDialog` as a documented command ID for `bindKey`. Avoid raw ops, direct Rust calls, ad hoc dialog options, shell commands, path passthrough, hidden workspace grants, network effects, package-manager actions, WASM, AI mutation, clipboard access, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/workspace.js::clientOpenFolderDialog`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/client/file_dialog.rs::open_folder_dialog`; `src/client/behavior.rs::ClientBehaviorState::route_key`; `src/server/connection/workspace.rs::ClientMessage::AddSelectedWorkspaceRoot`

## Lookup metadata

- Stable ID: `workspace.clientOpenFolderDialog`
- User-facing name: Open Folder Dialog
- Kind: `clay-js-api`
- Module/export: `clay:workspace` / `clientOpenFolderDialog`
- Default key bindings: none
- Custom properties: none
- Tags: `[workspace, open-folder, folder-dialog, xdg-desktop-portal, windows, macos, keybindings, js-api]`
