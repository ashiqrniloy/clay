---
id: workspace.serverAddWorkspaceRoot
kind: clay-js-api
js_module: "clay:workspace"
js_export: serverAddWorkspaceRoot
js_facade: runtime/js/workspace.js::serverAddWorkspaceRoot
backing_rust: src/server/workspace.rs::WorkspaceState::add_explicit_user_grant
deno_op: op_clay_workspace_add_root
deno_op_path: src/server/ops/workspace.rs::op_clay_workspace_add_root
name: serverAddWorkspaceRoot
user_facing_name: Add Workspace Root
summary: Add an explicit user-approved workspace grant through the server workspace authority boundary.
owner: server
phase: Phase 18.12
visibility: public
permissions: ["workspace-write"]
key_bindings: []
custom_properties: []
security: Requires explicit user workspace approval and server canonicalization; directories become bounded workspace roots and files become selected-file grants; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `workspace.serverAddWorkspaceRoot` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [workspace, roots, grants, file-browser, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverAddWorkspaceRoot

## Summary

Add an explicit user-approved workspace grant through the server workspace authority boundary.

## Description

`serverAddWorkspaceRoot` is the Phase 18.12 runtime-backed Clay JS API for **Add Workspace Root**. It is exposed through the curated `clay:workspace` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It must not run in ordinary typing, client paint, client layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs add workspace root behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverAddWorkspaceRoot } from "clay:workspace";

const rootId = await serverAddWorkspaceRoot("/home/me/project");
```

## Example

```ts
const rootId = await serverAddWorkspaceRoot("/home/me/project");
```

## Options

`path` (`string`): absolute or relative path approved by the user/session. Directories become workspace roots; files become selected-file grants.

## Key bindings

No default key binding is assigned. Users may bind a key to `workspace.serverAddWorkspaceRoot` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for the workspace root id. Directory grants add/deduplicate a root; file grants return a single-file grant id.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, unknown commands/roots/documents, unauthorized targets, traversal escapes, oversize arguments, stale or missing files, unsupported file types, workspace limits, cancellation, and permission-denied filesystem conditions as typed Clay errors where the backing server exposes diagnostics.

## Permissions and security

Requires: ["workspace-write"].

Requires explicit user workspace approval and server canonicalization; directories become bounded workspace roots and files become selected-file grants; does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `workspace.serverAddWorkspaceRoot` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/workspace.js::serverAddWorkspaceRoot`
- Deno op: `src/server/ops/workspace.rs::op_clay_workspace_add_root` (`op_clay_workspace_add_root`)
- Backing Rust/current owner: `src/server/workspace.rs::WorkspaceState::add_explicit_user_grant`

## Lookup metadata

- Stable ID: `workspace.serverAddWorkspaceRoot`
- User-facing name: Add Workspace Root
- Kind: `clay-js-api`
- Module/export: `clay:workspace` / `serverAddWorkspaceRoot`
- Default key bindings: none
- Tags: `[workspace, roots, grants, file-browser, phase18.12, js-api]`
