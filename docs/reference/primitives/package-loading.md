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

## Conflict Handling

Enabled packages are checked deterministically at enable/reload time. Clay rejects duplicate prefixes, mode IDs, command IDs, ambiguous key bindings, configuration key collisions, SDUI region collisions, decoration primitive collisions, duplicate package UI panel/component/overlay/theme-token IDs, duplicate fixed slot claims, duplicate input contribution IDs, duplicate UI state scope IDs, duplicate layout override target/property pairs, duplicate package option schemas, and behavior manifest entry collisions with package provenance. Conflicts do not silently override existing behavior.

## Hot-Path Policy

Package validation, package loading, mode activation, per-document manifest selection, decoration validation/publication, and parse-handler registration are outside typing, paint, layout, scroll, and text-event handlers. Ordinary keypress routing uses already-installed behavior manifests, and background parsing/decorations are versioned, bounded, and cancellable where implemented.

## Phase 18 Handoff

`DecorationRange` uses bounded `DecorationSet`/`DecorationSpan` protocol data validated by `src/server/decorations.rs` against `DECORATION_PAYLOAD_BUDGET_BYTES`, package provenance, permission, viewport range, and document version.

`IncrementalParseUpdate` uses `src/server/parse_coordinator.rs` to register permission-checked server-side parse handlers, schedule cancellable background parse tasks, reject stale versions, and enforce `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.

`serverLoadPackage` is runtime-backed for package record validation. It now validates Phase 18.4 slot-aware UI, input, UI state-scope, layout-override, package-option, API dependency, and theme-token metadata. The summary includes `input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions` contribution counts so fixture tests can verify the same load-time contract as enable/reload. It remains a validation helper rather than end-user package installation, enablement, default `loadEntry` execution, or package-manager authority. The preferred end-user setup remains an explicit one-line `loadPackage("@clay/markdown")` target from `~/.config/clay/init.js`.

Phase 18.6 shipped the generic one-line loader: `loadPackage` is a runtime-backed `clay:packages` facade export backed by a constrained first-party resolver that accepts `@clay/*` specifiers, validates package metadata through `PackageService`, enables the package, and imports and executes its declared `loadEntry` so that the package's mode, commands, parse handler, and keymaps are registered under Clay's authority. Phase 18.7 verifies the default `~/.config/clay/init.js` experience end-to-end: `await loadPackage("@clay/markdown")` runs once on the persistent server runtime, selected-file open reuses the registered mode/parse handler state, and generic open-time activation classifies the path, activates the document's major mode, and schedules `ParseCoordinator` without user config needing to copy package manifests, perform manual primitive registration, publish representative decoration publication payloads, or create per-open runtime roots. Repeated `loadPackage` calls are idempotent for one persistent runtime generation; Phase 19 hot reload replaces the `ClayJsRuntimeService`, reruns the configured/default `init.js`, rebuilds the first-party `loadEntry` allowlist, and starts `globalThis.__clayLoadedPackages` empty in the new generation. The `clay.packages.loadPackage` inventory entry is `status = "runtime-backed"` and `registry_public = true` with full Markdown documentation. The resolver op is `op_clay_packages_load_package_by_specifier` (`src/server/ops/packages.rs`). The module-loader extension (`src/server/js_runtime.rs::ClayModuleLoader`) is deny-by-default for all specifiers and only accepts a resolver-validated first-party `loadEntry` recorded in a shared `FirstPartyLoadEntryAllowlist`. Manifest `entry` and `loadEntry` values must be explicit relative `./... .js` module paths without traversal, absolute paths, URLs, backslashes, empty path segments, or raw op strings. The resolver canonicalizes both the package root and `loadEntry`, fails closed on canonicalization errors, and records the allowlist entry only when the canonical `loadEntry` remains under the canonical package root. The `loadEntry` is then confined to the validated package root for its own imports; escaping imports are rejected. The resolver reuses the Clay-owned `PackageService::enable` validation path (`assemble_package_record` + `check_enabled_packages`) so invalid metadata and conflicting packages are rejected before activation.

`clay package add <spec>` delegates installation to the configured npm-compatible backend (`PnpmBackend`). Lifecycle scripts are suppressed by default via `--ignore-scripts` so remote package code cannot execute before Clay validates package metadata. The `--allow-scripts` CLI flag (or `CLAY_ALLOW_LIFECYCLE_SCRIPTS=1` environment variable) opts into lifecycle scripts and is documented as dangerous. The package store directory is created before invoking the backend. `FakeBackend` is used in tests and never spawns a process or executes scripts.

The resolver is constrained to first-party `@clay/*` packages only. Non-`@clay/*` registry resolution (e.g. npm, custom registries, third-party packages) remains deferred to a future ecosystem hardening phase (Phase 23). Persistent shared enable state across runtime restarts remains deferred. See `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` for the full authority boundary, rationale, and security review. `serverLoadPackage` remains a lower-level validation helper used by fixtures and internally by `loadPackage`; it is not the documented end-user default.

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

- **Non-`@clay/*` registry resolution:** Third-party, npm, and custom registry packages are not resolved by the current first-party resolver. The authority boundary is limited to the shipped `@clay/*` packages under `CARGO_MANIFEST_DIR/packages`. Future ecosystem hardening (Phase 23) will widen the resolver to support registry packages, third-party verification, and package-manager integration.
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
