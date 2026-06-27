# Phase 17 Package Loading Runtime Facades

Phase 17 package loading connects package metadata, primitive contribution validation, and controlled server-side JavaScript facades without making the Rust client execute package JavaScript or client-side JavaScript, and without granting native widget authority, renderer callbacks, or raw ops authority.

## Scope

- `clay:packages` validates and loads package records from `package.json` Clay metadata at install/enable/reload time.
- `clay:modes` registers document classification metadata, activates one major mode per document, and keeps per-document manifest selection server-owned. The richer `serverSelectDocumentManifest` facade remains an explicit planned route until a later runtime op promotes the implemented Rust selector.
- `clay:commands` registers and lists package-owned inert command metadata.
- `clay:decorations` and `clay:parse` exist as Phase-18 handoff facades. Their public calls currently return planned-unavailable errors while the Rust validators and coordinator remain typed server infrastructure.

## Install, Enable, and Runtime Boundary

Package installation is delegated to an npm-compatible package manager by the package service. Installing a package records package files and metadata; it does not execute `entry`, `loadEntry`, command handlers, parse handlers, or decoration code.

Enabling or loading a package is Clay-owned. The server validates identity, `apiPrefix`, permissions, modes, entries, docs, performance metadata, API dependencies, and inert primitive contributions through typed Rust validators before contributions become active. Phase 18.3 package metadata may also declare slot-aware UI contribution descriptors (`ui.panels`, `ui.components`, `ui.overlays`) and typed `themeTokens`; validation checks package-prefixed IDs, fixed slot claims, component catalog kinds, typed style variables, action targets declared in package commands, same-type core token fallbacks, prohibited authority fields (including raw CSS, client JavaScript, direct Masonry widgets, and native handles), and bounded payload estimates against SDUI payload budgets. Phase 18.4 metadata may declare `input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions`; validation checks package-prefixed input/state/option/target IDs, supported pointer/focus/selection/state lifecycle/configuration schemas, registered actions, rejects unregistered actions, validates registered inputs/theme tokens for defaults/remaps, requires `package-configuration` permission for behavior-changing defaults, hidden-key rejection, state-value rejection, prohibited authority fields, and bounded payload estimates. Runtime facade calls route through those validators and never expose raw `Deno.core.ops.op_*` names as user-facing APIs.

Package JavaScript runs only in the controlled server-side runtime for load/configuration/activation work; no package JavaScript runs in Masonry paint/layout/input hot paths. The Rust client receives inert manifests, decorations, parse updates, SDUI data, and validated package UI state; it never receives package JavaScript callbacks.

## Registry and Integrity Verification Policy

Clay delegates registry access, package fetching, dependency resolution, version ranges, lockfile writing, integrity verification, caching, and offline store behavior to the npm-compatible package manager. Clay does not implement a registry client.

Third-party runtime authority needs a Clay-owned provenance record captured from package-manager state before any non-`@clay/*` package can execute:

```toml
[package_source]
name = "@vendor/example"
requested_spec = "@vendor/example@1.2.3"
resolved_version = "1.2.3"
registry = "https://registry.npmjs.org/"
integrity = "sha512-..."
lockfile = "pnpm-lock.yaml"
package_root = "/clay/packages/node_modules/@vendor/example"
tarball = "https://registry.npmjs.org/@vendor/example/-/example-1.2.3.tgz"
offline_cache_key = "@vendor/example/1.2.3"
```

Required policy:

