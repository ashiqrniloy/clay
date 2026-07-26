# Configuration Runtime

## Source

- `src/server/configuration.rs`
- `src/server/js_runtime.rs`
- `src/server/mod.rs`
- `src/server/ops/configuration.rs`
- `src/server/ops/theme.rs`
- `src/server/ops/typography.rs`
- `runtime/js/configuration.js`
- `runtime/js/theme.js`
- `docs/wiki/modules/package-input-state-configuration.md`
- `src/server/js_runtime.rs` tests

## Overview

Clay can now evaluate a constrained local configuration entry point from a configuration root containing `init.js`. Server startup loads the default `~/.config/clay/init.js` when it exists, and tests can supply explicit fixture roots. The runtime supports the documented `clay:configuration` facade, local relative `.js` modules under the same configuration directory, and read-only configuration state for the entry point, loaded modules, and validated package-option records.

## Responsibilities

- Treat `~/.config/clay/init.js` as the default user configuration entry point while allowing tests to provide an explicit configuration root.
- Resolve relative local JavaScript modules without changing the process current directory or invoking shell/package loading behavior.
- Reject URLs, package specifiers, absolute paths, extensionless imports, and traversal outside the configuration root.
- Expose `loadConfigurationModule({ path })` and `getConfigurationState()` through Clay-owned ops, not raw user-facing op calls.
- Runtime-back `setPackageOption` for Phase 18.4 package-owned options while preserving `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as explicit planned `clay:configuration` facade exports rather than ad hoc settings.
- Accept `clay:theme.setTypography` as one complete user-owned three-profile transaction, then hand its inert candidate to server-authoritative revision/publication state.
- Runtime-back Phase 18.4 package layout overrides through `clay.ui.serverSetLayoutOverride` while preserving hidden split/slot/panel/style keys as rejected; lower-level working-area/pane mutation APIs and package enable/disable through configuration remain planned.
- Keep Plan 060/061 trust-domain identity, package provenance, connection/document authority, queue/file/process ceilings, atomic-save policy, routing, worker concurrency, native-dialog/clipboard behavior, and Cargo/CI profiles outside configuration. Real package adoption/revocation/replacement decisions use the host CLI; existing `loadPackage` only consumes approved state.

## How It Works

`ClayJsRuntimeService::load_configuration_from_root` runs on the same blocking runtime worker used by controlled JavaScript evaluation. It constructs a `ConfigurationRuntime` from the supplied root, canonicalizes `init.js`, creates a file URL for that entry point, and installs both `ClayOpState` and the configuration state in `deno_core::OpState`.

`ClayModuleLoader` handles three allowed module families:

1. The main `init.js` file under the configuration root.
2. The built-in `clay:configuration` facade source.
3. Explicit relative `.js` modules that canonicalize under the configuration root.

The facade validates `loadConfigurationModule({ path })` through `op_clay_configuration_load_module` before using dynamic `import(path)`. The module loader performs the authoritative canonical path check again when resolving/loading the module, reads the file directly with Rust filesystem APIs, and records successfully loaded local modules in deterministic first-load order. `getConfigurationState()` returns JSON from `op_clay_configuration_get_state`, which the facade parses into `{ entryPoint, loadedModules }`.

Phase 18.16.5 adds [`setTypography`](../../reference/clay-js-api/theme/set-typography.md) to the existing `clay:theme` facade rather than creating an undocumented setting system. The op accepts exactly `monospace`, `proportional`, and `ui` objects, each with only `families` and `size`; it builds one `ActiveTypography` candidate, validates every bounded fallback stack and size before replacing runtime-local state, and returns its evaluation-local revision. `IpcServer` revalidates the successful candidate, assigns the persistent server revision only when profiles differ, and broadcasts one bounded `ActiveTypography` update. No configuration call discovers installed fonts, reads font files, fetches URLs, or grants package/network/shell authority. Client bootstrap installation is owned by the following client-registry task.

The Phase 16.5 primitive gate reviewed package options, mode preferences, decoration theme preferences, and parse policy preferences but did not implement concrete behavior-changing settings. Phase 18.4 promotes `setPackageOption` to a runtime-backed API for package-owned options while keeping `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as planned-unavailable APIs routed through `op_clay_runtime_unavailable`. This keeps `~/.config/clay/init.js` as the startup/configuration-change configuration entry point while preventing undocumented keys or package enable/disable authority from appearing without a dedicated server validator.

