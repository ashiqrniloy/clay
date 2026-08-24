# Configuration Runtime

## Source

- `src/server/configuration.rs`
- `src/server/js_runtime/mod.rs`
- `src/server/mod.rs`
- `src/server/config_watch.rs`
- `src/server/connection/mod.rs`
- `src/server/ops/configuration.rs`
- `src/server/ops/keybindings.rs`
- `src/protocol/mod.rs`
- `src/server/output_router.rs`
- `src/server/ops/theme.rs`
- `src/server/ops/typography.rs`
- `runtime/js/configuration.js`
- `runtime/js/configuration.d.ts`
- `runtime/js/theme.js`
- `docs/wiki/modules/package-input-state-configuration.md`
- `src/server/js_runtime/mod.rs` tests

## Overview

Clay evaluates a constrained local configuration entry point from a configuration root containing `init.js`. Server startup loads the default `~/.config/clay/init.js` when it exists, and tests can supply explicit fixture roots. While running, a bounded polling task watches the effective root and delegates changes to the serialized runtime reload service. The runtime supports the documented `clay:configuration` facade, local relative `.js` modules under the same configuration directory, and read-only configuration state for the entry point, loaded modules, and validated package-option records.

## Responsibilities

- Treat `~/.config/clay/init.js` as the default user configuration entry point while allowing tests to provide an explicit configuration root.
- Resolve relative local JavaScript modules without changing the process current directory or invoking shell/package loading behavior.
- Reject URLs, package specifiers, absolute paths, extensionless imports, and traversal outside the configuration root.
- Expose `loadConfigurationModule({ path, optional })` and `getConfigurationState()` through Clay-owned ops, not raw user-facing op calls; optional loads return a status object instead of weakening required-load failures.
- Runtime-back `setPackageOption` for Phase 18.4 package-owned options while preserving `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as explicit planned `clay:configuration` facade exports rather than ad hoc settings.
- Accept `clay:theme.setTypography` as one complete user-owned three-profile transaction, then hand its inert candidate to server-authoritative revision/publication state.
- Runtime-back Phase 18.4 package layout overrides through `ui.serverSetLayoutOverride` while preserving hidden split/slot/panel/style keys as rejected; lower-level working-area/pane mutation APIs and package enable/disable through configuration remain planned.
- Keep Plan 060/061 trust-domain identity, package provenance, connection/document authority, queue/file/process ceilings, atomic-save policy, routing, worker concurrency, native-dialog/clipboard behavior, and Cargo/CI profiles outside configuration. Real package adoption/revocation/replacement decisions use the host CLI; existing `loadPackage` only consumes approved state.
- Watch only the canonical effective configuration root, with bounded depth/file counts and no symlink traversal; route changes through `runtime.reloadConfiguration` rather than a watcher-specific command or IPC path.
- Ship global `Ctrl+Shift+R` for `runtime.reloadConfiguration`; configuration keymap overlays can override or remove the default without changing server-first locking.
- Retain runtime diagnostics for bootstrap/reconnect and broadcast them to live connections through the bounded generic output router.

## How It Works

`ClayJsRuntimeService::load_configuration_from_root` runs on the same blocking runtime worker used by controlled JavaScript evaluation. It constructs a `ConfigurationRuntime` from the supplied root, canonicalizes `init.js`, creates a file URL for that entry point, and installs both `ClayOpState` and the configuration state in `deno_core::OpState`.

`ClayModuleLoader` handles three allowed module families:

1. The main `init.js` file under the configuration root.
2. The built-in `clay:configuration` facade source.
3. Explicit relative `.js` modules that canonicalize under the configuration root.

The facade validates `loadConfigurationModule({ path, optional })` through `op_clay_configuration_load_module` before using dynamic `import(path)`. Required loads retain strict existing validation and propagation. Optional validation still rejects URLs, package specifiers, invalid extensions, and paths outside the canonical root, but permits a missing final file so import failure can be isolated. The facade catches only the subsequent import/parse/evaluation rejection, records its bounded message through `op_clay_configuration_record_module_error`, and returns `{ loaded: false, error }`; success returns `{ loaded: true }`. `ConfigurationRuntime` stores at most 64 module failures, caps each detail at 1 KiB, renders paths relative to the config root, and drains the store once into `ClayRuntimeEvaluation.configuration_diagnostics`. `IpcServer` retains those `configuration.module_failed` warnings in the existing runtime diagnostic store and includes them in reload outcomes. The module loader still performs the authoritative canonical path check when resolving/loading, reads files directly with Rust filesystem APIs, and records successfully loaded local modules in deterministic first-load order. `getConfigurationState()` returns JSON from `op_clay_configuration_get_state`, which the facade parses into `{ entryPoint, loadedModules }`.

`IpcServer::run` computes one effective watch root: an explicit `ServerConfig.configuration_root` wins; otherwise the default `~/.config/clay` root is used only when it contains `init.js`. `config_watch.rs` canonicalizes that root, polls it once per second with `MissedTickBehavior::Skip`, and records bounded `(path, modified-time, length)` snapshots for `.js` files and `preferences.json` through depth 8 and at most 256 watched files. Dotfiles, temporary names, and symlinks are ignored. A changed snapshot is re-scanned after a 300 ms quiet period, then invokes the same serialized `reload_runtime_generation` path. The snapshot is re-baselined after the attempt, so a failed configuration remains active without a reload loop and a later edit can recover it.

The canonical `examples/` tree demonstrates the modular structure as a convention, not a requirement: base `init.js` (theme, appearance alternatives, all three typography profiles plus the optional seven-field hierarchy, caret, bindings, shell policy) ends with two fault-isolated loads — `loadConfigurationModule({ path: "./packages/first-party.js", optional: true })` (LSP grants + one-line `loadPackage` calls) and a third-party equivalent (a fully commented template; real package adoption goes through the host `clay package add`/`adopt` CLI). Any local module layout under the config root works identically — `tests/fixtures/configuration/plan080-manual/` is a verbatim copy of the `examples/` tree used by the manual test plan and driven headlessly via `clay server --config-fixture plan080-manual`.

`RuntimeDiagnosticStore` retains bounded diagnostics for bootstrap/reconnect and owns a live `OutputRouter<RuntimeDiagnostic>` subscription map. Each connection subscribes before its handshake and removes its sender on every exit path. `record_runtime_diagnostic` publishes once to both retention and the bounded live lane; watcher failures therefore reach connected clients without a watcher-specific protocol message.

Phase 18.16.5 adds [`setTypography`](../../reference/clay-js-api/theme/set-typography.md) to the existing `clay:theme` facade rather than creating an undocumented setting system. The op accepts exactly `monospace`, `proportional`, and `ui` objects, each with only `families`, `size`, and optional per-role `ligatures`, plus an optional complete seven-field `hierarchy`; it builds one `ActiveTypography` candidate, validates every bounded fallback stack, size, feature policy, and hierarchy ratio before replacing runtime-local state, and returns its evaluation-local revision. `IpcServer` revalidates the successful candidate, assigns the persistent server revision only when profiles differ, and broadcasts one bounded `ActiveTypography` update. No configuration call discovers installed fonts, reads font files, fetches URLs, or grants package/network/shell authority. Client bootstrap installation is owned by the following client-registry task.

The Phase 16.5 primitive gate reviewed package options, mode preferences, decoration theme preferences, and parse policy preferences but did not implement concrete behavior-changing settings. Phase 18.4 promotes `setPackageOption` to a runtime-backed API for package-owned options while keeping `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as planned-unavailable APIs routed through `op_clay_runtime_unavailable`. This keeps `~/.config/clay/init.js` as the startup/configuration-change configuration entry point while preventing undocumented keys or package enable/disable authority from appearing without a dedicated server validator.

