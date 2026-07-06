# Configuration Runtime

## Source

- `src/server/configuration.rs`
- `src/server/js_runtime.rs`
- `src/server/mod.rs`
- `src/server/ops/configuration.rs`
- `runtime/js/configuration.ts`
- `docs/wiki/modules/package-input-state-configuration.md`
- `src/server/js_runtime.rs` tests

## Overview

Clay can now evaluate a constrained local configuration entry point from a configuration root containing `init.js`. Server startup loads the default `~/.config/clay/init.js` when it exists, and tests can supply explicit fixture roots. The runtime supports the documented `clay:configuration` facade, local relative `.js` modules under the same configuration directory, and read-only configuration state for the entry point and loaded modules.

## Responsibilities

- Treat `~/.config/clay/init.js` as the default user configuration entry point while allowing tests to provide an explicit configuration root.
- Resolve relative local JavaScript modules without changing the process current directory or invoking shell/package loading behavior.
- Reject URLs, package specifiers, absolute paths, extensionless imports, and traversal outside the configuration root.
- Expose `loadConfigurationModule({ path })` and `getConfigurationState()` through Clay-owned ops, not raw user-facing op calls.
- Runtime-back `setPackageOption` for Phase 18.4 package-owned options while preserving `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as explicit planned `clay:configuration` facade exports rather than ad hoc settings.
- Runtime-back Phase 18.4 package layout overrides through `clay.ui.serverSetLayoutOverride` while preserving hidden split/slot/panel/style keys as rejected; lower-level working-area/pane mutation APIs and package enable/disable through configuration remain planned.

## How It Works

`ClayJsRuntimeService::load_configuration_from_root` runs on the same blocking runtime worker used by controlled JavaScript evaluation. It constructs a `ConfigurationRuntime` from the supplied root, canonicalizes `init.js`, creates a file URL for that entry point, and installs both `ClayOpState` and the configuration state in `deno_core::OpState`.

`ClayModuleLoader` handles three allowed module families:

1. The main `init.js` file under the configuration root.
2. The built-in `clay:configuration` facade source.
3. Explicit relative `.js` modules that canonicalize under the configuration root.

The facade validates `loadConfigurationModule({ path })` through `op_clay_configuration_load_module` before using dynamic `import(path)`. The module loader performs the authoritative canonical path check again when resolving/loading the module, reads the file directly with Rust filesystem APIs, and records successfully loaded local modules in deterministic first-load order. `getConfigurationState()` returns JSON from `op_clay_configuration_get_state`, which the facade parses into `{ entryPoint, loadedModules }`.

The Phase 16.5 primitive gate reviewed package options, mode preferences, decoration theme preferences, and parse policy preferences but did not implement concrete behavior-changing settings. Phase 18.4 promotes `setPackageOption` to a runtime-backed API for package-owned options while keeping `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as planned-unavailable APIs routed through `op_clay_runtime_unavailable`. This keeps `~/.config/clay/init.js` as the startup/configuration-change configuration entry point while preventing undocumented keys or package enable/disable authority from appearing without a dedicated server validator.

The Phase 18.2 shell runtime added internal `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, inert update, and observability state, but it did not add a user-visible shell configuration API. Phase 18.3 adds `runtime/js/ui.ts` and server runtime support for `clay:ui` package declaration APIs (`serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`). Historical Phase 18.3 boundary text remains: `clay:ui` contribution APIs exist for package declarations, no `clay:ui` configuration override API or hidden split/slot/panel/style key system existed yet, and they were not user-visible configuration override APIs for default slots, panel visibility, component style overrides, theme-token remapping, or layout behavior. The historical inventory note was that `clay.ui.serverSetLayoutOverride` and `clay.configuration.setPackageOption` stay non-registry-public inventory rows until promotion. Phase 18.4 adds runtime-backed `serverRegisterInputContribution`, `serverRegisterUiStateScope`, and `serverSetLayoutOverride`. These APIs validate package-owned panel defaults, component style variables, fixed slots, overlays, semantic theme token declarations, input/focus/action metadata, UI state schema/lifecycle metadata, and user/package layout/theme/input/action override records at package-load/configuration/update time. `serverRegisterUiStateScope` is not state-value mutation work, and `serverSetLayoutOverride` is not a low-level pane/working-area mutation API.

Hidden JSON/TOML/ad hoc package UI configuration keys remain rejected; in lowercase policy wording, hidden JSON/TOML/ad hoc keys remain rejected. Examples such as `preview.position`, `preview.defaultVisibility`, `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `theme.markdown.heading.1`, `theme.markdown.preview.background`, `markdown.sidebar.width`, raw token override keys, and ad hoc style keys are invalid unless expressed through documented Clay JS APIs with `custom_properties`, allowed values, defaults, errors, security notes, and registry coverage. Phase 18.4's documented APIs use option names such as `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback` through `setPackageOption`, or layout override properties such as `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback` through `serverSetLayoutOverride`.

