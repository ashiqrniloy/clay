# Phase 18.4 Input, State, and Configuration Primitive Review

## Source

- `plans/027-Phase18.4-Package-Input-Actions-State-Data-and-Configuration-Integration.md`
- `.agents/skills/create-plan/references/clay.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/wiki/modules/phase18.1-shell-layout-primitive-review.md`
- `docs/wiki/modules/phase18.2-shell-runtime-primitive-review.md`
- `docs/wiki/modules/phase18.3-slot-ui-primitive-review.md`
- `docs/wiki/modules/masonry-shell.md`
- `docs/wiki/modules/slot-aware-package-ui.md`
- `docs/wiki/modules/server-driven-ui.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/configuration-runtime.md`
- `docs/wiki/modules/primitive-architecture.md`
- `runtime/js/ui.ts`
- `runtime/js/configuration.ts`
- `src/server/ui.rs`
- `src/server/ops/ui.rs`
- `src/server/ops/configuration.rs`
- `src/shell/layout.rs`
- `src/shell/package_ui.rs`
- `src/shell/components.rs`
- `src/shell/theme.rs`
- `src/masonry_sdui.rs`
- `src/packages/record.rs`
- `src/packages/conflict.rs`
- `docs/reference/clay-js-api/api-inventory.toml`
- `.agents/skills/project-patterns/references/mode-primitive-first.md`
- `.agents/skills/project-patterns/references/package-ui-layout.md`
- `.agents/skills/project-patterns/references/behavior-manifests.md`
- `.agents/skills/project-patterns/references/configuration-system.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`

## Overview

This page records the Phase 18.4 primitive-first review before implementing package input declarations, component-scoped actions, UI state/data scopes, layout overrides, package options, and user/package configuration precedence. Phase 18.1 defined the Clay-owned shell vocabulary, Phase 18.2 implemented internal `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout`, and Phase 18.3 promoted slot-aware package UI declarations for panels, components, overlays, and theme tokens. Phase 18.4 should now close the remaining generic state/config/input gaps without adding Markdown-specific, package-specific, or Masonry-specific Rust behavior.

The review confirms that existing behavior manifests, keybindings, command/action registry, SDUI/component catalog, shell slot state, package UI registry/runtime state, manifest contribution metadata, configuration runtime, documentation registry, and structural observability are reusable building blocks. They can support bounded inert declarations and explicit command/UI updates, but they do not yet expose package input interests, declared UI state scopes, user/package layout override precedence, or typed package options as runtime-backed public APIs.

## Existing Package Primitive Inventory