The Phase 18.2 shell runtime added internal `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, inert update, and observability state, but it did not add a user-visible shell configuration API. Phase 18.3 adds `runtime/js/ui.js` and server runtime support for `clay:ui` package declaration APIs (`serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`). Historical Phase 18.3 boundary text remains: `clay:ui` contribution APIs exist for package declarations, no `clay:ui` configuration override API or hidden split/slot/panel/style key system existed yet, and they were not user-visible configuration override APIs for default slots, panel visibility, component style overrides, theme-token remapping, or layout behavior. The historical inventory note was that `ui.serverSetLayoutOverride` and `configuration.setPackageOption` stay non-registry-public inventory rows until promotion. Phase 18.4 adds runtime-backed `serverRegisterInputContribution`, `serverRegisterUiStateScope`, and `serverSetLayoutOverride`. These APIs validate package-owned panel defaults, component style variables, fixed slots, overlays, semantic theme token declarations, input/focus/action metadata, UI state schema/lifecycle metadata, and user/package layout/theme/input/action override records at package-load/configuration/update time. `serverRegisterUiStateScope` is not state-value mutation work, and `serverSetLayoutOverride` is not a low-level pane/working-area mutation API.

Hidden JSON/TOML/ad hoc package UI configuration keys remain rejected; in lowercase policy wording, hidden JSON/TOML/ad hoc keys remain rejected. Examples such as `preview.position`, `preview.defaultVisibility`, `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `theme.markdown.heading.1`, `theme.markdown.preview.background`, `markdown.sidebar.width`, raw token override keys, and ad hoc style keys are invalid unless expressed through documented Clay JS APIs with `custom_properties`, allowed values, defaults, errors, security notes, and registry coverage. Phase 18.4's documented APIs use option names such as `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback` through `setPackageOption`, or layout override properties such as `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback` through `serverSetLayoutOverride`.

