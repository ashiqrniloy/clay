---
id: clay.documents.serverListDocuments
kind: clay-js-api
js_module: "clay:documents"
js_export: serverListDocuments
js_facade: runtime/js/documents.ts::serverListDocuments
backing_rust: src/server/workspace.rs::WorkspaceState::list_documents
deno_op: op_clay_documents_list_documents
deno_op_path: src/server/ops/documents.rs::op_clay_documents_list_documents
name: serverListDocuments
user_facing_name: List Open Documents
summary: List metadata for documents currently open in the server workspace registry.
owner: server
phase: Phase 9
visibility: public
permissions: ["document-read"]
key_bindings: []
custom_properties: []
security: Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.documents.serverListDocuments` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.
lookup_tags: [documents, workspace, metadata, list, dirty-state, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverListDocuments

## Summary

List metadata for documents currently open in the server workspace registry.

## Description

`serverListDocuments` is the runtime-backed public API for **List Open Documents**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols, protocol messages, or future raw op wrappers.

Authority: `server-authoritative-document-query`. Runtime path: `server-first-query`. Listing documents is an explicit metadata query and does not serialize full document text or participate in ordinary typing/rendering latency.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `List Open Documents` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverListDocuments } from "clay:documents";

await serverListDocuments();
```

## Example

```ts
const documents = await serverListDocuments();
for (const document of documents) console.log(document.documentId, document.dirty);
```

## Options

No options.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.documents.serverListDocuments` in `~/.config/clay/init.js` once configuration execution exists.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for an array of document metadata records; it does not return full text snapshots.

Current Phase 13 facade/runtime status is runtime-backed for server-side configuration and extension execution through explicit `deno_core` ops, while the API remains documented with the Phase 9 public contract.

## Errors

The runtime fails if arguments are malformed, the referenced workspace root or document does not exist, required permissions are absent, the server rejects workspace authorization, path traversal leaves the authorized root, the file is missing, permission is denied, the content is not valid UTF-8, the path is a directory or unsupported special file, stale file metadata is detected, or a dirty document would be overwritten without an explicit force option. The Phase 13 runtime-backed facade reports typed JavaScript errors converted from server workspace diagnostics rather than performing unauthorized filesystem operations.

## Permissions and security

Requires: `document-read`.

Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

The server owns filesystem/workspace authority and canonical documents. The client and Clay JS facade receive sanitized metadata, snapshots for explicit open/reload/resync paths, and typed errors; they do not receive raw host filesystem authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect arbitrary user files, access the network, or expose runtime user content beyond the requested authorized document metadata/snapshot.

## Agent guidance

Use `clay.documents.serverListDocuments` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/documents.ts::serverListDocuments`
- Deno op: `src/server/ops/documents.rs::op_clay_documents_list_documents` (`op_clay_documents_list_documents`)
- Backing Rust/current owner: `src/server/workspace.rs::WorkspaceState::list_documents`
- Current implementation audit path: `src/protocol/mod.rs`, `src/server/connection.rs`, and `src/server/workspace.rs`

## Lookup metadata

- Stable ID: `clay.documents.serverListDocuments`
- User-facing name: List Open Documents
- Kind: `clay-js-api`
- Module/export: `clay:documents` / `serverListDocuments`
- Default key bindings: none
- Custom properties: none
- Tags: `[documents, workspace, metadata, list, dirty-state, js-api]`