1. `pnpm add --ignore-scripts <pkg>@<version>` remains the default install shape; lifecycle scripts stay disabled unless an explicit dangerous opt-in is used.
2. Install/update records requested spec, resolved version, registry or source URL, lockfile path, lockfile integrity digest, package tarball or source path, package root, and offline/cache key when available.
3. Enable/load compares the recorded source and integrity evidence to the trusted package identity record before runtime execution.
4. Package-manager stdout, stderr, exit code, `package.json`, lockfile text, and registry metadata are diagnostic/provenance inputs only; they are not runtime authority and must not bypass Clay-owned manifest, permission, trust, or sandbox validation.
5. Diagnostics copied from package-manager output must be sanitized: no environment secrets, auth tokens, filesystem roots beyond the Clay package store, shell command expansion, raw registry credentials, or unbounded stderr/stdout blobs.
6. Offline/cache installs are allowed only when cached metadata still matches the trusted resolved version and integrity digest; cache hits do not widen runtime authority.
7. Updates are treated as new identities: a changed resolved version, registry, tarball/source path, or integrity digest requires a new matching trust record before execution.
8. Verification runs only at install, update, enable, load, reload, startup, or background audit time. It never runs from keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

Current implementation gap: `PackageManagerBackend`, `PnpmBackend`, `InstallResult`, `DiscoveredPackage`, and `PackageService` already isolate package-manager execution, capture stdout/stderr/exit status, read inert package metadata, and keep install separate from enable/load. They do not yet persist a source/integrity provenance record, parse lockfile integrity evidence, sanitize package-manager diagnostics, model offline/cache keys, or enforce update-as-new-identity checks for third-party packages. Until those generic fields and checks exist, non-`@clay/*` runtime execution remains denied.

## Conflict Handling

Enabled packages are checked deterministically at enable/reload time. Clay rejects duplicate prefixes, mode IDs, command IDs, ambiguous key bindings, configuration key collisions, SDUI region collisions, decoration primitive collisions, duplicate package UI panel/component/overlay/theme-token IDs, duplicate fixed slot claims, duplicate input contribution IDs, duplicate UI state scope IDs, duplicate layout override target/property pairs, duplicate package option schemas, and behavior manifest entry collisions with package provenance. Conflicts do not silently override existing behavior.

## Third-Party Disable, Update, Rollback, and Incident Policy

Third-party disable is an active withdrawal, not only a future-load block. Disable marks the package generation revoked, removes PackageService-enabled state for that package identity, rebuilds the next runtime generation without the package, cancels parse work for the revoked generation, unregisters handler tokens, withdraws commands, behavior manifests, SDUI/status trees, package UI/input/state/layout/theme declarations, decorations, folding, completion providers, and diagnostics owned by that package, then publishes only parent-validated replacement state.

Updates are new package identities. A changed version, registry/source, tarball/path, integrity digest, `apiPrefix`, publisher, permission set, or Clay compatibility range requires a new trust+permission grant before execution. If the new generation fails install provenance checks, enable validation, conflict checks, sandbox load/evaluation, output validation, or parse registration, Clay keeps the prior validated generation active and reports sanitized diagnostics; it does not partially merge new contributions.

Rollback uses the Phase 19 runtime-generation model: build and validate the candidate generation off to the side, swap only after success, and keep the last validated client state when candidate load/evaluation fails. On failure, stale generation outputs are rejected by runtime generation ID, document version, behavior version, handler token, and package provenance before any client publication.

Incident response for malicious, broken, or withdrawn packages must be fail-closed: revoke the package identity, stop scheduling new package work, kill or replace the sandbox child for that generation, cancel in-flight parse/completion tasks, remove active contributions for the revoked package, preserve unaffected packages and prior validated client state, and require explicit reload/update/re-trust before execution resumes.

Package-manager side effects are not runtime rollback authority. Clay may remove package-store files through the package-manager boundary, but install/remove stdout, stderr, exit code, lockfile changes, and package-store metadata do not keep commands, handlers, behavior manifests, SDUI, decorations, or client state active after disable/rollback.

Rollback/disable work runs at disable, update, reload, startup, incident-response, or background cleanup time. It never blocks keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

Current implementation gap: `PackageService::enable` already rolls back failed conflict candidates, Phase 19 runtime reload keeps the prior generation on failed evaluation, and `ParseCoordinator::cancel_generation` plus stale-result checks reject old-generation parse output. Third-party incident response still needs generic package-generation revocation, contribution ownership indexes, package-scoped withdrawal, sandbox-child replacement wiring, update-as-new-identity enforcement, and sanitized incident diagnostics.

