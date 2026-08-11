---
id: workspace.serverCreateListingCancelToken
kind: clay-js-api
js_module: "clay:workspace"
js_export: serverCreateListingCancelToken
js_facade: runtime/js/workspace.js::serverCreateListingCancelToken
backing_rust: src/server/workspace.rs::create_listing_cancel_token
deno_op: op_clay_workspace_create_listing_cancel_token
deno_op_path: src/server/ops/workspace.rs::op_clay_workspace_create_listing_cancel_token
name: serverCreateListingCancelToken
user_facing_name: Create Listing Cancel Token
summary: Create a server-side cancellation token for bounded directory listing work.
owner: server
phase: Phase 18.12
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Creates only an opaque cancellation token for workspace listing work and does not grant filesystem, workspace, file, root, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `workspace.serverCreateListingCancelToken` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [workspace, directory-listing, cancellation, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverCreateListingCancelToken

## Summary

Create a server-side cancellation token for bounded directory listing work.

## Description

`serverCreateListingCancelToken` is the Phase 18.12 runtime-backed Clay JS API for **Create Listing Cancel Token**. It is exposed through the curated `clay:workspace` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs create listing cancel token behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverCreateListingCancelToken } from "clay:workspace";

const tokenId = await serverCreateListingCancelToken();
```

## Example

```ts
const tokenId = await serverCreateListingCancelToken();
```

## Options

No options.

## Key bindings

No default key binding is assigned. Users may bind a key to `workspace.serverCreateListingCancelToken` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for an opaque cancellation token id string.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots/documents, unauthorized targets, traversal escapes, oversize arguments, stale or missing files, unsupported file types, workspace limits, cancellation, and permission-denied filesystem conditions as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: [].

Creates only an opaque cancellation token for workspace listing work and does not grant filesystem, workspace, file, root, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `workspace.serverCreateListingCancelToken` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/workspace.js::serverCreateListingCancelToken`
- Deno op: `src/server/ops/workspace.rs::op_clay_workspace_create_listing_cancel_token` (`op_clay_workspace_create_listing_cancel_token`)
- Backing Rust/current owner: `src/server/workspace.rs::create_listing_cancel_token`

## Lookup metadata

- Stable ID: `workspace.serverCreateListingCancelToken`
- User-facing name: Create Listing Cancel Token
- Kind: `clay-js-api`
- Module/export: `clay:workspace` / `serverCreateListingCancelToken`
- Default key bindings: none
- Tags: `[workspace, directory-listing, cancellation, phase18.12, js-api]`
