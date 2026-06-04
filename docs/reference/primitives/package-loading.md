# Phase 17 Package Loading Runtime Facades

Phase 17 package loading connects package metadata, primitive contribution validation, and controlled server-side JavaScript facades without making the Rust client execute package JavaScript.

## Scope

- `clay:packages` validates and loads package records from `package.json` Clay metadata at install/enable/reload time.
- `clay:modes` registers document classification metadata, activates one major mode per document, and keeps per-document manifest selection server-owned. The richer `serverSelectDocumentManifest` facade remains an explicit planned route until a later runtime op promotes the implemented Rust selector.
- `clay:commands` registers and lists package-owned inert command metadata.
- `clay:decorations` and `clay:parse` exist as Phase-18 handoff facades. Their public calls currently return planned-unavailable errors while the Rust validators and coordinator remain typed server infrastructure.

## Install, Enable, and Runtime Boundary

Package installation is delegated to an npm-compatible package manager by the package service. Installing a package records package files and metadata; it does not execute `entry`, `loadEntry`, command handlers, parse handlers, or decoration code.

Enabling or loading a package is Clay-owned. The server validates identity, `apiPrefix`, permissions, modes, entries, docs, performance metadata, and inert primitive contributions through typed Rust validators before contributions become active. Runtime facade calls route through those validators and never expose raw `Deno.core.ops.op_*` names as user-facing APIs.

Package JavaScript runs only in the controlled server-side runtime for load/configuration/activation work. The Rust client receives inert manifests, decorations, parse updates, and SDUI data; it never receives package JavaScript callbacks.

## Conflict Handling

Enabled packages are checked deterministically at enable/reload time. Clay rejects duplicate prefixes, mode IDs, command IDs, ambiguous key bindings, configuration key collisions, SDUI region collisions, decoration primitive collisions, and behavior manifest entry collisions with package provenance. Conflicts do not silently override existing behavior.

## Hot-Path Policy

Package validation, package loading, mode activation, per-document manifest selection, decoration validation/publication, and parse-handler registration are outside typing, paint, layout, scroll, and text-event handlers. Ordinary keypress routing uses already-installed behavior manifests, and background parsing/decorations are versioned, bounded, and cancellable where implemented.

## Phase 18 Handoff

`DecorationRange` uses bounded `DecorationSet`/`DecorationSpan` protocol data validated by `src/server/decorations.rs` against `DECORATION_PAYLOAD_BUDGET_BYTES`, package provenance, permission, viewport range, and document version.

`IncrementalParseUpdate` uses `src/server/parse_coordinator.rs` to register permission-checked server-side parse handlers, schedule cancellable background parse tasks, reject stale versions, and enforce `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.

`serverLoadPackage` is runtime-backed for package record validation. Per-document manifest selection plus the Phase-18 decoration/parse public facades remain explicit planned APIs until later runtime ops promote the implemented Rust infrastructure:

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
