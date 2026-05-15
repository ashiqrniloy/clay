---
id: clay.documents.serverGetDocumentSnapshot
kind: clay-js-api
js_module: "clay:documents"
js_export: serverGetDocumentSnapshot
js_facade: runtime/js/documents.ts::serverGetDocumentSnapshot
backing_rust: src/server/document.rs::DocumentState::resync_snapshot_message_for_client
deno_op: op_clay_documents_get_document_snapshot
deno_op_path: src/server/ops/documents.rs::op_clay_documents_get_document_snapshot
name: serverGetDocumentSnapshot
user_facing_name: Get Document Snapshot
summary: Get Document Snapshot through the planned `clay:documents` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: [document-read]
key_bindings: []
custom_properties: []
security: Requires read access to the document; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.documents.serverGetDocumentSnapshot` only for its documented documents responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [documents, js-api, leasereadonlystate]
app_visible: true
help_visible: true
stability: planned
async: true
---

# serverGetDocumentSnapshot

## Summary

Get Document Snapshot through the planned `clay:documents` Clay JavaScript facade.

## Description

`serverGetDocumentSnapshot` is the planned public API for **Get Document Snapshot**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `server-authoritative-document-read`. Runtime path: `server-first-query`. Snapshot/resync reads are explicit server queries and are not used for normal paint or ordinary edit updates.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Get Document Snapshot` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverGetDocumentSnapshot } from "clay:documents";

const snapshot = await serverGetDocumentSnapshot("current");
```

## Example

```ts
const snapshot = await serverGetDocumentSnapshot("current");
```

## Options

- `documentId` (`string`): Document to read.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.documents.serverGetDocumentSnapshot` in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for a server-authoritative document snapshot.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

Requires: `document-read`.

Requires read access to the document; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.documents.serverGetDocumentSnapshot` when the user asks for get document snapshot through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/documents.ts::serverGetDocumentSnapshot`
- Future Deno op: `src/server/ops/documents.rs::op_clay_documents_get_document_snapshot` (`op_clay_documents_get_document_snapshot`)
- Backing Rust/current owner: `src/server/document.rs::DocumentState::resync_snapshot_message_for_client`
- Current implementation audit path: `src/server/document.rs::DocumentState::initial_document_message; src/server/document.rs::DocumentState::resync_snapshot_message_for_client`

## Lookup metadata

- Stable ID: `clay.documents.serverGetDocumentSnapshot`
- User-facing name: Get Document Snapshot
- Kind: `clay-js-api`
- Module/export: `clay:documents` / `serverGetDocumentSnapshot`
- Default key bindings: none
- Custom properties: none
- Tags: `documents`, `js-api`, `leasereadonlystate`
