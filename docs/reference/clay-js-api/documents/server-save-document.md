---
id: documents.serverSaveDocument
kind: clay-js-api
js_module: "clay:documents"
js_export: serverSaveDocument
js_facade: runtime/js/documents.js::serverSaveDocument
backing_rust: src/server/workspace.rs::WorkspaceState::save_document
deno_op: op_clay_documents_save_document
deno_op_path: src/server/ops/documents.rs::op_clay_documents_save_document
name: serverSaveDocument
user_facing_name: Save Document
summary: Save the current server-canonical document text back to its authorized workspace file.
owner: server
phase: Phase 9
visibility: public
permissions: ["workspace-write", "document-read"]
key_bindings: []
custom_properties: []
security: The trusted-only documents facade uses server-internal save authority, validates any explicit knownVersion against canonical server state, confines writes to an already-authorized open workspace document with path traversal rejection, and performs exclusive same-directory atomic replacement with target-identity revalidation; it is absent from the third-party package runtime and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `documents.serverSaveDocument` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.
lookup_tags: [documents, workspace, file, save, dirty-state, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverSaveDocument

## Summary

Save the current server-canonical document text back to its authorized workspace file.

## Description

`serverSaveDocument` is the runtime-backed public API for **Save Document**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols, protocol messages, or future raw op wrappers.

Authority: `server-authoritative-file-save`. Runtime path: `server-first-file-io`. Saving is an explicit server file IO command and is never part of ordinary keypress-to-paint latency.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Save Document` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverSaveDocument } from "clay:documents";

await serverSaveDocument(/* options */);
```

## Example

```ts
const saved = await serverSaveDocument({ documentId: opened.metadata.documentId });
console.log(saved.dirty);
```

## Options

- `documentId` (`string`): Open document to save.
- `knownVersion` (`number`, optional): Confirmed server version known by the caller. Values at or below the canonical version are accepted; a value newer than the server is rejected as protocol/state confusion. Omission uses the server-internal baseline.

## Key bindings

Recommended default chord for daily editing:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
```

No built-in Rust shortcut is hardcoded; without an `init.js` (or fixture) binding, save is reachable through Control Center only if the command is listed there, or through the Clay JS facade. The client routes the bound command as a non-blocking `SaveDocument` protocol request for the active document. Stale on-disk metadata keeps the document dirty and opens a recovery menu instead of overwriting silently.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for the saved document id, resulting version, and dirty flag.

Current Phase 13 facade/runtime status is runtime-backed for server-side configuration and extension execution through explicit `deno_core` ops, while the API remains documented with the Phase 9 public contract.

## Errors

The runtime fails if arguments are malformed; `documentId` or `knownVersion` is not an unsigned integer/string; `knownVersion` claims a version newer than canonical server state; the document is not open in the server workspace registry; the file is missing, replaced, changed externally, permission-denied, outside its authorized root, or an unsupported type; or exclusive temp creation, permission restoration, sync, identity revalidation, or atomic replacement fails. Failures preserve external target bytes and keep the document dirty.

## Permissions and security

Requires: `workspace-write, document-read`.

`clay:documents` is trusted-only and absent from the shared third-party package runtime. Trusted configuration executes with server-internal identity rather than borrowing a connection's editable lease; this does not let third-party packages forge identity or save another connection's document. The op accepts only an already-open document ID, validates any explicit `knownVersion` against canonical state, and reaches disk only through workspace-owned root/path/file-identity checks.

Save uses a bounded snapshot plus an unpredictable, exclusive, owner-only same-directory temp file. Clay restores required permissions, syncs, revalidates the target's platform identity immediately before replacement, and fails closed on external edits or swaps. The API returns sanitized metadata and typed errors, never raw file handles or arbitrary paths; it does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect arbitrary user files, access the network, or expose runtime user content beyond the requested authorized document metadata/snapshot.

## Agent guidance

Use `documents.serverSaveDocument` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/documents.js::serverSaveDocument`
- Deno op: `src/server/ops/documents.rs::op_clay_documents_save_document` (`op_clay_documents_save_document`)
- Backing Rust/current owner: `src/server/workspace.rs::WorkspaceState::save_document`
- Current implementation audit path: `src/protocol/mod.rs`, `src/server/connection/mod.rs`, and `src/server/workspace.rs`

## Lookup metadata

- Stable ID: `documents.serverSaveDocument`
- User-facing name: Save Document
- Kind: `clay-js-api`
- Module/export: `clay:documents` / `serverSaveDocument`
- Default key bindings: none
- Custom properties: none
- Tags: `[documents, workspace, file, save, dirty-state, js-api]`
