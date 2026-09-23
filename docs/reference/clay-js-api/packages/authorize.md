---
id: packages.authorize
kind: clay-js-api
js_module: "clay:packages"
js_export: authorize
js_facade: runtime/js/packages.js::authorize
backing_rust: src/server/ops/packages.rs::op_clay_packages_authorize; src/packages/service.rs::PackageService::authorize_package; src/packages/authorization.rs::PackageAuthorizationRecord
deno_op: op_clay_packages_authorize
deno_op_path: src/server/ops/packages.rs::op_clay_packages_authorize
name: authorize
user_facing_name: Authorize Package Capabilities and Runtime Profile
summary: Record an explicit user capability grant and runtime profile for an installed package.
owner: server
phase: Phase 18.6
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: package
    type: string
    default: required
    description: Installed package name or the original requested source specifier, e.g. "@vendor/words" or "github:user/repo".
  - name: capabilities
    type: string[]
    default: required
    description: Manifest-declared capability names to approve, e.g. ["completion-provider", "parse-document"].
  - name: runtimeProfile
    type: enum
    default: native-trust
    description: One of "native-trust", "sandboxed", or "restricted"; recorded with the grant.
  - name: source
    type: string
    default: optional
    description: Optional provenance match; must equal the original requested specifier or the resolved package name.
  - name: approvedBy
    type: enum
    default: required
    description: One of "user", "cli", or "config"; names who approved the grant. Package activation can never approve.
security: Records an explicit user/CLI/config capability grant and runtime profile for an installed package's exact provenance. The trusted-only packages facade cannot be imported by third-party code, the op refuses while a package load entry is activating, and every granted capability must be declared by the manifest, so no package can grant itself authority. Grants are visible, revocable, provenance-bound, and fail-closed when absent; hidden JSON/TOML/ad hoc keys cannot grant capabilities. The grant authorizes a package to use separately implemented documented APIs; it does not materialize surfaces, bypass client validation, or grant filesystem, network, shell, extension loading, AI mutation, workspace, WASM, client-side JavaScript, package-manager, native-widget, raw-op, or implicit package-control authority.
agent_guidance: Call this only from trusted user configuration (init.js), the CLI, or an explicit user action, after installing the package. Grant only the capabilities the package declares and the user intends. Grants are required before a package that requests powerful capabilities can be enabled; revoke with `clay package revoke`.
lookup_tags: [packages, js-api, authorize, capability-grant, provenance, init]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# authorize

## Summary

Record an explicit user capability grant and runtime profile for an installed package.

## Description

`authorize({ package, capabilities, runtimeProfile, approvedBy })` writes an authorization record for one installed package. The record is bound to the installed package's provenance (requested specifier, source kind, resolved version, api prefix), so a version or source change makes the grant inert until the user authorizes again. Enable-time enforcement stays in one place: `PackageService::capability_granted` — read by `ensure_capability_grants` and by package-op dispatch — fails closed with `MissingCapabilityGrant` when a package requests a capability without a current grant.

The grant is durable: when the package has a current approval record, the grant is persisted in that record's explicit `grant` section (`clay-package-approvals.json`, owner-only, atomic write), so a later process — a fresh server or `clay package inspect`/`enable` — reads the same authority. Without a current approval record the grant stays in-memory for this generation and no durable section is written: a grant annotates an adoption the user already made, it never manufactures one.

Every granted capability must be declared by the package manifest (`clay.permissions` or the legacy `clay.capabilities` alias). A grant for an undeclared capability is rejected instead of being silently ignored, so an authorization record never claims authority the package did not request. Capabilities are validated with the same closed vocabulary used by the manifest validator; unknown names are rejected. Grant-only authorities the validator refuses in `clay.permissions` (`filesystem`, `network`, `shell`, `wasm`, `ai-tools`, `workspace-mutation`, `native-ui`, `client-runtime`, `raw-ops`, `language-server`, `package-control`, `package-import`) are declared through the legacy `clay.capabilities` alias when a package needs one; the declaration check accepts either field.

The op is trusted-only: `clay:packages` is absent from the shared third-party runtime, and the op additionally refuses while a package load entry is activating. Package code therefore cannot grant capabilities to itself or to another package.

Authority: `user-authorized-package-capability-grant`. Runtime path: `server-side-package-authorization-grant`. Grant work is install/enable/load/reload/explicit-user-command work only; the enforcement read is a cheap check against already-loaded authorization state at the enable, load, registration, and request boundaries, and never runs from keypress, paint, layout, scroll, text-event, edit-acknowledgement, pointer, or client hot paths.

## When to use

Use this API when the user decides to trust a specific installed package with specific capabilities. Bundled `@clay/*` packages are authorized automatically by the `loadPackage` resolver from the compiled bundled inventory and do not need this call. User-installed packages must be installed first (`clay package install <spec>`) and adopted before execution; `authorize` is the capability grant, not the adoption record.

Typical order for a third-party package: install, adopt (`clay package adopt`), `authorize` the capabilities the user wants, then `loadPackage` it from `init.js`. Adoption first matters for durability: the grant is stored on the approval record, so authorizing before adopting leaves an in-memory-only grant that does not survive the process. Running `authorize` without a current adoption still fails closed at enable with the adoption diagnostic, and a grant never substitutes for adoption.

## JavaScript usage

```ts
import { authorize } from "clay:packages";

authorize({
  package: "@vendor/words",
  capabilities: ["completion-provider"],
  runtimeProfile: "native-trust",
  approvedBy: "config",
});
```

