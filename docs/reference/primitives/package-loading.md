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

## Package Sources and Provenance

Clay delegates registry access, package fetching, dependency resolution, version ranges, lockfile writing, integrity verification, caching, GitHub/git handling, tarball handling, and local-path installs to the npm-compatible package manager where possible. Clay does not implement a registry client.

Supported target sources for the unified package model include:

```bash
clay package add @vendor/example
clay package add github:user/repo
clay package add https://github.com/user/repo.git
clay package add ./local-package
```

Clay records source provenance for diagnostics and user approval, not as an automatic runtime grant:

```toml
[package_source]
name = "@vendor/example"
requested_spec = "@vendor/example@1.2.3"
resolved_version = "1.2.3"
source = "npm"
registry = "https://registry.npmjs.org/"
package_root = "/clay/packages/node_modules/@vendor/example"
lockfile = "pnpm-lock.yaml"
integrity = "sha512-..."
```

Policy:

1. Install/update records requested spec, resolved identity, package root, registry/source URL, lockfile/integrity data when available, and package-manager diagnostics.
2. Install still does not execute `entry`, `loadEntry`, command handlers, parse handlers, or package runtime.
3. Enable/load validates Clay metadata and prompts/uses user authorization for requested capabilities.
4. Package-manager stdout, stderr, exit code, `package.json`, lockfile text, and registry metadata are diagnostic/provenance inputs only; they do not bypass manifest validation or user authorization.
5. Lifecycle scripts remain a user-controlled dangerous install option, not a runtime authority model.
6. Diagnostics copied from package-manager output must be bounded and should avoid secrets, auth tokens, unbounded stderr/stdout blobs, and unnecessary absolute paths.
7. Verification runs only at install, update, enable, load, reload, startup, or background audit time. It never runs from keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

Current implementation: `PackageSourceKind` classifies npm, GitHub shorthand, git URL, tarball, local-path, and Clay-shipped specs at the package-manager boundary. `PackageProvenance` records requested spec, source kind, resolved name/version, package root, optional lockfile/integrity fields, and bounded sanitized diagnostics. `DiscoveredPackage`, `InstalledPackage`, and `PackageInspection` carry that provenance without enabling runtime behavior. `PackageService::enable` evaluates declarative package graph relations at enable/load/reload time: `dependsOn` and `extends` targets are enabled first, `disables` and `replaces` withdraw enabled targets only when the package has a `package-control` authorization grant, and missing targets or cycles return deterministic diagnostics. Remaining gaps are durable on-disk provenance/authorization persistence across real store refreshes, package-scoped revocation indexes, package-import boundary enforcement, and conflict override/extend/replace resolution.

## Conflict Handling

Enabled packages are checked deterministically at enable/reload time. `check_enabled_packages` reports duplicate prefixes, mode IDs, command IDs, ambiguous key bindings, configuration key collisions, SDUI region collisions, decoration primitive collisions, duplicate package UI panel/component/overlay/theme-token IDs, duplicate fixed slot claims, duplicate input contribution IDs, duplicate UI state scope IDs, duplicate layout override target/property pairs, duplicate package option schemas, and behavior manifest entry collisions with package provenance. `PackageConflictResolutionPolicy` resolves only explicit cases: user conflict overrides, package graph `replaces`/`disables` with a `package-control` grant, and distinct key-binding priority/routing metadata. Unresolved conflicts still fail closed with deterministic diagnostics; conflicts do not silently override existing behavior by load order.

## Unified Disable, Update, Rollback, and Incident Policy

Disable is active withdrawal for any package source. Disable removes `PackageService` enabled state, rebuilds the next runtime generation without the package, cancels package-owned parse/completion work, unregisters handler tokens, and withdraws commands, behavior manifests, SDUI/status trees, package UI/input/state/layout/theme declarations, decorations, folding, completion providers, and diagnostics before publishing replacement state.

