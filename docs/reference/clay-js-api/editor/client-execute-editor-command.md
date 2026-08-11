---
id: editor.clientExecuteEditorCommand
kind: clay-js-api
js_module: "clay:editor"
js_export: clientExecuteEditorCommand
js_facade: runtime/js/editor.js::clientExecuteEditorCommand
backing_rust: src/server/ops/editor.rs::op_clay_editor_execute_command; src/masonry_editor.rs::EditorWidget::apply_editor_command_request
deno_op: op_clay_editor_execute_command
deno_op_path: src/server/ops/editor.rs::op_clay_editor_execute_command
name: clientExecuteEditorCommand
user_facing_name: Execute Editor Command
summary: Programmatically trigger one known editor command ID through the gated `editor-control` execution channel.
owner: client
phase: Phase 23
visibility: public
permissions: ["editor-control"]
key_bindings: []
custom_properties:
  - name: commandId
    type: string
    default: none
    description: Known direction-specific argless editor command ID to execute (e.g. `editor.clientMoveCursor.nextWordStart`).
security: Changes only transient client cursor/selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `editor.clientExecuteEditorCommand` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editor, js-api, editor-control, execution-channel]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientExecuteEditorCommand

## Summary

Programmatically trigger one known editor command ID through the gated `editor-control` execution channel.

## Description

`clientExecuteEditorCommand` is the public API for **programmatic editor-command execution** (Plan 071 follow-up round, `editor-control` trust boundary). The `op_clay_editor_execute_command` deno op validates the `commandId` against the known editor command allowlist (deny-by-default: movement, selection, caret, multi-cursor, text-object, and smart-select IDs only), passes the same `editor-control` gate as every editor op (approved `editor-control` permission AND an active major mode named in the caller's `clay.editorControl.modes` declaration), then publishes an advisory `EditorCommandRequest` that connection loops forward to the client. The client re-parses the command ID deny-by-default and dispatches it through the exact same path as keybinding-routed command IDs; unknown IDs are dropped silently on both sides. Trusted user-configuration callers (no package context) may call the API without a mode declaration; package callers never bypass the mode gate.

Authority: `client-local-ui-state`. Runtime path: `server-gated-push`.

## When to use

Use this API when an activated package (or user configuration) needs to move the caret or change selection programmatically — gesture-free control such as AI-assisted symbol selection, snippet flows, or macro replay. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientExecuteEditorCommand } from "clay:editor";

clientExecuteEditorCommand({ commandId: "editor.clientMoveCursor.nextWordStart" });
```

## Example

```ts
clientExecuteEditorCommand({ commandId: "editor.clientSetSelection.selectLine" });
```

## Options

- `commandId` (`string`): a known direction-specific argless editor command ID.

## Key bindings

Default key bindings: none. This API is programmatic; key-driven execution uses the direction-specific command IDs directly via `bindKey`/mode keymaps.

## Custom properties

- `commandId` (`string`): the editor command ID to execute (see Options).

## Return and async behavior

Returns `{ requested, published, commandId }` synchronously. `published` is `false` when no connection publisher is wired (advisory degrade). Delivery to the client is asynchronous and advisory: a dropped request never blocks editing.

## Errors

The op fails (deny-by-default) if `commandId` is missing, unbounded, or not a known editor command ID; if the caller is a package lacking approved `editor-control`; or if the active major mode is not in the caller's declared `clay.editorControl.modes`.

## Permissions and security

Package callers require the approved `editor-control` permission and must declare the exact modes they operate in (`clay.editorControl.modes` in package.json). Clay enforces the mode match per call; when several packages hold `editor-control` for one mode they coexist, and the user resolves conflicts by deactivating packages.

Changes only transient client cursor/selection state; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientExecuteEditorCommand` when the user asks for programmatic cursor/selection control through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientExecuteEditorCommand`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_execute_command`
- Push wire: `src/protocol/editor_control.rs::EditorCommandRequest` (`ServerMessage::EditorCommandRequest`, protocol version 8)
- Client dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_command_request`

## Lookup metadata

- Stable ID: `editor.clientExecuteEditorCommand`
- User-facing name: Execute Editor Command
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientExecuteEditorCommand`
- Default key bindings: none
- Custom properties: `commandId`
- Tags: `editor`, `js-api`, `editor-control`, `execution-channel`
