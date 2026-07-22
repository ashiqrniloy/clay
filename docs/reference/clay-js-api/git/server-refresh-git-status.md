---
id: clay.git.serverRefreshGitStatus
kind: clay-js-api
js_module: "clay:git"
js_export: serverRefreshGitStatus
js_facade: runtime/js/git.js::serverRefreshGitStatus
backing_rust: src/server/git.rs::GitStatusCache::refresh_root
deno_op: op_clay_git_refresh_status
deno_op_path: src/server/ops/git.rs::op_clay_git_refresh_status
name: serverRefreshGitStatus
user_facing_name: Refresh Git Status
summary: Explicitly refresh read-only Git status for one authorized workspace root.
owner: server
phase: Phase 18.13
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: workspaceRootId
    type: WorkspaceRootId
    default: required
    description: Authorized workspace root to refresh.
security: Runs only Clay's closed read-only Git discovery commands under the selected workspace root; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.git.serverRefreshGitStatus` only through the documented `clay:git` facade for explicit refresh of a known workspace root. Do not expose arbitrary Git subcommands, shell arguments, repository mutation, or path traversal.
lookup_tags: [git, refresh, status, branch, workspace, phase18.13, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverRefreshGitStatus

## Summary

Explicitly refresh read-only Git status for one authorized workspace root.

## Description

`serverRefreshGitStatus` is the Phase 18.13 runtime-backed Clay JS API for **Refresh Git Status**. It is exposed through the curated `clay:git` facade so package/configuration/runtime code does not call raw ops or Rust internals.

It refreshes one workspace root through `GitStatusCache`. The server resolves the root id to a known workspace directory and runs only the closed Git discovery command set behind timeout/output caps. This API is background/action work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs to explicitly refresh read-only Git status for a known workspace root through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings. No shell command, argv, cwd, repository path, remote, branch name, or mutation option is accepted.

## JavaScript usage

```ts
import { serverRefreshGitStatus } from "clay:git";

const entry = await serverRefreshGitStatus({ workspaceRootId: "1" });
```

## Example

```ts
const entry = await serverRefreshGitStatus({ workspaceRootId: "1" });
const head = entry.snapshot?.head;
```

## Options

- `workspaceRootId` (`WorkspaceRootId`, required): authorized workspace root to refresh. No shell command, argv, cwd, repository path, remote, branch name, or mutation option is accepted.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.git.serverRefreshGitStatus` through documented keybinding/configuration APIs where appropriate.

## Custom properties

- `workspaceRootId`
- name: workspaceRootId
- type: WorkspaceRootId
- default: required
- description: Authorized workspace root to refresh.

## Return and async behavior

Returns a promise for one `GitCachedStatus`. On refresh failure, the cache reports `refreshState.kind === "last-error"` and may preserve the previous successful snapshot.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments, unavailable runtime ops, and unknown workspace root ids as typed Clay errors. Roots that are not Git repositories surface as `lastRefresh.kind === "non-repository"` snapshots, not thrown errors. Concurrent refreshes for the same root are coalesced.

## Permissions and security

Requires: [].

Runs only Clay's closed read-only Git discovery commands under the selected workspace root; the server resolves the root id to a known workspace directory and re-canonicalizes before any Git process runs. Server-side validation enforces workspace-root confinement, the closed command table, timeout/output caps, and read-only defaults. It does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.git.serverRefreshGitStatus` only through the documented `clay:git` facade for explicit refresh of a known workspace root. Do not expose arbitrary Git subcommands, shell arguments, repository mutation, or path traversal.

## Backing implementation

- JS facade: `runtime/js/git.js::serverRefreshGitStatus`
- Deno op: `src/server/ops/git.rs::op_clay_git_refresh_status` (`op_clay_git_refresh_status`)
- Backing Rust/current owner: `src/server/git.rs::GitStatusCache::refresh_root`

## Lookup metadata

- Stable ID: `clay.git.serverRefreshGitStatus`
- User-facing name: Refresh Git Status
- Kind: `clay-js-api`
- Module/export: `clay:git` / `serverRefreshGitStatus`
- Default key bindings: none
- Tags: `[git, refresh, status, branch, workspace, phase18.13, js-api]`