Phase 18.5 (plan `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`) replans Markdown end-user loading on top of these generic primitives. Its task-8 configuration audit confirms every Markdown-relevant behavior-changing surface is either a runtime-backed Clay JS API or an explicitly planned/unavailable API: Markdown package options and theme-token/layout overrides go through the same `setPackageOption`, `serverSetLayoutOverride`, `serverRegisterThemeToken`, `serverRegisterPanelContribution`, `serverRegisterInputContribution`, and `serverRegisterUiStateScope` APIs already promoted in Phase 18.3/18.4; `setModePreference`, `setDecorationTheme`, and `setParsePolicy` remain planned, while `loadPackage` is runtime-backed and consumes installed/authorized/adopted package state. The Markdown preview defaults to `defaultVisibility: "hidden"` through `serverRegisterPanelContribution` rather than a hard-coded side panel or hidden key. No Markdown-specific configuration validator, hidden-key system, or package-specific Rust configuration branch was added.

## Code Examples

```js
// ~/.config/clay/init.js
import { getConfigurationState, loadConfigurationModule } from "clay:configuration";

const packageConfig = await loadConfigurationModule({
  path: "./packages/first-party.js",
  optional: true,
});
console.log(packageConfig.loaded, getConfigurationState().loadedModules);
```

```rust
let service = ClayJsRuntimeService::default();
let result = service.load_configuration_from_root(config_root).await?;
```

## Plan 080 configuration primitive review

Plan 080's pre-implementation primitive review confirms that configuration auto-reload and modular package configuration should compose existing Clay-owned surfaces rather than add package-specific Rust branches:

- `ConfigurationRuntime` (`src/server/configuration.rs`) canonicalizes the root, resolves explicit local `.js` modules, repeats the root-boundary check while reading sources, and records loaded-module state. Optional loading adds only bounded module-error collection/drain; path containment is validated before optional import failure is caught, with a missing final file allowed to reach the catch.
- `ClayJsRuntimeService` and `IpcServer::reload_runtime_generation` (`src/server/js_runtime/mod.rs`, `src/server/mod.rs`) already evaluate fresh candidates, serialize attempts, validate before commit, and preserve the active generation on failure. A watcher delegates to this service; it does not add IPC, generation, lock, or package logic. Its effective root must match configuration loading: explicit `ServerConfig.configuration_root` wins, otherwise the existing default-root resolver watches `~/.config/clay` when `init.js` exists.
- `RuntimeDiagnosticStore` (`src/server/connection/mod.rs`) provides bounded deduplicating bootstrap retention, while `OutputRouter<T>` (`src/server/output_router.rs`) provides bounded per-connection broadcast. A server-level `RuntimeDiagnostic` subscription is the one generic fanout gap needed so watcher failures reach live clients and reconnecting clients still receive retained diagnostics.
- `default_keymaps`, `ClayOpState::bind_key`/`unbind_key`, `configured_keymaps`, behavior-manifest validation, and client routing provide static defaults plus durable configuration overlays. The reload chord is a default rule, not a new primitive; the built-in command metadata exposes the same default to Control Center.

