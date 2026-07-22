---
id: clay.packages.serverLoadPackage
kind: clay-js-api
js_module: "clay:packages"
js_export: serverLoadPackage
js_facade: runtime/js/packages.js::serverLoadPackage
backing_rust: src/packages/record.rs::assemble_package_record
deno_op: op_clay_packages_load_package
deno_op_path: src/server/ops/packages.rs::op_clay_packages_load_package
name: serverLoadPackage
user_facing_name: Load Package
summary: Validate a package.json-shaped Clay package record and return inert load-time summary metadata.
owner: server
phase: Phase 17
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: packageJson
    type: PackageJson
    default: required
    description: The package.json-shaped object containing name, version, and clay metadata to validate.
  - name: clay.apiPrefix
    type: string
    default: required
    description: Package-owned API prefix used for provenance and contribution ID validation.
  - name: clay.docs
    type: string
    default: required
    description: Package documentation path required by the load contract.
  - name: clay.performance.estimatedManifestBytes
    type: number
    default: required
    description: Declared package manifest payload estimate checked against server budgets.
security: Validates a Clay package record through server validation and returns inert summary metadata only; package installation, enable/disable mutation, external package-manager execution, and package JavaScript execution remain separate and it does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, remote listener, native widget mutation, package installation, enable/disable, or package execution authority.
agent_guidance: Use this API to verify package load metadata in the controlled server runtime; do not use it to install packages, enable packages, execute package JavaScript, or call raw Deno ops.
lookup_tags: [js-api, packages, package-loading, primitive-gate]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverLoadPackage

## Summary

Validate a package.json-shaped Clay package record and return inert load-time summary metadata.

## Description

`serverLoadPackage` is the Phase 17 package-loading Clay JS facade for validating a full package record through the server-owned Rust assembler. It accepts package metadata, Clay package metadata, docs/performance declarations, API dependencies, and inert contribution descriptors, then returns summary metadata for load-time inspection.

This API does **not** install packages, enable or disable packages, spawn a package manager, or execute package entry points. It routes through `op_clay_packages_load_package`, which calls `assemble_package_record` and the typed validators behind the package contract.

## When to use

Use this API from controlled server-side package/configuration fixtures when you need to validate Clay package metadata before enable/load. Use the package service or CLI for install/enable/disable workflows; do not treat this API as package-manager authority.

## JavaScript usage

```ts
import { serverLoadPackage } from "clay:packages";

const loaded = serverLoadPackage(packageJson);
```

## Example

```ts
const loaded = serverLoadPackage({
  name: "@clay/markdown",
  version: "0.1.0",
  clay: {
    apiPrefix: "markdown",
    entry: "./dist/index.js",
    loadEntry: "./dist/load.js",
    permissions: ["mode-registration", "mode-activation", "command-registration"],
    modes: ["markdown"],
    docs: "./docs/index.md",
    performance: { estimatedManifestBytes: 1024 },
    apiDependencies: ["clay.modes.serverRegisterModePattern"],
    contributions: { commands: [] },
  },
});
```

## Options

- `packageJson` (`PackageJson`, required): Package metadata object to validate.
- `clay.apiPrefix` (`string`, required): Package-owned API prefix used for provenance and contribution IDs.
- `clay.docs` (`string`, required): Package documentation path.
- `clay.performance.estimatedManifestBytes` (`number`, required): Declared manifest size estimate checked by server validation.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.packages.serverLoadPackage` in `~/.config/clay/init.js`, but package loading itself is intended for package/load tooling rather than ordinary editor commands.

## Custom properties

- `packageJson` (`PackageJson`, default `required`): package.json-shaped object to validate.
- `clay.apiPrefix` (`string`, default `required`): package-owned API prefix.
- `clay.docs` (`string`, default `required`): documentation path.
- `clay.performance.estimatedManifestBytes` (`number`, default `required`): budgeted manifest estimate.

## Return and async behavior

Returns synchronously in the controlled server runtime with JSON-serializable summary metadata: package identity, entry paths, docs path, estimated manifest bytes, declared API dependencies, and contribution counts.

## Errors

Fails with actionable Clay package load diagnostics when JSON is malformed, required package contract fields are missing, a contribution claims a reserved `clay.*` ID, package-owned IDs do not use the package prefix, required contribution permissions are undeclared, docs/performance metadata is missing, or payload budgets are exceeded.

## Permissions and security

No additional permission is required because this API validates metadata and grants no authority by itself.

Validation is server-side. The facade hides raw op names from callers; users should not call `Deno.core.ops` directly. This API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, remote listener, native widget mutation, package installation, enable/disable, or package execution authority.

## Agent guidance

Use `clay.packages.serverLoadPackage` only for package metadata validation and load-contract inspection. Do not infer that a successful result means the package is installed, enabled, trusted, or executed. Prefer documented Clay JS facades over direct Rust paths or raw op calls.

## Backing implementation

- JS facade: `runtime/js/packages.js::serverLoadPackage`
- Deno op: `src/server/ops/packages.rs::op_clay_packages_load_package` (`op_clay_packages_load_package`)
- Rust function: `src/packages/record.rs::assemble_package_record`
- Current owner: `src/packages/record.rs::PackageRecord`; `src/server/ops/packages.rs::op_clay_packages_load_package`

## Lookup metadata

- Stable ID: `clay.packages.serverLoadPackage`
- User-facing name: Load Package
- Kind: `clay-js-api`
- Module/export: `clay:packages` / `serverLoadPackage`
- Default key bindings: none
- Custom properties: `packageJson`, `clay.apiPrefix`, `clay.docs`, `clay.performance.estimatedManifestBytes`
- Tags: `js-api`, `packages`, `package-loading`, `primitive-gate`
