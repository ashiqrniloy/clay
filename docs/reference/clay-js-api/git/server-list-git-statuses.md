---
id: clay.git.serverListGitStatuses
kind: clay-js-api
js_module: "clay:git"
js_export: serverListGitStatuses
js_facade: runtime/js/git.js::serverListGitStatuses
backing_rust: src/server/git.rs::GitStatusCache::list_cached
deno_op: op_clay_git_list_statuses
deno_op_path: src/server/ops/git.rs::op_clay_git_list_statuses
name: serverListGitStatuses
user_facing_name: List Git Statuses
summary: List cached read-only Git status metadata for known workspace roots.
owner: server
phase: Phase 18.13
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Reads the server-owned Git status cache for authorized workspace roots only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.git.serverListGitStatuses` only through the documented `clay:git` facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not spawn Git, invent shell arguments, mutate repositories, or bypass workspace roots.
lookup_tags: [git, status, branch, workspace, phase18.13, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverListGitStatuses

## Summary

List cached read-only Git status metadata for known workspace roots.

## Description

`serverListGitStatuses` is the Phase 18.13 runtime-backed Clay JS API for **List Git Statuses**. It is exposed through the curated `clay:git` facade so package/configuration/runtime code does not call raw ops or Rust internals.

It returns the current `GitStatusCache` entries for server-authorized workspace roots. It does not spawn Git by default; use `serverRefreshGitStatus` for explicit refresh. This API is background/query work. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs read-only Git branch/dirty metadata for known workspace roots through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings. Packages such as `@clay/git` consume it to render status UI.

## JavaScript usage

```ts
import { serverListGitStatuses } from "clay:git";

const statuses = await serverListGitStatuses();
```

## Example

```ts
const statuses = await serverListGitStatuses();
for (const entry of statuses) {
  const head = entry.snapshot?.head;
  console.log(entry.workspaceRootId, entry.refreshState.kind, head);
}
```

## Options

No options. The API lists cached status for every known workspace root.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.git.serverListGitStatuses` through documented keybinding/configuration APIs where appropriate.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns a promise for `GitCachedStatus[]`. Each entry carries `workspaceRootId`, `workspaceRoot`, an optional `snapshot` (repository root, head state, dirty flag, changed-file count, last refresh status), and a `refreshState` (`idle`, `refreshing`, `last-success`, or `last-error`).

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

The runtime rejects malformed arguments and unavailable runtime ops as typed Clay errors. Roots that are not Git repositories surface as cached `lastRefresh.kind === "non-repository"` snapshots, not thrown errors.

## Permissions and security

Requires: [].

Reads the server-owned Git status cache for authorized workspace roots only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.git.serverListGitStatuses` only through the documented `clay:git` facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not spawn Git, invent shell arguments, mutate repositories, or bypass workspace roots.

## Backing implementation

- JS facade: `runtime/js/git.js::serverListGitStatuses`
- Deno op: `src/server/ops/git.rs::op_clay_git_list_statuses` (`op_clay_git_list_statuses`)
- Backing Rust/current owner: `src/server/git.rs::GitStatusCache::list_cached`

## Lookup metadata

- Stable ID: `clay.git.serverListGitStatuses`
- User-facing name: List Git Statuses
- Kind: `clay-js-api`
- Module/export: `clay:git` / `serverListGitStatuses`
- Default key bindings: none
- Tags: `[git, status, branch, workspace, phase18.13, js-api]`
