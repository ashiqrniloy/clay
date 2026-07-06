---
id: clay.workspace.serverCancelListing
kind: clay-js-api
js_module: "clay:workspace"
js_export: serverCancelListing
js_facade: runtime/js/workspace.ts::serverCancelListing
backing_rust: src/server/workspace.rs::cancel_listing
deno_op: op_clay_workspace_cancel_listing
deno_op_path: src/server/ops/workspace.rs::op_clay_workspace_cancel_listing
name: serverCancelListing
user_facing_name: Cancel Listing
summary: Cancel a bounded server directory listing by opaque token id.
owner: server
phase: Phase 18.12
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Cancels only server-owned listing work for an opaque token and does not grant filesystem, workspace, file, root, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.workspace.serverCancelListing` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [workspace, directory-listing, cancellation, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverCancelListing

## Summary

Cancel a bounded server directory listing by opaque token id.

## Description

`serverCancelListing` is the Phase 18.12 runtime-backed Clay JS API for **Cancel Listing**. It is exposed through the curated `clay:workspace` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs cancel listing behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverCancelListing } from "clay:workspace";

await serverCancelListing(tokenId);
```

## Example

```ts
await serverCancelListing(tokenId);
```

## Options

`tokenId` (`string`): token returned by `serverCreateListingCancelToken` or supplied to `serverListDirectory`.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.workspace.serverCancelListing` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for `true` when a live listing token was found and marked cancelled.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots/documents, unauthorized targets, traversal escapes, oversize arguments, stale or missing files, unsupported file types, workspace limits, cancellation, and permission-denied filesystem conditions as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: [].

Cancels only server-owned listing work for an opaque token and does not grant filesystem, workspace, file, root, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.workspace.serverCancelListing` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/workspace.ts::serverCancelListing`
- Deno op: `src/server/ops/workspace.rs::op_clay_workspace_cancel_listing` (`op_clay_workspace_cancel_listing`)
- Backing Rust/current owner: `src/server/workspace.rs::cancel_listing`

## Lookup metadata

- Stable ID: `clay.workspace.serverCancelListing`
- User-facing name: Cancel Listing
- Kind: `clay-js-api`
- Module/export: `clay:workspace` / `serverCancelListing`
- Default key bindings: none
- Tags: `[workspace, directory-listing, cancellation, phase18.12, js-api]`