The Phase 18.2 shell runtime added internal `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, inert update, and observability state, but it did not add a user-visible shell configuration API. Phase 18.3 adds `runtime/js/ui.js` and server runtime support for `clay:ui` package declaration APIs (`serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`). Historical Phase 18.3 boundary text remains: `clay:ui` contribution APIs exist for package declarations, no `clay:ui` configuration override API or hidden split/slot/panel/style key system existed yet, and they were not user-visible configuration override APIs for default slots, panel visibility, component style overrides, theme-token remapping, or layout behavior. The historical inventory note was that `clay.ui.serverSetLayoutOverride` and `clay.configuration.setPackageOption` stay non-registry-public inventory rows until promotion. Phase 18.4 adds runtime-backed `serverRegisterInputContribution`, `serverRegisterUiStateScope`, and `serverSetLayoutOverride`. These APIs validate package-owned panel defaults, component style variables, fixed slots, overlays, semantic theme token declarations, input/focus/action metadata, UI state schema/lifecycle metadata, and user/package layout/theme/input/action override records at package-load/configuration/update time. `serverRegisterUiStateScope` is not state-value mutation work, and `serverSetLayoutOverride` is not a low-level pane/working-area mutation API.

Hidden JSON/TOML/ad hoc package UI configuration keys remain rejected; in lowercase policy wording, hidden JSON/TOML/ad hoc keys remain rejected. Examples such as `preview.position`, `preview.defaultVisibility`, `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `theme.markdown.heading.1`, `theme.markdown.preview.background`, `markdown.sidebar.width`, raw token override keys, and ad hoc style keys are invalid unless expressed through documented Clay JS APIs with `custom_properties`, allowed values, defaults, errors, security notes, and registry coverage. Phase 18.4's documented APIs use option names such as `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback` through `setPackageOption`, or layout override properties such as `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback` through `serverSetLayoutOverride`.

Phase 18.5 (plan `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`) replans Markdown end-user loading on top of these generic primitives. Its task-8 configuration audit confirms every Markdown-relevant behavior-changing surface is either a runtime-backed Clay JS API or an explicitly planned/unavailable API: Markdown package options and theme-token/layout overrides go through the same `setPackageOption`, `serverSetLayoutOverride`, `serverRegisterThemeToken`, `serverRegisterPanelContribution`, `serverRegisterInputContribution`, and `serverRegisterUiStateScope` APIs already promoted in Phase 18.3/18.4; `setModePreference`, `setDecorationTheme`, and `setParsePolicy` remain planned, while `loadPackage` is runtime-backed and consumes installed/authorized/adopted package state. The Markdown preview defaults to `defaultVisibility: "hidden"` through `serverRegisterPanelContribution` rather than a hard-coded side panel or hidden key. No Markdown-specific configuration validator, hidden-key system, or package-specific Rust configuration branch was added.

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

## Phase 20.6 persisted preferences and precedence

Phase 20.6 adds `PersistedPreferences` in `src/server/configuration.rs`: a closed `~/.config/clay/preferences.json` store with at most three keys (`theme`, `appearance`, `typography`), bounded to `PREFERENCES_PAYLOAD_BUDGET_BYTES` (8 KiB), validated at load and persist time, and authority-rejecting. `load_preferences` skips `null` fields and drops corrupted/oversized/manual-edit fields field-by-field with a diagnostic so startup never breaks. `persist_preference` writes atomically (tmp + rename); `clear_preferences` backs `settings.reset`. The `setPackageOption` source taxonomy is extended with `ui-session` to label these persisted values, but no new `clay:configuration` export is added — appearance is a `clay:theme` API (`clay.theme.setAppearance`), and the `clay:configuration` module stays closed.

