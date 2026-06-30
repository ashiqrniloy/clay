# Primitive Architecture

## Source

- `docs/reference/primitives/index.md`
- `docs/reference/primitives/audit.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/phase18.1-shell-layout-primitive-review.md`
- `docs/wiki/modules/phase18.2-shell-runtime-primitive-review.md`
- `docs/wiki/modules/phase18.3-slot-ui-primitive-review.md`
- `docs/wiki/modules/phase18.4-input-state-config-primitive-review.md`
- `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md`
- `docs/wiki/modules/phase18.8-transient-menu-command-execution-primitive-review.md`
- `docs/wiki/modules/phase18.9-generic-text-code-modes-primitive-review.md`
- `docs/wiki/modules/package-input-state-configuration.md`
- `docs/wiki/modules/slot-aware-package-ui.md`
- `docs/wiki/modules/masonry-shell.md`
- `src/shell/layout.rs`
- `src/masonry_shell.rs`
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
4. The Phase 18.1 [Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md) extends that primitive-first flow to the Clay shell/working-area architecture gate. It records that existing SDUI/action/configuration/package primitives are useful building blocks but do not yet implement reusable shell/layout primitives.
5. The Phase 18.2 [Shell Runtime Primitive Review](phase18.2-shell-runtime-primitive-review.md) narrows that architecture gate into an implementation-time map: reuse the existing editor, SDUI, behavior-manifest, command/action, configuration, package-loading, parse/decorations, and Masonry surfaces where they fit; add only generic `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, and internal shell observability before package UI work.
6. The Phase 18.2 [Masonry Shell Runtime](masonry-shell.md) implementation now backs the internal shell primitives in Rust: `src/masonry_shell.rs` owns the Clay root widget above `EditorWidget`, while `src/shell/layout.rs` owns `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state, split validation, active pane metadata, fixed-slot size/visibility/collapse state, and deterministic split/slot geometry. Public/package-facing `clay:ui` APIs remain planned until later tasks promote them with docs and tests.
7. The Phase 18.3 [Slot-Aware Package UI Primitive Review](phase18.3-slot-ui-primitive-review.md) maps existing SDUI helpers, shell slot state, command/action validation, package manifest/provenance validation, package loading descriptors, documentation registry machinery, and structural observability to the generic slot UI gaps that should be implemented next: `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration`. It also records that `PackageUiStateScope` and `PackageLayoutOverride` stay Phase 18.4 work unless deliberately promoted with full Clay JS facade/op/reference-doc/registry/test coverage.
8. The Phase 18.3 [Slot-Aware Package UI](slot-aware-package-ui.md) implementation now backs those four contribution primitives with public `clay:ui` facade exports, op wrappers, server-side package UI validators, typed component/style/theme-token modules, crate-internal runtime state, generated API registry coverage, and native fixed-panel/overlay composition. The public APIs are documented under `docs/reference/clay-js-api/ui/`; Rust layout/config/state override primitives remain internal or planned.
9. The Phase 18.4 [Input, State, and Configuration Primitive Review](phase18.4-input-state-config-primitive-review.md) maps existing behavior manifests, keybindings, command/action registry, SDUI/component catalog, shell `PaneSlotLayout`, package UI registry/runtime state, package metadata validation, configuration runtime, docs registry, and observability to the generic Phase 18.4 gaps: `PackageInputContribution`, component-scoped action/focus metadata, `PackageUiStateScope`, `PackageLayoutOverride`, `PackageOwnedConfiguration`, package option schemas, and typed theme-token remaps. The Phase 18.4 [Package Input, State, and Configuration Integration](package-input-state-configuration.md) wiki records the final implementation status: runtime-backed `clay.ui.serverRegisterInputContribution`, `clay.ui.serverRegisterUiStateScope`, `clay.ui.serverSetLayoutOverride`, and `clay.configuration.setPackageOption` facade/op/validator/docs/registry paths; inert `PackageInputRouting` runtime state; package option/layout override records; and deferred durable state values, pane selectors, multi-panel ordering, overlay z-order, cross-window layout, and package enable/disable authority. It keeps key/text behavior on behavior manifests, side effects on command intents, package options on documented `~/.config/clay/init.js` Clay JS APIs, and rejects Markdown-specific Rust branches, hidden config keys, raw Masonry access, raw CSS, raw ops, and client-side JavaScript.
9b. The Phase 18.5 [Markdown Replan Primitive Review](phase18.5-markdown-replan-primitive-review.md) inventories the generic shell/runtime, package UI, input/state/config, layout override, mode activation, command registry, behavior manifest, decoration transport, parse coordinator, configuration runtime, and selected-file open primitives that Markdown end-user loading must consume. It maps every Markdown need (main editor placement, optional preview, no default side panel, mode activation, commands/key routing, parse handler, decorations, user overrides, selected-file open) to an existing generic primitive and identifies the one-line `loadPackage("@clay/markdown")` specifier resolver as the only generic gap blocking the Markdown replan. It rejects Markdown-specific Rust editor/parser/render/shell branches, fixture-only inline package manifests, and `serverLoadPackage(packageJson)` as the documented end-user path.
9c. The Phase 18.8 [Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md) inventories existing command registry/listing, SDUI action intents, behavior-manifest routing, shell bottom-slot/overlay state, slot-aware package UI/input contributions, and persistent server runtime boundaries. It identifies two generic gaps before Control Center work: `CommandExecution` as the server-owned command activation path shared by SDUI actions, package UI actions, keybindings, and transient-menu selections, and `TransientMenuSession` as reusable query/selection/status/focus state for bottom-pane command browsing and future picker workflows. It rejects Control Center-specific widgets/dispatchers and preserves the no-new-authority, no-package-JavaScript-in-client-hot-path boundary.
9d. The Phase 18.8 [Transient Menu Session](transient-menu-session.md) implementation adds the generic `TransientMenuSession` state model in `src/shell/transient_menu.rs`: bounded prompt/query/item list, selection index, status text, focus policy, accessibility labels, and inert activation actions. It keeps filtering and selection movement local to the session, projects rendering onto existing shell transient-overlay primitives, and feeds activation through the server-owned `CommandExecutor` path.
9e. The Phase 18.8 [Control Center](control-center.md) implementation in `src/server/control_center.rs` builds a `TransientMenuSession` from the current `CommandRegistry` snapshot plus the built-in command table. It filters out client-first and native-client-UI commands, exposes key bindings/routing/provenance in item detail strings, supports bounded query filtering, and routes selected-item activation back through `CommandExecutor`. This makes the Control Center package-aware without introducing a separate dispatcher.
9f. The Phase 18.9 [Generic Text/Code Modes Primitive Review](phase18.9-generic-text-code-modes-primitive-review.md) inventories existing `ModeRegistry` classification/activation, behavior-manifest `EditorBehaviorRules` (`Enter`/`Tab`/`pairs`/`comments`/`ContinueLineMarkers`/`PreserveFenceBodyIndent`/`AutocompleteTrigger`), keybinding/command-execution, SDUI/status, decoration, and document-open primitives. It records that most generic key behavior already exists as `TextTransform` manifest data and that the genuine gaps are narrower: always-on built-in `core.text`/`core.code` fallback modes registered through `MajorModeActivation`/`DocumentClassification` (no new primitive), classification shebang and bounded leading-content probes plus a documented precedence ladder, an electric-character manifest kind extending `EnterRule`/`PairRule`, and mode discovery/listing commands as read-only `CommandDeclaration` consumers routed through `CommandExecution`. It rejects language-specific Rust classification/transform branches, package-shaped built-in modes requiring `loadPackage`, parallel fallback registries, and any new filesystem/network/shell/AI/WASM/raw-op/client-JS/package-manager authority.
10. Phase 18.1 finalized the canonical shell/layout reference in `docs/reference/primitives/shell-layout-strategy.md`, and the registry/backlog rows now define `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, `PackageUiStateScope`, `PackageLayoutOverride`, `CommandExecution`, `TransientMenuSession`, and `ControlCenter` categories. The rows keep Masonry/native widgets internal, record no-hot-path policy and bounded payload expectations, and map the primitives to Phase 18.2, 18.3, 18.4, and 18.8 implementation queues.
10. New package/mode capabilities are represented as Clay JS API inventory entries in `docs/reference/clay-js-api/api-inventory.toml` rather than hidden runtime hooks. Implemented public entries must have facade/op/docs/index/generated-registry/test coverage; planned entries keep `status = "planned"`, `registry_public = false`, and do not imply implemented facades or ops.
11. `docs/reference/primitives/backlog.md` prioritizes registry entries into Phase-17-required, Phase-18-required, Phase-18.2-shell-runtime, Phase-18.3-slot-ui, Phase-18.4-state-config, and deferred work so package loading, Markdown mode, and shell/package UI phases can be planned without rediscovering prerequisites.
12. `tests/primitives_docs.rs` statically verifies that reference docs, registry entries, planned API stubs, and budget constants remain linked and traceable.

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

- `tests/primitives_docs.rs`: static documentation coverage for links, registry categories, planned API stubs, security rules, Markdown requirements, shell/layout primitive rows, backlog phase mapping, package-security notes, and budget constants.
- `cargo test --test primitives_docs`: runs the primitive documentation coverage suite, including the Phase 18.1 shell/layout registry and planned `clay:ui` inventory guards.
- Manual wiki review: confirm this page and related Phase 16 wiki pages remain linked from `docs/wiki/index.md`.

## Related

- [Primitive Architecture Documentation](primitive-architecture-docs.md)
- [Phase 18.1 Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md)
- [Phase 18.2 Shell Runtime Primitive Review](phase18.2-shell-runtime-primitive-review.md)
- [Phase 18.3 Slot-Aware Package UI Primitive Review](phase18.3-slot-ui-primitive-review.md)
- [Phase 18.4 Input, State, and Configuration Primitive Review](phase18.4-input-state-config-primitive-review.md)
- [Phase 18.5 Markdown Replan Primitive Review](phase18.5-markdown-replan-primitive-review.md)
- [Phase 18.8 Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md)
- [Phase 18.9 Generic Text/Code Modes Primitive Review](phase18.9-generic-text-code-modes-primitive-review.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Phase 18 Markdown Primitive Review](phase18-markdown-primitive-review.md)
- [Rendering Primitives](rendering-primitives.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Behavior Manifests](behavior-manifests.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Clay JS Documentation Registry](clay-js-doc-registry.md)
- `plans/016-Phase16-Mode-and-Package-Primitive-Architecture-Analysis.md`