| Primitive area | Current source paths | What Phase 18.4 can reuse now | Runtime classification | Security and validation boundary |
| --- | --- | --- | --- | --- |
| Behavior manifests and text/key routing | `src/behavior/manifest.rs`, `src/protocol/mod.rs`, `src/editor/surface.rs`, `runtime/js/keybindings.ts`, `docs/wiki/modules/behavior-manifests.md` | Installed behavior manifests already express client-first predictable key/text transforms and server-routed command keybindings. Package input work should leave keypress and text-event semantics on this route. | Behavior-manifest update work for install/publication; editor hot-path work reads Rust-known manifest data only. | No arbitrary client JavaScript, executable callbacks, raw ops, or server round trip before local typing paint. Package commands cannot become `ClientFirstPredictable` arbitrary handlers. |
| Keybindings | `runtime/js/keybindings.ts`, `src/server/ops/keybindings.rs`, `src/packages/commands.rs`, `docs/wiki/modules/command-registry.md` | Key declarations can continue to bind registered commands and preserve routing policy/provenance. Non-key pointer/focus interests should not overload keybinding metadata. | Package load/configuration work for registration; editor keypress hot path reads installed inert routing data only. | Ambiguous keybindings, unknown command targets, unregistered actions, and package attempts to bypass behavior manifest policy are rejected. |
| Command registry and action intents | `runtime/js/commands.ts`, `src/server/ops/commands.rs`, `src/packages/commands.rs`, `src/protocol/sdui.rs`, `docs/wiki/modules/command-registry.md` | `SduiActionIntent` and package command metadata provide the reusable action target authority model for component-scoped clicks, buttons, menus, and panel actions. | Load/activation work for command metadata; explicit command/UI update work for user action execution. | Action targets must resolve to registered commands; arguments must be bounded primitive data and cannot smuggle callbacks, raw op names, native handles, arbitrary filesystem paths, network/shell/AI/WASM authority, or executable code. |
| SDUI and Clay component catalog | `runtime/js/sdui.ts`, `src/protocol/sdui.rs`, `src/server/sdui.rs`, `src/server/ops/sdui.rs`, `src/shell/components.rs`, `src/masonry_sdui.rs`, `docs/wiki/modules/server-driven-ui.md` | Existing component schemas, action regions, typed style variables, accessibility role mapping, and native renderer bridge can host component-scoped input/action metadata as inert data. | Package load/config/update work for validation; protocol/client update work for install; paint/layout state read for native composition. | Component nodes are not Masonry widgets. Validators reject raw CSS, raw colors outside typed token contracts, native handles, renderer callbacks, client-side JavaScript, raw `Deno.core.ops`, unsupported component kinds, unregistered actions, and oversize payloads. |
| Shell `PaneSlotLayout` and internal shell runtime | `src/shell/layout.rs`, `src/masonry_shell.rs`, `docs/wiki/modules/masonry-shell.md` | The mandatory `main` slot plus optional fixed `left`, `right`, `top`, and `bottom` slots provide geometry for panel visibility/default-slot and later layout override work. | Startup/update work for layout state; Masonry layout/paint read installed state only. | Shell safety preserves at least one pane and one `main` slot, rejects invalid ratios/sizes/stale updates, and keeps native widget IDs, direct Masonry mutation, raw CSS, raw ops, and client JS internal/non-authoritative. |
| Package UI registry and runtime state | `runtime/js/ui.ts`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/package_ui.rs`, `src/shell/theme.rs`, `docs/wiki/modules/slot-aware-package-ui.md` | Runtime-backed `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken` already validate package provenance, action targets, typed theme tokens, fixed-slot panels, overlays, and bounded component trees. | Package load/config/update work for registration; explicit UI update work for show/hide/open/dismiss when added; paint/layout state read for fixed panel/overlay composition. | Current gaps are state scopes, user overrides, package options, and richer input interests. Existing validators already reject duplicate IDs/slots/tokens, unregistered actions, raw CSS/native handles/raw ops/client JS, and payload over budget. |
| Package manifest and contribution metadata | `src/packages/manifest.rs`, `src/packages/permissions.rs`, `src/packages/record.rs`, `src/packages/conflict.rs`, `docs/wiki/modules/package-loading.md` | Package records retain name/version/apiPrefix/docs/dependencies and Phase 18.3 UI contribution descriptors. The same pattern should add input, UI state scopes, package options, and layout override metadata before activation. | Package enable/load/reload work only; never Masonry paint/layout/pointer/scroll/key/text-event work. | Package-owned IDs and option keys must be package-prefixed; conflicts are deterministic and provenance-preserving; prohibited authorities include filesystem, network, shell, AI, WASM, raw ops, native widget, client JS, package-manager execution, and package enable/disable mutation. |
| Configuration runtime and planned configuration APIs | `runtime/js/configuration.ts`, `src/server/configuration.rs`, `src/server/ops/configuration.rs`, `src/server/js_runtime.rs`, `docs/wiki/modules/configuration-runtime.md`, `docs/reference/clay-js-api/configuration.md` | `~/.config/clay/init.js` and local relative module loading are implemented. `setPackageOption`, `setModePreference`, `setDecorationTheme`, and `setParsePolicy` are discoverable planned-unavailable Clay JS facade exports. | Startup/configuration reload/explicit setting-change work only. | Configuration cannot grant filesystem outside the config root, network, shell, extension loading, AI mutation, workspace mutation, package installation/enable/disable, WASM, raw ops, native widgets, direct Masonry access, raw CSS, renderer callbacks, or client-side JavaScript authority. |
| Clay JS API inventory and docs registry | `docs/reference/clay-js-api/api-inventory.toml`, `docs/reference/clay-js-api/`, `docs/generated/clay-js-api-registry.json`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`, `tests/rust_visibility_api_mapping.rs` | Phase 18.4 now marks `clay.ui.serverRegisterInputContribution`, `clay.ui.serverRegisterUiStateScope`, `clay.ui.serverSetLayoutOverride`, and `clay.configuration.setPackageOption` runtime-backed and registry-public; working-area, pane-split, and direct pane-slot mutation rows remain planned. | Documentation/build/test work, not runtime hot-path work. | A public API is not callable until facade/op/docs/index/generated-registry/test coverage exists. Raw Rust paths, raw ops, Masonry names, and protocol DTOs are not public API names. |
| Structural observability | `src/masonry_sdui.rs`, `src/masonry_shell.rs`, `docs/wiki/modules/server-driven-ui.md`, `docs/wiki/modules/masonry-shell.md` | Existing observations can verify installed panels, overlays, slots, action hit regions, accessibility roles, and privacy. Phase 18.4 should extend observability only with structural input/state/config metadata where useful. | Test/agent inspection and explicit update work only; not a public live UI query API. | Observations omit document text, secrets, native handles, Masonry widget IDs, raw action payload authority, raw CSS, raw ops, callbacks, executable package code, and unbounded state payloads. |

