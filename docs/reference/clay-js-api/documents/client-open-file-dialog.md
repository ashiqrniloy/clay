---
id: documents.clientOpenFileDialog
kind: clay-js-api
js_module: "clay:documents"
js_export: clientOpenFileDialog
js_facade: runtime/js/documents.js::clientOpenFileDialog
backing_rust: src/client/file_dialog.rs::FileDialogResult; src/client/file_dialog.rs::open_markdown_file_dialog; src/app_driver.rs::handle_client_ui_command
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientOpenFileDialog
user_facing_name: Open File Dialog
summary: Return the stable bindable command ID for Clay's native client file-open dialog route.
owner: client
phase: Phase 19
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; native dialog execution requires explicit user key routing, selected files are server-validated as single-file grants, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, broad filesystem/workspace authority, or client-side JavaScript authority.
agent_guidance: Use `documents.clientOpenFileDialog` as a documented command ID for `bindKey`; do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`, and do not invent dialog options, broad filesystem access, workspace expansion, package loading, shell/network effects, WASM, AI mutation, or client-side JavaScript execution.
lookup_tags: [documents, open-dialog, file-dialog, windows, linux, macos, markdown, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientOpenFileDialog

## Summary

Return the stable bindable command ID for Clay's native client file-open dialog route.

## Description

`clientOpenFileDialog` is the public Clay JS API descriptor for **Open File Dialog**. It returns the stable command ID `documents.clientOpenFileDialog` so configuration, help, key-binding discovery, and agents can refer to the native file-open command without hard-coding Rust shortcuts, raw protocol messages, or raw `Deno.core.ops` names.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. Binding/routing is a local inert behavior-manifest lookup; the native dialog and selected-file open happen only after an explicit user command. Opening reads one initial UTF-8 snapshot through the server-selected-file path; ordinary editing remains delta-based, and Markdown parse/decorations run as background, viewport-bounded work rather than keypress, paint, scroll, layout, or text-event JavaScript.

## When to use

Use this API when JavaScript configuration, extension metadata, help, or future Clay automation needs the documented command ID for opening the native file dialog. Bind the returned ID with `bindKey`; do not expect calling this helper to open the OS dialog directly.

## JavaScript usage

```ts
import { clientOpenFileDialog } from "clay:documents";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientOpenFileDialog } from "clay:documents";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
```

On Windows, Linux, and macOS, the configured key opens the native file browser with fixed Markdown filters for `.md`, `.markdown`, and `.mdown`, plus an all-files fallback (Windows/Linux filter dropdown; macOS Markdown extensions with other types allowed). On unsupported platforms the command reports a diagnostic/status instead of panicking. Cancellation is a non-error no-op, and selected-file-only server validation applies before Clay opens the document.

## Options

No options are accepted by `clientOpenFileDialog`. Dialog filters, default directory behavior, and save behavior are not configurable through this API. The fixed defaults are native dialog support on Windows, Linux (xdg-desktop-portal), and macOS (`NSOpenPanel`), Markdown/all-files filters, cancellation as a no-op, and selected-file opening that still consumes server-issued single-use capabilities.

## Key bindings

No default key binding is assigned. Users may bind a key to `documents.clientOpenFileDialog` in `~/.config/clay/init.js`, for example:

```ts
bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
```

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns the string literal command ID `"documents.clientOpenFileDialog"` synchronously. The helper itself does not open a dialog, read files, call the server, or execute client-side JavaScript. The actual file dialog is reached later through an inert behavior manifest route after a user presses a configured key.

## Errors

The helper has no runtime errors. `bindKey` can reject malformed key chords, unsupported scopes, unsupported `when` clauses, or undocumented command IDs. The native command path can report sanitized diagnostics for unsupported platforms or dialog failures. After selection, the server can reject directories, special files, missing files, permission failures, invalid UTF-8, or paths that do not validate as the explicit selected file.

## Permissions and security

No additional permission is required to name or bind the command ID.

Bindable client UI command ID only; native dialog execution requires explicit user key routing, selected files are server-validated as single-file grants, and this API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, broad filesystem/workspace authority, or client-side JavaScript authority.

The client owns the native UI prompt and returns only the user-selected path to the server-open flow. The server owns canonical document state, canonicalizes the path, validates a regular UTF-8 file, sanitizes diagnostics, grants at most that selected file, sends the initial snapshot once, and keeps later edits on the existing delta path. Package JavaScript, including Markdown parsing, runs server-side only through documented facades and validators; the client receives inert behavior manifests, decorations, and status data.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect arbitrary files, access the network, run shell commands, enable packages, expose WASM, mutate AI state, or broaden workspace authority.

## Agent guidance

Use `documents.clientOpenFileDialog` as a documented command ID for `bindKey`. Avoid inventing direct Rust calls, raw op names, dialog filter options, default-directory keys, filesystem scanning, workspace expansion, shell commands, network effects, package loading, WASM, AI mutation, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/documents.js::clientOpenFileDialog`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key` (`op_clay_keybindings_bind_key`)
- Backing Rust/current owner: `src/client/file_dialog.rs::FileDialogResult; src/client/file_dialog.rs::open_markdown_file_dialog; src/app_driver.rs::handle_client_ui_command`
- Current implementation audit path: `src/client/file_dialog.rs`, `src/app_driver.rs`, `src/client/behavior.rs`, `src/server/workspace.rs::WorkspaceState::open_selected_file`, and `src/protocol/mod.rs::ClientMessage::OpenSelectedFile`

## Lookup metadata

- Stable ID: `documents.clientOpenFileDialog`
- User-facing name: Open File Dialog
- Kind: `clay-js-api`
- Module/export: `clay:documents` / `clientOpenFileDialog`
- Default key bindings: none
- Custom properties: none
- Tags: `[documents, open-dialog, file-dialog, windows, linux, macos, markdown, keybindings, js-api]`
