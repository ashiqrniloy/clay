# Package Extension and Adoption Authority (Plan 061)

## Source

- `src/packages/bundled.rs` — `BUNDLED_PACKAGES` inventory (11 entries, FNV-1a-64 fingerprints), `verify_bundled_trust`, `RuntimeDomain` enum.
- `src/packages/manifest.rs` — `ExtensionPointDeclaration`, `StructuredRelationRequest`, `parse_extension_point`, `parse_mixed_relation_array`, contribution namespace validation.
- `src/packages/extension_points.rs` — `RelationOperation`, `ExtensionContributionKind` (16 variants), validation constants and charset rules.
- `src/packages/approvals.rs` — `PackageApprovalStore`, `PackageApprovalRecord`, `ApprovedRelation`, `ApprovedReplacement`, `approval_covers`, `AdoptionState`, atomic file persistence.
- `src/packages/service.rs` — `install_from_value_at_root_with_spec`, `approve_package`, `adoption_state`, `enable` with adoption gating and replacement approval revocation, `rollback_replacement`, `enable_graph` with `verify_relation_authority`, `enable` transactionality (snapshot/restore), `force_enabled_runtime_domain_for_test`.
- `src/packages/record.rs` — `PackageRecord` with `runtime_domain` field, `PartialEq` excluding `runtime_domain`.
- `src/packages/conflict.rs` — `reconcile_enabled_conflicts` post-enable, `PackageReplaces` conflict resolution.
- `src/server/cross_domain.rs` — `CrossDomainRequestEnvelope`, cross-domain invocation validation, `dispatch_to_domain` with provider routing.
- `src/server/ops/packages.rs` — `op_clay_packages_load_package_by_specifier` (sync trusted-only, stamps domain in result), `op_clay_packages_load_in_package_domain` (async, bridge dispatch + absorption).
- `src/server/js_runtime.rs` — Two-domain runtime topology, `production_reload`, cross-domain bridge wiring, `replay_third_party_domain`, `dispatch_to_domain` with replacement.
- `src/server/mod.rs` — `TrustedOpState`, connected-loop references wired to `PackageService` and bridge.
- `src/main.rs` — CLI `clay package adopt/revoke/rollback/inspect`, `PackageService::open` for durable store.
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` (superseded authority model retained for provenance)
- `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`

## Architecture

Package authority in Clay is built on four layers:

1. **Identity**: immutable bundled inventory (`BUNDLED_PACKAGES`) for Clay-shipped packages, everything else is third-party.
2. **Provenance**: package name + version + canonical root + manifest fingerprint matched against the bundled inventory, or an installed provenance record for third-party packages.
3. **Adoption**: durable user-approved `PackageApprovalRecord` stored at `~/.config/clay/packages` with exact identity/version/integrity/capabilities/processes/relations/replacements. No code executes before adoption.
4. **Extension**: versioned extension points (`clay-extension-point-v1`) declared by package owners, combined with structured relation requests (`clay-package-relation-v1`) from consuming packages. Both owner consent (extension point declarations) and user consent (durable approval) are required before enable.

## Bundled Trust Inventory

`src/packages/bundled.rs` defines a compile-time `BUNDLED_PACKAGES` array of 11 first-party packages. Each entry has:

- Exact name (e.g., `@clay/markdown`), version, canonical root relative to `CARGO_MANIFEST_DIR/packages/<slug>`.
- An FNV-1a-64 fingerprint over the canonical root directory for drift detection.
- An `inventory_matches_source_tree` unit test that fails if source changes without fingerprint regeneration.

`verify_bundled_trust` is the single choke point: it validates `source_kind == ClayShipped`, exact name/version/canonical root match, and fingerprint equality before granting `Trusted` domain. The `PackageSourceKind::from_spec` `@clay/*` prefix classification remains a "claimed family" heuristic; trust decisions are always deferred to `verify_bundled_trust`.

`RuntimeDomain::Trusted` is stamped on a `PackageRecord` only after `verify_bundled_trust` passes in `enable_graph`. All other packages default to `ThirdParty` and cannot be promoted. Clay core (`@clay/core`) is not replaceable.

## Extension Points

Extension points are versioned schema declarations in `package.json` under `clay.extensionPoints`. Each point declares:

- `id`: `{apiPrefix}.{camelCaseName}` with ascii_alphanumeric charset.
- `version`: integer ≥ 1.
- `operations`: array of `RelationOperation` (`dependOn`, `extend`, `disable`, `replace`).
- `contributionKinds`: closed enum of `ExtensionContributionKind` (16 variants: `mode`, `command`, `keyRouting`, `textTransform`, `syntaxGrammar`, `parseHandler`, `completionProvider`, `languageIntelligenceProvider`, `documentAnalyzer`, `languageServer`, `sdui`, `decoration`, `diagnostic`, `ui`, `theme`, `statusItem`).
- `scopes`: optional array of contribution IDs the point controls (defaults to all owner contributions).
- `summary`: human-readable string.

Schema constants enforce: max 64 extension points per manifest, max 32 scopes per entry, max 128 chars per scope, max point ID length 64, max summary length 200, payload budget `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` (8192 bytes).

A `bundle_extension_points_match_real_contributions` test validates every bundled package declares extension points, and every scope references a real contribution ID or known runtime-registered ID.

## Structured Relations and Graph Resolution

Package manifests declare `clay.graph.relations` with mixed string/object arrays. String entries (`"@clay/markdown"`) are bare target references. Object entries are `StructuredRelationRequest` with:

- `package`: target package name.
- `extensionPoint`: target owner-declared extension point ID.
- `operation`: specific `RelationOperation` from the extension point's allowed set.
- `scopes`: optional subset of the extension point's declared scopes.

`parse_mixed_relation_array` handles both forms and deduplicates targets. The existing `enable_graph` cycle detection, topological ordering, and conflict resolution apply uniformly.

`verify_relation_authority` runs during `enable_graph` and enforces:

1. **Owner consent**: every `StructuredRelationRequest` must match a declared extension point on the TARGET package with matching operation.
2. **User consent**: every ThirdParty enable requires a `PackageApprovalRecord` with `approval_covers` returning `Approved`. Approvals cover both relation targets and replacement targets.

## Durable Approval Store

`PackageApprovalStore` (at `~/.config/clay/packages`) persists one JSON document per approved package. Each `PackageApprovalRecord` contains:

- Exact `package_name`, `package_version`, `api_prefix`, `integrity`.
- `approved_permissions`: snapshot of approved capability strings at adoption time.
- `processes`: language-server contribution IDs requiring external processes.
- `relations`: array of `ApprovedRelation` (target + extension_point + operation).
- `replacements`: array of `ApprovedReplacement` (replaced target + withdrawn contribution IDs + `rollback_restore_target` flag).
- `approved_at`: RFC3339 timestamp (manual conversion, no chrono dependency).
- `approved_by`: human label ("cli" for CLI adopt, or user name for UI adopt).

Serialization is manual `serde_json::Value` conversion (Clay has no `serde` dependency). Atomic writes use temp-file + fsync + rename with `0o600` owner-only permissions. Corruption at open time fails closed (in-memory empty store).

`approval_covers` validates exact identity match (name, version, api_prefix, integrity), permissions subset, and relations/replacements subset. Version drift, scope expansion, and target replacement invalidate the approval (returning `Stale`). Permission narrowing requires re-adoption.

## Adoption Lifecycle

```text
Installed → Pending → (user/cli approve) → Approved → (loadPackage) → Enabled → (revoke/disable) → Revoked
                ↑                                              ↓
                └── stale (version drift, scope expansion, target replacement) ←┘
```

- **Install**: `PackageService::install_from_value_at_root_with_spec` records the package root and manifest. No code executes.
- **Authorize**: `authorize_package` sets the `RuntimeProfile` (currently `Restricted` for all third-party packages) and approved capabilities.
- **Adopt**: `approve_package` builds and persists a `PackageApprovalRecord` from host-side facts (provenance, assembled manifest, permissions, LS contribution IDs, graph relations, replacement targets).
- **Enable**: `loadPackage` / `enable` checks `adoption_state`. If `Approved`, enables the package with capability verification, graph resolution, and conflict reconciliation. Rejected otherwise.
- **Stale**: `adoption_state` returns `Stale` when the installed version, api_prefix, or integrity no longer matches the approval record, or when scope/replacement expansion is detected.
- **Revoke**: `revoke_package_approval` removes the approval and (if enabled) disables the package.

### Enable Transactionality

`enable` snapshots enabled records, conflict resolutions, revocation records, and approval store before mutation. On `enable_graph` failure, all snapshots are restored (full rollback). On success for replacements: the replaced target's durable approval is revoked, and if that revoke or approval restore fails, the snapshot rollback fires.

### Replacement Atomicity

When a third-party package with `replaces` relation is enabled:

1. `enable_graph` resolves conflicts with `PackageReplaces` resolution.
2. Post-enable, conflict resolution delta is scanned for `PackageReplaces` with winner matching the enabling package.
3. Each replaced target's durable approval is revoked (Trusted targets are no-ops).
4. Replaced targets are disabled as a transactional side effect.
5. `rollback_replacement(target)` disables the replacement, re-adopts the target, and re-enables it.

## Cross-Domain Typed Invocation

`src/server/cross_domain.rs` validates `clay-cross-domain-envelope-v1` requests:

- Requester must be enabled ThirdParty (Trusted blocked at ingress).
- Target must be enabled with declared `extension_point/version/operation`.
- `approval_ref` must bind to a matching durable approval.
- Durable approval must cover the relation (`approval_covers`).

Denial reasons: stale requester, revoked approval, wrong target/point/operation, oversize payload, forged approval_ref. Constants: max 16 pending cross-domain requests, 250ms deadline.

## First-Party Package Replacement

All 11 bundled packages declare extension points covering their contribution surfaces. A third-party package can:

- **Disable** a first-party package: requires a `clay.graph.disables` declaration matching a declared owner extension point, plus user approval.
- **Replace** a first-party package: requires both `clay.graph.replaces` + owner extension point consent + user approval. The replacement stays ThirdParty (no promotion). Replaced target is atomically disabled. Contribution IDs must stay within the replacement's own `apiPrefix` namespace (cross-prefix claiming is structurally unrepresentable in `assemble_package_record`).

```bash
clay package inspect @vendor/markdown-repl   # shows pending adoption state
clay package adopt @vendor/markdown-repl     # shows capabilities, processes, relations, replacements; user confirms
# in init.js: await loadPackage("@vendor/markdown-repl");  # one-line replacement
clay package rollback @clay/markdown          # disables replacement, re-adopts target, re-enables
clay package revoke @vendor/markdown-repl    # removes approval, disables if enabled
```

## Host-Stamped Package Provenance (P0-1)

All 66 ops now receive host-stamped `PackageContext` (package_name, package_version, api_prefix) rather than caller-assembled manifest objects. `set_current_package` is called by `loadPackage` / `load_in_package_domain` after successful enable. `begin_evaluation` clears context before each command batch. Dead self-assertion functions (`package_from_options`, `package_value_from_options`, `parse_manifest`) are deleted.

Language-server session spoofing is closed: `start_session` resolves identity host-side from `current_package_record`; `stop_session` verifies identity (package, contribution, descriptor_fingerprint) against session records; data-path ops are gated by session ownership.

## Invariants

- Trusted domain = compiled bundled inventory only. No runtime promotion into Trusted.
- Third-party shared runtime = one trust cohort. Packages within the third-party runtime can mutate each other; this is disclosed at adoption.
- All third-party enables require durable approval (no blanket approval for packages without relations).
- Replacement requires: (a) owner extension point declaration, (b) user durable approval, (c) replacement stays ThirdParty. Clay core is not replaceable.
- Extension point payload budget 8192 bytes. Cross-domain envelope payload 8192 bytes. Both are compiled constants.
- LS grants are non-transferable: each package gets its own, keyed to the package name in the grant map.
- Contribution IDs must use the owning package's `apiPrefix` namespace (validated at manifest assembly).
- `enable` is atomic; failure leaves no partial state.
- No `serde` dependency in Clay; all JSON serialization is manual `serde_json::Value` conversion.

## Tests

Run focused coverage with:

```text
cargo test --test security package_graph::        # extension point validation, structured relations, adoption lifecycle, replacement + rollback
cargo test --test security package_loading::      # replacement withdraws trusted target atomically, LS lifecycles, adoption gating
cargo test --test security package_conflicts::    # replacement edge approval, stale-on-commit
cargo test --test protocol primitives_docs::      # op/extension/subset inventory tests, wiki doc completeness
cargo test --lib package_approval      # PackageApprovalStore round-trip, corruption, version drift
cargo test --lib bundled_trust         # inventory matches source, extension points match real contributions
cargo test --lib cross_domain          # cross-domain envelope validation, requester/target checks
cargo test --lib third_party_config    # plan 061 task 15 config verification (adoption, stale, load)
cargo test --lib runtime_resource      # two-runtime RSS/thread/candidate reload baselines
cargo test --lib rust_visibility       # facade allowlist parity, internal type audit
```

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md) — Two Runtime Trust Domains section
- [Package Loading](package-loading.md)
- [Parse Coordinator](parse-coordinator.md)
- [Language Server Process Service](language-server-process-service.md)
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/package-loading.md`
- `docs/reference/packages/creating-packages.md`
- `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`
