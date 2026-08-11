---
id: language-server.authorizeLanguageServer
kind: clay-js-api
js_module: "clay:language-server"
js_export: authorizeLanguageServer
js_facade: runtime/js/language-server.js::authorizeLanguageServer
backing_rust: src/packages/service.rs::PackageService::authorize_language_server; src/server/ops/language_server.rs::op_clay_language_server_authorize
deno_op: op_clay_language_server_authorize
deno_op_path: src/server/ops/language_server.rs::op_clay_language_server_authorize
name: authorizeLanguageServer
user_facing_name: Authorize Language Server
summary: Approve one fixed package language-server contribution for current workspace roots before loadPackage seals authority.
owner: server
phase: Phase 18.20
visibility: public
permissions: ["language-server"]
key_bindings: []
custom_properties:
  - name: package
    type: string
    default: required
    description: Package name/version/specifier of the installed package that owns the contribution.
  - name: contribution
    type: string
    default: required
    description: Package-prefixed contribution id declared in clay.contributions.languageServers.
  - name: workspaceRootIds
    type: array<number|string>
    default: required
    description: Known directory workspace-root ids the session may bind to.
hot_path_policy: Evaluated during configuration root evaluation only (init.js); never executed during typing, parsing, Masonry layout, or paint hot paths. First loadPackage call seals authority mutation for the runtime generation.
security: deny-by-default; never auto-authorized for bundled packages; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript; binds exact package provenance, contribution fingerprint, canonical executable, inherited-environment declaration, and approved directory roots; starts no process at grant time; grant evaluation happens before any package code executes; loaded package code cannot self-grant even though it can import the same facade.
agent_guidance: Use only for documented language-server contributions. Never expose hidden env vars, JSON/TOML keys, shell strings, or unvalidated executables.
lookup_tags: [configuration, language-server, grant, init-js, phase18.20, runtime-backed, deny-by-default]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# authorizeLanguageServer

## Summary

`authorizeLanguageServer` records one fixed language-server contribution grant from `~/.config/clay/init.js`. The grant binds exact package provenance, contribution descriptor fingerprint, resolved canonical executable path, declared inheritance environment, and current directory workspace roots.

## Description

This is a **configuration-only** API. Grants are accepted only during configuration root evaluation (`init.js`) and **before** the first `loadPackage` call. Once `loadPackage` seals authority, no further grants are accepted and loaded package code cannot self-grant even though it can import the same `clay:language-server` facade.

The grant layer starts **no process**. A process spawns only later when `startLanguageServerSession` is called with matching package, contribution, and an approved workspace root id. Grant metadata is validated at authorization time (contribution must exist, executable must resolve via `PATH`, workspace roots must be known) and revalidated on every session operation.

`language-server` is deny-by-default. Bundled `@clay/*` package auto-authorization explicitly excludes `language-server` unless a current exact grant already exists. User-installed packages require an explicit grant and an explicit `loadPackage` specifier.

## When to use

Use this API from `~/.config/clay/init.js` before any `loadPackage` call to authorize a language-server bridge package. Never call from loaded package code — the grant seal enforced at first `loadPackage` will reject the call.

## JavaScript usage

```ts
import { authorizeLanguageServer } from "clay:language-server";
```

## Example

```ts
// ~/.config/clay/init.js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@vendor/lsp-bridge",
  contribution: "lsp-bridge.server",
  workspaceRootIds: [1],
});

await loadPackage("@vendor/lsp-bridge");
```

## Options

- `package`: installed package name, specifier, or version pattern that owns the contribution.
- `contribution`: contribution id from the package's `clay.contributions.languageServers` array.
- `workspaceRootIds`: array of known directory workspace-root ids; session spawn later binds exactly one.

## Key bindings

No key bindings are registered by this API.

## Custom properties

- `package`
- `contribution`
- `workspaceRootIds`

## Return and async behavior

Returns a `LanguageServerGrantSummary` with `package`, `version`, `sourceKind`, `contribution`, `executable`, `workspaceRootIds`, and `approvedBy`. Always awaited (`async: true`).

## Errors

- `language_server.invalid_grant` — missing or excess fields, invalid JSON.
- `language_server.unknown_package` — package not installed.
- `language_server.invalid_contribution` — no matching contribution declared.
- `language_server.executable_not_found` — executable not resolvable via `PATH`.
- `language_server.unknown_workspace_root` — workspace root id not known.
- `language_server.authorization_sealed` — grant attempted after `loadPackage` sealed authority.
- `language_server.duplicate_grant` — grant already exists for this (package, contribution) pair.

## Permissions and security

Requires: `language-server`. Deny-by-default; never auto-authorized for bundled packages. server-side validation checks package provenance, contribution descriptor fingerprint, canonical executable resolution, and workspace root membership before recording the grant. does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript. Grant binds exact package provenance (name/version/source), contribution descriptor fingerprint, resolved canonical executable, declared inheritance environment names, and approved directory roots. Revalidated on every session operation. Revocation, package update, contribution change, or root removal terminates all associated sessions. See `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Agent guidance

Use only for documented package-prefixed language-server contributions with validated fixed executable/argv metadata. Never introduce hidden env vars, JSON/TOML keys, shell strings, or unvalidated executables.

## Backing implementation

- Facade: `runtime/js/language-server.js::authorizeLanguageServer`
- Op: `src/server/ops/language_server.rs::op_clay_language_server_authorize`
- Rust: `src/server/language_server.rs PackageService grant registry; src/server/ops/language_server.rs`

## Lookup metadata

Tags: configuration, language-server, grant, init-js, phase18.20, runtime-backed, deny-by-default.
