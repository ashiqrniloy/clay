---
id: editor.clientShowOpenDocuments
kind: clay-js-api
js_module: "clay:editor"
js_export: clientShowOpenDocuments
js_facade: runtime/js/editor.js::clientShowOpenDocuments
backing_rust: src/client_commands.rs::EditorClientCommand
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: clientShowOpenDocuments
user_facing_name: Show Open Documents
summary: Return the stable bindable command ID for the focused pane's open-documents switcher, listing every pane's open document plus retained sessions and activating one without re-downloading text.
owner: client
phase: Phase 22.2
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client UI command ID only; after explicit user routing it opens a transient menu over already-retained client sessions on the focused pane (plus other panes' sessions for focus-and-switch activation) and activates a chosen DocumentId locally, and this API does not grant filesystem/workspace expansion, package/configuration/AI document mutation APIs, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority. Cross-pane activation stays capability/workspace-grant gated: it only focuses a pane the server already authorized for that document.
agent_guidance: Use `editor.clientShowOpenDocuments` only as a documented command ID for `bindKey`; do not invent client filesystem authority, tab hosts with package-owned native widgets, or raw Deno ops. Prefer `serverListDocuments` for server-authoritative open-registry metadata.
lookup_tags: [editor, multi-document, sessions, documents, panes, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# clientShowOpenDocuments

## Summary

Return the stable bindable command ID for the focused pane's open-documents switcher, listing every pane's open document plus retained sessions and activating one without re-downloading text.

## Description

`clientShowOpenDocuments` is the public Clay JS API descriptor for **Show Open Documents**. It returns the stable command ID `editor.clientShowOpenDocuments` so configuration, help, and agents can name the multi-document switcher without hard-coding Rust UI.

Authority: `client-ui-command-id`. Runtime path: `configuration-bindKey-to-client-ui-command`. The helper is synchronous and side-effect free. The menu opens later only after an explicit user key/command route reaches the **focused pane's** document view (Phase 22.2: each pane hosts at most one open document of the active tab's workspace; Phase 22.3: each tab is a separate client view with its own connection, split tree, and documents — the switcher follows pane focus inside the active tab, not a window-global editor).

Selecting an item activates `editor.clientActivateDocument` with a `documentId` argument. Entries owned by the focused pane restore that retained client session's shadow text, caret/selection, viewport, history, and dirty chrome locally, exactly as before. Since Phase 22.2 the menu also lists every other pane's open document and retained sessions (`pane N: <name>` entries with active/dirty markers); activating a cross-pane entry carries a `paneId` argument, switches the **owning** pane to that document (stashing its prior session), and focuses it — consistent with the one-view-per-document rule. The server remains open-registry/lease/dirty authority; this command does not expand workspace grants, focus a pane the server has not authorized, or re-download text for sessions the client already retains.

Opening a second file through the normal `DocumentOpened` path retains the previous session automatically (bound at 64 total sessions including the active document). Opening a file that is already open in another pane focuses that pane instead of creating a second view; the redundant server snapshot is a no-op on the live surface.

## When to use

Use this API when a user wants a bindable chord that opens the open-documents switcher in `~/.config/clay/init.js`.

## JavaScript usage

```ts
import { clientShowOpenDocuments } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+E", clientShowOpenDocuments(), { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { clientShowOpenDocuments } from "clay:editor";
import { bindKey } from "clay:keybindings";
import { serverListDocuments } from "clay:documents";

bindKey("Ctrl+Shift+E", clientShowOpenDocuments(), { scope: "editor" });

// Server-authoritative metadata (leases/dirty/path) remains available separately:
const documents = await serverListDocuments();
```

## Options

No options are accepted. Session retention ceilings and eviction policy are not configurable through this API in Phase 20.

## Key bindings

No native default shortcut is assigned. Bind with `bindKey` when desired.

## Custom properties

None.

## Return and async behavior

Returns the string `"editor.clientShowOpenDocuments"`. Synchronous. No IPC.

## Errors

This helper does not throw. If no document has been opened yet, the command is a no-op on the focused pane. Missing retained sessions selected from a stale menu (including a cross-pane entry whose pane closed since the menu opened) produce a sanitized runtime diagnostic.

## Permissions and security

Bindable client UI command ID only. Does not grant filesystem, workspace expansion, package/configuration/AI document authority, network, shell, extension loading, package manager, AI mutation, WASM, raw Deno ops, native widget, or client-side JavaScript authority. Display names in the menu are basename-sanitized.

## Agent guidance

Use `editor.clientShowOpenDocuments` only as a documented command ID for `bindKey`. For server open-registry inspection use `serverListDocuments`. Do not invent tab widgets, client filesystem reads, or raw ops.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientShowOpenDocuments`
- Pane menu: `src/client_commands.rs::EditorClientCommand (client-local; React command surface)`
- Activate path: `src/client_commands.rs::EditorClientCommand (client-local; executed by the React workspace controller, frontend/src/shell/workspace-controller.ts)` (own pane); `src/client_commands.rs` (client command routing) cross-pane aggregation and `ActivateDocumentInPane` routing
- Session store: `src/editor/document_session.rs::DocumentSessionStore`
- Keybinding allowlist: `src/server/ops/keybindings.rs`

## Stability notes

Runtime-backed Phase 22.2 client UI command. Activate-by-id remains an internal menu argument (`documentId`, plus `paneId` for cross-pane entries) rather than a separate options-taking Clay JS helper in this phase.

## Lookup metadata

- Stable ID: `editor.clientShowOpenDocuments`
- User-facing name: Show Open Documents
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientShowOpenDocuments`
- Default key bindings: none
- Custom properties: none
- Tags: `[editor, multi-document, sessions, documents, panes, keybindings, js-api]`