## Example

```ts
// ~/.clay/init.js
import { authorize, loadPackage } from "clay:packages";

// The package was adopted earlier (`clay package adopt @vendor/words`); the
// grant below is recorded on that durable approval record. Grant only the
// capability this package declares and the user wants.
authorize({
  package: "@vendor/words",
  capabilities: ["completion-provider"],
  approvedBy: "config",
});

await loadPackage("@vendor/words");
```

## Options

- `package` (string, required): Installed package name or the original requested source specifier recorded at install time.
- `capabilities` (string[], required): Capability names to approve, 1..=21 entries, each declared by the manifest (either field; see Description for grant-only authorities). Duplicates are collapsed.
- `runtimeProfile` (string, optional): One of `"native-trust"`, `"sandboxed"`, `"restricted"`; defaults to `"native-trust"`. The profile is recorded and reported by `clay package inspect`; it does not itself change sandboxing behavior.
- `source` (string, optional): Provenance match. When present it must equal the original requested specifier or the resolved package name, otherwise the call fails with `packages.provenance_mismatch`.
- `approvedBy` (string, required): One of `"user"`, `"cli"`, `"config"`. Package activation can never approve.

Unknown option keys are rejected with `packages.invalid_grant` rather than ignored.

## Key bindings

No default key binding is assigned. This is a configuration/CLI-time API, not an interactive command.

## Custom properties

- `package` (`string`, required): installed package identity (name or original source specifier). Selects the package only; it cannot promote a package into the trusted runtime.
- `capabilities` (`string[]`, required): declared capability names to approve. Unknown or undeclared names are rejected.
- `runtimeProfile` (`enum`, default `native-trust`): recorded runtime profile.
- `source` (`string`, optional): provenance match for the installed package.
- `approvedBy` (`enum`, required): `user`, `cli`, or `config`.

## Return and async behavior

Synchronous. Returns the recorded grant summary:

```ts
{
  packageName: "@vendor/words",
  version: "1.2.0",
  sourceKind: "registry",
  capabilities: ["completion-provider"],
  runtimeProfile: "native-trust",
  approvedBy: "config",
  granted: true,
}
```

Re-authorizing the same package with the same capabilities is idempotent and returns the same shape. A grant is a complete set: a call replaces the package's previous grant (it never adds to it), so pass every capability the package should hold. The CLI verb `clay package authorize <name> --capability <cap>...` records the same grant and requires a current adoption first, so a grant made from the CLI is durable rather than an in-memory record in a process that is about to exit.

## Errors

- `packages.invalid_grant`: Options are not an object, an unknown option key was passed, `package`/`approvedBy` are missing or malformed, `capabilities` is not a 1..=21 entry array, or `runtimeProfile` is not one of the three documented values.
- `packages.unknown_permission`: A capability name is not in the closed Clay capability vocabulary.
- `packages.prohibited_authority`: A capability name is reserved for Clay-owned authority.
- `packages.not_installed`: No installed package matches `package`.
- `packages.provenance_mismatch`: `source` does not match the installed package's recorded provenance.
- `packages.undeclared_capability`: A granted capability is not declared by the package manifest.
- `packages.invalid_manifest`: The installed package's Clay metadata is invalid, so no grant can be recorded.
- `packages.grant_during_activation`: The call happened while a package load entry was activating.
- `packages.authorization_failed`: The service rejected the grant for another reason; the previous authorization state is unchanged.

## Permissions and security

`clay:packages` is trusted-only and absent from the shared third-party runtime, so third-party packages cannot call `authorize`. The op additionally refuses during package activation, so a bundled package's `loadEntry` cannot grant capabilities to itself or to another package.

This API grants no filesystem, network, shell, extension loading, AI mutation, workspace, WASM, client-side JavaScript, package-manager, native-widget, raw-op, or implicit package-control authority. It authorizes the use of separately implemented, separately validated documented Clay APIs; it does not materialize surfaces, bypass client validation, or widen the trusted runtime domain. Grants are provenance-bound and fail closed: an absent, stale, or revoked grant blocks enable with `MissingCapabilityGrant`, `clay package inspect` shows the current grants, and `clay package revoke` clears the grant together with the approval.

## Agent guidance

Call this only from trusted user configuration, the CLI, or an explicit user action, and only for capabilities the manifest declares and the user intends. Do not invent capability names and do not treat a grant as a way to enable a package that is still pending adoption. Verify the installed package first (`clay package inspect <name>`), then grant, then load.

## Backing implementation

- JS facade: `runtime/js/packages.js::authorize` (included by `src/server/facades.rs`)
- Deno op: `src/server/ops/packages.rs::op_clay_packages_authorize` (trusted extension only)
- Rust grant record: `src/packages/service.rs::PackageService::authorize_package` → `src/packages/authorization.rs::PackageAuthorizationRecord`, persisted as `src/packages/approvals.rs::CapabilityGrant` on the approval record
- Enforcement point: `src/packages/service.rs::PackageService::capability_granted` (read by `ensure_capability_grants`, the `package-control` gate, and package-op dispatch)

## Lookup metadata

- Stable ID: `packages.authorize`
- User-facing name: Authorize Package Capabilities and Runtime Profile
- Kind: `clay-js-api`
- Default key bindings: none
- Custom properties: `package` (required), `capabilities` (required), `runtimeProfile` (optional), `source` (optional), `approvedBy` (required)
- Tags: `packages`, `js-api`, `authorize`, `capability-grant`, `provenance`, `init`
