# Clay Primitives Reference

This directory is the Phase 16 architecture source for package- and mode-controlled Clay primitives. These documents define the registry, security baseline, rendering/parse strategies, Markdown POC prerequisites, and implementation backlog for Phase 17 and Phase 18, including Phase 18.3 runtime-backed slot-aware package UI contribution primitives, Phase 18.4 runtime-backed package input/state/layout-override/configuration primitives, Phase 18.9 generic `core.text`/`core.code` fallback modes, shebang/content-probe classification, generic key-behavior (electric/pair/comment) transforms, mode-discovery commands, and Phase 18.10 package-provided `SyntaxGrammarContribution` Tree-sitter grammar metadata for grammar-only syntax packages (package-author contract in [Creating Clay Packages](../packages/creating-packages.md#phase-1810-authoring-contract-grammar-only-syntax-packages)).

## Documents

- [Existing Primitive Audit](audit.md) — existing behavior manifest, SDUI, configuration, document/workspace, editor, and observability primitives.
- [Primitive Registry Schema](registry.md) — canonical primitive taxonomy, schema vocabulary, authority boundaries, performance budgets, planned Clay JS API shape stubs, and the Phase 18.10 `SyntaxGrammarContribution` grammar primitive row.
- [Rendering Customization Strategy](rendering-strategy.md) — inert decoration/layout/render declarations and SDUI reuse for package rendering.
- [Clay Shell and Package UI/Layout Strategy](shell-layout-strategy.md) — Phase 18.1/18.2 architecture and runtime status plus Phase 18.3 runtime-backed package panel/component/overlay/theme-token contribution status and Phase 18.4 runtime-backed input/state-scope/layout-override/package-option status for the working area, pane/split tree, pane slots, package UI/state/style declarations, and Masonry boundary.
- [Incremental Parse and Background Parse Update Strategy](parse-update-strategy.md) — server-side parse task lifecycle, cancellation, viewport filtering, and fallback behavior.
- [Markdown Mode POC Requirements](markdown-mode-requirements.md) — Phase 18 readiness checklist for `@clay/markdown`.
- [Package Primitive Security and Provenance Requirements](package-security.md) — package prefix, permission, validation, conflict, and prohibited-authority baseline.
- [Phase 17 Package Loading Runtime Facades](package-loading.md) — package load/runtime boundaries, conflict handling, runtime facade wiring, hot-path policy, and Phase 18 decoration/parse handoff.
- [Primitive Implementation Gate](implementation-gate.md) — Phase 16.5 runtime validation gate, fixture format, load/activation scope boundary, and Phase 17/18 handoff.
- [Prioritized Primitive Backlog](backlog.md) — sortable Phase-17-required, Phase-18-required, and deferred primitive implementation backlog plus the Phase 17 prerequisite checklist.

## Phase 17 Readiness Summary

Phase 17 should implement package loading and mode primitives before Phase 18 starts the Markdown POC. The minimum Phase 17 gates are:

1. Package manifests carry Clay metadata (`apiPrefix`, permissions, modes, load/runtime entries) and are validated at enable/load time.
2. `DocumentClassification`, `MajorModeActivation`, and `CommandDeclaration` have planned Clay JS API stubs and implementation tasks.
3. Phase 16.5's [Primitive Implementation Gate](implementation-gate.md) validates fixtures and future package metadata before Phase 17 package installation/enable/load workflows expand.
4. Package contributions preserve prefix/provenance and reject duplicate mode names, duplicate command IDs, ambiguous key bindings, and undeclared permissions deterministically.
5. Per-document/per-mode behavior manifest selection can atomically install client-safe `ClientFirstPredictable` text transforms and server-routed commands.
6. Phase 18 primitives (`DecorationRange`, `IncrementalParseUpdate`, and Markdown SDUI/keybinding extensions) have explicit handoff entries in [backlog.md](backlog.md).
7. Phase 18.2 shell runtime primitives (`WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout`) are implemented as internal Rust foundations; Phase 18.3 package UI primitives (`PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration`) are runtime-backed inventory APIs through `clay:ui`; Phase 18.4 package input, UI state-scope, layout override, and package option APIs are runtime-backed through documented Clay JS APIs; working-area/split/direct slot mutation, durable state-value persistence, pane selector, multi-panel ordering, overlay z-order, cross-window layout, and package enable/disable remain planned/deferred.
