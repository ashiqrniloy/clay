---
id: packages.serverValidatePackagePermissions
kind: clay-js-api
js_module: "clay:packages"
js_export: serverValidatePackagePermissions
js_facade: runtime/js/packages.js::serverValidatePackagePermissions
backing_rust: src/packages/permissions.rs::parse_permission
deno_op: op_clay_packages_validate_permissions
deno_op_path: src/server/ops/packages.rs::op_clay_packages_validate_permissions
name: serverValidatePackagePermissions
user_facing_name: Validate Package Permissions
summary: Validate Package Permissions through the runtime-backed `clay:packages` Clay JavaScript facade.
owner: server
phase: Phase 16.5
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: permissions
    type: string[]
    default: required
    description: Behavior-changing setting `permissions` for this primitive gate API.
security: Validates declared package permissions and grants no authority by itself; rejects unknown, undeclared, prohibited, or unapproved scopes through server validation and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, remote listener, native widget mutation, package installation, enable/disable, or package execution authority.
agent_guidance: Use `packages.serverValidatePackagePermissions` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, packagepermissionvalidation, packages]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverValidatePackagePermissions

## Summary

Validate Package Permissions through the runtime-backed `clay:packages` Clay JavaScript facade.

## Description

`serverValidatePackagePermissions` is the runtime-backed public primitive gate API for **Validate Package Permissions**. It is documented so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or `Deno.core.ops` bindings.

Authority: `server-first-package-permission-validation`. Runtime path: `server-first-load-time-validation`. Package permission validation runs at package enable/load time and must not appear on every edit, keypress, layout, or paint hot path.

## When to use

Use this API when server-side Clay JavaScript package/configuration code needs the documented `Validate Package Permissions` behavior. Do not use lower-level Rust functions, protocol structures, or raw `Deno.core.ops` names for this capability.

## JavaScript usage

```ts
import { serverValidatePackagePermissions } from "clay:packages";

const result = serverValidatePackagePermissions(["mode-registration", "mode-activation"]);
```

## Example

```ts
const result = serverValidatePackagePermissions(["mode-registration", "mode-activation"]);
```

## Options

- `permissions` (`string[]`, default `required`): Behavior-changing setting `permissions` for this API.

## Key bindings

No default key binding is assigned. Users may bind a key to `packages.serverValidatePackagePermissions` in `~/.config/clay/init.js`.

## Custom properties

- `permissions` (`string[]`, default `required`): Behavior-changing setting `permissions` for this API.

## Return and async behavior

Returns JSON-serializable primitive gate metadata from the server-owned validator or registry. The facade is synchronous in the controlled server runtime and is intended for load-time, configuration-time, document-open, or activation-time work only.

The Phase 16.5 facade/runtime status is `runtime-backed`; the `deno_core` op wiring is executable during server-side configuration evaluation for runtime-backed entries.

## Errors

The runtime fails with actionable Clay error codes when arguments are malformed, package metadata fails server validation, required permissions are absent, duplicate prefixes/modes/commands are detected, ambiguous key bindings are found, or the requested primitive is intentionally unavailable.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Validates declared package permissions and grants no authority by itself; rejects unknown, undeclared, prohibited, or unapproved scopes through server validation and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, remote listener, native widget mutation, package installation, enable/disable, or package execution authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `packages.serverValidatePackagePermissions` when the user asks for Validate Package Permissions through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/packages.js::serverValidatePackagePermissions`
- Deno op: `src/server/ops/packages.rs::op_clay_packages_validate_permissions` (`op_clay_packages_validate_permissions`)
- Backing Rust/current owner: `src/packages/permissions.rs::parse_permission`
- Current implementation audit path: `src/packages/permissions.rs::PackagePermission; src/packages/permissions.rs::parse_permission`

## Lookup metadata

- Stable ID: `packages.serverValidatePackagePermissions`
- User-facing name: Validate Package Permissions
- Kind: `clay-js-api`
- Module/export: `clay:packages` / `serverValidatePackagePermissions`
- Default key bindings: none
- Custom properties: `permissions`
- Tags: `js-api`, `packagepermissionvalidation`, `packages`
