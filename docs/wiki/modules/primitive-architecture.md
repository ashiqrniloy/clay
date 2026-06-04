# Primitive Architecture

## Source

- `docs/reference/primitives/index.md`
- `docs/reference/primitives/audit.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/clay-js-api/api-inventory.toml`
- `src/perf/budgets.rs`
- `tests/primitives_docs.rs`

## Overview

Phase 16 defines Clay's package/mode primitive architecture as documentation-as-code. It records which package-controllable capabilities exist today, which primitives are planned for Phase 17/18, and how future packages can affect editor behavior without hard-coding mode-specific Rust logic.

The authoritative public architecture lives in `docs/reference/primitives/`. This wiki page explains the internal design flow behind those reference documents and how future implementation work should extend them.

## Responsibilities

- Explain the primitive registry schema and category taxonomy used by package and mode planning.
- Show how registry entries connect to planned Clay JS API inventory stubs, performance budgets, package permissions, and future implementation tasks.
- Keep public programmatic API details in `docs/reference/clay-js-api/` and `docs/reference/primitives/`; use the wiki for implementation reasoning and cross-document flow.
- Preserve the Phase 16 boundary: design documents and planned API stubs only, with no runtime package loading, protocol messages, op wrappers, or client rendering hooks implemented in this phase.

## How It Works

1. `docs/reference/primitives/audit.md` identifies existing primitive surfaces from behavior manifests, SDUI, configuration, file/workspace APIs, document editing, editor rendering, and observability.
2. `docs/reference/primitives/registry.md` turns that audit into stable primitive categories. Each category records the owner, authority boundary, hot-path policy, Clay JS API shape stub, permissions, budget reference, documentation metadata, test expectations, and status.
3. Category names such as `DocumentClassification`, `MajorModeActivation`, `TextTransform`, `IncrementalParseUpdate`, `DecorationRange`, `CommandDeclaration`, `SduiPanelStatusContribution`, `PackageOwnedConfiguration`, and `PackagePermissionDeclaration` become trace anchors for later plans.
4. New package/mode capabilities are represented as planned Clay JS API inventory entries in `docs/reference/clay-js-api/api-inventory.toml` rather than hidden runtime hooks. Planned entries keep `status = "planned"` and do not imply implemented facades or ops.
5. `docs/reference/primitives/backlog.md` prioritizes registry entries into Phase-17-required, Phase-18-required, and deferred work so package loading and the Markdown mode POC can be planned without rediscovering prerequisites.
6. `tests/primitives_docs.rs` statically verifies that reference docs, registry entries, planned API stubs, and budget constants remain linked and traceable.

## Registry Schema Fields

The registry intentionally uses the same vocabulary as `api-inventory.toml`:

- `owner`, `authority`, and `permissions` define where canonical state and privilege live.
- `hot_path_policy` states whether work can happen locally, asynchronously, or only outside the typing path.
- `js_module`, `js_export`, `stable_id`, and `user_facing_name` describe the future public API shape.
- `budget_ref` ties each primitive to constants in `src/perf/budgets.rs` or an explicit `no-hot-path` rationale.
- `primitive_kind`, `documentation_metadata`, `test_expectations`, and `status` make the entry actionable for future implementation plans.

## Performance Budgets

Phase 16 adds advisory primitive budget names in `src/perf/budgets.rs` so docs and future protocol work can compile against stable constants:

```rust
pub const DECORATION_PAYLOAD_BUDGET_BYTES: usize = 8192;
pub const INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES: usize = 4096;
pub const MODE_ACTIVATION_P95_BUDGET_MS: u64 = 100;
pub const COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const FOLDING_RANGE_PAYLOAD_BUDGET_BYTES: usize = 2048;
pub const PRIMITIVES_REGISTRY_VERSION: &str = "phase16-primitives-v1";
```

These are advisory in Phase 16. Later phases should promote them to hard checks only after concrete protocol messages and representative fixtures exist.

## Package Security Model

`docs/reference/primitives/package-security.md` is the canonical security source for package primitives. This wiki summarizes the implementation implications rather than duplicating the full permission table: package load must validate package identity, API prefix uniqueness, package-prefixed contribution IDs, declared permissions, primitive schemas, payload budgets, raw-op prohibitions, and deterministic conflict handling before a contribution becomes active.

By default, package primitives cannot claim filesystem access outside already-open document content, network, shell, AI mutation, remote listeners, WASM execution, raw `Deno.core.ops`, direct Masonry/widget mutation, arbitrary GPU draw calls, native widget handles, client-side JavaScript, package enable/disable mutation, or workspace mutation outside documented workspace APIs. Future exceptions require an approved decision log, explicit permissions, documentation-as-code coverage, tests, and load-time validation.

## Invariants and Constraints

- Packages contribute inert declarations or server-side handlers; the Rust client never executes package JavaScript in paint, layout, keypress, pointer, scroll, or text-event handlers.
- Server-owned primitives validate package prefix, schema, permissions, provenance, payload size, and conflicts before activation or publication.
- `ClientFirstPredictable` behavior remains Rust-known manifest data, not arbitrary JavaScript callbacks.
- Package-owned IDs and configuration keys must be package-prefixed; only first-party Clay APIs may use `clay.*` stable IDs.
- Package enable/disable authority is intentionally deferred until a future approved decision log defines a permission model.

## Tests

- `tests/primitives_docs.rs`: static documentation coverage for links, registry categories, planned API stubs, security rules, Markdown requirements, backlog traces, and budget constants.
- `cargo test --test primitives_docs`: runs the Phase 16 primitive documentation coverage suite.
- Manual wiki review: confirm this page and related Phase 16 wiki pages remain linked from `docs/wiki/index.md`.

## Related

- [Primitive Architecture Documentation](primitive-architecture-docs.md)
- [Phase 18 Markdown Primitive Review](phase18-markdown-primitive-review.md)
- [Rendering Primitives](rendering-primitives.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Behavior Manifests](behavior-manifests.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Clay JS Documentation Registry](clay-js-doc-registry.md)
- `plans/016-Phase16-Mode-and-Package-Primitive-Architecture-Analysis.md`