A single documented precedence applies on every startup/reload (highest wins): `ui-session` (`preferences.json`, written by `settings.setTheme`/`settings.setAppearance`) > `init-js` (`init.js` `setTheme`/`setAppearance`/`setTypography`) > canonical/package default (appearance-derived Modus default or Clay core default). `apply_persisted_preferences` runs in the `src/server/js_runtime.rs` harvest immediately after `init.js` evaluation, so a UI choice always overrides the equivalent `init.js` call. Canonical-default resolution (Modus Operandi/Vivendi) also runs in the harvest when no explicit theme was set. Full implementation, settings surface, and the `@clay/settings` package details: [Phase 20.6 Theme Package Segregation and Settings UI](phase20.6-theme-segregation-settings-ui.md).

## Invariants and Constraints

- Configuration JavaScript runs server-side only; the native client never executes arbitrary configuration JavaScript.
- Module loading is startup/reload work and must stay off Masonry paint, text-event, and ordinary edit acknowledgement hot paths.
- Only explicit relative `.js` files under the configuration root are loadable. No network, npm/jsr/package, shell, workspace scan, extension loading, WASM, AI mutation, or direct client filesystem authority is introduced.
- `loadConfigurationModule` does not implement Deno/npm-style resolution: callers must provide the exact `.js` filename.
- Runtime-backed package option/layout override APIs do not grant package installation, enable/disable mutation, mode activation authority, decoration rendering authority, parse-document authority, component style override authority beyond typed tokens, raw Deno ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, client-side JavaScript, or external filesystem/network/shell/AI/workspace access. Planned package/mode/parse/decor configuration exports remain unavailable stubs until documented validators ship.
- `setTypography` is an all-or-nothing inert data transaction. It rejects raw JSON envelopes over `TYPOGRAPHY_PAYLOAD_BUDGET_BYTES` (1024), missing profiles, and unknown fields (including paths, URLs, font bytes, and font-download metadata) before mutation; server state retains defaults when no configuration selects typography.
- **Phase 18.9 mode and generic key-behavior defaults are hardcoded structural defaults, not runtime-configurable Clay JS settings.** The `core.text`/`core.code` fallback modes, the classification precedence ladder, the electric-character outdent set, and the generic pair-insertion/comment-continuation rule sets are compiled into `ModeRegistry`/`EditorBehaviorRules` and are never read from configuration at mode-activation or paint/text time (`src/packages/modes.rs` and `src/editor/surface.rs` consult `behavior_manifest` only, never the package-option store). Because `setPackageOption` uses a closed suffix allowlist, behavior-changing Phase 18.9 keys such as `core.preferredFallbackMode`, `core.electricCharacters`, `core.pairInsertion`, and `core.commentContinuation` are rejected as unsupported options rather than silently accepted as undocumented settings, so built-in mode defaults cannot be overridden to grant package authority. Users who want different classification/behavior should register a package mode or behavior manifest through the documented Clay JS APIs, not an undocumented configuration key. (See `phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected` in `src/server/configuration.rs`.)
- **Phase 18.12 workspace file-browser defaults and workflow routes are Clay-owned runtime constants or existing command bindings, not new `clay:configuration` APIs.** Fuzzy-open, file-browser toggle, selected-folder picker, and copy-selection chords use the existing `clay.keybindings.bindKey` API with fixed command ids (`clay.workspace.openFuzzyFile`, `clay.workspace.toggleFileBrowser`, `clay.workspace.clientOpenFolderDialog`, `clay.editor.clientCopySelection`) and no Rust default chord. The left panel/default slot, bounded marker set (`KNOWN_PROJECT_MARKERS`), default ignore names, listing depth/count budgets, left-panel item budget, fuzzy item budget, native folder-picker backend, and OS clipboard backend are compiled Clay-owned workspace/client boundaries. They are not hidden `init.js` keys and cannot grant extra filesystem/workspace authority, clipboard read, paste/cut, arbitrary clipboard writes, or server/package clipboard authority. Public programmatic file-browser work goes through the Phase 18.12 `clay:workspace` and `clay:commands` facades documented in `docs/reference/clay-js-api/`, while configuration remains limited to existing documented APIs.
- **Plan 060/061 remediation and two-domain package controls do not add configuration APIs.** The exact `clay:configuration` export set remains three runtime-backed APIs (`loadConfigurationModule`, `getConfigurationState`, `setPackageOption`) plus three planned/unavailable stubs (`setModePreference`, `setDecorationTheme`, `setParsePolicy`). The facade is trusted-only. Runtime domains, package context/provenance, approval scope, connection identity, leases/versions, result routing, document/connection/queue/actor/process/frame/file/listing ceilings, atomic-save replacement rules, dialog generations, clipboard backend, build debug profile, target layout, audit policy, and CI gates are compiled/host policy. `setPackageOption` accepts only seven documented package-owned suffixes, so package-prefixed attempts to tune these controls fail closed without entering configuration state. See `docs/reference/clay-js-api/configuration.md#plan-060061-configuration-closure`.
- **Phase 20 daily-editing defaults and command routes are Clay-owned runtime constants or existing command bindings, not new `clay:configuration` APIs.** Cut/paste/undo/redo/open-documents/resync/dismiss/save/reload/open-file chords reuse `clay.keybindings.bindKey` with fixed command ids and empty `custom_properties`. Undo depth (`EDIT_HISTORY_MAX_DEPTH` = 256), history entry bytes (`EDIT_HISTORY_MAX_ENTRY_BYTES` = 64 KiB), retained sessions (`CLIENT_DOCUMENT_SESSION_MAX` = 64), accessibility label budgets, and status-chrome AA contrast (`STATUS_CHROME_MIN_CONTRAST` = 4.5) are compiled constants — not `init.js` keys. Phase 20 does not invent clipboard-exfiltration, filesystem, network, shell, package-manager, recovery-toggle, or dialog-filter configuration APIs; broader package/config/AI authority remains deferred. See `docs/reference/clay-js-api/configuration.md#phase-20-daily-editing-product-hardening-configuration-review`.