Implemented generic additions are: bounded module-error collection/drain for optional configuration imports, a cancellable bounded config-root polling/debounce task, and reusable live runtime-diagnostic fanout. Polling stays off keypress, edit acknowledgement, paint, layout, and parse paths. Optional loading and watching retain the existing deny-by-default authority model: canonical root containment remains mandatory, and no filesystem, network, shell, package, workspace, AI, WASM, native-widget, raw-op, or client-JavaScript authority is added. Optional-module coverage lives in `src/server/js_runtime/mod.rs` and `src/server/configuration.rs`; watcher coverage lives in `src/server/config_watch.rs` and `src/server/mod.rs`, while live-fanout coverage is in `src/server/connection/mod.rs`.

## Phase 20.6 persisted preferences and precedence

Phase 20.6 adds `PersistedPreferences` in `src/server/configuration.rs`: a closed `~/.config/clay/preferences.json` store with at most three keys (`theme`, `appearance`, `typography`), bounded to `PREFERENCES_PAYLOAD_BUDGET_BYTES` (8 KiB), validated at load and persist time, and authority-rejecting. `load_preferences` skips `null` fields and drops corrupted/oversized/manual-edit fields field-by-field with a diagnostic so startup never breaks. `persist_preference` writes atomically (tmp + rename); `clear_preferences` backs `settings.reset`. The `setPackageOption` source taxonomy is extended with `ui-session` to label these persisted values, but no new `clay:configuration` export is added — appearance is a `clay:theme` API (`theme.setAppearance`), and the `clay:configuration` module stays closed.

A single documented precedence applies on every startup/reload (highest wins): `ui-session` (`preferences.json`, written by `settings.setTheme`/`settings.setAppearance`) > `init-js` (`init.js` `setTheme`/`setAppearance`/`setTypography`) > canonical/package default (appearance-derived Modus default or Clay core default). `apply_persisted_preferences` runs in the `src/server/js_runtime/mod.rs` harvest immediately after `init.js` evaluation, so a UI choice always overrides the equivalent `init.js` call. Canonical-default resolution (Modus Operandi/Vivendi) also runs in the harvest when no explicit theme was set. Full implementation, settings surface, and the `@clay/settings` package details: [Phase 20.6 Theme Package Segregation and Settings UI](phase20.6-theme-segregation-settings-ui.md).

## Plan 097 Phase 9 React diagnostics and settings

The Tauri bridge continues to forward the existing retained/live
`RuntimeDiagnostic` family and complete runtime snapshots. React stores the
latest diagnostic per tab and projects its sanitized message in the shell
status footer; it adds no watcher/reload protocol or frontend configuration
store. `runtime.reloadConfiguration` still runs through the server command
executor and failed candidates preserve the prior generation.

The trusted `@clay/settings` React panel now completes typography value
carriage: it sends one JSON argument containing all three profiles and all seven
hierarchy ratios. `execute_settings` and `validate_typography_request` reject
missing, malformed, unknown, or out-of-bound values before
`persist_preference("typography", ...)`; reload applies the same parser again.
Theme, appearance, and reset retain their existing closed-key atomic store and
precedence. No new `clay:configuration` API, option, keybinding, or example
configuration entry was introduced.

## Phase 28 editor command configuration review

Phase 28 reuses existing configuration surfaces instead of adding a second
option system. The built-in `editor.toggleComment` command keeps its default
`Ctrl+/` binding; `toggleListMarker`, `rotateHeading`, `clientToggleFold`, and
`toggleInlayHints` are stable command-ID helpers that users can pass to
`bindKey`. The first three text transforms route through the client-first
manifest lane; fold and inlay visibility route through client UI commands.
`command_routing_policy` must keep `editor.toggleInlayHints` aligned with its
`ClientUi` declaration so a configured binding survives command validation.

Wrapping uses the trusted `editor.clientSetEditorLayout` override. Inlay
visibility defaults and the remaining editor chrome stay inert
`editorRules.chrome` manifest data (`inlayHints` included); no fold, inlay,
comment, list, heading, or chrome `setPackageOption` key was added. The
existing closed package-option validator remains available only for actual
package-owned defaults, rejects ad hoc keys, and exposes its `ui-session`
source in the TypeScript facade for persisted settings metadata.