Updates are package identity changes. Changed version, source, package root, `apiPrefix`, capability set, package graph relations, or Clay API compatibility should be shown to the user when they affect prior authorization. If a new generation fails source lookup, metadata validation, authorization, conflict resolution, runtime load/evaluation, output validation, or parse registration, Clay keeps the prior validated generation active and reports diagnostics; it does not partially merge new contributions.

Package control is user-authorized. A package with `package-control` may disable, extend, or replace another package through package graph declarations and Clay APIs. The same rules apply whether the affected package is Clay-shipped or user-installed.

Rollback uses the Phase 19 runtime-generation model: build and validate the candidate generation off to the side, swap only after success, and keep the last validated client state when candidate load/evaluation fails. Stale generation outputs are rejected by runtime generation ID, document version, behavior version, handler token, and package provenance before any client publication.

Incident response should preserve user control: revoke or downgrade the package authorization, stop scheduling new package work, cancel in-flight tasks, withdraw package-owned contributions, preserve unaffected packages and prior validated client state, and require explicit reload/update/re-authorization before execution resumes.

Package-manager side effects are not active runtime state. Installing/removing package-store files does not keep commands, handlers, behavior manifests, SDUI, decorations, or client state active after disable/rollback.

Rollback/disable work runs at disable, update, reload, startup, incident-response, or background cleanup time. It never blocks keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

Current implementation: `PackageService::enable` rolls back failed conflict and package-graph candidates by restoring the prior enabled set, conflict diagnostics, revocation records, and package generation. `PackageService::disable` records package-scoped active withdrawal through `PackageRevocationRecord` and `PackageContributionWithdrawalCounts` for commands, behavior manifests, SDUI, parse handlers, decorations, completions, layout, input, state, theme, and diagnostics. Phase 19 runtime reload keeps the prior generation on failed evaluation. `ParseCoordinator::cancel_generation` and `ParseCoordinator::cancel_package` cancel stale generation/package parse work, and stale-result checks reject old output. `PackageLoadEntryAllowlist::revoke_package` withdraws package-owned loadEntry/transitive module entries. Remaining gaps are durable revocation persistence, package-import boundary enforcement, and publication wiring for future client/runtime contribution caches beyond the enabled `PackageRecord` set.

## Hot-Path Policy

Package validation, package loading, mode activation, per-document manifest selection, decoration validation/publication, and parse-handler registration are outside typing, paint, layout, scroll, and text-event handlers. Ordinary keypress routing uses already-installed behavior manifests, and background parsing/decorations are versioned, bounded, and cancellable where implemented.

## Phase 18 Handoff

`DecorationRange` uses bounded `DecorationSet`/`DecorationSpan` protocol data validated by `src/server/decorations.rs` against `DECORATION_PAYLOAD_BUDGET_BYTES`, package provenance, permission, viewport range, and document version.

