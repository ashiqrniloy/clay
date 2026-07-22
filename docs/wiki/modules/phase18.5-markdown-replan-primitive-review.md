# Phase 18.5 Markdown Replan Primitive Review

## Source

- `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`
- `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md`
- `plans/024-Phase18.1-Clay-Shell-Working-Area-and-Package-UI-Layout-Architecture-Gate.md`
- `plans/025-Phase18.2-Masonry-Clay-Shell-and-Pane-Runtime-Foundation.md`
- `plans/026-Phase18.3-Slot-Aware-Package-UI-Components-Panels-and-Theme-Tokens.md`
- `plans/027-Phase18.4-Package-Input-Actions-State-Data-and-Configuration-Integration.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-loading.md`
- `docs/wiki/modules/phase18.1-shell-layout-primitive-review.md`
- `docs/wiki/modules/phase18.2-shell-runtime-primitive-review.md`
- `docs/wiki/modules/phase18.3-slot-ui-primitive-review.md`
- `docs/wiki/modules/phase18.4-input-state-config-primitive-review.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/masonry-shell.md`
- `docs/wiki/modules/slot-aware-package-ui.md`
- `docs/wiki/modules/server-driven-ui.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/markdown-mode-activation.md`
- `docs/wiki/modules/first-party-markdown-package.md`
- `docs/wiki/modules/configuration-runtime.md`
- `src/shell/layout.rs`
- `src/shell/package_ui.rs`
- `src/shell/components.rs`
- `src/shell/theme.rs`
- `src/masonry_shell.rs`
- `src/masonry_sdui.rs`
- `src/server/ui.rs`
- `src/server/ops/ui.rs`
- `src/server/ops/configuration.rs`
- `src/server/ops/packages.rs`
- `src/server/ops/modes.rs`
- `src/server/connection.rs`
- `src/packages/modes.rs`
- `src/packages/record.rs`
- `src/packages/conflict.rs`
- `src/packages/service.rs`
- `src/protocol/mod.rs`
- `runtime/js/ui.js`
- `runtime/js/configuration.js`
- `runtime/js/packages.js`
- `runtime/js/modes.js`
- `runtime/js/commands.js`
- `runtime/js/parse.js`
- `runtime/js/decorations.js`
- `runtime/js/sdui.js`
- `packages/markdown/dist/load.js`
- `packages/markdown/dist/index.js`
- `packages/markdown/dist/parser.js`
- `packages/markdown/dist/sdui.js`
- `tests/fixtures/configuration/markdown-mode/init.js`
- `tests/fixtures/configuration/windows-markdown-open/init.js`
- `docs/reference/clay-js-api/api-inventory.toml`
- `.agents/skills/project-patterns/references/mode-primitive-first.md`
- `.agents/skills/project-patterns/references/package-ui-layout.md`
- `.agents/skills/project-patterns/references/behavior-manifests.md`
- `.agents/skills/project-patterns/references/package-distribution.md`
- `.agents/skills/project-patterns/references/configuration-system.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
- `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`

## Overview

This page records the Phase 18.5 primitive-first review performed **before** the `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md` replan. Phases 18.1–18.4 promoted the generic Clay-owned shell/runtime, package UI, input/state/config, and layout override primitives that Markdown end-user loading needs to consume. Phase 18.5 must now confirm that every Markdown end-user need maps onto an existing generic primitive, and that the only new generic primitive required is a one-line `loadPackage("@clay/markdown")` specifier resolver.

The review rejects Markdown-specific Rust editor/parser/render/shell branches. Markdown's needs — main editor placement, optional preview, no default side panel, mode activation, command/key routing, parse handler registration, decoration publication, user configuration override, and selected-file open activation — all map onto existing generic primitives. The one generic gap this review originally identified — a generic end-user package specifier resolver that replaces fixture-only inline package manifests and per-facade manual registration in `init.js` — was **closed by Plan 029** (`plans/029-Phase18.6-Generic-Package-Loader-and-First-Party-Module-Bridge.md`), which shipped the constrained first-party `loadPackage("@clay/*")` resolver. Every Markdown need now maps onto an implemented generic primitive.

## Existing Generic Primitive Inventory

| Primitive | Current source paths | What Markdown can use it for now | Runtime classification | Security and validation boundary |
| --- | --- | --- | --- | --- |
| `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout` | `src/shell/layout.rs`, `src/masonry_shell.rs` | Main editor already lives in the mandatory `main` slot; optional fixed `left`/`right`/`top`/`bottom` slots exist for an optional preview. | Startup/update work for layout state; Masonry layout/paint reads installed state only. | Shell safety preserves at least one pane and one `main` slot, rejects invalid ratios/sizes/stale updates, and keeps native widget IDs, direct Masonry mutation, raw CSS, raw ops, and client JS internal/non-authoritative. |
| `PanelContribution` (`serverRegisterPanelContribution`) | `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/package_ui.rs` | Optional Markdown preview/status panel as a package-prefixed `PanelContribution` targeting the `right` slot with `defaultVisibility: "hidden"`. | Package load/config/update work for registration; paint/layout state read for fixed-panel composition. | Server validators reject duplicate IDs/slots/tokens, unregistered actions, raw CSS/native handles/raw ops/client JS, and payloads over budget. Phase 18.3 supports `kind: "fixed"`; transient UI must use `TransientOverlayContribution`. |
| `ComponentContribution` (`serverRegisterComponentContribution`) | `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/components.rs` | Component nodes for any preview panel body (`panel`, `label`, `button`, `list`, `editorView`, `flex`, `stack`). | Package load/config/update work for validation; protocol/client update work for install; paint/layout state read for native composition. | Component IDs must be package-prefixed or Clay-owned; validators reject raw CSS, raw colors outside typed token contracts, native handles, renderer callbacks, client-side JavaScript, raw `Deno.core.ops`, unsupported component kinds, unregistered actions, and oversize payloads. |
| `TransientOverlayContribution` (`serverRegisterTransientOverlayContribution`) | `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs` | Optional Markdown status/preview overlays (separate from fixed-slot geometry) if ever needed. | Package load/config/update work; explicit UI update work for open/dismiss; paint/layout state read for overlay composition. | Validates package provenance, focus policy, dismissal policy, accessibility role/label, action targets, and payload budget. |
| `PackageThemeTokenDeclaration` (`serverRegisterThemeToken`) | `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/theme.rs` | Typed semantic Markdown tokens (heading sizes, list-marker colors, inline-code colors) consumed by the component catalog. | Package load/config/update work for declaration and validation; paint/layout state read through `ThemeTokenResolver`. | Token IDs must be package-prefixed; same-type core-token fallbacks only; raw CSS, raw colors outside token contracts, and renderer callbacks are rejected. |
| `PackageInputContribution` (`serverRegisterInputContribution`) | `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/package_ui.rs` | Bounded pointer/focus/selection/action interests for a Markdown preview panel button or list, if such UI is enabled. | Package load/config/update work; explicit command/UI update work for user action. | Input interests are inert and bounded; raw native events, executable callbacks, raw op names, native handles, and client-side JavaScript are rejected. |
| `PackageUiStateScope` (`serverRegisterUiStateScope`) | `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs` | Declares a package-prefixed Markdown preview visibility/scroll state scope if persistence is needed. | Package load/config/update work; runtime storage only for implemented shell/UI state. | Scope vocabulary stays generic (`package-global`, `user-config`, `workspace`, `document`, `pane`, `component`, `transient-overlay`); hidden globals, unbounded payloads, and persisted workspace/document mutation without explicit permissions are rejected. |
| `PackageLayoutOverride` (`serverSetLayoutOverride`) | `runtime/js/ui.js`, `src/server/ops/configuration.rs`/`ui.rs`, `src/server/ui.rs` | User/package override of Markdown preview slot visibility/splitRatio/themeToken through documented Clay JS APIs. | Startup/configuration reload/explicit setting-change work only. | Validates slot, visibility, splitRatio, themeToken targets; precedence preserves Clay shell safety, user config, active mode defaults, compatible minor modes, global package contributions, and package fallback; hidden JSON/TOML/ad hoc keys remain rejected. |
| `PackageOwnedConfiguration` (`setPackageOption`) | `runtime/js/configuration.js`, `src/server/ops/configuration.rs`, `src/server/configuration.rs` | Markdown-relevant package options (preview default visibility, theme-token remap) declared as typed schemas. | Startup/configuration reload/explicit setting-change work only. | Only package-declared typed option schemas; package prefix, value type, default, allowed values, source, permission (`package-configuration`), and target primitive are validated; hidden JSON/TOML/ad hoc option keys remain rejected. |
| `MajorModeActivation` and `DocumentClassification` | `src/packages/modes.rs`, `src/server/ops/modes.rs`, `runtime/js/modes.js` | Markdown classification by extension/MIME and major-mode activation that installs behavior manifest, commands, keymaps, and editor rules. | Package load/configuration work for pattern registration; document-open work for classification; behavior-manifest install work for activation. | Classification patterns and activation payloads must be package-prefixed; commands and keymaps must target registered commands; no Markdown-specific Rust branch in `op_clay_modes_activate_major_mode`. |
| `CommandDeclaration` and behavior-manifest commands | `src/protocol/mod.rs`, `src/server/ops/commands.rs`, `src/packages/commands.rs`, `runtime/js/commands.js` | Package-prefixed commands (`markdown.togglePreview`, `markdown.insertHeading`, `markdown.toggleList`) and key bindings (`Ctrl+Shift+M`, `Ctrl+Alt+1`, `Ctrl+Shift+8`) routed as `ServerFirst` intents. | Load/activation work for command metadata; behavior-manifest update work; editor hot-path reads installed inert routing data only. | Command IDs must be package-prefixed and registered; ambiguous keybindings, unknown command targets, and unregistered actions are rejected. |
| Behavior manifests, `TextTransform`, `PairRule`, `ContinueLineMarkers` | `src/protocol/mod.rs`, `src/packages/modes.rs`, `runtime/js/keybindings.js` | Client-first Markdown editing rules (list continuation, bold/italic/code pair rules, fence indent preservation) declared as generic manifest data. | Behavior-manifest update work for install; editor keypress/text-event hot path reads installed manifest data only. | `ClientFirstPredictable` behavior stays Rust-known manifest data, not arbitrary JavaScript callbacks; no arbitrary client-side JavaScript, executable callbacks, raw ops, or server round trip before local typing paint. |
| `serverRegisterParseHandler` and parse coordinator | `runtime/js/parse.js`, `src/server/parse_coordinator.rs`, `packages/markdown/dist/parser.js` | Registers the markdown-it package parse handler with viewport priority, parse window, guard bytes, memory budget, and timeout. | Background parse/decor work only; never editor typing/paint/layout/scroll/text-event work. | Permission-gated handler registration, bounded range snapshots, memory-budget validation, per-document cancellation, stale-result rejection, viewport-prioritized scheduling, and payload budgets. |
| `serverPublishDecorations` and decoration transport | `runtime/js/decorations.js`, `src/server/ops/decorations.rs`, `src/protocol/decorations.rs`, `src/editor/decoration_store.rs` | Publishes bounded `DecorationSet`/`DecorationSpan` Markdown syntax decorations to the editor. | Background parse/decor work for publication; editor hot-path reads installed decoration store only. | Bounded payload budgets; decorations must be tied to known document versions; stale document versions are rejected. |
| `loadPackage("@clay/*")` (`clay.packages.loadPackage`) | `runtime/js/packages.js`, `src/server/ops/packages.rs` (`op_clay_packages_load_package_by_specifier`), `src/packages/service.rs` | Generic one-line end-user package loader: `await loadPackage("@clay/markdown")` resolves the first-party specifier, validates + enables the package through `PackageService`, and imports/executes the declared `loadEntry`. Implemented by Plan 029. | Package load/reload work only. | Deny-by-default for arbitrary external specifiers, package-manager execution, registry fetching, and arbitrary specifier expansion; only resolver-validated first-party `@clay/*` packages may load. Reuses the existing `PackageService` identity/permission/contribution/budget/conflict validation before executing the `loadEntry`. |
| `serverLoadPackage(packageJson)` | `runtime/js/packages.js`, `src/server/ops/packages.rs`, `src/packages/service.rs` | Validates and enables a package from an inline package manifest object. Used today by the Markdown package's own `loadMarkdownPackage(clay, options)` entry and by fixture `init.js`. | Package install/enable/load/reload work only. | Deny-by-default for arbitrary specifiers; validates package identity, API prefix, permissions, contribution schemas, payload budgets, and conflicts. **Not** the end-user one-line package loader — it requires the caller to provide the manifest object; the end-user path is `loadPackage("@clay/markdown")`. |
| Open-document activation (`clientOpenFileDialog` binding, `open_selected_file`, `open_document_followup_messages`) | `runtime/js/keybindings.js`, `src/server/connection.rs`, client file dialog backend | `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` lets the user pick a `.md` file; the server applies a single-file grant, classifies the document, installs the active major mode, and publishes behavior/decorations. | Explicit command work for the dialog; document-open work for snapshot/grant/classification/activation; background parse/decor work for decorations. | Selected-file authority is a single-file grant only; broad filesystem, workspace mutation, shell, network, AI, WASM, raw-op, package enable/disable, and client-side JavaScript authority remain out of scope. |
| `~/.config/clay/init.js` configuration runtime | `runtime/js/configuration.js`, `src/server/configuration.rs`, `src/server/ops/configuration.rs` | Loads user configuration, local relative modules, and documented Clay JS facades; preserves the explicit `Ctrl+O` binding separation. | Startup/configuration reload work only. | Cannot grant filesystem outside the config root, network, shell, extension loading, AI mutation, workspace mutation, package installation/enable/disable, WASM, raw ops, native widgets, direct Masonry access, raw CSS, renderer callbacks, or client-side JavaScript authority. |
| Package manifest, permissions, conflict, provenance validation | `src/packages/manifest.rs`, `src/packages/permissions.rs`, `src/packages/record.rs`, `src/packages/conflict.rs`, `src/packages/service.rs` | Validates Markdown package identity, apiPrefix, permissions, contribution descriptors (panels/components/overlays/tokens/input/state-scopes/layout-overrides/options), payload budgets, and deterministic conflict handling before activation. | Package enable/load/reload work only; never Masonry paint/layout/pointer/scroll/key/text-event work. | Package-owned IDs and option keys must be package-prefixed; prohibited authorities include filesystem, network, shell, AI, WASM, raw ops, native widget, client JS, package-manager execution, and package enable/disable mutation. |
| Clay JS API inventory, docs registry, generated registry | `docs/reference/clay-js-api/api-inventory.toml`, `docs/reference/clay-js-api/`, `docs/generated/clay-js-api-registry.json`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`, `tests/rust_visibility_api_mapping.rs` | Records which generic primitives are runtime-backed vs planned; a public API is not callable until facade/op/docs/index/generated-registry/test coverage exists. | Documentation/build/test work, not runtime hot-path work. | Raw Rust paths, raw ops, Masonry names, and protocol DTOs are not public API names; planned rows must not imply implemented facades or ops. |
| Structural observability | `src/masonry_sdui.rs`, `src/masonry_shell.rs` | Verifies installed panels, overlays, slots, action hit regions, accessibility roles, and privacy. | Test/agent inspection and explicit update work only; not a public live UI query API. | Observations omit document text, secrets, native handles, Masonry widget IDs, raw action payload authority, raw CSS, raw ops, callbacks, executable package code, and unbounded state payloads. |

## Markdown Needs Mapped to Generic Primitives

```text
Markdown need                            -> Generic primitive (status)
- Main editor placement                  -> PaneSlotLayout.main (implemented)
- Optional preview panel                 -> PanelContribution targeting `right` slot (implemented)
- No default side panel                  -> Do not publish PanelContribution by default (configuration choice)
- Mode classification                    -> DocumentClassification (implemented)
- Major-mode activation + behavior       -> MajorModeActivation + BehaviorManifest (implemented)
- Package commands and key bindings      -> CommandDeclaration + behavior-manifest keymaps (implemented)
- Client-first editor rules              -> ContinueLineMarkers / PairRule / PreserveFenceBodyIndent (implemented)
- Background parse handler               -> serverRegisterParseHandler (implemented)
- Syntax decorations                     -> serverPublishDecorations (implemented)
- User configuration override            -> setPackageOption / serverSetLayoutOverride (implemented)
- Theme tokens for preview styling       -> PackageThemeTokenDeclaration + ThemeTokenResolver (implemented)
- Selected-file open activation          -> bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog") + open_selected_file (implemented)
- One-line end-user package loading      -> loadPackage("@clay/markdown") (implemented by Plan 029)
```

Every Markdown need now maps onto an implemented generic primitive, including one-line end-user package loading, which Plan 029 closed by shipping the constrained first-party `loadPackage("@clay/*")` resolver. No Markdown-specific Rust editor/parser/render/shell branch is required.

## Generic Phase 18.5 Primitive Gaps — `loadPackage` (closed by Plan 029)

> **Status (2026-06-16): CLOSED.** Plan 029 (`plans/029-Phase18.6-Generic-Package-Loader-and-First-Party-Module-Bridge.md`) implemented the constrained first-party `loadPackage("@clay/*")` resolver and promoted `clay.packages.loadPackage` to a runtime-backed Clay JS API. The end-user default is `await loadPackage("@clay/markdown")`. The historical analysis below is retained as the security rationale for package-root confinement and for why the resolver must not become Markdown-specific; its first-party-only scope is superseded by the unified package authority decision.

### `loadPackage("@clay/markdown")` — generic one-line package specifier resolver

The former generic gap blocking the Markdown replan was the absence of a generic end-user package loader that resolves an installed package specifier, reads its declared `loadEntry` (e.g., `./dist/load.js`), enables the package through the existing `PackageService` validation path, and executes the package-owned load entry against the validated `PackageRecord`. Plan 029 closed this gap.

Before Plan 029, only `serverLoadPackage(packageJson)` existed. It required the caller to construct the inline package manifest object — exactly what the fixture `init.js` files at `tests/fixtures/configuration/markdown-mode/init.js` and `tests/fixtures/configuration/windows-markdown-open/init.js` still do for dev validation, and exactly what Plan 023 removes from ordinary end-user config. The Markdown package's own `loadMarkdownPackage(clay, options)` entry already encapsulates mode pattern registration, major-mode activation, command registration, and parse handler registration; Plan 029 added the generic resolver that invokes it from a one-line specifier.

Candidate public API target: `clay.packages.loadPackage` / `loadPackage("@clay/markdown")`, exported from `runtime/js/packages.js` (or a new `clay:packages` facade entry) and backed by an op such as `op_clay_packages_load_package_by_specifier` in `src/server/ops/packages.rs` plus a specifier resolver in `src/packages/service.rs`.

Plan 029 deliberately implemented a constrained `@clay/*` resolver for Phase 18.6 by resolving against the package distribution directory, validating the manifest through the existing `PackageService`, and executing the declared `loadEntry` with the `clay` facade object. That implementation limit is superseded by the unified package authority decision: Plan 035 replaces it with source-aware loading for user-authorized packages while preserving package-root confinement and package-manager/runtime separation.

Plan 029 implemented the safe constrained path, so no temporary fallback is the primary end-user setup. The package-owned `markdownLoadMode()` entry remains available as a fallback/test-only alias for per-load package options:

```js
import { markdownLoadMode } from "@clay/markdown";
await markdownLoadMode();
```

The alias must still consume implemented generic primitives (`serverLoadPackage`, `serverRegisterModePattern`, `serverActivateMajorMode`, `serverRegisterCommand`, `serverRegisterParseHandler`) internally rather than fixture-only copied manifests or inline package objects in user `init.js`. Either way, the end-user `init.js` must be concise and the explicit `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` separation is preserved.

No Markdown-specific loader primitive is proposed. The Markdown package publishes a generic `loadEntry` (`./dist/load.js`) that any future `loadPackage` resolver can invoke; the Markdown package does not require a `MarkdownLoader`, `LoadMarkdown`, or any `if package == "@clay/markdown"` Rust branch.

## Hot-Path Classification

| Work category | Phase 18.5 examples | Allowed timing |
| --- | --- | --- |
| Configuration/load time | `~/.config/clay/init.js` load, module resolution, `loadPackage` specifier resolution, package manifest validation, contribution descriptor parsing, package option schema validation | Startup/configuration load/reload only. |
| Package validation time | API dependency checks, permission validation, payload budget checks, provenance and conflict diagnostics | Package install/enable/load/reload only. |
| Explicit command/UI update time | `Ctrl+O` dialog, selected-file grant application, panel show/hide, action intent emission | User action, server-routed command, or explicit UI/config update paths; never synchronous typing paint. |
| Background parse/decor time | markdown-it parse handler execution, decoration span publication, viewport-prioritized scheduling | Background work outside typing/paint/layout/scroll/text-event handlers; per-document cancellation; stale-result rejection. |
| Behavior-manifest update work | client-first Markdown editor rule installation, command/keymap install | Server publication/client install outside ordinary keypress execution; keypress/text-event reads installed manifest data only. |
| Editor hot-path work | keypress, text-event, pointer selection, scroll, caret movement, local edit application, first local paint after input | Rust-known/client-first; no package JavaScript, package validation, package parsing, configuration evaluation, blocking IPC, full-document serialization, raw ops, or package-authored native widget mutation. |

The no-hot-path-package-JS rule is preserved: every primitive listed above is startup/load/configuration/explicit-update/background work, not Masonry paint/layout/input/text-event work.

## Security and Authority Boundaries

- Slot targets (`main`, `left`, `right`, `top`, `bottom`) are validated by `PaneSlotLayout` and the package UI registry. Unknown slots, invalid ratios/sizes, duplicate fixed-slot claims, and stale updates are rejected. Direct Masonry widget mutation, raw CSS, raw ops, and client-side JavaScript are non-authoritative.
- Panel IDs, component IDs, transient overlay IDs, theme token IDs, input contribution IDs, UI state scope IDs, layout override IDs, and package option keys must be package-prefixed or Clay-owned. Duplicate IDs and cross-package conflicts are deterministic and provenance-preserving.
- Action targets must resolve to registered commands. Unregistered actions, executable action arguments, callbacks, raw op names, native handles, arbitrary filesystem paths, network/shell/AI/WASM authority, and oversize arguments are rejected.
- Theme tokens must use the package prefix and same-type core-token fallbacks. Unknown tokens, type-incompatible fallbacks/remaps, raw CSS, raw style strings, raw colors outside typed token contracts, renderer callbacks, and direct native style mutation are rejected.
- State scopes must be declared before state affects shell behavior. Hidden globals, hidden state keys, unsupported scopes, unbounded payloads, persisted workspace/document mutation without explicit permissions, package/user override bypass attempts, and state that smuggles native handles or executable code are rejected.
- Layout overrides and package options must flow through documented Clay JS APIs from `~/.config/clay/init.js` or validated package defaults. Hidden JSON/TOML/ad hoc layout, style, input, theme, or package option keys are not configuration surfaces and remain rejected.
- Package loading must keep package-manager execution, registry fetching, and runtime execution as separate authority boundaries. The current constrained `@clay/*` resolver reuses the existing `PackageService` validation path and executes the declared `loadEntry` against the validated `PackageRecord`; Plan 035 generalizes that path to source-aware user-authorized packages.
- Payload ceilings and provenance diagnostics include package name, package version, apiPrefix, primitive category, contribution ID, slot/component/pane/action/state/token/option target, payload size, source, failed precedence rule, and failed validation rule.
- None of these primitives grant filesystem (outside already-open document content and the config root), network, shell, extension loading, AI mutation, workspace mutation, package-control, package-manager execution, WASM, raw `Deno.core.ops`, native widget, direct Masonry widget, renderer callback, raw CSS, or client-side JavaScript authority merely by being loaded; user-approved package capabilities are required when those APIs exist.

## Rejected Implementation Shapes

- Do not add `MarkdownLoader`, `MarkdownLoadEntry`, `MarkdownSidebar`, `MarkdownPreviewPanel`, `MarkdownModeDefault`, `MarkdownPanelVisibility`, `MarkdownPaneSelector`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust editor/parser/render/shell branch.
- Do not implement the one-line loader as a Markdown-specific resolver. The loader must be a generic `loadPackage(specifier)` that any enabled user-authorized package can consume; Markdown only publishes its `loadEntry`.
- Do not keep the inline package manifest object (`const markdownPackage = { ... }`), the manual per-facade registration imports (`serverRegisterModePattern`, `serverActivateMajorMode`, `serverRegisterCommand`, `serverRegisterParseHandler`, `serverPublishDecorations`), or the fixture-only `publishTree` call as the documented end-user `init.js` path. These remain fixture-only or move behind a generic `loadPackage` resolver.
- Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, native handles, event callbacks, focus callbacks, layout callbacks, Vello callbacks, Parley callbacks, renderer callbacks, or raw op names as package APIs.
- Do not run package validation, package parsing, configuration evaluation, JavaScript execution, blocking IPC, full-document serialization, or child mutation from Masonry paint/layout/pointer/scroll/key/text-event handlers.
- Do not treat hidden config keys, raw CSS, raw style strings, raw colors, or arbitrary JSON state blobs as temporary package authoring APIs.
- Do not promote `clay.packages.loadPackage` (if implemented) by wiring only an inventory row or raw op. Public callable APIs require facade, op wrapper, server validator, reference docs, docs-index links, generated registry coverage, API inventory metadata, tests, and Rust visibility mapping.
- Do not present `serverLoadPackage(packageJson)` as the ordinary end-user one-line setup. It remains a validation helper/gap; the default end-user path is `loadPackage("@clay/markdown")` or, while deferred, a documented package-owned fallback entry.

## Implementation Plan Summary

1. Preserve the explicit one-line package loading convention and the separate explicit `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` user configuration.
2. Confirm that every Markdown end-user need maps to an existing generic primitive: `PaneSlotLayout.main` for the editor, optional `PanelContribution` for the preview, `MajorModeActivation` + `DocumentClassification` for mode activation, `CommandDeclaration` + behavior-manifest keymaps for commands, `serverRegisterParseHandler` for parsing, `serverPublishDecorations` for decorations, `setPackageOption` / `serverSetLayoutOverride` for user overrides.
3. ~~Implement a constrained first-party `loadPackage("@clay/*")` resolver if it can be done safely within the phase; otherwise document the gap in a decision log and ship a package-owned fallback entry that consumes generic primitives internally.~~ **Done by Plan 029**: the constrained first-party `loadPackage("@clay/*")` resolver is implemented and runtime-backed; `markdownLoadMode()` remains as a package-owned fallback/test-only alias.
4. Update the Markdown package load path so it does not publish a default fixed or transient panel; keep the optional preview as a `PanelContribution` with `defaultVisibility: "hidden"` invoked only by an optional helper.
5. Simplify the fixture `init.js` files to use the default load path and remove inline package manifests and `publishTree` calls from the end-user-facing baseline.
6. Update `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md`, the Markdown package, package docs, authoring guide, configuration audit, Clay JS API inventory, tests, roadmap follow-ups, and wiki to reflect the generic-primitive consumption.

## Verification

- Inventory reviewed: shell `WorkingAreaLayout` / `PaneSplitTree` / `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, `PackageOwnedConfiguration`, `serverLoadPackage`, `MajorModeActivation`, `DocumentClassification`, `CommandDeclaration`, behavior manifests, command registry, decoration transport, parse coordinator, and selected-file open activation.
- Markdown-to-generic mapping reviewed: every Markdown need except one-line end-user loading maps to an implemented generic primitive; no Markdown-specific Rust branch is required.
- Generic gap review: the former generic gap (`loadPackage("@clay/markdown")` as a one-line specifier resolver) is closed by Plan 029; it remains a generic primitive and must not become a Markdown-specific loader.
- Hot-path review: configuration/load time, package validation time, explicit command/UI update time, background parse/decor time, behavior-manifest update time, and editor hot-path work are classified separately; package JavaScript, package validation, configuration evaluation, package parsing, raw IPC waits, full-document serialization, and package-authored native widget mutation stay out of Masonry hot paths.
- Security review: validation/authority boundaries are recorded for slot targets, panel IDs, component IDs, action targets, theme tokens, state scopes, layout overrides, package options, package permissions, and the deny-by-default package loading boundary.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Command: `cargo test --test protocol primitives_docs:: --quiet`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.1 Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md)
- [Phase 18.2 Shell Runtime Primitive Review](phase18.2-shell-runtime-primitive-review.md)
- [Phase 18.3 Slot-Aware Package UI Primitive Review](phase18.3-slot-ui-primitive-review.md)
- [Phase 18.4 Input, State, and Configuration Primitive Review](phase18.4-input-state-config-primitive-review.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Command Registry](command-registry.md)
- [Package Loading](package-loading.md)
- [Configuration Runtime](configuration-runtime.md)
- [First-Party Markdown Package](first-party-markdown-package.md)
- [Markdown Mode Activation](markdown-mode-activation.md)
- [Decoration Transport](decoration-transport.md)
- [Parse Coordinator](parse-coordinator.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Package Loading Reference](../../reference/primitives/package-loading.md)
- [Package Security Reference](../../reference/primitives/package-security.md)