## Tests

- `src/server/js_runtime.rs`: loads an `init.js` fixture, loads `./ui.js` via `loadConfigurationModule`, reports entry/module state, rejects traversal/URL/npm/package-style specifiers, and verifies planned package/mode configuration facade exports return clear unavailable errors.
- `src/server/configuration.rs`: `package_option_configuration_accepts_supported_typed_options_only`, `package_option_configuration_rejects_hidden_ad_hoc_and_raw_authority_keys`, `plan060_internal_security_and_performance_controls_are_not_configurable`, and the Phase 18.9 `phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected` test pin the closed package-option allowlist (no ad-hoc, internal-control, or behavior-changing core.* keys accepted).
- `tests/clay_js_api_inventory.rs::configuration_surface_is_closed_and_security_controls_are_not_properties`: pins the exact six configuration exports/inventory rows, runtime-backed-vs-planned status, trusted-only facade classification, and absence of internal control names from `custom_properties`.
- `src/server/js_runtime.rs`: `set_typography_replaces_all_profiles_atomically`, `set_typography_failure_preserves_previous_revision`, and `typography_configuration_grants_no_additional_authority` cover facade parsing and failure atomicity; `src/server/mod.rs` covers default state and one broadcast per changed replacement.
- `tests/rust_visibility_api_mapping.rs`: `set_typography_rust_op_facade_and_doc_mapping_is_complete`, `typography_client_helpers_are_not_public_server_api`, and `raw_typography_op_is_not_user_facing` pin the single public facade/op/docs route while keeping client layout/registry helpers and the raw op name outside the user-facing API.
- Command: `cargo test js_runtime --quiet && cargo test configuration_runtime --quiet`

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- [Protocol and Performance Pattern](../../../.agents/skills/project-patterns/references/protocol-and-performance.md)
- `docs/reference/clay-js-api/configuration.md`
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