`IncrementalParseUpdate` uses `src/server/parse_coordinator.rs` to register permission-checked server-side parse handlers, schedule cancellable background parse tasks, reject stale versions, and enforce `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.

`serverLoadPackage` is runtime-backed for package record validation. It now validates Phase 18.4 slot-aware UI, input, UI state-scope, layout-override, package-option, API dependency, and theme-token metadata. The summary includes `input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions` contribution counts so fixture tests can verify the same load-time contract as enable/reload. It remains a validation helper rather than end-user package installation, enablement, default `loadEntry` execution, or package-manager authority. The preferred end-user setup remains an explicit one-line `loadPackage("@clay/markdown")` target from `~/.config/clay/init.js`.

Phase 18.6 shipped the one-line loader: `loadPackage` is a runtime-backed `clay:packages` facade export. Plan 035 now generalizes the resolver so bundled `@clay/*` packages and installed npm/GitHub/git/tarball/local-path packages use the same `PackageService` validation, authorization, package-root confinement, module-loader allowlist, and `loadEntry` execution path. Source-aware packages resolve by package name or original requested specifier from installed provenance; bundled packages are seeded from Clay's shipped `packages/` directory. Phase 18.7 verifies the default `~/.config/clay/init.js` experience end-to-end: `await loadPackage("@clay/markdown")` runs once on the persistent server runtime, selected-file open reuses the registered mode/parse handler state, and generic open-time activation classifies the path, activates the document's major mode, and schedules `ParseCoordinator` without user config needing to copy package manifests, perform manual primitive registration, publish representative decoration publication payloads, or create per-open runtime roots. Repeated `loadPackage` calls are idempotent for one persistent runtime generation; Phase 19 hot reload replaces the `ClayJsRuntimeService`, reruns the configured/default `init.js`, rebuilds the package load-entry allowlist, and starts `globalThis.__clayLoadedPackages` empty in the new generation. The `clay.packages.loadPackage` inventory entry is `status = "runtime-backed"` and `registry_public = true` with full Markdown documentation. The resolver op is `op_clay_packages_load_package_by_specifier` (`src/server/ops/packages.rs`). Manifest `entry` and `loadEntry` values must be explicit relative `./... .js` module paths without traversal, absolute paths, URLs, backslashes, empty path segments, or raw op strings. The resolver canonicalizes both the package root and `loadEntry`, fails closed on canonicalization errors, and records the allowlist entry only when the canonical `loadEntry` remains under the canonical package root. The `loadEntry` is then confined to the validated package root for its own imports; escaping imports are rejected. The resolver reuses the Clay-owned `PackageService::enable` validation path (`assemble_package_record` + authorization + `check_enabled_packages`) so invalid metadata, missing grants, and unresolved conflicts are rejected before activation.

`clay package add <spec>` delegates installation to the configured npm-compatible backend (`PnpmBackend`). Lifecycle scripts are suppressed by default via `--ignore-scripts` so remote package code cannot execute before Clay validates package metadata. The `--allow-scripts` CLI flag (or `CLAY_ALLOW_LIFECYCLE_SCRIPTS=1` environment variable) opts into lifecycle scripts and is documented as dangerous. The package store directory is created before invoking the backend. `FakeBackend` is used in tests and never spawns a process or executes scripts.

Installed package-manager metadata does not automatically activate runtime behavior: `pnpm add`/package-store records can be inspected, but enable/load still requires Clay metadata validation and user-approved capabilities. Persistent shared enable/authorization state across runtime restarts remains deferred. See `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` for the superseding authority model. `serverLoadPackage` remains a lower-level validation helper used by fixtures and internally by `loadPackage`; it is not the documented end-user default.

The supported customization path after the one-line load is unchanged. Optional package customization is expressed through documented Clay JS APIs such as `clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride`; hidden JSON/TOML/ad hoc layout, input, style, or theme keys remain rejected, and these APIs do not provide package enable/disable authority. These APIs evaluate at startup, package-load, configuration-change, or explicit setting-change time and install inert validated state for Masonry hot paths to read later.

```javascript
// Implemented end-user default from ~/.config/clay/init.js:
import { loadPackage } from "clay:packages";
import { setPackageOption } from "clay:configuration";
import { serverSetLayoutOverride } from "clay:ui";

await loadPackage("@clay/markdown");
// Bundled and user-installed packages share the one-line path after install
// and user authorization. init.js grants no capabilities of its own; every
// powerful capability (filesystem/network/shell/AI/WASM/raw-ops/native-ui/
// client-runtime/package-control) is a separate user-approved grant recorded
// against the package identity/source/provenance.
await loadPackage("@vendor/foo");
await loadPackage("github:user/repo");
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

- **Durable package state:** Runtime `PackageService` can resolve installed/source-aware packages already present in its registry, but durable enable/authorization hydration across server restarts is not implemented; packages are reloaded from configuration each runtime generation.
- **Hot-reload:** Phase 19 invalidates `loadPackage` state by replacing the runtime generation. The old generation's `PackageLoadEntryAllowlist` and `globalThis.__clayLoadedPackages` are dropped with the old service after a successful swap; failed reloads keep the prior service active.
- **Authority model update:** `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` supersedes the strict third-party deny-first policy. Resolver tests now cover source-aware package loading through the shared package service path rather than treating source as a capability ceiling.

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
