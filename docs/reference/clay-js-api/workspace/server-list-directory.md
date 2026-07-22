---
id: clay.workspace.serverListDirectory
kind: clay-js-api
js_module: "clay:workspace"
js_export: serverListDirectory
js_facade: runtime/js/workspace.js::serverListDirectory
backing_rust: src/server/workspace.rs::WorkspaceState::list_directory
deno_op: op_clay_workspace_list_directory
deno_op_path: src/server/ops/workspace.rs::op_clay_workspace_list_directory
name: serverListDirectory
user_facing_name: List Directory
summary: List a bounded page of workspace-root-relative file entries with server ignore rules and diagnostics.
owner: server
phase: Phase 18.12
visibility: public
permissions: ["workspace-read"]
key_bindings: []
custom_properties:
  - name: maxDepth
    type: number
    default: 8
    description: Maximum listing depth accepted by the bounded server file-list service.
  - name: maxEntries
    type: number
    default: 1000
    description: Maximum returned entries before the page is marked truncated.
  - name: cancelTokenId
    type: string
    default: none
    description: Optional listing cancellation token created by serverCreateListingCancelToken.
security: Uses server validation to list only paths inside a known workspace root after traversal checks and bounded ignore/depth/count rules using the component grammar; malformed, unsupported, or oversized root .gitignore input aborts with a bounded diagnostic instead of broadening traversal. Packages cannot list arbitrary paths or bypass diagnostics; the API does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `clay.workspace.serverListDirectory` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader workspace, filesystem, network, shell, extension loading, AI mutation, package, WASM, native-widget, or client-side JavaScript authority.
lookup_tags: [workspace, directory-listing, file-browser, cancellation, phase18.12, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# serverListDirectory

## Summary

List a bounded page of workspace-root-relative file entries with server ignore rules and diagnostics.

## Description

`serverListDirectory` is the Phase 18.12 runtime-backed Clay JS API for **List Directory**. It is exposed through the curated `clay:workspace` facade so package/configuration/runtime code does not call raw ops or Rust internals.

This API is server-first background/action work. It snapshots root authority under the workspace lock, then traverses through a bounded blocking worker without holding that lock. It must not run in ordinary typing, Masonry paint, Masonry layout, pointer, scroll, keypress, or text-event hot paths.

## When to use

Use this API when server-side Clay JavaScript needs list directory behavior through the documented facade. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings.

## JavaScript usage

```ts
import { serverListDirectory } from "clay:workspace";

const page = await serverListDirectory({ rootId, relativePath: "src", maxDepth: 2, maxEntries: 256 });
```

## Example

```ts
const page = await serverListDirectory({ rootId, relativePath: "src", maxDepth: 2, maxEntries: 256 });
```

## Options

`rootId` (`WorkspaceRootId`, required), `relativePath` (`string`, default `""`), `maxDepth` (`number`, clamped to `8`), `maxEntries` (`number`, clamped to `1000`), and `cancelTokenId` (`string`, optional).

Root `.gitignore` supports a deliberately bounded grammar: blank lines, column-zero comments, literal Unicode scalars, `?` for one scalar, per-segment backtracking `*` for zero or more scalars, slash-separated root-relative paths, optional leading `/` root anchoring, and an optional trailing `/` for directory-only matching. Negation, escaping, character classes, `**`, empty path components, control characters, invalid UTF-8, more than 4096 lines/1024 retained rules/256 scalars per rule, and files above the 1 MiB auxiliary-read ceiling are rejected visibly.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.workspace.serverListDirectory` through documented keybinding/configuration APIs where appropriate.

## Custom properties

- `maxDepth` (`number`, default `8`): maximum listing depth.
- `maxEntries` (`number`, default `1000`): maximum returned entries.
- `cancelTokenId` (`string`, optional): cancellation token id.

## Return and async behavior

Returns a `FileListPage` with entries, `truncated`, `cancelled`, and diagnostics. Entries include kind, relative path, size hint, child count, and optional per-entry diagnostic.

The facade is asynchronous and uses an explicit `deno_core` op wrapper behind the public Clay JS API.

## Errors

Malformed arguments, unknown roots, unauthorized targets, traversal escapes, missing/special targets, and blocking-task failures reject the call. Cancellation returns a bounded page with `cancelled: true`. Invalid or oversized root-ignore input returns an empty, truncated page with one bounded `WorkspaceDiagnostic`; Clay never silently treats unsupported syntax as a broader traversal.

## Permissions and security

Requires: ["workspace-read"].

Uses server validation to list only paths inside a known workspace root after traversal checks and bounded depth/count/component-ignore rules. Authority is captured before traversal, the canonical target is revalidated inside the root, and malformed/unsupported/oversized `.gitignore` input aborts with a bounded diagnostic instead of broadening visibility. Packages cannot list arbitrary paths or bypass diagnostics; this API does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.workspace.serverListDirectory` only through the documented Clay JS facade. Do not call raw Rust functions, protocol DTOs, or `Deno.core.ops`; do not invent broader authority.

## Backing implementation

- JS facade: `runtime/js/workspace.js::serverListDirectory`
- Deno op: `src/server/ops/workspace.rs::op_clay_workspace_list_directory` (`op_clay_workspace_list_directory`)
- Backing Rust/current owner: `src/server/workspace.rs::WorkspaceState::list_directory`

## Lookup metadata

- Stable ID: `clay.workspace.serverListDirectory`
- User-facing name: List Directory
- Kind: `clay-js-api`
- Module/export: `clay:workspace` / `serverListDirectory`
- Default key bindings: none
- Tags: `[workspace, directory-listing, file-browser, cancellation, phase18.12, js-api]`
