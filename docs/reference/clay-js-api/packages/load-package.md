---
id: packages.loadPackage
kind: clay-js-api
js_module: "clay:packages"
js_export: loadPackage
js_facade: runtime/js/packages.js::loadPackage
backing_rust: src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier; src/server/js_runtime.rs::ClayModuleLoader::resolve; src/server/js_runtime.rs::ClayModuleLoader::load
deno_op: op_clay_packages_load_package_by_specifier
deno_op_path: src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier
name: loadPackage
user_facing_name: Load Package by Specifier
summary: Resolve and activate an installed, user-authorized package from a single specifier string.
owner: server
phase: Phase 18.6
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: specifier
    type: string
    default: required
    description: Package specifier, e.g. "@clay/markdown", "@vendor/foo", or an installed source spec such as "github:user/repo".
security: The trusted-only packages facade cannot be imported by third-party code. Bundled trust requires an exact compiled inventory/provenance/integrity match; every other source remains third-party and must have a current durable user adoption record before enable or execution. Approved third-party load entries execute only in the shared third-party runtime, never the trusted runtime, and are not mutually isolated from sibling third-party packages. Graph relations and first-party mutations require declared extension scope plus durable user consent; stale/revoked approval fails closed. Root-confined loading grants no filesystem, network, shell, extension loading, AI mutation, workspace, WASM, raw-op, native-widget, client-side JavaScript, package-manager, or implicit package-control authority.
agent_guidance: Use as the one-line default for loading packages from ~/.config/clay/init.js. Do not pass enable/disable flags. Ensure packages are installed and authorized before loading.
lookup_tags: [packages, js-api, load, source-aware, init]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# loadPackage

## Summary

Resolve and activate an installed, user-authorized package from a single specifier string.

## Description

`loadPackage("@clay/markdown")` is the one-line end-user default for loading a package from `~/.config/clay/init.js`; installed source-aware packages can use the same API, e.g. `loadPackage("@vendor/foo")` or `loadPackage("github:user/repo")` after install and authorization. The resolver validates package metadata through Clay-owned `PackageService` validators, checks user-approved capability grants, enables the package (recording its contributions in a validated, conflict-checked set), and imports and executes the package's declared `loadEntry` so that its mode, commands, parse handler, and keymaps are registered under Clay's authority. No inline manifest object, no per-primitive registration, and no manual `clay` facade plumbing are required in user configuration.

The resolved `loadEntry` is confined to the validated package root for its own imports; it cannot load modules outside its root or escape the config root for any non-package specifier. Bundled trust comes only from Clay's compiled exact inventory/root/integrity check. Every other source executes in one shared adopted-third-party runtime after durable approval; third-party packages are a disclosed trust cohort and are not mutually isolated from sibling packages.

## When to use

Use this API as the default way to load a package from `~/.config/clay/init.js`. It is the preferred path over `serverLoadPackage(packageJson)` (which is a lower-level validation helper for fixtures) and over `markdownLoadMode()` (which remains a documented convenience alias for per-load options).

Packages must be installed and authorized before loading. Third-party packages must also be adopted out-of-band with `clay package adopt <name>`; JavaScript cannot approve itself or promote a package into the trusted runtime. Install/provenance discovery, capability authorization, adoption validation, relation/replacement consent, conflict checking, canonicalization, and allowlist recording happen before load-entry execution. Revoked or stale approval fails closed.

## JavaScript usage

```ts
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@vendor/foo");
await loadPackage("github:user/repo");
```

## Example

```ts
// ~/.config/clay/init.js
import { loadPackage } from "clay:packages";
import { bindKey } from "clay:keybindings";

await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
```

## Options

- `specifier` (string, required): An installed package name or original requested source specifier, such as `"@clay/markdown"`, `"@vendor/foo"`, `"github:user/repo"`, or a local-path spec recorded during install.

## Key bindings

No default key binding is assigned. Users may bind a key to `packages.loadPackage` in `~/.config/clay/init.js` if they need a reload command, but reloading is not a default hot key.

## Custom properties

- `specifier` (`string`, required): installed package name or original source specifier. It selects package identity only; it cannot grant capabilities, adoption, graph relations, replacement, or trusted-domain status.

## Return and async behavior

Returns a promise that resolves to the resolver's typed summary, including exact package identity, validated load-entry specifier, contribution metadata, and owning runtime domain. Before resolution, a trusted package load entry has run in the trusted runtime or an approved third-party load entry has run through the Rust bridge in the shared third-party runtime, with resulting inert registrations absorbed by the host.

## Errors

- `packages.invalid_specifier`: The specifier is not a non-empty string or an invalid bundled `@clay/*` shape.
- `packages.not_installed`: The specifier is not present in the installed/source registry and is not a bundled package on disk.
- `packages.load_failed`: Package metadata/provenance/integrity is invalid, a capability grant is missing, third-party adoption is pending/stale/revoked, a relation/replacement lacks owner scope or user approval, the load entry escapes its canonical root, or contributions are malformed. No package code executes before these gates pass.
- `packages.conflict`: The package graph has a duplicate/cycle/unapproved conflict. Enable is transactional; the prior enabled set remains intact.

## Permissions and security

`clay:packages` is trusted-only and absent from the shared third-party runtime, so a third-party package cannot call `loadPackage`, adopt/revoke itself, or drive package control. This API activates only an installed, resolver-validated source. Trusted classification requires an exact compiled bundled inventory match; normal user approval cannot promote code. Every other package requires a current durable adoption record, and its JavaScript executes only in the shared third-party runtime. Third-party packages in that runtime are not mutually isolated.

Loading does not grant these capabilities unless separate Clay APIs implement them and the user approves them:
- Filesystem or network access.
- Shell or process execution.
- AI model mutation or inference.
- WASM execution or native extension loading.
- Raw Deno ops or native widget handles.
- Client-side JavaScript execution.
- Package-control authority such as package enable/disable/replace/extend mutation.
- Package-manager install/remove/list authority.
- Arbitrary module loading from outside the config root or the validated package root.

The resolver reuses the Clay-owned `PackageService::enable` transaction: manifest assembly/namespace validation, exact capability grants, durable adoption coverage, graph cycle/conflict checks, target-declared extension points, user-approved relation/replacement scope, and rollback on failure all run before activation. `ClayModuleLoader` accepts only a validated `loadEntry` recorded in `PackageLoadEntryAllowlist`; transitive imports remain inside that canonical package root. Rust chooses the runtime domain from host provenance, never caller fields.

## Agent guidance

Use this API as the one-line default when a user or script needs to load a package. Prefer it over `serverLoadPackage` (a lower-level validation helper) and over package-owned convenience aliases like `markdownLoadMode()`. Do not construct or pass inline manifest objects. Ensure the package has already been installed and user-authorized.

## Backing implementation

- JS facade: `runtime/js/packages.js::loadPackage` (included by `src/server/facades.rs`)
- Deno op: `src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier`
- Rust validation: `src/packages/service.rs::PackageService::enable` (calls `assemble_package_record` + authorization checks + `check_enabled_packages`)
- Module loader gate: `src/server/js_runtime.rs::ClayModuleLoader` (package allowlist branch via `PackageLoadEntryAllowlist`)

## Lookup metadata

- Stable ID: `packages.loadPackage`
- User-facing name: Load Package by Specifier
- Kind: `clay-js-api`
- Default key bindings: none
- Custom properties: `specifier` (required package identity/source selector)
- Tags: `packages`, `js-api`, `load`, `source-aware`, `init`