## What Existing Primitives Can Achieve Before New Phase 18.4 Code

- Packages can already declare package-prefixed commands, keybindings, behavior manifest metadata, SDUI trees, fixed panels, component trees, transient overlays, and typed theme tokens through server-side validation.
- Fixed panels can compose into Clay-owned `PaneSlotLayout` geometry and transient overlays can render separately without consuming fixed slot geometry.
- Component/button/list action regions can emit registered command intents without executing package code in the Rust client.
- Configuration can load `~/.config/clay/init.js` and local modules, but behavior-changing package/layout/state settings remain planned unavailable API stubs.
- Package records can preserve provenance, dependencies, contribution counts, and conflict diagnostics for existing primitive categories.
- Documentation registry tests can ensure any promoted API includes facade path, op path, backing Rust path, custom properties, key bindings, docs-index links, generated registry entries, lookup tags, and security notes.

These surfaces still do not provide non-key package input interests, component-scoped focus/mouse policy beyond overlay focus/dismissal metadata, declared `PackageUiStateScope` lifecycles, user/package `PackageLayoutOverride` precedence, typed package option values, theme-token remaps, hidden-key rejection diagnostics for concrete options, or a one-line end-user package loading path for ordinary package defaults.

## Generic Phase 18.4 Primitive Gaps

### `PackageInputContribution`

Phase 18.4 promoted the standalone package input surface as `clay.ui.serverRegisterInputContribution` / `serverRegisterInputContribution`. The implementation adds the facade, op wrapper, server validator, API inventory row, public Markdown API docs, `docs/index.md` registry link, generated registry entry, runtime `PackageInputRouting` state, and tests. It remains bounded to inert pointer/focus/selection/action metadata; key routing stays on behavior manifests/keybindings.

The generic input declaration should describe non-key input interests for component, pane, or transient-overlay scopes: pointer click interests, hover/menu hints if needed, mouse selection and drag policies, focus restore/trap/none policies, active mode/component context conditions, and target action IDs. It should compose with behavior manifests for key/text behavior and with the command registry for side effects.

Implementation targets are generic: `runtime/js/ui.ts` or `runtime/js/input.ts`, `src/server/ops/ui.rs` or `src/server/ops/input.rs`, `src/server/ui.rs`, `src/shell/components.rs`, `src/shell/package_ui.rs`, `src/masonry_sdui.rs`, `src/packages/record.rs`, and `src/packages/conflict.rs`. Do not add raw native event callbacks, package-owned pointer handlers, client-side JavaScript, or Masonry event branches.

