---
id: clay.documents.serverGetDocumentStatus
kind: clay-js-api
js_module: "clay:documents"
js_export: serverGetDocumentStatus
js_facade: runtime/js/documents.ts::serverGetDocumentStatus
backing_rust: src/server/workspace.rs::WorkspaceState::document_metadata
deno_op: op_clay_documents_get_document_status
deno_op_path: src/server/ops/documents.rs::op_clay_documents_get_document_status
name: serverGetDocumentStatus
user_facing_name: Get Document Status
summary: Query server-owned metadata for an open workspace document, including dirty state and workspace-relative path.
owner: server
phase: Phase 9
visibility: public
permissions: ["document-read"]
key_bindings: []
custom_properties: []
security: Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.documents.serverGetDocumentStatus` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.
lookup_tags: [documents, workspace, metadata, dirty-state, js-api]
app_visible: true
help_visible: true
stability: planned
async: true
---

# serverGetDocumentStatus

## Summary

Query server-owned metadata for an open workspace document, including dirty state and workspace-relative path.

## Description

`serverGetDocumentStatus` is the planned public API for **Get Document Status**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols, protocol messages, or future raw op wrappers.

Authority: `server-authoritative-document-query`. Runtime path: `server-first-query`. Status queries are background/help/programmatic metadata queries and are not needed for ordinary local paint or edit hot paths.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Get Document Status` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverGetDocumentStatus } from "clay:documents";

await serverGetDocumentStatus(/* options */);
```

## Example

```ts
const status = await serverGetDocumentStatus(documentId);
console.log(status.dirty, status.path);
```

## Options

- `documentId` (`string`): Open document whose metadata should be returned.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.documents.serverGetDocumentStatus` in `~/.config/clay/init.js` once configuration execution exists.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for server-owned document metadata.

Current Phase 9 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced workspace root or document does not exist, required permissions are absent, the server rejects workspace authorization, path traversal leaves the authorized root, the file is missing, permission is denied, the content is not valid UTF-8, the path is a directory or unsupported special file, stale file metadata is detected, or a dirty document would be overwritten without an explicit force option. Current Phase 9 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

Requires: `document-read`.

Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

The server owns filesystem/workspace authority and canonical documents. The client and Clay JS facade receive sanitized metadata, snapshots for explicit open/reload/resync paths, and typed errors; they do not receive raw host filesystem authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect arbitrary user files, access the network, or expose runtime user content beyond the requested authorized document metadata/snapshot.

## Agent guidance

Use `clay.documents.serverGetDocumentStatus` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/documents.ts::serverGetDocumentStatus`
- Future Deno op: `src/server/ops/documents.rs::op_clay_documents_get_document_status` (`op_clay_documents_get_document_status`)
- Backing Rust/current owner: `src/server/workspace.rs::WorkspaceState::document_metadata`
- Current implementation audit path: `src/protocol/mod.rs`, `src/server/connection.rs`, and `src/server/workspace.rs`

## Lookup metadata

- Stable ID: `clay.documents.serverGetDocumentStatus`
- User-facing name: Get Document Status
- Kind: `clay-js-api`
- Module/export: `clay:documents` / `serverGetDocumentStatus`
- Default key bindings: none
- Custom properties: none
- Tags: `[documents, workspace, metadata, dirty-state, js-api]`
