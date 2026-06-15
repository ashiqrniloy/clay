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

Phase 18.4 verified that the generic one-line loader is not implemented yet: there is no public `loadPackage` export in `runtime/js/packages.ts`, no server runtime op that resolves a package specifier, enables the installed package, imports its `loadEntry`, and records package activation from `init.js`, and no `clay.packages.loadPackage` registry entry. The generic loader/API gap is a Clay package-service bridge that can resolve an installed package specifier, enable the package, import its `loadEntry`, and record activation under Clay's package-service authority. Phase 18.5 (`plans/028` Task 4) investigated this bridge and deferred it with a decision-log-backed rationale: the controlled server-side runtime is deny-by-default (`src/server/js_runtime.rs::ClayModuleLoader`) and confines loadable modules to the configuration root (`src/server/configuration.rs::canonical_local_file`), so a working `loadPackage("@clay/*")` requires a security-critical module-loader extension that lets the runtime import a resolver-validated first-party `loadEntry` from outside the config root, plus a `PackageService` resolve/enable/execute path and a new op. That authority expansion warrants its own focused phase rather than being folded into the Markdown replan; see `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`. Until that authority exists, fixtures may call `serverLoadPackage(packageJson)` plus package-owned load helpers as a temporary validation/loading gap, not as the documented default.

Phase 18.4 also verifies the supported customization path that should follow the future one-line load. Optional package customization is expressed through documented Clay JS APIs, specifically documented runtime-backed Clay JS APIs such as `clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride`; hidden JSON/TOML/ad hoc layout, input, style, or theme keys remain rejected, and these APIs do not provide package enable/disable authority. These APIs evaluate at startup, package-load, configuration-change, or explicit setting-change time and install inert validated state for Masonry hot paths to read later.

```javascript
// Preferred target once the generic loader ships:
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

Current fixture-only validation still uses the lower-level helper and explicit package-owned setup while the loader gap remains:

```javascript
import { serverLoadPackage } from "clay:packages";
import { serverActivateMajorMode, serverSelectDocumentManifest } from "clay:modes";
import { serverRegisterCommand } from "clay:commands";
import { serverPublishDecorations } from "clay:decorations";
import { serverRegisterParseHandler } from "clay:parse";
```

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