### Component-scoped action and focus metadata

Component-scoped action and focus metadata should reuse existing `actionTargets`, `UiActionIntent`, registered command IDs, `focusPolicy`, `dismissalPolicy`, component IDs, and structural observability where possible. The implementation plan treats component-scoped action and focus metadata as generic reusable package UI data, not a mode-specific branch. The implementation should add generic fields only when they are reusable by future packages/modes, for example `input.scope`, `pointer.clickAction`, `pointer.dragPolicy`, `focus.policy`, or `context.mode`. The exact field names should be recorded in public API docs when promoted.

Button/list/panel/overlay actions remain explicit command/UI update work. Editor key/text behavior remains behavior-manifest update work and client-first editor hot-path work, not package input callback work.

### `PackageUiStateScope`

Phase 18.4 promoted `clay.ui.serverRegisterUiStateScope` / `serverRegisterUiStateScope` as a runtime-backed inert state-scope declaration primitive before adding mutable/persisted state behavior. Supported scope vocabulary stays generic: `package-global`, `user-config`, `workspace`, `document`, `pane`, `component`, and `transient-overlay`. Each scope declaration records package-prefixed ID/key, owner (`package`, `shell`, or `server`), lifetime (`session`, `workspace`, `document`, or `transient`), persistence (`none`, `client-local`, `server-canonical`, or `deferred`), implementation status (`implemented` or `deferred`), value schema, payload budget, provenance, and diagnostics.

Runtime storage should be added only for the minimum shell/UI state the phase implements. Deferred persistence, workspace/document mutation, pane selector syntax, cross-window sync, and multi-panel ordering must be explicitly documented rather than implied.

### `PackageLayoutOverride`

Selected public API target: `clay.ui.serverSetLayoutOverride` / `serverSetLayoutOverride`.

The implementation should accept only documented `~/.config/clay/init.js` or package-default override records that target declared package panels/components/options/tokens. Candidate override properties are `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback`, but each property must be implemented only with typed validators, custom properties, precedence tests, and diagnostics.

Precedence remains: Clay shell safety invariants and hard prohibitions, user configuration through documented Clay JS APIs, active major mode layout defaults, compatible minor mode contributions, global package contributions, package fallback/defaults. Precedence never bypasses validation, permissions, action target checks, token type checks, state-scope checks, payload ceilings, or shell safety.

### `PackageOwnedConfiguration`

Selected public API target for package options: `clay.configuration.setPackageOption` / `setPackageOption`.