## Hot-Path Policy

Package validation, package loading, mode activation, per-document manifest selection, decoration validation/publication, and parse-handler registration are outside typing, paint, layout, scroll, and text-event handlers. Ordinary keypress routing uses already-installed behavior manifests, and background parsing/decorations are versioned, bounded, and cancellable where implemented.

## Phase 18 Handoff

`DecorationRange` uses bounded `DecorationSet`/`DecorationSpan` protocol data validated by `src/server/decorations.rs` against `DECORATION_PAYLOAD_BUDGET_BYTES`, package provenance, permission, viewport range, and document version.

`IncrementalParseUpdate` uses `src/server/parse_coordinator.rs` to register permission-checked server-side parse handlers, schedule cancellable background parse tasks, reject stale versions, and enforce `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.

`serverLoadPackage` is runtime-backed for package record validation. It now validates Phase 18.4 slot-aware UI, input, UI state-scope, layout-override, package-option, API dependency, and theme-token metadata. The summary includes `input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions` contribution counts so fixture tests can verify the same load-time contract as enable/reload. It remains a validation helper rather than end-user package installation, enablement, default `loadEntry` execution, or package-manager authority. The preferred end-user setup remains an explicit one-line `loadPackage("@clay/markdown")` target from `~/.config/clay/init.js`.

Phase 18.6 shipped the generic one-line loader: `loadPackage` is a runtime-backed `clay:packages` facade export backed by a constrained first-party resolver that accepts `@clay/*` specifiers, validates package metadata through `PackageService`, enables the package, and imports and executes its declared `loadEntry` so that the package's mode, commands, parse handler, and keymaps are registered under Clay's authority. Phase 18.7 verifies the default `~/.config/clay/init.js` experience end-to-end: `await loadPackage("@clay/markdown")` runs once on the persistent server runtime, selected-file open reuses the registered mode/parse handler state, and generic open-time activation classifies the path, activates the document's major mode, and schedules `ParseCoordinator` without user config needing to copy package manifests, perform manual primitive registration, publish representative decoration publication payloads, or create per-open runtime roots. Repeated `loadPackage` calls are idempotent for one persistent runtime generation; Phase 19 hot reload replaces the `ClayJsRuntimeService`, reruns the configured/default `init.js`, rebuilds the first-party `loadEntry` allowlist, and starts `globalThis.__clayLoadedPackages` empty in the new generation. The `clay.packages.loadPackage` inventory entry is `status = "runtime-backed"` and `registry_public = true` with full Markdown documentation. The resolver op is `op_clay_packages_load_package_by_specifier` (`src/server/ops/packages.rs`). The module-loader extension (`src/server/js_runtime.rs::ClayModuleLoader`) is deny-by-default for all specifiers and only accepts a resolver-validated first-party `loadEntry` recorded in a shared `FirstPartyLoadEntryAllowlist`. Manifest `entry` and `loadEntry` values must be explicit relative `./... .js` module paths without traversal, absolute paths, URLs, backslashes, empty path segments, or raw op strings. The resolver canonicalizes both the package root and `loadEntry`, fails closed on canonicalization errors, and records the allowlist entry only when the canonical `loadEntry` remains under the canonical package root. The `loadEntry` is then confined to the validated package root for its own imports; escaping imports are rejected. The resolver reuses the Clay-owned `PackageService::enable` validation path (`assemble_package_record` + `check_enabled_packages`) so invalid metadata and conflicting packages are rejected before activation.

`clay package add <spec>` delegates installation to the configured npm-compatible backend (`PnpmBackend`). Lifecycle scripts are suppressed by default via `--ignore-scripts` so remote package code cannot execute before Clay validates package metadata. The `--allow-scripts` CLI flag (or `CLAY_ALLOW_LIFECYCLE_SCRIPTS=1` environment variable) opts into lifecycle scripts and is documented as dangerous. The package store directory is created before invoking the backend. `FakeBackend` is used in tests and never spawns a process or executes scripts.

The resolver is constrained to first-party `@clay/*` packages only. Non-`@clay/*` registry resolution (e.g. `left-pad`, `@scope/pkg`, URL, local path, traversal, npm, custom registries, and third-party packages) remains deferred to a future ecosystem hardening phase (Phase 23). Installed package-manager metadata does not imply runtime execution authority: `pnpm add`/package-store records can be inspected, but `loadPackage` rejects non-`@clay/*` specifiers before module loading until an approved third-party authority decision exists. Persistent shared enable state across runtime restarts remains deferred. See `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` for the full authority boundary, rationale, and security review. `serverLoadPackage` remains a lower-level validation helper used by fixtures and internally by `loadPackage`; it is not the documented end-user default.

The supported customization path after the one-line load is unchanged. Optional package customization is expressed through documented Clay JS APIs such as `clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride`; hidden JSON/TOML/ad hoc layout, input, style, or theme keys remain rejected, and these APIs do not provide package enable/disable authority. These APIs evaluate at startup, package-load, configuration-change, or explicit setting-change time and install inert validated state for Masonry hot paths to read later.

```javascript
// Implemented end-user default from ~/.config/clay/init.js:
import { loadPackage } from "clay:packages";
import { setPackageOption } from "clay:configuration";
import { serverSetLayoutOverride } from "clay:ui";

await loadPackage("@clay/markdown");
setPackageOption({
  packagePrefix: "markdown",
  option: "layout.defaultVisibility",
  value: "hidden",
  source: "init-js",
});
serverSetLayoutOverride({
  targetId: "markdown.preview",
  property: "slot",
  value: "right",
  source: "user-config",
});
```

The lower-level `serverLoadPackage` validation helper remains available for fixture tests and controlled configuration scenarios:

```javascript
import { serverLoadPackage } from "clay:packages";
import { serverActivateMajorMode, serverSelectDocumentManifest } from "clay:modes";
import { serverRegisterCommand } from "clay:commands";
import { serverPublishDecorations } from "clay:decorations";
import { serverRegisterParseHandler } from "clay:parse";
```

## Carried-forward deferrals

- **Non-`@clay/*` registry resolution:** Third-party, npm, custom registry, scoped (`@scope/pkg`), bare (`left-pad`), URL, path, and traversal specifiers are not resolved by the current first-party resolver. The authority boundary is limited to the shipped `@clay/*` packages under `CARGO_MANIFEST_DIR/packages`; package-manager installation/metadata records do not grant runtime execution authority. Future ecosystem hardening (Phase 23) will widen the resolver only after registry packages, third-party verification, package-manager integration, sandboxing, and an approved authority decision are complete.
- **Hot-reload:** Phase 19 invalidates `loadPackage` state by replacing the runtime generation. The old generation's `FirstPartyLoadEntryAllowlist` and `globalThis.__clayLoadedPackages` are dropped with the old service after a successful swap; failed reloads keep the prior service active.
- **Persistent shared enable state:** `PackageService` is instantiated per runtime with an empty `FakeBackend` and a `Mutex`. Persistent enable state across server restarts is not implemented; packages are re-installed and re-enabled at each configuration load.
- **Security review:** The deny-by-default boundary, the constrained `@clay/*` allowlist, and the validation reuse are covered by an executable test suite (see `tests/package_loading_docs.rs` and `src/server/js_runtime.rs`). The decision log records the explicit authority expansion and the authorities NOT granted.

## References

- `src/server/js_runtime.rs`
- `src/server/ops/packages.rs`
- `src/packages/record.rs`
- `src/packages/service.rs`
- `src/packages/conflict.rs`
- `src/server/decorations.rs`
- `src/server/parse_coordinator.rs`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/backlog.md`
