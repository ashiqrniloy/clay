# Package Primitive Gate

## Scope

Covers the Phase 16.5 package manifest and permission validation primitives in:

- `src/packages/mod.rs`
- `src/packages/manifest.rs`
- `src/packages/permissions.rs`
- `tests/package_primitive_gate.rs`

## Responsibilities

The package primitive gate validates Clay-owned package metadata before any future package enable/load workflow can activate package primitives. It accepts `package.json`-style `serde_json::Value` fixtures with top-level package identity and a `clay` metadata object, then returns typed Rust structs or a structured load-time diagnostic.

Validated fields include:

- `name` and semver-like `version`
- `clay.apiPrefix`
- `clay.permissions`
- `clay.modes`
- `clay.entry`
- optional `clay.loadEntry`

## How It Works

`validate_manifest_value` performs load-time-only checks and produces `ClayPackageManifest` when all rules pass. `validate_manifest_values` composes the single-manifest validator and adds enabled-package prefix uniqueness checks.

Validation keeps provenance in every `PackageDiagnostic`:

- `package_name`
- `package_version`
- `api_prefix`
- deterministic `PackageValidationRule`
- actionable `message`

`permissions.rs` owns the known permission table and rejects both unknown permissions and prohibited default authorities such as `network`, `shell`, `raw-deno-ops`, and `client-javascript`.

## Security and Invariants

- The validator does not shell out to npm/pnpm or execute package code.
- Package validation is not called from keypress, paint, layout, or text-event paths.
- `apiPrefix` must match the Phase 16 package security rule: `^[a-z][a-z0-9-]{1,31}$`.
- Package-owned mode IDs cannot claim reserved `clay.*` IDs and must use the owning prefix or `prefix.*` namespace.
- Metadata containing `Deno.core.ops` or client-side JavaScript hook fields is rejected before load.
- Static payload validation uses `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` as the Phase 16.5 package primitive fixture budget.

## Testing

Run focused coverage with:

```text
cargo test --test package_primitive_gate
```

The tests cover the first-party Markdown fixture, invalid/reserved prefixes, unknown/prohibited permissions, duplicate package prefixes, raw op metadata, and client hook metadata.

## Related Documentation

- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/backlog.md`
- `plans/018-Phase16.5-Primitive-Implementation-Gate-for-Package-and-Mode-Loading.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/behavior-manifests.md`