Package options should be available only for package-declared typed option schemas. The generic package record/configuration model should validate package prefix, option name, value type, default, allowed values, source (`init-js`, package default, or Clay default as implemented), permission (`package-configuration` when behavior-changing), and target primitive. Hidden JSON/TOML/ad hoc keys such as `preview.position`, `preview.defaultVisibility`, `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `theme.markdown.heading.1`, raw token override keys, and ad hoc style/input keys remain rejected outside documented Clay JS APIs.

### Theme-token remaps and package fallback/defaults

Reuse `PackageThemeTokenDeclaration` and `ThemeTokenResolver` for typed package tokens. Future implementation tasks should reuse `PackageThemeTokenDeclaration` and `ThemeTokenResolver` rather than adding raw style maps. User remaps should be implemented only through `setPackageOption` or `serverSetLayoutOverride` when the remap is typed, same-type, package-prefixed, documented, and tested. Raw CSS, raw style strings, raw colors outside token contracts, Vello/Parley callbacks, renderer callbacks, native widget mutation, and hidden token keys remain prohibited.

## Hot-Path Classification

| Work category | Phase 18.4 examples | Allowed timing |
| --- | --- | --- |
| Package load/config/update work | Configuration/load/update work for manifest parsing, API dependency checks, input/state/config/layout metadata validation, package option schema validation, payload budget checks, provenance and conflict diagnostics | Package install/enable/load/reload, configuration load/reload, or explicit package UI update only. |
| Behavior-manifest update work | key routing, client-first text transforms, keyboard shortcut changes, behavior version install | Server publication/client install outside ordinary keypress execution; keypress reads installed manifest data only. |
| Explicit command/UI update work | action intent emission, panel show/hide, overlay open/dismiss, focus restore/trap transition, applying validated setting changes | User action, server-routed command, or explicit UI/config update paths; never synchronous typing paint. |
| Protocol/client update work | installing versioned layout/input/state snapshots or deltas, replacing bounded component/panel/overlay state | Typed protocol/client event handling outside paint/text-event handlers. |
| Paint/layout state read | computing slot geometry, painting fixed panels/overlays, hit testing already-installed action regions, resolving already-installed theme tokens, reading bounded local state | Masonry hot paths may read cached inert state only. |
| Editor hot-path work | keypress, text-event, pointer selection, scroll, caret movement, local edit application, first local paint after input | Rust-known/client-first; no package JavaScript, package validation, package parsing, configuration evaluation, blocking IPC, full-document serialization, raw ops, or package-authored native widget mutation. |

## Security and Authority Boundaries

- Input interests are declarative and bounded. Packages cannot subscribe to raw arbitrary native events, install callbacks, receive event-loop handles, receive Masonry `WidgetId`s, mutate focus directly, or run client-side JavaScript.
- Pointer/click/focus scopes must name known component, pane, or transient-overlay scopes and package-prefixed contribution IDs. Unsupported focus scopes, mouse selection/drag policies, ambiguous routing, and stale component/pane/action provenance are rejected or disabled with diagnostics.
- Component-scoped actions must target registered commands. Unregistered actions, duplicate commands/actions, permission-incompatible routes, executable action arguments, callbacks, raw op names, native handles, arbitrary filesystem paths, network/shell/AI/WASM authority, and oversize arguments are rejected.
- State scopes must be declared before state affects shell behavior. Hidden globals, hidden state keys, unsupported scopes, unbounded payloads, persisted workspace/document mutation without explicit permissions, package/user override bypass attempts, and state that smuggles native handles or executable code are rejected.
- Layout overrides and package options must flow through documented Clay JS APIs from `~/.config/clay/init.js` or validated package defaults. Hidden JSON/TOML/ad hoc layout, style, input, theme, or package option keys are not configuration surfaces; hidden JSON/TOML/ad hoc layout, style, input, theme, or package option keys remain rejected outside documented APIs.
- Theme-token remaps must target known same-type tokens. Unknown tokens, type-incompatible fallbacks/remaps, raw CSS, raw style strings, raw colors outside typed token contracts, renderer callbacks, and direct native style mutation are rejected.
- Payload ceilings and provenance diagnostics should include package name, package version, apiPrefix, primitive category, contribution ID, component/pane/slot/action/state/token/option target, payload size, source, failed precedence rule, and failed validation rule.
- None of these primitives grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package installation/enable/disable, package-manager execution, WASM, raw `Deno.core.ops`, native widget, direct Masonry widget, renderer callback, raw CSS, or client-side JavaScript authority by default.

## Rejected Implementation Shapes

- Do not add `MarkdownPreviewInput`, `MarkdownPreviewState`, `MarkdownLayoutOverride`, `MarkdownPanelVisibility`, `MarkdownThemeOverride`, `MarkdownPaneSelector`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust input/state/config/layout branch.
- Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, native handles, event callbacks, focus callbacks, layout callbacks, Vello callbacks, Parley callbacks, renderer callbacks, or raw op names as package APIs.
- Do not implement package input by delivering raw pointer/key/text events to package JavaScript or client-side JavaScript.
- Do not run package validation, package parsing, configuration evaluation, JavaScript execution, blocking IPC, full-document serialization, or child mutation from Masonry paint/layout/pointer/scroll/key/text-event handlers.
- Do not treat hidden config keys, raw CSS, raw style strings, raw colors, or arbitrary JSON state blobs as temporary package authoring APIs.
- Do not promote `clay.ui.serverRegisterUiStateScope`, `clay.ui.serverSetLayoutOverride`, or `clay.configuration.setPackageOption` by wiring only inventory rows or raw ops. Public callable APIs require facade, op wrapper, server validator, reference docs, docs-index links, generated registry coverage, API inventory metadata, tests, and Rust visibility mapping. `clay.ui.serverRegisterInputContribution` and `clay.ui.serverRegisterUiStateScope` now satisfy that promotion checklist.

## Implementation Plan Summary

1. Keep behavior manifests/keybindings responsible for key/text routing and use command registry/action intents for side effects.
2. Add generic input/action/focus metadata only as bounded inert declarations. Phase 18.4 selected and implemented `PackageInputContribution` / `clay.ui.serverRegisterInputContribution` as the standalone API target.
3. `PackageUiStateScope` is promoted through `clay.ui.serverRegisterUiStateScope` as an inert declaration primitive; add runtime storage only for implemented shell/UI state and mark persistence/mutation semantics explicitly implemented or deferred.
4. Promote `PackageLayoutOverride` through `clay.ui.serverSetLayoutOverride` only for supported user/package overrides with deterministic precedence and validation.
5. Promote `PackageOwnedConfiguration` through `clay.configuration.setPackageOption` only for package-declared typed options and hidden-key rejection diagnostics.
6. Extend package records/conflicts/provenance for input/state/config/layout metadata and payload budgets before activation.
7. Update package guide, primitive docs, Clay JS API inventory/docs/generated registry, configuration docs, wiki pages, and deterministic tests before marking any new API runtime-backed.

## Verification

- Inventory reviewed: behavior manifests, keybindings, command/action registry, SDUI/component catalog, shell `PaneSlotLayout`, package UI registry/runtime state, package manifest/contribution metadata, configuration runtime, docs registry, and structural observability.
- Generic gap review: Phase 18.4 implements reusable `PackageInputContribution`, component-scoped action/focus metadata, and `PackageUiStateScope`; remaining generic gaps include `PackageLayoutOverride`, `PackageOwnedConfiguration`, package option schema validation, and typed theme-token remaps.
- Hot-path review: package load/config/update work, behavior-manifest update work, explicit command/UI update work, protocol/client update work, paint/layout state reads, and editor hot-path work are classified separately; package JavaScript, package validation, configuration evaluation, package parsing, raw IPC waits, full-document serialization, and package-authored native widget mutation stay out of Masonry hot paths.
- Security review: validation/authority boundaries are recorded for input interests, pointer/click/focus scopes, component-scoped actions, state scopes, layout override targets, package options, theme-token remaps, payload ceilings, provenance, package permissions, and hidden-key rejection.

## Tests

- `tests/primitives_docs.rs::phase18_4_input_state_config_review_records_existing_inventory`
- `tests/primitives_docs.rs::phase18_4_input_state_config_review_maps_generic_primitives`
- `tests/primitives_docs.rs::phase18_4_input_state_config_review_rejects_mode_specific_branches`
- `tests/clay_js_api_inventory.rs::phase18_4_clay_ui_and_configuration_api_inventory_status_matches_runtime`
- `tests/clay_js_doc_registry.rs::generated_registry_contains_phase18_4_public_apis`
- `tests/rust_visibility_api_mapping.rs::phase18_4_public_rust_surfaces_have_clay_js_mapping_or_internal_visibility`
- Command: `CARGO_TARGET_DIR=target/pi-verify cargo test --test primitives_docs --test clay_js_api_inventory --test clay_js_doc_registry --test clay_js_facade_layout --test rust_visibility_api_mapping --quiet`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.1 Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md)
- [Phase 18.2 Shell Runtime Primitive Review](phase18.2-shell-runtime-primitive-review.md)
- [Phase 18.3 Slot-Aware Package UI Primitive Review](phase18.3-slot-ui-primitive-review.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Command Registry](command-registry.md)
- [Package Loading](package-loading.md)
- [Configuration Runtime](configuration-runtime.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Package Security Reference](../../reference/primitives/package-security.md)
