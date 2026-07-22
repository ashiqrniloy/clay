---
id: clay.documents.serverOpenDocument
kind: clay-js-api
js_module: "clay:documents"
js_export: serverOpenDocument
js_facade: runtime/js/documents.js::serverOpenDocument
backing_rust: src/server/workspace.rs::WorkspaceState::open_existing_file
deno_op: op_clay_documents_open_document
deno_op_path: src/server/ops/documents.rs::op_clay_documents_open_document
name: serverOpenDocument
user_facing_name: Open Document
summary: Open an authorized workspace text file through the runtime-backed `clay:documents` server-authoritative facade.
owner: server
phase: Phase 9
visibility: public
permissions: ["workspace-read", "document-read"]
key_bindings: []
custom_properties: []
security: Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.documents.serverOpenDocument` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.
lookup_tags: [documents, workspace, file, open, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverOpenDocument

## Summary

Open an authorized workspace text file through the runtime-backed `clay:documents` server-authoritative facade.

## Description

`serverOpenDocument` is the runtime-backed public API for **Open Document**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols, protocol messages, or future raw op wrappers.

Authority: `server-authoritative-file-open`. Runtime path: `server-first-file-io`. Opening a file is an explicit server command that may read a full UTF-8 snapshot once; ordinary keypress-to-paint editing remains asynchronous and does not call JavaScript, workspace validation, or file IO.

`serverOpenDocument` opens files under configured workspace roots. The Phase 19 native file dialog uses the separate bindable [`clientOpenFileDialog`](client-open-file-dialog.md) command ID followed by private selected-file IPC and a server single-file grant; do not pass arbitrary host paths to `serverOpenDocument` to emulate user-selected files.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Open Document` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverOpenDocument } from "clay:documents";

await serverOpenDocument({ workspaceRootId: "1", path: "src/main.rs" });
```

## Example

```ts
const opened = await serverOpenDocument({ workspaceRootId: "1", path: "src/main.rs" });
console.log(opened.metadata.documentId, opened.text);
```

## Options

- `workspaceRootId` (`string`): Configured server workspace root identifier advertised by Clay.
- `path` (`string`): Workspace-relative path to an existing UTF-8 text file.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.documents.serverOpenDocument` in `~/.config/clay/init.js` once configuration execution exists.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for document metadata plus the initial text snapshot.

Current Phase 13 facade/runtime status is runtime-backed for server-side configuration and extension execution through explicit `deno_core` ops, while the API remains documented with the Phase 9 public contract.

## Errors

The runtime fails if arguments are malformed, the referenced workspace root or document does not exist, required permissions are absent, the server rejects workspace authorization, path traversal leaves the authorized root, the file is missing, permission is denied, the content is not valid UTF-8, the path is a directory or unsupported special file, stale file metadata is detected, or a dirty document would be overwritten without an explicit force option. The Phase 13 runtime-backed facade reports typed JavaScript errors converted from server workspace diagnostics rather than performing unauthorized filesystem operations.

## Permissions and security

Requires: `workspace-read, document-read`.

Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

The server owns filesystem/workspace authority and canonical documents. The client and Clay JS facade receive sanitized metadata, snapshots for explicit open/reload/resync paths, and typed errors; they do not receive raw host filesystem authority.

For native file-dialog opens, [`clientOpenFileDialog`](client-open-file-dialog.md) routes through user-mediated client UI and selected-file-only server validation rather than broad workspace expansion. That selected-file grant is internal server authority, not a public `serverOpenDocument` option.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect arbitrary user files, access the network, or expose runtime user content beyond the requested authorized document metadata/snapshot.

## Agent guidance

Use `clay.documents.serverOpenDocument` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/documents.js::serverOpenDocument`
- Deno op: `src/server/ops/documents.rs::op_clay_documents_open_document` (`op_clay_documents_open_document`)
- Backing Rust/current owner: `src/server/workspace.rs::WorkspaceState::open_existing_file`
- Current implementation audit path: `src/protocol/mod.rs`, `src/server/connection.rs`, and `src/server/workspace.rs`

## Lookup metadata

- Stable ID: `clay.documents.serverOpenDocument`
- User-facing name: Open Document
- Kind: `clay-js-api`
- Module/export: `clay:documents` / `serverOpenDocument`
- Default key bindings: none
- Custom properties: none
- Tags: `[documents, workspace, file, open, js-api]`
