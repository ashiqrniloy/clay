---
date: 2026-06-16 15:26
status: approved
decision_about: "Expand the controlled server-side runtime's module-loading authority to load resolver-validated first-party @clay/* package loadEntry modules from outside the configuration root"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Expand module-loading authority for first-party `@clay/*` package `loadEntry` modules

## Decision

The controlled server-side runtime's module-loading authority is expanded to load first-party `@clay/*` package `loadEntry` modules from outside the configuration root, gated by:

1. A constrained `@clay/*` specifier allowlist enforced at the op boundary (no bare specifiers, no forward slashes, no backslashes, no `..` path traversal in the package name segment).
2. A resolver-validated opaque `clay://packages/...` URL scheme that the `ClayModuleLoader` accepts only when the exact specifier is present in the `FirstPartyLoadEntryAllowlist`.
3. A `PackageService` resolve/enable/execute path that validates the on-disk `packages/<name>/package.json` manifest through the existing `assemble_package_record` and `check_enabled_packages` gates before any load entry is recorded.
4. A transitive relative-import resolver (`resolve_relative`) that canonicalizes sibling paths against the validated `package_root` and rejects any candidate that escapes the package root via `starts_with` confinement.

This expansion supersedes `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`, which deferred the resolver to a dedicated phase. Phase 18.6 (`plans/029-Phase18.6-Generic-Package-Loader-and-First-Party-Module-Bridge.md`) implements the resolver, and this log records the finalized authority expansion.

## Context

- The original deferral log (`2026-06-15-1015`) recorded that making `loadPackage("@clay/markdown")` work end-to-end required a security-relevant authority expansion: the controlled runtime must be allowed to load first-party package JavaScript from outside the configuration root, gated by a validated `@clay/*` resolver. That boundary is exactly what `clay_facade_source` and the config-root canonicalization exist to protect.
- The deferral log's "Consequences" section specified that a future phase must implement: (a) a constrained first-party `@clay/*` specifier resolver op, (b) a `PackageService` resolve/enable/execute path, (c) a `ClayModuleLoader` extension, (d) a `loadPackage` facade export, (e) full docs/registry/tests coverage, and (f) its own decision log recording the authority expansion.
- Phase 18.6 (`plans/029`) implemented all of the above. This log records the finalized authority expansion and the security boundary tests that enforce deny-by-default.

## Approval

- Proposed by: both (agent during `plans/029` Task 3 design; user approved execution of Task 3 which includes the authority expansion).
- Approved by user: Yes.
- Approval evidence: The user directed execution of `plans/029-Phase18.6-Generic-Package-Loader-and-First-Party-Module-Bridge.md` Task 3 ("Add the generic one-line package resolver op and OpState wiring"), which explicitly implements the authority expansion described in this log. The user further directed execution of Tasks 4-10, all of which depend on and verify the authority expansion. The user's directive to complete Task 9 ("Create the decision log recording the first-party module-loading authority expansion") is explicit approval to finalize this log.

## Alternatives Considered

1. **Keep the deferral and do not expand module-loading authority.** — Rejected. The one-line `loadPackage("@clay/markdown")` target is the preferred end-user convention established by `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`. Without the expansion, the end-user setup remains the package-owned `markdownLoadMode()` fallback, which is not the preferred design.
2. **Expand authority to arbitrary external packages (npm registry, third-party specifiers).** — Rejected. The scope is constrained to first-party `@clay/*` packages shipped under `packages/` in the Clay source tree. External package resolution, package-manager execution, registry fetching, and arbitrary specifier expansion remain prohibited. Non-`@clay/*` packages are deferred to Phase 23.
3. **Ship a Markdown-specific `MarkdownLoader` / `if package == "@clay/markdown"` Rust branch.** — Rejected by the primitive-first decision (`decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`) and by the Phase 18.5 primitive review. Package loading must stay generic.
4. **Inline the `loadEntry` source as a string returned by the op and `eval` it.** — Rejected during Task 5 design. The `loadEntry` imports sibling modules (e.g. `./index.js`) and Clay facades (`clay:modes`, `clay:commands`, `clay:parse`), so it must participate in ES-module graph resolution. The opaque URL scheme + allowlist gate preserves deny-by-default while enabling transitive relative imports.
5. **Loosen `canonical_local_file` to allow the `packages/` tree generally.** — Rejected during Task 4 design. It would open arbitrary imports under `packages/` not just the single validated `loadEntry`, breaking deny-by-default. The dedicated opaque URL scheme confines loading to resolver-validated entries.

## Rationale and Evidence

### Authority expansion

The expansion grants the controlled server-side runtime the ability to load first-party `@clay/*` package `loadEntry` modules from outside the configuration root. This is a new authority beyond the existing config-root confinement enforced by `src/server/configuration.rs::canonical_local_file`.

### Constrained `@clay/*` allowlist

The resolver op (`op_clay_packages_load_package_by_specifier` at `src/server/ops/packages.rs:256`) enforces:
- Specifier must start with `@clay/`.
- Package name must be non-empty, contain no `/`, `\`, or `..` path traversal.
- The op reads `packages/<name>/package.json` from disk using `env!("CARGO_MANIFEST_DIR")` and canonicalizes both the `loadEntry` path and the package root before recording them in the allowlist.

### Opaque URL scheme + allowlist gate

The `FirstPartyLoadEntryAllowlist` struct (`src/server/ops/packages.rs:36`) maps opaque `clay://packages/@clay/<name>/<tail>` specifiers to absolute filesystem paths. `ClayModuleLoader` (`src/server/js_runtime.rs:613`) accepts a specifier from this allowlist only when the exact opaque URL is present. Transitive relative imports (e.g. `./index.js`) are resolved by `resolve_relative` (`src/server/ops/packages.rs`), which canonicalizes the candidate path, checks `starts_with(package_root)` for confinement, and records a new opaque specifier.