Phase 18.5 (plan `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`) replans Markdown end-user loading on top of these generic primitives. Its task-8 configuration audit confirms every Markdown-relevant behavior-changing surface is either a runtime-backed Clay JS API or an explicitly planned/unavailable API: Markdown package options and theme-token/layout overrides go through the same `setPackageOption`, `serverSetLayoutOverride`, `serverRegisterThemeToken`, `serverRegisterPanelContribution`, `serverRegisterInputContribution`, and `serverRegisterUiStateScope` APIs already promoted in Phase 18.3/18.4; `setModePreference`, `setDecorationTheme`, `setParsePolicy`, and `loadPackage` remain planned. The Markdown preview defaults to `defaultVisibility: "hidden"` through `serverRegisterPanelContribution` rather than a hard-coded side panel or hidden key. No Markdown-specific configuration validator, hidden-key system, or package-specific Rust configuration branch was added.

## Code Examples

```js
// ~/.config/clay/init.js
import { getConfigurationState, loadConfigurationModule } from "clay:configuration";

await loadConfigurationModule({ path: "./ui.js" });
console.log(getConfigurationState().loadedModules);
```

```rust
let service = ClayJsRuntimeService::default();
let result = service.load_configuration_from_root(config_root).await?;
```

## Invariants and Constraints

- Configuration JavaScript runs server-side only; the native client never executes arbitrary configuration JavaScript.
- Module loading is startup/reload work and must stay off Masonry paint, text-event, and ordinary edit acknowledgement hot paths.
- Only explicit relative `.js` files under the configuration root are loadable. No network, npm/jsr/package, shell, workspace scan, extension loading, WASM, AI mutation, or direct client filesystem authority is introduced.
- `loadConfigurationModule` does not implement Deno/npm-style resolution: callers must provide the exact `.js` filename.
- Runtime-backed package option/layout override APIs do not grant package installation, enable/disable mutation, mode activation authority, decoration rendering authority, parse-document authority, component style override authority beyond typed tokens, raw Deno ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, client-side JavaScript, or external filesystem/network/shell/AI/workspace access. Planned package/mode/parse/decor configuration exports remain unavailable stubs until documented validators ship.
- **Phase 18.9 mode and generic key-behavior defaults are hardcoded structural defaults, not runtime-configurable Clay JS settings.** The `core.text`/`core.code` fallback modes, the classification precedence ladder, the electric-character outdent set, and the generic pair-insertion/comment-continuation rule sets are compiled into `ModeRegistry`/`EditorBehaviorRules` and are never read from configuration at mode-activation or paint/text time (`src/packages/modes.rs` and `src/editor/surface.rs` consult `behavior_manifest` only, never the package-option store). Because `setPackageOption` uses a closed suffix allowlist, behavior-changing Phase 18.9 keys such as `core.preferredFallbackMode`, `core.electricCharacters`, `core.pairInsertion`, and `core.commentContinuation` are rejected as unsupported options rather than silently accepted as undocumented settings, so built-in mode defaults cannot be overridden to grant package authority. Users who want different classification/behavior should register a package mode or behavior manifest through the documented Clay JS APIs, not an undocumented configuration key. (See `phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected` in `src/server/configuration.rs`.)
- **Phase 18.12 workspace file-browser defaults are Clay-owned runtime constants or existing command bindings, not new `clay:configuration` APIs.** Fuzzy-open and file-browser toggle chords use the existing `clay.keybindings.bindKey` API with fixed command ids (`clay.workspace.openFuzzyFile`, `clay.workspace.toggleFileBrowser`) and no Rust default chord. The left panel/default slot, bounded marker set (`KNOWN_PROJECT_MARKERS`), default ignore names, listing depth/count budgets, left-panel item budget, and fuzzy item budget are compiled Clay-owned workspace/file-browser boundaries. They are not hidden `init.js` keys and cannot grant extra filesystem/workspace authority. Public programmatic file-browser work goes through the Phase 18.12 `clay:workspace` and `clay:commands` facades documented in `docs/reference/clay-js-api/`, while configuration remains limited to existing documented APIs.

## Tests

- `src/server/js_runtime.rs`: loads an `init.js` fixture, loads `./ui.js` via `loadConfigurationModule`, reports entry/module state, rejects traversal/URL/npm/package-style specifiers, and verifies planned package/mode configuration facade exports return clear unavailable errors.
- `src/server/configuration.rs`: `package_option_configuration_accepts_supported_typed_options_only`, `package_option_configuration_rejects_hidden_ad_hoc_and_raw_authority_keys`, and the Phase 18.9 `phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected` test pin the closed package-option allowlist (no ad-hoc or behavior-changing core.* keys accepted).
- Command: `cargo test js_runtime --quiet && cargo test configuration_runtime --quiet`

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- [Protocol and Performance Pattern](../../../.agents/skills/project-patterns/references/protocol-and-performance.md)
- `docs/reference/clay-js-api/configuration.md`
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