The canonical `examples/init.js` mirrors this contract in its keybinding section:
`editor.toggleComment` is re-declared with its shipped `Ctrl+/` default, while
`editor.toggleListMarker`, `editor.rotateHeading`, `editor.clientToggleFold`,
and `editor.toggleInlayHints` are documented as commented opt-in bindings
because none has a core default chord. The example records each command's
client-first/client-UI routing, manifest-driven list/heading transforms, and
code/prose inlay defaults without adding imports, grants, or runtime work.
Package `loadPackage` calls remain explicit one-line loads in
`examples/packages/first-party.js`. `tests/clay_js_doc_registry.rs` keeps all
five Phase 28 command IDs present exactly once as bindings and checks their
name/type/default comments.

## Invariants and Constraints

- Configuration JavaScript runs server-side only; the native client never executes arbitrary configuration JavaScript.
- Module loading is startup/reload work and must stay off Masonry paint, text-event, and ordinary edit acknowledgement hot paths.
- Only explicit relative `.js` files under the configuration root are loadable. No network, npm/jsr/package, shell, workspace scan, extension loading, WASM, AI mutation, or direct client filesystem authority is introduced.
- `loadConfigurationModule` does not implement Deno/npm-style resolution: callers must provide the exact `.js` filename.
- The watcher polls only `.js` files and `preferences.json`, skips dotfiles, temporary names, and symlinks, and caps scans at 256 files and depth 8; a 300 ms quiet debounce and one-second skipped-tick interval keep it off editor hot paths.
- Runtime-backed package option/layout override APIs do not grant package installation, enable/disable mutation, mode activation authority, decoration rendering authority, parse-document authority, component style override authority beyond typed tokens, raw Deno ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, client-side JavaScript, or external filesystem/network/shell/AI/workspace access. Planned package/mode/parse/decor configuration exports remain unavailable stubs until documented validators ship.
- `setTypography` is an all-or-nothing inert data transaction. It rejects raw JSON envelopes over `TYPOGRAPHY_PAYLOAD_BUDGET_BYTES` (1024), missing profiles, and unknown fields (including paths, URLs, font bytes, and font-download metadata) before mutation; server state retains defaults when no configuration selects typography.
- **Phase 18.9 mode and generic key-behavior defaults are hardcoded structural defaults, not runtime-configurable Clay JS settings.** The `core.text`/`core.code` fallback modes, the classification precedence ladder, the electric-character outdent set, and the generic pair-insertion/comment-continuation rule sets are compiled into `ModeRegistry`/`EditorBehaviorRules` and are never read from configuration at mode-activation or paint/text time (`src/packages/modes.rs` and `src/editor/surface/mod.rs` consult `behavior_manifest` only, never the package-option store). Because `setPackageOption` uses a closed suffix allowlist, behavior-changing Phase 18.9 keys such as `core.preferredFallbackMode`, `core.electricCharacters`, `core.pairInsertion`, and `core.commentContinuation` are rejected as unsupported options rather than silently accepted as undocumented settings, so built-in mode defaults cannot be overridden to grant package authority. Users who want different classification/behavior should register a package mode or behavior manifest through the documented Clay JS APIs, not an undocumented configuration key. (See `phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected` in `src/server/configuration.rs`.)
- **Phase 18.12/22.8 workspace file-browser defaults and workflow routes are Clay-owned runtime constants or existing command bindings, not new `clay:configuration` APIs.** Fuzzy-open, file-browser toggle, selected-folder picker, and copy-selection chords use the existing `keybindings.bindKey` API with fixed command ids (`workspace.openFuzzyFile`, `workspace.toggleFileBrowser`, `workspace.clientOpenFolderDialog`, `editor.clientCopySelection`) and no Rust default chord; the canonical `examples/init.js` binds `Ctrl+B` for the toggle. The per-tab pane defaults hidden, and the left panel is published only after an explicit toggle. The left panel/default slot, bounded marker set (`KNOWN_PROJECT_MARKERS`), default ignore names, listing depth/count budgets, left-panel item budget, fuzzy item budget, native folder-picker backend, and OS clipboard backend are compiled Clay-owned workspace/client boundaries. They are not hidden `init.js` keys and cannot grant extra filesystem/workspace authority, clipboard read, paste/cut, arbitrary clipboard writes, or server/package clipboard authority. Public programmatic file-browser work goes through the Phase 18.12 `clay:workspace` and `clay:commands` facades documented in `docs/reference/clay-js-api/`, while configuration remains limited to existing documented APIs.
- **Plan 060/061 remediation and two-domain package controls do not add configuration APIs.** The exact `clay:configuration` export set remains three runtime-backed APIs (`loadConfigurationModule`, `getConfigurationState`, `setPackageOption`) plus three planned/unavailable stubs (`setModePreference`, `setDecorationTheme`, `setParsePolicy`). The facade is trusted-only. Runtime domains, package context/provenance, approval scope, connection identity, leases/versions, result routing, document/connection/queue/actor/process/frame/file/listing ceilings, atomic-save replacement rules, dialog generations, clipboard backend, build debug profile, target layout, audit policy, and CI gates are compiled/host policy. `setPackageOption` accepts only seven documented package-owned suffixes, so package-prefixed attempts to tune these controls fail closed without entering configuration state. See `docs/reference/clay-js-api/configuration.md#plan-060061-configuration-closure`.
- **Phase 20 daily-editing defaults and command routes are Clay-owned runtime constants or existing command bindings, not new `clay:configuration` APIs.** Cut/paste/undo/redo/open-documents/resync/dismiss/save/reload/open-file chords reuse `keybindings.bindKey` with fixed command ids and empty `custom_properties`. Undo depth (`EDIT_HISTORY_MAX_DEPTH` = 256), history entry bytes (`EDIT_HISTORY_MAX_ENTRY_BYTES` = 64 KiB), retained sessions (`CLIENT_DOCUMENT_SESSION_MAX` = 64), accessibility label budgets, and status-chrome AA contrast (`STATUS_CHROME_MIN_CONTRAST` = 4.5) are compiled constants — not `init.js` keys. Phase 20 does not invent clipboard-exfiltration, filesystem, network, shell, package-manager, recovery-toggle, or dialog-filter configuration APIs; broader package/config/AI authority remains deferred. See `docs/reference/clay-js-api/configuration.md#phase-20-daily-editing-product-hardening-configuration-review`.
- **Phase 28 editor commands add no configuration option keys.** `editor.toggleComment`, `editor.toggleListMarker`, `editor.rotateHeading`, `editor.clientToggleFold`, and `editor.toggleInlayHints` are fixed bindable command IDs; their mode transforms, fold ranges, and inlay visibility defaults come from validated behavior-manifest/client state. `editorRules.chrome.inlayHints` is manifest data, not a `setPackageOption` key. `bindKey` remains the only configuration route for user chords, and `phase28_editor_defaults_are_not_configuration_keys` rejects attempted `editor.fold.enabled`, `editor.inlayHints.enabled`, `editor.headingPrefixes`, `editor.commentPrefix`, `editor.chrome`, and `editor.wrapPolicy` package options.
- **Plan 086 safety controls remain unconditional.** `setPackageOption` rejects `accessibility.enabled`, `accessibility.validation`, `protocol.archiveValidation`, `protocol.codecValidation`, and `protocol.rkyvValidation`; no hidden `init.js` key can disable AccessKit tree safety or checked archive decoding. The three whole-workflow configuration/Control Center tests use unique mode-700 roots and five-second timeouts so ambient configuration and leaked pending sessions cannot affect the result.

