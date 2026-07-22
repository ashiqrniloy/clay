# Primitive Architecture Documentation

## Source

- `docs/reference/primitives/audit.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/markdown-mode-requirements.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/implementation-gate.md`
- `docs/reference/clay-js-api/api-inventory.toml`
- `docs/index.md`
- `tests/primitives_docs.rs`

## Overview

Phase 16 keeps the mode/package primitive architecture as documentation-as-code, and Phase 16.5 adds a documented primitive implementation gate for package, mode, command, and facade validation. The primitive reference documents define the server/client authority boundaries, hot-path policies, payload budgets, implemented gate scope, planned/deferred API boundaries, and Clay JS API shapes that future package and Markdown-mode phases must follow.

## Responsibilities

- Record existing primitive surfaces and new primitive categories without adding runtime code.
- Map each package-controllable capability to a registry entry with owner, authority, hot-path policy, budget reference, API stub, permission expectation, and status.
- Define inert rendering and background parse update strategies that keep package JavaScript out of Rust client paint, layout, keypress, and text-event handlers.
- Define the Phase 18 Markdown mode readiness checklist, including `@clay/markdown` package identity, `markdown` API prefix, detection rules, text transforms, decoration span kinds, commands, key bindings, SDUI panel scope, security constraints, and performance targets.
- Statically test that the reference documents stay linked from `docs/index.md` and cite required registry entries/budget constants.
- Keep deferred Phase 17/18 primitive API stubs in `docs/reference/clay-js-api/api-inventory.toml` as `status = "planned"` with `registry_public = false` until facade exports, op wrappers, and Markdown API reference pages are intentionally implemented.
- Verify the Phase 16.5 implemented primitive gate docs for package validation, mode registration/classification/activation, command registration/listing, security scope, hot-path exclusions, backlog handoff, and index links.
- Verify Phase 16.5 user-configurable primitive surfaces through planned configuration API stubs: package options, mode preferences, decoration themes, and parse policies. Package enable/disable remains intentionally unexposed until a future approved decision log defines explicit authority.

## How It Works

1. `docs/reference/primitives/registry.md` is the category source of truth. It names stable primitive entries such as `DocumentClassification`, `MajorModeActivation`, `TextTransform`, `DecorationRange`, `IncrementalParseUpdate`, `CommandDeclaration`, and `SduiPanelStatusContribution`.
2. Strategy documents refine subsets of the registry. `rendering-strategy.md` defines the implemented `DecorationSet`/`DecorationSpan` transport and SDUI reuse. `parse-update-strategy.md` defines server-side cancellable parse tasks and viewport-prioritized result publication.
3. `markdown-mode-requirements.md` consumes the registry as a Phase 18 readiness checklist. Every Markdown capability is expressed through existing, new, or deferred primitive registry entries rather than direct Rust implementation hooks.
4. `docs/index.md` links the primitive documents under Developer Guides so the project documentation registry and agents can discover them.
5. `docs/reference/primitives/implementation-gate.md` documents the Phase 16.5 runtime gate separately from full Phase 17 package installation. It explains the supported fixture format, validation failures, security boundaries, hot-path exclusions, and Phase 17/18 handoff.
6. `docs/reference/clay-js-api/api-inventory.toml` now distinguishes implemented primitive/package-loading APIs (`clay:packages`, `clay:modes`, `clay:commands`, and promoted `clay.packages.serverLoadPackage`) from planned Phase 18 provider APIs (`clay:decorations`, `clay:parse`, `clay:folding`, mode manifest selection, and planned configuration setters). Implemented entries point to facade files, op wrappers, Rust validators/registries, Markdown API docs, and generated-registry coverage.
7. The Phase 16.5 configuration review uses the same inventory rather than hidden config keys. `clay.configuration.setPackageOption`, `clay.configuration.setModePreference`, `clay.configuration.setDecorationTheme`, and `clay.configuration.setParsePolicy` are planned-only stubs scoped to `~/.config/clay/init.js`; package enable/disable is documented as deferred because it would grant package-management authority.
8. `tests/primitives_docs.rs` performs static checks over the Markdown files, typed budget constants, implemented primitive gate docs, and planned primitive API/configuration stubs. These tests are intentionally non-mutating and do not generate artifacts.

## Code Examples

```bash
cargo test --test protocol primitives_docs::
```

```rust
assert_eq!(DECORATION_PAYLOAD_BUDGET_BYTES, 8192);
assert!(requirements.contains("`DocumentClassification`"));
assert!(index.contains("reference/primitives/markdown-mode-requirements.md"));
```

## Invariants and Constraints

- Phase 16 primitive documents remain architecture deliverables only, while Phase 16.5 may expose only the explicitly implemented package/mode/command validation gate through controlled server runtime facades and op wrappers. Full package installation/loading, Phase 18 decoration/parse/folding providers, protocol expansion, and editor rendering hooks remain deferred until later plans.
- Configuration remains a Clay JS API surface rooted at `~/.config/clay/init.js`; planned settings must list behavior-changing `custom_properties` and must not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw op, or client-side JavaScript authority.
- Package-provided rendering and parsing produce validated inert declarations; the Rust client never executes package JavaScript in paint/input hot paths.
- Markdown mode POC requirements and new planned API stubs must trace capabilities to registry entries so Phase 17/18 plans can derive implementation tasks without rediscovering prerequisites.
- Budget names referenced in docs must compile through `src/perf/budgets.rs`.
- Package identity/provenance rules follow `@clay/markdown` plus prefix `markdown` for the first-party Markdown POC.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- `cargo test --test protocol primitives_docs::`: runs the primitive documentation coverage suite.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Rendering Primitives](rendering-primitives.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Behavior Manifests](behavior-manifests.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Clay JS Documentation Registry](clay-js-doc-registry.md)
- `plans/016-Phase16-Mode-and-Package-Primitive-Architecture-Analysis.md`
- `.agents/skills/project-patterns/references/package-distribution.md`
- `.agents/skills/project-patterns/references/behavior-manifests.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
