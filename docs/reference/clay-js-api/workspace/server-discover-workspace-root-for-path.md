---
id: clay.workspace.serverDiscoverWorkspaceRootForPath
kind: clay-js-api
js_module: "clay:workspace"
js_export: serverDiscoverWorkspaceRootForPath
js_facade: runtime/js/workspace.ts::serverDiscoverWorkspaceRootForPath
backing_rust: src/server/workspace.rs::WorkspaceState::discover_root_for_path
deno_op: op_clay_workspace_discover_root_for_path
deno_op_path: src/server/ops/workspace.rs::op_clay_workspace_discover_root_for_path
name: serverDiscoverWorkspaceRootForPath
user_facing_name: Discover Workspace Root For Path
summary: Discover a bounded marker-based workspace root for an already-authorized path.
owner: server
phase: Phase 18.12
visibility: public
permissions: ["workspace-read"]
key_bindings: []
custom_properties: []
security: Uses a closed Clay-owned marker set and bounded ancestry scan over an already-authorized path; packages cannot add marker names or root rules; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.workspace.serverDiscoverWorkspaceRootForPath` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [workspace, discovery, roots, markers, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverDiscoverWorkspaceRootForPath

## Summary

Discover a bounded marker-based workspace root for an already-authorized path.

## Description

`serverDiscoverWorkspaceRootForPath` is the Phase 18.12 runtime-backed Clay JS API for **Discover Workspace Root For Path**. It is exposed through the curated `clay:workspace` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs discover workspace root for path behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverDiscoverWorkspaceRootForPath } from "clay:workspace";

const result = await serverDiscoverWorkspaceRootForPath("/home/me/project/src/main.rs");
```

## Example

```ts
const result = await serverDiscoverWorkspaceRootForPath("/home/me/project/src/main.rs");
```

## Options

`path` (`string`): path whose ancestry is scanned up to Clay bounds for closed marker names.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.workspace.serverDiscoverWorkspaceRootForPath` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns `{ workspaceRootId, discovered }`; `workspaceRootId` is `null` when no marker root is found.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots/documents, unauthorized targets, traversal escapes, oversize arguments, stale or missing files, unsupported file types, workspace limits, cancellation, and permission-denied filesystem conditions as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: ["workspace-read"].

Uses a closed Clay-owned marker set and bounded ancestry scan over an already-authorized path; packages cannot add marker names or root rules; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.workspace.serverDiscoverWorkspaceRootForPath` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/workspace.ts::serverDiscoverWorkspaceRootForPath`
- Deno op: `src/server/ops/workspace.rs::op_clay_workspace_discover_root_for_path` (`op_clay_workspace_discover_root_for_path`)
- Backing Rust/current owner: `src/server/workspace.rs::WorkspaceState::discover_root_for_path`

## Lookup metadata

- Stable ID: `clay.workspace.serverDiscoverWorkspaceRootForPath`
- User-facing name: Discover Workspace Root For Path
- Kind: `clay-js-api`
- Module/export: `clay:workspace` / `serverDiscoverWorkspaceRootForPath`
- Default key bindings: none
- Tags: `[workspace, discovery, roots, markers, phase18.12, js-api]`
