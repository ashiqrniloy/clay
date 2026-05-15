---
id: clay.documents.serverGetDocumentLease
kind: clay-js-api
js_module: "clay:documents"
js_export: serverGetDocumentLease
js_facade: runtime/js/documents.ts::serverGetDocumentLease
backing_rust: src/server/document.rs::DocumentState::acquire_access
deno_op: op_clay_documents_get_document_lease
deno_op_path: src/server/ops/documents.rs::op_clay_documents_get_document_lease
name: serverGetDocumentLease
user_facing_name: Get Document Lease
summary: Get Document Lease through the planned `clay:documents` Clay JavaScript facade.
owner: server
phase: Phase 7
visibility: public
permissions: [document-read]
key_bindings: []
custom_properties: []
security: Reports or requests lease metadata according to server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.documents.serverGetDocumentLease` only for its documented documents responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [documents, js-api, leasereadonlystate]
app_visible: true
help_visible: true
stability: planned
async: true
---

# serverGetDocumentLease

## Summary

Get Document Lease through the planned `clay:documents` Clay JavaScript facade.

## Description

`serverGetDocumentLease` is the planned public API for **Get Document Lease**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `server-authoritative-lease-query`. Runtime path: `server-first-query`. Lease state is server-owned and queried outside the local input hot path.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Get Document Lease` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverGetDocumentLease } from "clay:documents";

const lease = await serverGetDocumentLease("current");
```

## Example

```ts
const lease = await serverGetDocumentLease("current");
```

## Options

- `documentId` (`string`): Document whose lease/read-only state should be queried.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.documents.serverGetDocumentLease` in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for server-owned lease/read-only metadata.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

Requires: `document-read`.

Reports or requests lease metadata according to server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.documents.serverGetDocumentLease` when the user asks for get document lease through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/documents.ts::serverGetDocumentLease`
- Future Deno op: `src/server/ops/documents.rs::op_clay_documents_get_document_lease` (`op_clay_documents_get_document_lease`)
- Backing Rust/current owner: `src/server/document.rs::DocumentState::acquire_access`
- Current implementation audit path: `src/server/document.rs::DocumentState::acquire_access; src/server/document.rs::DocumentState::release_access`

## Lookup metadata

- Stable ID: `clay.documents.serverGetDocumentLease`
- User-facing name: Get Document Lease
- Kind: `clay-js-api`
- Module/export: `clay:documents` / `serverGetDocumentLease`
- Default key bindings: none
- Custom properties: none
- Tags: `documents`, `js-api`, `leasereadonlystate`
