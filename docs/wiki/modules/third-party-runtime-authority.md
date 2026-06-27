# Third-Party Runtime Authority

## Source

- `src/packages/manager.rs`
- `src/packages/service.rs`
- `src/packages/manifest.rs`
- `src/packages/record.rs`
- `src/packages/permissions.rs`
- `src/packages/conflict.rs`
- `src/server/ops/packages.rs`
- `src/server/js_runtime.rs`
- `src/server/runtime_sandbox.rs`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/package-loading.md`
- `docs/design/persistent-runtime-sandbox.md`
- `docs/wiki/modules/persistent-runtime-hardening.md`
- `plans/034-Persistent-Runtime-Hardening-Before-Third-Party-Package-Authority.md`

## Scope

This page records the current package/runtime primitive inventory and the third-party authority gaps. It is policy evidence only. Non-`@clay/*` package execution stays deny-by-default until a later approved authority decision log grants exact runtime authority.

Authority boundaries are separate:

```text
install != enable != load != runtime execution != package-manager execution != client behavior delivery
```

## Current Primitive Inventory

- **Install:** `PackageService::install` delegates package download, registry access, dependency resolution, lockfile/integrity/caching, and package-store mutation to `PackageManagerBackend` / `PnpmBackend`. `pnpm add` uses `--ignore-scripts` by default. `FakeBackend` never spawns a process. Install records package metadata; it does not enable or execute package JavaScript.
- **Enable:** `PackageService::enable` reads an installed `package.json`, builds a `PackageRecord` through `assemble_package_record`, then runs `check_enabled_packages`. On conflict, enable rolls back the candidate record. Enable does not execute `entry`, `loadEntry`, command handlers, parse handlers, or package-manager scripts.
- **Load:** `op_clay_packages_load_package_by_specifier` accepts only resolver-validated first-party `@clay/*` specifiers. It rejects bare names such as `left-pad`, custom scoped names such as `@scope/pkg`, URLs, local path, traversal, registry-style, and malformed `@clay/*` specifiers before module loading.
- **Runtime execution:** `ClayModuleLoader` is deny-by-default. It loads curated `clay:*` facades, configuration-root relative modules, the vendored markdown-it shim, and first-party `loadEntry` modules recorded in `FirstPartyLoadEntryAllowlist`. Transitive package imports are confined to the validated package root.
- **Package-manager execution:** Package-manager process execution is only the backend boundary. Package-manager stdout/stderr/exit code and discovered `package.json` metadata are not runtime authority, and package-manager installation/metadata records do not grant runtime-execution authority.
- **Sandbox:** `RuntimeSandboxSupervisor` and `clay-runtime-sandbox` are internal harness evidence for spawn/handshake, controlled evaluation, timeout kill/restart, payload-budget rejection, and denied filesystem/network/shell globals. They are not production routing and do not grant third-party package execution.
- **Client behavior delivery:** Clients receive validated inert behavior manifests, SDUI snapshots/updates, decorations, parse updates, and protocol messages. Package JavaScript never runs in the Rust client or Masonry paint/layout/input handlers.
- **Permissions:** Known package permissions are `mode-registration`, `mode-activation`, `command-registration`, `package-configuration`, `parse-document`, `render-decorations`, `render-folding`, and `completion-provider`. Prohibited authorities such as filesystem, network, shell, AI mutation, remote listener, WASM execution, raw Deno ops, native widget, client JavaScript, package installation, package enable/disable, and workspace mutation are rejected by default.

## Third-Party Authority Gaps

Third-party runtime execution still lacks these approved, tested primitives/policies:

1. **Trust and identity:** No trusted third-party publisher/source policy, namespace ownership rule, package-source provenance record, typosquat handling, compatibility rule, or signed/explicit trust record exists.
2. **Registry and integrity:** Clay delegates package management to pnpm-compatible tooling, but it does not yet record third-party resolved version, registry, lockfile/integrity digest, tarball/source path, update policy, offline/cache behavior, or sanitized package-manager diagnostic policy as runtime authority evidence.
3. **Permission model:** Current permissions cover first-party inert/runtime primitives only. No third-party grant source, request syntax, approval workflow, or runtime enforcement matrix exists for non-`@clay/*` packages.
4. **Denied authorities:** Filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget, client-JS, raw-op, remote listener, and workspace mutation remain denied unless a future approved decision grants one narrow capability with tests.
5. **Production sandbox:** The current sandbox is a harness, not production API. Third-party execution needs parent-validated typed requests/responses, payload budgets, timeout/heap policy, restart semantics, stale generation rejection, sanitized diagnostics, and no child access to workspace roots, file descriptors, package-manager handles, V8 handles, raw op names, or capability tokens.

Denied authorities stay explicit: filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget, client-JS, raw-op, remote listener, and workspace mutation remain denied unless a later approval grants a narrow capability.
6. **Rollback and incident response:** Clay has package enable rollback and runtime generation invalidation primitives, but no third-party disable/update/rollback policy for withdrawing behavior manifests, parse handlers, SDUI state, commands, package-manager side effects, or last-valid client state.
7. **Executable gates:** Existing tests keep non-`@clay/*` execution blocked. Future widening needs docs/source tests that require an approved authority decision-log reference before resolver policy changes.

## Trust and Identity Policy

Non-`@clay/*` packages require an explicit trust record before any resolver, loader, sandbox, or facade can treat them as executable. Trust is exact identity, not a permission shortcut.

Minimum trusted identity record:

```toml
[[trusted_package]]
name = "@vendor/example"
version = "1.2.3"
registry = "https://registry.npmjs.org/"
integrity = "sha512-..."
clay_prefix = "example"
source_kind = "npm-registry"
publisher = "vendor"
clay_api_compatibility = "^0.1"
```

Policy:

- Package `name`, resolved `version`, `registry` or source location, package-manager `integrity`, `clay_prefix`, `source_kind`, `publisher`/owner, and `clay_api_compatibility` are the identity tuple.
- `clay_prefix` must match `clay.apiPrefix`, pass existing prefix validation, and remain unique through existing conflict checks.
- Version ranges, install specs, package-manager metadata, and Clay manifest metadata are not runtime identity until matched to the exact trusted identity tuple.
- Accepted source kinds start at `npm-registry`; `local-path`, `tarball`, `git`, and `custom-registry` stay denied until a trust record names the source kind and a later decision approves source-specific checks.
- Bare names, custom scopes, URLs, local paths, tarballs, git sources, aliases, registry redirects, ambiguous local paths, unknown publishers, namespace hijacks, typosquats, incompatible Clay API ranges, missing signatures/provenance, conflicting prefixes, and conflicting contribution IDs fail closed.
- Existing `PackageRecord`, `PackageService`, and conflict primitives carry package name/version/prefix/contribution provenance; third-party trust still needs generic source provenance, publisher/source-owner, registry/integrity, and trust-record storage fields before resolver widening.
- Trust records grant identity only. Runtime authority still requires explicit permissions, sandbox routing, denied-authority enforcement, tests, and an approved decision log.

Trust validation belongs to install, enable, load, reload, startup, or background verification. It is never keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot-path work.

## Third-Party Permission Model and Denied Authorities

Third-party package permissions are narrow parent-enforced grants, not broad trust. A package manifest can request permissions, but the grant source must be an explicit user/admin/decision-approved trust+permission record tied to package name, version, registry/source, integrity, and `apiPrefix`.

Allowed initial permission strings reuse the existing package permission primitive: `mode-registration`, `mode-activation`, `command-registration`, `package-configuration`, `parse-document`, `render-decorations`, `render-folding`, and `completion-provider`.

Enforcement points stay server-owned:

- enable/load validates requested permissions and rejects unknown/prohibited strings;
- mode, command, configuration, layout, UI, input, state, parse, decoration, folding, and completion registrations validate the matching permission before activation;
- sandbox parse/completion/evaluation requests carry only parent-approved permission data;
- parent revalidates bounded inert outputs before publishing behavior manifests, SDUI, decorations, folding, completion, or parse updates.

Broad/catch-all permissions are rejected: `trusted-third-party`, `all`, `admin`, `system`, `host`, `runtime`, `raw-op`, `raw-deno-ops`, and any unknown string are not aliases for authority.

Denied authorities remain denied unless a later approved decision grants one narrow capability with tests: filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget, client-JS, raw-op, remote listener, workspace mutation, package installation, package enable/disable, raw `Deno.core.ops`, native handles, client-side JavaScript, direct Masonry/widget mutation, and workspace roots/file descriptors/capability tokens in the child process.

Diagnostics for permission failures must include package name, package version, `apiPrefix`, requested permission, grant source, primitive category, contribution ID or handler token when available, and failed rule. They must not include source text, environment variables, credentials, raw package-manager output, absolute workspace roots, or V8 internals.

Current primitive status: `src/packages/permissions.rs::parse_permission` accepts only known permission strings and returns `ProhibitedAuthority` for blocked host capabilities; `validate_manifest_value` rejects prohibited or unknown permissions before enable/load. Missing generic pieces are persisted third-party grant records, trust-record matching, parent-side sandbox request enforcement, and grant-aware diagnostics. Non-`@clay/*` execution remains denied until those exist and an approved decision log grants exact authority.

Permission checks run at install, enable, load, reload, registration, request, or output-publication boundaries. They are never keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot-path work.

## Sandbox Enforcement and Parent Validation

The current `RuntimeSandboxSupervisor` and `clay-runtime-sandbox` newline-delimited JSON harness proves spawn/handshake/evaluate/timeout/restart/payload rejection/no filesystem-network-shell globals. It is not production API and does not grant third-party authority.

Production third-party execution requires a bounded typed protocol like the main IPC `Codec`: length-prefixed frames, maximum frame size, typed variants, decode validation, generation IDs, stable diagnostics, and frame-too-large/protocol-failure metrics.

Required flow:

```text
parent validates package metadata + permissions -> child evaluates -> parent validates inert outputs -> publish
```

Parent pre-validates every load/evaluate/parse request: trust record, registry integrity, package manifest, approved permissions, `loadEntry`/module path confinement, payload budget, timeout/heap budget, runtime generation, handler token, document version, and stale-generation rejection.

Parent post-validates every response: allowed response kind, generation match, package provenance, payload size, inert JSON shape, behavior manifest schema, SDUI schema, decoration/folding/completion/parse output validators, no executable callbacks, no raw op names, no client JavaScript, and no path-like authority payloads.

Timeout, heap-limit, malformed response, oversized output, protocol mismatch, unknown variant, stale generation, stale handler token, or invalid output kills the child and requires a fresh child/generation before more third-party work. Parent keeps last validated behavior/SDUI/decorations/parse state until replacement output validates.

The child receives no workspace roots, absolute source paths, file descriptors, package-manager handles, raw op names, V8 handles, Rust internals, capability tokens, client handles, native widget handles, environment variables, credentials, registry auth tokens, or client authority.

Production routing needs measured evidence first: startup plus handshake under 250 ms target, first package load overhead recorded against in-process runtime, small parse round trip under 10 ms added overhead target, timeout kill plus fresh handshake under 500 ms target, and no keypress/paint/layout/scroll/text-event/edit-ack dependency.

## Rollback, Disable, Update, and Incident Response

Disable is active withdrawal. Clay must mark the third-party package generation revoked, remove enabled state for that package identity, rebuild the next runtime generation without it, cancel parse/completion work for the revoked generation, unregister handler tokens, and withdraw commands, behavior manifests, SDUI/status trees, package UI/input/state/layout/theme declarations, decorations, folding, completion providers, and package diagnostics before publishing replacement state.

Updates are new identities. Any changed version, registry/source, tarball/path, integrity digest, `apiPrefix`, publisher, permission set, or Clay compatibility range requires a new trust+permission grant. Failed update provenance checks, enable validation, conflict checks, sandbox load/evaluation, output validation, or parse registration keep the prior validated generation active; Clay reports sanitized diagnostics and does not partially merge candidate contributions.

Rollback reuses Phase 19 generation semantics: construct the candidate generation off to the side, run `PackageService` validation and conflict checks, run sandbox load/evaluation, validate all inert outputs, then swap only on success. Failed third-party generation -> keep prior validated manifest/UI -> cancel generation parse -> require explicit reload/update.

Stale output rejection is mandatory. Parent rejects parse, completion, SDUI, decoration, folding, behavior, command, and diagnostic updates when runtime generation ID, document version, behavior version, handler token, package identity, or provenance no longer matches active state.

Incident response is fail-closed: revoke package identity, stop scheduling new work, kill/replace the sandbox child for that generation, cancel in-flight tasks, withdraw package-owned contributions, preserve unaffected packages and last validated client state, and require explicit reload/update/re-trust before the package executes again.

Package-manager side effects do not imply active runtime state. Removing package-store files or changing lockfiles cannot leave commands, parse handlers, behavior manifests, SDUI, decorations, completion providers, raw package-manager handles, or client state active after rollback/disable.

Current reusable primitives: `PackageService::enable` removes conflict candidates, Phase 19 reload keeps the previous runtime on failed evaluation, `RuntimeGenerationStore` makes swaps generation-based, `ParseCoordinator::cancel_generation` cancels old work, and parse result validation rejects stale generation/document output. Missing generic pieces are package-generation revocation state, contribution ownership indexes, package-scoped withdrawal, sandbox-child replacement wiring, update-as-new-identity enforcement, and sanitized incident diagnostics.

Rollback and disable run at disable, update, reload, startup, incident-response, or background cleanup time only. They never block keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

## Hot-Path Policy

All third-party policy work must run at startup, install, enable, load, reload, sandbox supervision, parse scheduling, or background validation time. It must never run in keypress, paint, layout, scroll, text-event, ordinary edit acknowledgement, or Masonry hot paths. Runtime and sandbox outputs must be bounded and parent-validated before publication.

## Current Decision

No approved decision log means no non-`@clay/*` runtime execution. This page supports Plan 035 policy work; it does not approve third-party execution.

## Tests

Run focused coverage with:

```text
cargo test --test package_loading_docs third_party_runtime_authority_policy_is_documented
cargo test op_clay_packages_load_package_by_specifier_rejects_non_first_party_specifier --lib
cargo test --test package_loading third_party_install_metadata_does_not_imply_runtime_execution_authority
cargo test --test runtime_sandbox_harness
```

## Related

- [Package Loading](package-loading.md)
- [Persistent Runtime Hardening](persistent-runtime-hardening.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Parse Coordinator](parse-coordinator.md)
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/package-loading.md`
- `docs/design/persistent-runtime-sandbox.md`
- `plans/035-Third-Party-Package-Runtime-Authority-Policy.md`