### PackageService validation reuse

The resolver op reuses the existing `PackageService` validation path (`src/packages/service.rs`):
- `install_from_value` records the package from the on-disk `package.json`.
- `enable` validates the manifest via `assemble_package_record` and checks for contribution conflicts via `check_enabled_packages`.
- Invalid metadata or conflicting packages are rejected with `PackageServiceError::InvalidClayMetadata` or `PackageServiceError::ContributionConflict`, and the enable is rolled back.

### Security boundary tests

The following tests enforce deny-by-default:
- `tests/package_loading_docs.rs::resolver_validation_rejects_invalid_first_party_metadata`: asserts that invalid manifest metadata (missing required fields) is rejected by `PackageService` validation.
- `tests/package_loading_docs.rs::resolver_validation_rejects_conflicting_packages`: asserts that conflicting packages (duplicate `apiPrefix`) are rejected with rollback.
- `tests/package_loading_docs.rs::load_package_facade_exposes_no_enable_disable_or_package_manager_authority`: asserts that the `loadPackage` facade does not expose `serverEnablePackage`, `serverDisablePackage`, or package-manager authority.
- `src/server/js_runtime.rs::clay_module_loader_denies_load_entry_imports_outside_package_root`: asserts that `resolve_relative` rejects transitive imports that escape the package root.
- Existing Task 3/4 tests assert deny-by-default for non-`@clay/*` specifiers, unknown packages, unallowlisted URLs, arbitrary `file://`/`https://`/bare specifiers, and config-root confinement for non-package imports.

### Authority NOT granted

This expansion does **not** grant:
- Filesystem authority outside the validated `packages/` tree and the existing config root.
- Network authority (no registry fetching, no HTTP imports).
- Shell authority (no package-manager execution, no `spawn`).
- Extension-loading authority (no arbitrary external packages; non-`@clay/*` packages deferred to Phase 23).
- AI mutation authority.
- Workspace mutation authority.
- WASM authority.
- Raw-op authority (the op is not exposed as a user-facing `Deno.core.ops.op_*` name; it is wrapped by the `clay:packages` facade).
- Native-widget authority.
- Client-side JavaScript authority (the `loadEntry` runs in the controlled server-side runtime, not in the editor client).
- Package enable/disable authority (the `PackageService::enable` path is internal; the `loadPackage` facade does not expose enable/disable to JS).
- Package-manager execution authority.
- Hot-reload authority (package changes require restart; deferred to Phase 19).
- Persistent shared enable-state authority (enabled packages are not persisted across restarts; deferred to Phase 19).

## References

- `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` — the deferral this log supersedes.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md` — the one-line package loading convention.
- `plans/029-Phase18.6-Generic-Package-Loader-and-First-Party-Module-Bridge.md` — the Phase 18.6 plan that implemented the resolver.
- `src/server/ops/packages.rs` — `FirstPartyLoadEntryAllowlist` (line 36), `op_clay_packages_load_package_by_specifier` (line 256).
- `src/server/ops/mod.rs` — `ClayOpState` with `first_party_packages: Mutex<PackageService>` and `load_entry_allowlist: Arc<FirstPartyLoadEntryAllowlist>` (line 69).
- `src/server/js_runtime.rs` — `ClayModuleLoader` with `first_party_load_entry_allowlist: Arc<FirstPartyLoadEntryAllowlist>` field (line 613), allowlist gates in `resolve` (line 668) and `load` (line 683).
- `runtime/js/packages.ts` — `loadPackage` facade export (line 42).
- `tests/package_loading_docs.rs` — security boundary tests (invalid metadata, conflicting packages, no authority exposure).
- `docs/reference/primitives/package-loading.md` — package-loading primitive contract and the implemented resolver.
- `docs/reference/packages/creating-packages.md` — package authoring guide with `loadPackage` as the implemented one-line default.
- `docs/wiki/modules/package-loading.md` — package loading implementation wiki.

## Consequences

### Positive outcomes

- The one-line `loadPackage("@clay/markdown")` target is now a callable API, not just a documented target.
- The end-user `~/.config/clay/init.js` setup is concise: `import { loadPackage } from "clay:packages"; await loadPackage("@clay/markdown");`.
- The package-owned `markdownLoadMode()` fallback remains available for tests and per-load options.
- The security boundary is enforceable and tested: deny-by-default for arbitrary specifiers, confined transitive imports, no authority expansion beyond first-party `@clay/*` packages.
- The resolver reuses existing `PackageService` validation, so invalid or conflicting packages are rejected with the same error paths as the fixture-based `serverLoadPackage` path.

### Risks and follow-up work

- **Non-`@clay/*` packages** are deferred to Phase 23. If a future requirement demands external package resolution, the authority expansion must be revisited with a new decision log.
- **Hot reload** is deferred to Phase 19. Package changes require restart today. If hot reload is implemented, the resolver must be extended to support dynamic invalidation of the allowlist.
- **Persistent shared enable state** is deferred to Phase 19. Enabled packages are not persisted across restarts today. If persistence is implemented, the resolver must coordinate with the persistence layer.
- **Transitive imports beyond siblings** (e.g. nested subdirectories) are supported by `resolve_relative` as long as the candidate path stays within the `package_root`. If a package requires imports from outside its root (e.g. shared utilities), the authority expansion must be revisited.

### Conditions for revisiting this decision

- A requirement to load non-`@clay/*` packages (external registry, third-party specifiers).
- A requirement to grant package enable/disable authority to JS.
- A requirement to load `loadEntry` modules from outside the `packages/` tree (e.g. user-installed packages).
- A security incident or audit finding that the deny-by-default boundary is insufficient.
