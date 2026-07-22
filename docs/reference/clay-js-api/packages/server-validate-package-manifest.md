---
id: clay.packages.serverValidatePackageManifest
kind: clay-js-api
js_module: "clay:packages"
js_export: serverValidatePackageManifest
js_facade: runtime/js/packages.js::serverValidatePackageManifest
backing_rust: src/packages/manifest.rs::validate_manifest_value
deno_op: op_clay_packages_validate_manifest
deno_op_path: src/server/ops/packages.rs::op_clay_packages_validate_manifest
name: serverValidatePackageManifest
user_facing_name: Validate Package Manifest
summary: Validate Package Manifest through the runtime-backed `clay:packages` Clay JavaScript facade.
owner: server
phase: Phase 16.5
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: manifest
    type: ClayPackageManifest
    default: required
    description: Behavior-changing setting `manifest` for this primitive gate API.
  - name: apiPrefix
    type: string
    default: required
    description: Behavior-changing setting `apiPrefix` for this primitive gate API.
  - name: entry
    type: string
    default: required
    description: Behavior-changing setting `entry` for this primitive gate API.
  - name: loadEntry
    type: string
    default: optional
    description: Behavior-changing setting `loadEntry` for this primitive gate API.
security: Validates package identity, semver, apiPrefix, permissions, modes, entry metadata, and prohibited authority metadata through server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, remote listener, native widget mutation, package installation, enable/disable, or package execution authority.
agent_guidance: Use `clay.packages.serverValidatePackageManifest` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, packagemanifestvalidation, packages]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverValidatePackageManifest

## Summary

Validate Package Manifest through the runtime-backed `clay:packages` Clay JavaScript facade.

## Description

`serverValidatePackageManifest` is the runtime-backed public primitive gate API for **Validate Package Manifest**. It is documented so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or `Deno.core.ops` bindings.

Authority: `server-first-package-manifest-validation`. Runtime path: `server-first-load-time-validation`. Package manifest validation runs at package fixture/enable/load time and must not appear on every edit, keypress, layout, or paint hot path.

## When to use

Use this API when server-side Clay JavaScript package/configuration code needs the documented `Validate Package Manifest` behavior. Do not use lower-level Rust functions, protocol structures, or raw `Deno.core.ops` names for this capability.

## JavaScript usage

```ts
import { serverValidatePackageManifest } from "clay:packages";

const validated = serverValidatePackageManifest({ name: "@clay/markdown", version: "0.1.0", clay: { apiPrefix: "markdown", permissions: ["mode-registration"], modes: ["markdown"], entry: "./dist/index.js" } });
```

## Example

```ts
const validated = serverValidatePackageManifest({ name: "@clay/markdown", version: "0.1.0", clay: { apiPrefix: "markdown", permissions: ["mode-registration"], modes: ["markdown"], entry: "./dist/index.js" } });
```

## Options

- `manifest` (`ClayPackageManifest`, default `required`): Behavior-changing setting `manifest` for this API.
- `apiPrefix` (`string`, default `required`): Behavior-changing setting `apiPrefix` for this API.
- `entry` (`string`, default `required`): Behavior-changing setting `entry` for this API.
- `loadEntry` (`string`, default `optional`): Behavior-changing setting `loadEntry` for this API.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.packages.serverValidatePackageManifest` in `~/.config/clay/init.js`.

## Custom properties

- `manifest` (`ClayPackageManifest`, default `required`): Behavior-changing setting `manifest` for this API.
- `apiPrefix` (`string`, default `required`): Behavior-changing setting `apiPrefix` for this API.
- `entry` (`string`, default `required`): Behavior-changing setting `entry` for this API.
- `loadEntry` (`string`, default `optional`): Behavior-changing setting `loadEntry` for this API.

## Return and async behavior

Returns JSON-serializable primitive gate metadata from the server-owned validator or registry. The facade is synchronous in the controlled server runtime and is intended for load-time, configuration-time, document-open, or activation-time work only.

The Phase 16.5 facade/runtime status is `runtime-backed`; the `deno_core` op wiring is executable during server-side configuration evaluation for runtime-backed entries.

## Errors

The runtime fails with actionable Clay error codes when arguments are malformed, package metadata fails server validation, required permissions are absent, duplicate prefixes/modes/commands are detected, ambiguous key bindings are found, or the requested primitive is intentionally unavailable.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Validates package identity, semver, apiPrefix, permissions, modes, entry metadata, and prohibited authority metadata through server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, remote listener, native widget mutation, package installation, enable/disable, or package execution authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.packages.serverValidatePackageManifest` when the user asks for Validate Package Manifest through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/packages.js::serverValidatePackageManifest`
- Deno op: `src/server/ops/packages.rs::op_clay_packages_validate_manifest` (`op_clay_packages_validate_manifest`)
- Backing Rust/current owner: `src/packages/manifest.rs::validate_manifest_value`
- Current implementation audit path: `src/packages/manifest.rs::ClayPackageManifest; src/packages/manifest.rs::validate_manifest_value`

## Lookup metadata

- Stable ID: `clay.packages.serverValidatePackageManifest`
- User-facing name: Validate Package Manifest
- Kind: `clay-js-api`
- Module/export: `clay:packages` / `serverValidatePackageManifest`
- Default key bindings: none
- Custom properties: `manifest`, `apiPrefix`, `entry`, `loadEntry`
- Tags: `js-api`, `packagemanifestvalidation`, `packages`
