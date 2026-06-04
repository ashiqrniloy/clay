---
id: clay.workspace.serverListWorkspaceRoots
kind: clay-js-api
js_module: "clay:workspace"
js_export: serverListWorkspaceRoots
js_facade: runtime/js/workspace.ts::serverListWorkspaceRoots
backing_rust: src/server/mod.rs::ServerConfig::workspace_roots; src/server/workspace.rs::WorkspaceState::add_root
deno_op: op_clay_workspace_list_roots
deno_op_path: src/server/ops/workspace.rs::op_clay_workspace_list_roots
name: serverListWorkspaceRoots
user_facing_name: List Workspace Roots
summary: List server-configured workspace root metadata without exposing unrestricted host filesystem authority.
owner: server
phase: Phase 9
visibility: public
permissions: ["workspace-read"]
key_bindings: []
custom_properties: []
security: Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.workspace.serverListWorkspaceRoots` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.
lookup_tags: [workspace, roots, metadata, file, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverListWorkspaceRoots

## Summary

List server-configured workspace root metadata without exposing unrestricted host filesystem authority.

## Description

`serverListWorkspaceRoots` is the runtime-backed public API for **List Workspace Roots**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols, protocol messages, or future raw op wrappers.

Authority: `server-authoritative-workspace-query`. Runtime path: `server-first-query`. Workspace root metadata lookup is a background/help/programmatic query and never runs in editor input, Masonry paint/layout, or ordinary edit acknowledgement hot paths.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `List Workspace Roots` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { serverListWorkspaceRoots } from "clay:workspace";

await serverListWorkspaceRoots();
```

## Example

```ts
const roots = await serverListWorkspaceRoots();
console.log(roots.map((root) => root.workspaceRootId));
```

## Options

No options.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.workspace.serverListWorkspaceRoots` in `~/.config/clay/init.js` once configuration execution exists.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for sanitized workspace root metadata advertised by the server.

Current Phase 13 facade/runtime status is runtime-backed for server-side configuration and extension execution through explicit `deno_core` ops, while the API remains documented with the Phase 9 public contract.

## Errors

The runtime fails if arguments are malformed, the referenced workspace root or document does not exist, required permissions are absent, the server rejects workspace authorization, path traversal leaves the authorized root, the file is missing, permission is denied, the content is not valid UTF-8, the path is a directory or unsupported special file, stale file metadata is detected, or a dirty document would be overwritten without an explicit force option. The Phase 13 runtime-backed facade reports typed JavaScript errors converted from server workspace diagnostics rather than performing unauthorized filesystem operations.

## Permissions and security

Requires: `workspace-read`.

Requires server-side validation of document/workspace permissions, workspace root authorization, path traversal rejection, and typed file errors; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

The server owns filesystem/workspace authority and canonical documents. The client and Clay JS facade receive sanitized metadata, snapshots for explicit open/reload/resync paths, and typed errors; they do not receive raw host filesystem authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect arbitrary user files, access the network, or expose runtime user content beyond the requested authorized document metadata/snapshot.

## Agent guidance

Use `clay.workspace.serverListWorkspaceRoots` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent filesystem access, network effects, shell commands, extension loading, AI mutation, broader workspace authority, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/workspace.ts::serverListWorkspaceRoots`
- Deno op: `src/server/ops/workspace.rs::op_clay_workspace_list_roots` (`op_clay_workspace_list_roots`)
- Backing Rust/current owner: `src/server/mod.rs::ServerConfig::workspace_roots; src/server/workspace.rs::WorkspaceState::add_root`
- Current implementation audit path: `src/protocol/mod.rs`, `src/server/connection.rs`, and `src/server/workspace.rs`

## Lookup metadata

- Stable ID: `clay.workspace.serverListWorkspaceRoots`
- User-facing name: List Workspace Roots
- Kind: `clay-js-api`
- Module/export: `clay:workspace` / `serverListWorkspaceRoots`
- Default key bindings: none
- Custom properties: none
- Tags: `[workspace, roots, metadata, file, js-api]`
