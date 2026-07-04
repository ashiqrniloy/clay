---
id: clay.packages.loadPackage
kind: clay-js-api
js_module: "clay:packages"
js_export: loadPackage
js_facade: runtime/js/packages.ts::loadPackage
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
security: Resolves enabled user-authorized packages from bundled @clay/*, npm, GitHub/git, tarball, and local path sources through the same PackageService path. The module loader accepts only validated loadEntry modules recorded by the resolver and confines package imports to the validated package root. Loading a package does not grant filesystem, network, shell, extension loading, AI mutation, workspace, WASM, raw-op, native-widget, client-side JavaScript, package-manager, or package-control authority unless those capabilities are separately implemented and user-approved. The loadEntry default export is executed server-side only; no package JavaScript runs in the client.
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

The resolved `loadEntry` is confined to the validated package root for its own imports; it cannot load modules outside its root or escape the config root for any non-package specifier.

## When to use

Use this API as the default way to load a package from `~/.config/clay/init.js`. It is the preferred path over `serverLoadPackage(packageJson)` (which is a lower-level validation helper for fixtures) and over `markdownLoadMode()` (which remains a documented convenience alias for per-load options).

Packages must be installed and authorized before loading. Install/provenance discovery, authorization, metadata validation, conflict checking, canonicalization, and allowlist recording happen at load/reload time; module execution uses only the recorded package load-entry allowlist.

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
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

## Options

- `specifier` (string, required): An installed package name or original requested source specifier, such as `"@clay/markdown"`, `"@vendor/foo"`, `"github:user/repo"`, or a local-path spec recorded during install.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.packages.loadPackage` in `~/.config/clay/init.js` if they need a reload command, but reloading is not a default hot key.

## Custom properties

No behavior-changing custom properties. The `specifier` is the only user input; the rest of the activation (mode, commands, parse handler, keymaps) is package-owned and validated by the server.

## Return and async behavior

Returns a promise that resolves to the resolver's typed summary (`name`, `version`, `apiPrefix`, `loadEntrySpecifier`, `modes`, `permissions`, and contribution counts including `syntaxGrammars`). The loadEntry default export is invoked before the promise resolves, so the package's mode, commands, parse handler, and syntax grammar metadata are registered by the time the caller receives the result.

## Errors

- `clay.packages.invalid_specifier`: The specifier is not a non-empty string or an invalid bundled `@clay/*` shape.
- `clay.packages.not_installed`: The specifier is not present in the installed/source registry and is not a bundled package on disk.
- `clay.packages.load_failed`: The package metadata is invalid, a requested capability lacks user authorization, the `loadEntry` cannot be canonicalized inside the package root, or the package has malformed contributions.
- `clay.packages.conflict`: The package would conflict with an already-enabled package (duplicate prefix, mode, command, or keymap). The conflicting package is not enabled; the already-enabled set is unchanged.

## Permissions and security

This API grants package load authority only for installed, resolver-validated, user-authorized package sources. Loading a package does not grant these capabilities unless separate Clay APIs implement them and the user approves them:
- Filesystem or network access.
- Shell or process execution.
- AI model mutation or inference.
- WASM execution or native extension loading.
- Raw Deno ops or native widget handles.
- Client-side JavaScript execution.
- Package-control authority such as package enable/disable/replace/extend mutation.
- Package-manager install/remove/list authority.
- Arbitrary module loading from outside the config root or the validated package root.

The resolver reuses the Clay-owned `PackageService::enable` validation path: `assemble_package_record` for metadata validation, authorization grant checks, and `check_enabled_packages` for deterministic conflict detection before any contribution is activated. The module loader (`ClayModuleLoader`) only accepts a validated `loadEntry` that was explicitly recorded by the resolver in `PackageLoadEntryAllowlist`. The `loadEntry` is confined to the validated package root for its own imports; escaping imports are rejected.

## Agent guidance

Use this API as the one-line default when a user or script needs to load a package. Prefer it over `serverLoadPackage` (a lower-level validation helper) and over package-owned convenience aliases like `markdownLoadMode()`. Do not construct or pass inline manifest objects. Ensure the package has already been installed and user-authorized.

## Backing implementation

- JS facade: `runtime/js/packages.ts::loadPackage` (mirrored in the embedded `CLAY_FACADE_PACKAGES` constant in `src/server/js_runtime.rs`)
- Deno op: `src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier`
- Rust validation: `src/packages/service.rs::PackageService::enable` (calls `assemble_package_record` + authorization checks + `check_enabled_packages`)
- Module loader gate: `src/server/js_runtime.rs::ClayModuleLoader` (package allowlist branch via `PackageLoadEntryAllowlist`)

## Lookup metadata

- Stable ID: `clay.packages.loadPackage`
- User-facing name: Load Package by Specifier
- Kind: `clay-js-api`
- Default key bindings: none
- Custom properties: none (only the required `specifier`)
- Tags: `packages`, `js-api`, `load`, `source-aware`, `init`