## Tests

- `src/server/js_runtime/mod.rs`: loads an `init.js` fixture, loads `./ui.js` via `loadConfigurationModule`, verifies optional syntax/missing-module isolation and required-load failure, reports root-relative `configuration.module_failed` warnings, rejects traversal/URL/npm/package-style specifiers, and verifies planned package/mode configuration facade exports return clear unavailable errors.
- `src/server/configuration.rs`: verifies module-error detail is root-relative, capped at 1 KiB, bounded, and drained exactly once; package options accept only the documented closed suffixes and reject hidden/ad hoc/authority-bearing keys.
- `src/server/ops/keybindings.rs::phase28_editor_configuration_commands_are_bindable_and_routed`: pins `Ctrl+/`'s client-first comment route, client-first prose transforms, and client-UI fold/inlay routes through the `bindKey` validation boundary.
- `src/server/mod.rs`: verifies a successful reload returns and retains optional-module warnings; watcher tests cover modified roots, failed-reload preservation/recovery, and newly created modules; `example_configuration_loads_cleanly_and_applies_effects` boots the whole `examples/` tree via `temp_example_config_root` and asserts the explicit dark theme, all three typography profiles, documented default hierarchy, first-party `loadPackage` outcomes (markdown parse handler, rust/typescript/javascript/markdown syntax grammars, all handlers `@clay/*`-prefixed, zero `packages.*` diagnostics); `example_configuration_survives_broken_package_module` breaks `packages/first-party.js` and asserts the base config still reloads with a `configuration.module_failed` warning; `alternate_configuration_layout_loads_identical_packages` drives `init.js → loadConfigurationModule("./a/b.js") → static import "./c.js" → one-line loadPackage` in a nested folder and asserts identical package outcomes (layout freedom regression).
- `src/server/config_watch.rs`: bounded snapshot filtering, creation/deletion detection, and edit-storm debounce tests.
- `src/server/connection/mod.rs`: bootstrap retention and live runtime-diagnostic delivery share one bounded diagnostic store/router.
- `src/server/configuration.rs`: `package_option_configuration_accepts_supported_typed_options_only`, `phase28_editor_defaults_are_not_configuration_keys`, `package_option_configuration_rejects_hidden_ad_hoc_and_raw_authority_keys`, `plan060_internal_security_and_performance_controls_are_not_configurable`, `configuration_rejects_watcher_control_keys` (`core.watch.intervalMs`/`debounceMs`/`enabled` fail closed on the allowlist — the watcher has no hidden tuning keys), and the Phase 18.9 `phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected` test pin the closed package-option allowlist (no ad-hoc, internal-control, or behavior-changing core.* keys accepted). Plan 086 extends the internal-control test with the accessibility/archive-validation suffixes.
- `tests/clay_js_api_inventory.rs::configuration_surface_is_closed_and_security_controls_are_not_properties`: pins the exact six configuration exports/inventory rows, runtime-backed-vs-planned status, trusted-only facade classification, and absence of internal control names from `custom_properties`.
- `tests/clay_js_doc_registry.rs::canonical_example_covers_theme_typography_and_modular_configuration`: keeps the canonical imports, one active theme/typography transaction, all seven hierarchy fields, appearance alternatives, optional module paths, and configuration documentation markers aligned.
- `tests/clay_js_doc_registry.rs::phase28_canonical_example_lists_all_bindable_editor_commands`: keeps each Phase 28 editor command in the canonical keybinding section exactly once, with its documented name/type/default metadata and safe commented opt-in status where no core chord exists.
- `src/server/js_runtime/mod.rs`: `set_typography_replaces_all_profiles_atomically`, `set_typography_failure_preserves_previous_revision`, and `typography_configuration_grants_no_additional_authority` cover facade parsing and failure atomicity; `src/server/mod.rs` covers default state and one broadcast per changed replacement.
- `tests/rust_visibility_api_mapping.rs`: `set_typography_rust_op_facade_and_doc_mapping_is_complete`, `typography_client_helpers_are_not_public_server_api`, and `raw_typography_op_is_not_user_facing` pin the single public facade/op/docs route while keeping client layout/registry helpers and the raw op name outside the user-facing API.
- Commands: `cargo test --lib js_runtime:: -- --test-threads=1`, `cargo test --lib example_configuration_loads_cleanly_and_applies_effects -- --test-threads=1`, `cargo test --test protocol clay_js_doc_registry`, and `for file in examples/init.js examples/packages/first-party.js examples/packages/third-party.js; do node --check "$file"; done`.
- Plan 086 bounded workflows: `cargo test --lib example_configuration_loads_cleanly -- --test-threads=1`, `cargo test --lib control_center_opens_filters_activates_and_cancels -- --test-threads=1`, and `cargo test --lib runtime_generation_replacement_cancels_open_control_center -- --test-threads=1`.

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- [Protocol and Performance Pattern](../../../.agents/skills/project-patterns/references/protocol-and-performance.md)
- `docs/reference/clay-js-api/configuration.md`
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
